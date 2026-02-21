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
    #[serde(rename = "@Message.ExtendedInfo", skip_serializing_if = "Vec::is_empty")]
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

    fn message(&self) -> &str {
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
