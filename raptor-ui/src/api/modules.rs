//! Software module & artifact endpoints: listing, detail, upload, and download.

use raptor_api_types::*;

#[cfg(target_arch = "wasm32")]
use super::ApiError;
use super::{ApiResult, delete, get_json, list_path, post_json};

pub async fn list_modules(
    offset: u64,
    limit: u64,
    q: Option<&str>,
) -> ApiResult<PagedList<SmRest>> {
    get_json(&list_path("/rest/v1/softwaremodules", offset, limit, q)).await
}

pub async fn get_module(id: i64) -> ApiResult<SmRest> {
    get_json(&format!("/rest/v1/softwaremodules/{id}")).await
}

pub async fn create_module(m: &SmCreate) -> ApiResult<Vec<SmRest>> {
    post_json("/rest/v1/softwaremodules", std::slice::from_ref(m)).await
}

pub async fn delete_module(id: i64) -> ApiResult<()> {
    delete(&format!("/rest/v1/softwaremodules/{id}")).await
}

pub async fn module_artifacts(id: i64) -> ApiResult<Vec<ArtifactRest>> {
    get_json(&format!("/rest/v1/softwaremodules/{id}/artifacts")).await
}

pub async fn delete_artifact(module_id: i64, artifact_id: i64) -> ApiResult<()> {
    delete(&format!(
        "/rest/v1/softwaremodules/{module_id}/artifacts/{artifact_id}"
    ))
    .await
}

pub fn artifact_download_href(module_id: i64, artifact_id: i64) -> String {
    format!("/rest/v1/softwaremodules/{module_id}/artifacts/{artifact_id}/download")
}

/// Multipart upload with progress (0.0-1.0). XmlHttpRequest instead of fetch:
/// XHR exposes upload.onprogress; fetch does not.
#[cfg(target_arch = "wasm32")]
pub async fn upload_artifact(
    module_id: i64,
    filename: &str,
    bytes: Vec<u8>,
    mut on_progress: impl FnMut(f64) + 'static,
) -> ApiResult<()> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;

    /// Keeps the XHR and its closures alive together, and aborts the request
    /// (detaching handlers first) if the upload future is dropped mid-flight
    /// (e.g. the user navigates away). Without this, a late-firing XHR event
    /// would invoke an already-dropped Closure and panic/poison the wasm
    /// instance.
    struct XhrGuard {
        xhr: web_sys::XmlHttpRequest,
        _onprog: Closure<dyn FnMut(web_sys::ProgressEvent)>,
        _onloadend: Closure<dyn FnMut()>,
    }

    impl Drop for XhrGuard {
        fn drop(&mut self) {
            // Detach handlers BEFORE abort: abort() can synchronously drive
            // request-error/loadend handling, and it must not reach into a
            // Rust closure that is about to be freed.
            self.xhr.set_onloadend(None);
            if let Ok(upload) = self.xhr.upload() {
                upload.set_onprogress(None);
            }
            // No-op if the request already completed.
            self.xhr.abort().ok();
        }
    }

    let xhr =
        web_sys::XmlHttpRequest::new().map_err(|_| ApiError::Network("XHR unavailable".into()))?;
    xhr.open(
        "POST",
        &format!(
            "{}/rest/v1/softwaremodules/{module_id}/artifacts",
            super::base()
        ),
    )
    .map_err(|_| ApiError::Network("XHR open failed".into()))?;
    xhr.set_request_header(super::AJAX_HEADER.0, super::AJAX_HEADER.1)
        .map_err(|_| ApiError::Network("XHR header failed".into()))?;

    let form = web_sys::FormData::new().unwrap();
    let arr = js_sys::Uint8Array::from(bytes.as_slice());
    let parts = js_sys::Array::new();
    parts.push(&arr.buffer());
    let blob = web_sys::Blob::new_with_u8_array_sequence(&parts).unwrap();
    form.append_with_blob_and_filename("file", &blob, filename)
        .unwrap();

    let onprog =
        Closure::<dyn FnMut(web_sys::ProgressEvent)>::new(move |e: web_sys::ProgressEvent| {
            if e.length_computable() && e.total() > 0.0 {
                on_progress(e.loaded() / e.total());
            }
        });
    xhr.upload()
        .unwrap()
        .set_onprogress(Some(onprog.as_ref().unchecked_ref()));

    let (tx, rx) = futures::channel::oneshot::channel::<()>();
    let mut tx = Some(tx);
    let onloadend = Closure::<dyn FnMut()>::new(move || {
        if let Some(tx) = tx.take() {
            let _ = tx.send(());
        }
    });
    xhr.set_onloadend(Some(onloadend.as_ref().unchecked_ref()));

    let guard = XhrGuard {
        xhr: xhr.clone(),
        _onprog: onprog,
        _onloadend: onloadend,
    };

    xhr.send_with_opt_form_data(Some(&form))
        .map_err(|_| ApiError::Network("XHR send failed".into()))?;
    rx.await
        .map_err(|_| ApiError::Network("upload interrupted".into()))?;

    let status = guard.xhr.status().unwrap_or(0);
    let body = guard.xhr.response_text().ok().flatten().unwrap_or_default();

    match status {
        201 => Ok(()),
        401 => {
            super::redirect_to_login();
            Err(ApiError::Unauthorized)
        }
        s => {
            let message = serde_json::from_str::<ErrorBody>(&body)
                .map(|e| e.message)
                .unwrap_or_else(|_| format!("HTTP {s}"));
            Err(ApiError::Server { status: s, message })
        }
    }
}
