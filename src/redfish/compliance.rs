use axum::body::{Body, to_bytes};
use axum::http::{Method, Request, Response, header};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Tower layer that adds OData compliance to all responses:
/// - `OData-Version: 4.0` header
/// - `Link: </redfish/v1/$metadata>; rel=describedby` header
/// - `@odata.context` injected into JSON response bodies that have `@odata.type`
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

/// Derive `@odata.context` from `@odata.type`.
///
/// `@odata.type` is formatted as `#Namespace.Version.TypeName`,
/// e.g. `#ComputerSystem.v1_20_0.ComputerSystem`.
/// The context should be `/redfish/v1/$metadata#Namespace.TypeName`,
/// e.g. `/redfish/v1/$metadata#ComputerSystem.ComputerSystem`.
fn odata_context_from_type(odata_type: &str) -> Option<String> {
    let stripped = odata_type.strip_prefix('#')?;
    let parts: Vec<&str> = stripped.split('.').collect();
    if parts.len() >= 3 {
        // Namespace.version_parts.TypeName — take first and last
        let namespace = parts[0];
        let type_name = parts[parts.len() - 1];
        Some(format!("/redfish/v1/$metadata#{namespace}.{type_name}"))
    } else if parts.len() == 2 {
        Some(format!("/redfish/v1/$metadata#{stripped}"))
    } else {
        None
    }
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
        let mut inner = self.inner.clone();

        // Convert HEAD to GET so handlers run normally
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

            // Add OData compliance headers
            parts.headers.insert(
                header::HeaderName::from_static("odata-version"),
                header::HeaderValue::from_static("4.0"),
            );
            parts.headers.insert(
                header::LINK,
                header::HeaderValue::from_static("</redfish/v1/$metadata>; rel=describedby"),
            );

            // Strip body for HEAD requests
            if is_head {
                return Ok(Response::from_parts(parts, Body::empty()));
            }

            // For JSON responses, inject @odata.context if missing
            let is_json = parts
                .headers
                .get(header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|ct| ct.contains("application/json"))
                .unwrap_or(false);

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
                    // Update content-length
                    parts.headers.insert(
                        header::CONTENT_LENGTH,
                        header::HeaderValue::from(new_body.len()),
                    );
                    return Ok(Response::from_parts(parts, Body::from(new_body)));
                }

                // JSON parse failed, return original bytes
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
}
