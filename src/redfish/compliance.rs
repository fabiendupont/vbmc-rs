use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, Response, StatusCode, header};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Tower layer that adds OData compliance to all responses:
/// - `OData-Version: 4.0` header
/// - `Link: </redfish/v1/$metadata>; rel=describedby` header
/// - `@odata.context` injected into JSON response bodies that have `@odata.type`
/// - `@odata.etag` injected into JSON response bodies + `ETag` header
/// - `If-None-Match` support (304 Not Modified)
/// - HEAD requests: run GET handler, strip body
#[derive(Clone)]
pub struct ODataComplianceLayer;

impl<S> Layer<S> for ODataComplianceLayer {
    type Service = ODataComplianceService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ODataComplianceService { inner }
    }
}

#[derive(Clone)]
pub struct ODataComplianceService<S> {
    inner: S,
}

fn odata_context_from_type(odata_type: &str) -> Option<String> {
    let stripped = odata_type.strip_prefix('#')?;
    let parts: Vec<&str> = stripped.split('.').collect();
    if parts.len() >= 3 {
        let namespace = parts[0];
        let type_name = parts[parts.len() - 1];
        Some(format!("/redfish/v1/$metadata#{namespace}.{type_name}"))
    } else if parts.len() == 2 {
        Some(format!("/redfish/v1/$metadata#{stripped}"))
    } else {
        None
    }
}

fn compute_etag(body: &[u8]) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    body.hash(&mut hasher);
    let hash = hasher.finish();
    format!("W/\"{hash:016X}\"")
}

impl<S> Service<Request<Body>> for ODataComplianceService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let is_head = req.method() == Method::HEAD;
        let is_get = req.method() == Method::GET;
        let if_none_match = req
            .headers()
            .get(header::IF_NONE_MATCH)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        let mut inner = self.inner.clone();

        let req = if is_head {
            let (mut parts, body) = req.into_parts();
            parts.method = Method::GET;
            Request::from_parts(parts, body)
        } else {
            req
        };

        Box::pin(async move {
            let response = inner.call(req).await?;
            let (mut parts, body) = response.into_parts();

            parts.headers.insert(
                header::HeaderName::from_static("odata-version"),
                header::HeaderValue::from_static("4.0"),
            );
            parts.headers.insert(
                header::LINK,
                header::HeaderValue::from_static("</redfish/v1/$metadata>; rel=describedby"),
            );

            if is_head {
                return Ok(Response::from_parts(parts, Body::empty()));
            }

            let is_json = parts
                .headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|ct| ct.contains("application/json"));

            if is_json {
                let bytes = to_bytes(body, 10_000_000).await.unwrap_or_default();

                if let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                    if let Some(obj) = json.as_object_mut()
                        && !obj.contains_key("@odata.context")
                        && let Some(odata_type) = obj.get("@odata.type").and_then(|v| v.as_str())
                        && let Some(context) = odata_context_from_type(odata_type)
                    {
                        obj.insert(
                            "@odata.context".to_string(),
                            serde_json::Value::String(context),
                        );
                    }

                    let new_body = serde_json::to_vec(&json).unwrap_or_else(|_| bytes.to_vec());
                    let etag = compute_etag(&new_body);

                    if let Some(obj) = json.as_object_mut() {
                        obj.insert(
                            "@odata.etag".to_string(),
                            serde_json::Value::String(etag.clone()),
                        );
                    }
                    let final_body = serde_json::to_vec(&json).unwrap_or(new_body);

                    if (is_get || is_head)
                        && let Some(ref client_etag) = if_none_match
                        && *client_etag == etag
                    {
                        parts.status = StatusCode::NOT_MODIFIED;
                        parts.headers.insert(
                            header::ETAG,
                            header::HeaderValue::from_str(&etag).expect("etag is valid ASCII"),
                        );
                        return Ok(Response::from_parts(parts, Body::empty()));
                    }

                    parts.headers.insert(
                        header::ETAG,
                        header::HeaderValue::from_str(&etag).expect("etag is valid ASCII"),
                    );
                    parts.headers.insert(
                        header::CONTENT_LENGTH,
                        header::HeaderValue::from(final_body.len()),
                    );
                    return Ok(Response::from_parts(parts, Body::from(final_body)));
                }

                return Ok(Response::from_parts(parts, Body::from(bytes)));
            }

            Ok(Response::from_parts(parts, body))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_odata_context_from_type_three_parts() {
        assert_eq!(
            odata_context_from_type("#ComputerSystem.v1_20_0.ComputerSystem"),
            Some("/redfish/v1/$metadata#ComputerSystem.ComputerSystem".to_string())
        );
    }

    #[test]
    fn test_odata_context_from_type_collection() {
        assert_eq!(
            odata_context_from_type("#ComputerSystemCollection.ComputerSystemCollection"),
            Some(
                "/redfish/v1/$metadata#ComputerSystemCollection.ComputerSystemCollection"
                    .to_string()
            )
        );
    }

    #[test]
    fn test_odata_context_from_type_version_with_dots() {
        assert_eq!(
            odata_context_from_type("#Manager.v1_19_0.Manager"),
            Some("/redfish/v1/$metadata#Manager.Manager".to_string())
        );
    }

    #[test]
    fn test_odata_context_from_type_no_hash() {
        assert_eq!(odata_context_from_type("NoHash"), None);
    }

    #[test]
    fn test_odata_context_from_type_empty() {
        assert_eq!(odata_context_from_type(""), None);
        assert_eq!(odata_context_from_type("#"), None);
    }

    #[test]
    fn test_compute_etag_deterministic() {
        let body = b"{\"Id\":\"vm1\"}";
        let etag1 = compute_etag(body);
        let etag2 = compute_etag(body);
        assert_eq!(etag1, etag2);
        assert!(etag1.starts_with("W/\""));
        assert!(etag1.ends_with('"'));
    }

    #[test]
    fn test_compute_etag_different_bodies() {
        let etag1 = compute_etag(b"{\"Id\":\"vm1\"}");
        let etag2 = compute_etag(b"{\"Id\":\"vm2\"}");
        assert_ne!(etag1, etag2);
    }
}
