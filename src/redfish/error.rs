use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RedfishError {
    pub error: RedfishErrorBody,
}

#[derive(Debug, Clone, Serialize)]
pub struct RedfishErrorBody {
    pub code: String,
    pub message: String,
    #[serde(
        rename = "@Message.ExtendedInfo",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub extended_info: Vec<RedfishMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RedfishMessage {
    #[serde(rename = "MessageId")]
    pub message_id: String,
    #[serde(rename = "Message")]
    pub message: String,
    #[serde(rename = "MessageArgs", skip_serializing_if = "Vec::is_empty")]
    pub message_args: Vec<String>,
    #[serde(rename = "Severity")]
    pub severity: String,
    #[serde(rename = "Resolution", skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub enum RedfishApiError {
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    InternalError(String),
    Unauthorized(String),
    Forbidden(String),
    ServiceUnavailable(String),
    ActionNotAllowed(String),
}

impl RedfishApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::ActionNotAllowed(_) => StatusCode::METHOD_NOT_ALLOWED,
        }
    }

    fn code(&self) -> &str {
        match self {
            Self::NotFound(_) => "Base.1.0.GeneralError",
            Self::BadRequest(_) => "Base.1.0.GeneralError",
            Self::Conflict(_) => "Base.1.0.GeneralError",
            Self::InternalError(_) => "Base.1.0.InternalError",
            Self::Unauthorized(_) => "Base.1.0.GeneralError",
            Self::Forbidden(_) => "Base.1.0.GeneralError",
            Self::ServiceUnavailable(_) => "Base.1.0.GeneralError",
            Self::ActionNotAllowed(_) => "Base.1.0.ActionNotSupported",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            Self::NotFound(m)
            | Self::BadRequest(m)
            | Self::Conflict(m)
            | Self::InternalError(m)
            | Self::Unauthorized(m)
            | Self::Forbidden(m)
            | Self::ServiceUnavailable(m)
            | Self::ActionNotAllowed(m) => m,
        }
    }
}

impl IntoResponse for RedfishApiError {
    fn into_response(self) -> Response {
        let body = RedfishError {
            error: RedfishErrorBody {
                code: self.code().to_string(),
                message: self.message().to_string(),
                extended_info: vec![],
            },
        };
        (self.status_code(), axum::Json(body)).into_response()
    }
}

impl From<anyhow::Error> for RedfishApiError {
    fn from(err: anyhow::Error) -> Self {
        RedfishApiError::InternalError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_codes() {
        assert_eq!(
            RedfishApiError::NotFound("x".into()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            RedfishApiError::BadRequest("x".into()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            RedfishApiError::Conflict("x".into()).status_code(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            RedfishApiError::InternalError("x".into()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            RedfishApiError::Unauthorized("x".into()).status_code(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            RedfishApiError::Forbidden("x".into()).status_code(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            RedfishApiError::ServiceUnavailable("x".into()).status_code(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            RedfishApiError::ActionNotAllowed("x".into()).status_code(),
            StatusCode::METHOD_NOT_ALLOWED
        );
    }

    #[test]
    fn test_error_codes() {
        assert_eq!(
            RedfishApiError::InternalError("x".into()).code(),
            "Base.1.0.InternalError"
        );
        assert_eq!(
            RedfishApiError::ActionNotAllowed("x".into()).code(),
            "Base.1.0.ActionNotSupported"
        );
        assert_eq!(
            RedfishApiError::NotFound("x".into()).code(),
            "Base.1.0.GeneralError"
        );
    }

    #[test]
    fn test_error_message_preserved() {
        let msg = "System 'vm99' not found";
        assert_eq!(RedfishApiError::NotFound(msg.into()).message(), msg);
        assert_eq!(RedfishApiError::InternalError(msg.into()).message(), msg);
    }

    #[test]
    fn test_from_anyhow() {
        let err = anyhow::anyhow!("something broke");
        let redfish_err = RedfishApiError::from(err);
        assert_eq!(redfish_err.message(), "something broke");
        assert_eq!(redfish_err.status_code(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_into_response_body() {
        let err = RedfishApiError::NotFound("missing resource".into());
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
