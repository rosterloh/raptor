use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../target/dx/raptor-ui/release/web/public"]
struct UiAssets;

/// Serve /ui/* from the embedded `dx build --release` output. Extensionless
/// paths fall back to index.html so client-side routes survive refresh.
pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches("/ui").trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    if let Some(f) = UiAssets::get(path) {
        return (
            [(header::CONTENT_TYPE, f.metadata.mimetype().to_string())],
            f.data.into_owned(),
        )
            .into_response();
    }
    let last = path.rsplit('/').next().unwrap_or("");
    if !last.contains('.') {
        if let Some(f) = UiAssets::get("index.html") {
            return (
                [(header::CONTENT_TYPE, "text/html".to_string())],
                f.data.into_owned(),
            )
                .into_response();
        }
    }
    StatusCode::NOT_FOUND.into_response()
}
