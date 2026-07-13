use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use raptor_api_types::ErrorBody;

#[derive(Debug)]
pub enum AppError {
    NotFound(&'static str),
    BadRequest(String),
    Unauthorized,
    /// Same as `Unauthorized` but omits `WWW-Authenticate`, so browsers don't
    /// pop their native Basic-Auth dialog for the SPA's own session checks.
    UnauthorizedQuiet,
    Conflict(String),
    Gone,
    Db(sea_orm::DbErr),
    Io(std::io::Error),
}

impl From<sea_orm::DbErr> for AppError {
    fn from(e: sea_orm::DbErr) -> Self {
        AppError::Db(e)
    }
}
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
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
            AppError::Unauthorized | AppError::UnauthorizedQuiet => {
                let body = ErrorBody {
                    exception_class:
                        "org.springframework.security.authentication.BadCredentialsException".into(),
                    error_code: "hawkbit.server.error.unauthorized".into(),
                    message: "unauthorized".into(),
                };
                return if matches!(self, AppError::Unauthorized) {
                    (
                        StatusCode::UNAUTHORIZED,
                        [("WWW-Authenticate", "Basic realm=\"raptor\"")],
                        Json(body),
                    )
                        .into_response()
                } else {
                    (StatusCode::UNAUTHORIZED, Json(body)).into_response()
                };
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
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "java.lang.RuntimeException",
                    "hawkbit.server.error.internal",
                    "internal error".into(),
                )
            }
            AppError::Io(e) => {
                tracing::error!(error = %e, "io error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "java.lang.RuntimeException",
                    "hawkbit.server.error.internal",
                    "internal error".into(),
                )
            }
        };
        (
            status,
            Json(ErrorBody {
                exception_class: class.into(),
                error_code: code.into(),
                message: msg,
            }),
        )
            .into_response()
    }
}
