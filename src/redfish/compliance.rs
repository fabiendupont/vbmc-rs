use axum::body::Body;
use axum::http::{header, Method, Request, Response};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Tower layer that adds OData-Version and Link headers to all responses,
/// and handles HEAD requests by stripping the response body.
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
            let body = if is_head {
                Body::empty()
            } else {
                body
            };

            Ok(Response::from_parts(parts, body))
        })
    }
}
