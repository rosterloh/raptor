use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    NotFound(&'static str),
    BadRequest(String),
    Unauthorized,
    Conflict(String),
    Gone,
    Db(sea_orm::DbErr),
    Io(std::io::Error),
}

impl From<sea_orm::DbErr> for AppError {
    fn from(e: sea_orm::DbErr) -> Self { AppError::Db(e) }
}
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self { AppError::Io(e) }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // errorCode strings mirror hawkBit's (incl. its "entitiy" typo); clients mostly
        // branch on status codes, which are the hard contract.
        let (status, class, code, msg) = match self {
            AppError::NotFound(what) => (
                StatusCode::NOT_FOUND,
                "org.eclipse.hawkbit.repository.exception.EntityNotFoundException",
                "hawkbit.server.error.repo.entitiyNotFound",
                format!("{what} not found"),
            ),
            AppError::BadRequest(m) => (
                StatusCode::BAD_REQUEST,
                "org.eclipse.hawkbit.rest.exception.MessageNotReadableException",
                "hawkbit.server.error.rest.body.notReadable",
                m,
            ),
            AppError::Unauthorized => {
                let body = json!({
                    "exceptionClass": "org.springframework.security.authentication.BadCredentialsException",
                    "errorCode": "hawkbit.server.error.unauthorized",
                    "message": "unauthorized",
                });
                return (StatusCode::UNAUTHORIZED, [("WWW-Authenticate", "Basic realm=\"raptor\"")], Json(body)).into_response();
            }
            AppError::Conflict(m) => (
                StatusCode::CONFLICT,
                "org.eclipse.hawkbit.repository.exception.EntityAlreadyExistsException",
                "hawkbit.server.error.repo.entitiyAlreadyExists",
                m,
            ),
            AppError::Gone => (
                StatusCode::GONE,
                "org.eclipse.hawkbit.repository.exception.CancelActionNotAllowedException",
                "hawkbit.server.error.repo.actionNotActive",
                "action is not active".into(),
            ),
            AppError::Db(e) => {
                tracing::error!(error = %e, "database error");
                (StatusCode::INTERNAL_SERVER_ERROR, "java.lang.RuntimeException", "hawkbit.server.error.internal", "internal error".into())
            }
            AppError::Io(e) => {
                tracing::error!(error = %e, "io error");
                (StatusCode::INTERNAL_SERVER_ERROR, "java.lang.RuntimeException", "hawkbit.server.error.internal", "internal error".into())
            }
        };
        (status, Json(json!({"exceptionClass": class, "errorCode": code, "message": msg}))).into_response()
    }
}
