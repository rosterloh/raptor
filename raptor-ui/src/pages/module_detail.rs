use crate::components::*;
use crate::{api, Route};
use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
struct UploadState {
    filename: String,
    progress: f64, // 0.0-1.0, 1.0 = server processing
}

#[component]
pub fn ModuleDetail(id: i64) -> Element {
    let module = use_resource(move || async move { api::get_module(id).await });
    let mut artifacts = use_resource(move || async move { api::module_artifacts(id).await });
    let mut uploads = use_signal(Vec::<UploadState>::new);
    let mut confirm_delete = use_signal(|| false);
    let nav = use_navigator();

    let on_files = move |e: FormEvent| {
        for file in e.files() {
            let name = file.name();
            uploads.write().push(UploadState {
                filename: name.clone(),
                progress: 0.0,
            });
            spawn(async move {
                let result = match file.read_bytes().await {
                    Ok(bytes) => {
                        #[cfg(target_arch = "wasm32")]
                        {
                            let n = name.clone();
                            api::upload_artifact(id, &n.clone(), bytes.to_vec(), move |p| {
                                for u in uploads.write().iter_mut() {
                                    if u.filename == n {
                                        u.progress = p;
                                    }
                                }
                            })
                            .await
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let _ = bytes;
                            Ok(())
                        }
                    }
                    Err(e) => Err(api::ApiError::Network(format!("read failed: {e:?}"))),
                };
                uploads.write().retain(|u| u.filename != name);
                match result {
                    Ok(()) => {
                        toast_ok(format!("uploaded {name}"));
                        artifacts.restart();
                    }
                    Err(e) => toast_error(format!("{name}: {e}")),
                }
            });
        }
    };

    rsx! {
        match &*module.read_unchecked() {
            Some(Ok(m)) => rsx! {
                h1 { class: HEADING, "{m.name} {m.version}" }
                p { class: "mb-4 text-sm text-zinc-400", "type {m.module_type}" }
            },
            Some(Err(e)) => rsx! { p { class: "text-red-400", "{e}" } },
            None => rsx! { p { class: "text-zinc-500", "Loading…" } },
        }
        div { class: "mb-4 flex gap-2",
            button { class: BTN_DANGER, onclick: move |_| confirm_delete.set(true), "Delete module" }
        }
        div { class: CARD,
            h2 { class: "mb-2 font-semibold text-zinc-100", "Artifacts" }
            // A file input stretched over the dropzone: browsers natively accept
            // drag-and-drop onto <input type=file>, no JS drop handling needed.
            label { class: "relative mb-4 block cursor-pointer rounded border-2 border-dashed border-zinc-700 p-6 text-center text-sm text-zinc-500 hover:border-emerald-700 hover:text-zinc-300",
                "Drop files here or click to browse"
                input {
                    r#type: "file",
                    multiple: true,
                    class: "absolute inset-0 h-full w-full cursor-pointer opacity-0",
                    onchange: on_files,
                }
            }
            for u in uploads() {
                div { key: "{u.filename}", class: "mb-2 text-sm",
                    span { class: "text-zinc-400", "{u.filename} " }
                    div { class: "mt-1 h-1.5 w-full rounded bg-zinc-800",
                        div {
                            class: "h-1.5 rounded bg-emerald-600 transition-all",
                            style: "width: {u.progress * 100.0}%",
                        }
                    }
                }
            }
            match &*artifacts.read_unchecked() {
                Some(Ok(list)) if list.is_empty() => rsx! {
                    p { class: "text-sm text-zinc-500", "No artifacts yet." }
                },
                Some(Ok(list)) => rsx! {
                    table { class: TABLE,
                        thead {
                            tr {
                                th { class: TH, "Filename" }
                                th { class: TH, "Size" }
                                th { class: TH, "SHA1" }
                                th { class: TH, "" }
                            }
                        }
                        tbody {
                            for a in list.clone() {
                                tr { key: "{a.id}",
                                    td { class: TD,
                                        a {
                                            class: "text-emerald-400 hover:underline",
                                            href: api::artifact_download_href(id, a.id),
                                            download: "{a.provided_filename}",
                                            "{a.provided_filename}"
                                        }
                                    }
                                    td { class: TD, "{a.size} B" }
                                    td { class: "{TD} font-mono text-xs", "{a.hashes.sha1}" }
                                    td { class: TD,
                                        button {
                                            class: "text-xs text-red-400 hover:underline",
                                            onclick: move |_| {
                                                spawn(async move {
                                                    match api::delete_artifact(id, a.id).await {
                                                        Ok(()) => {
                                                            toast_ok("artifact deleted");
                                                            artifacts.restart();
                                                        }
                                                        Err(e) => toast_error(e.to_string()),
                                                    }
                                                });
                                            },
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! { p { class: "text-sm text-red-400", "{e}" } },
                None => rsx! { p { class: "text-zinc-500", "Loading…" } },
            }
        }
        ConfirmDialog {
            title: "Delete module".to_string(),
            message: "Delete this module and all its artifacts?".to_string(),
            open: confirm_delete,
            on_confirm: move |_| {
                spawn(async move {
                    match api::delete_module(id).await {
                        Ok(()) => {
                            toast_ok("module deleted");
                            nav.push(Route::Modules {});
                        }
                        Err(e) => toast_error(e.to_string()),
                    }
                });
            },
        }
    }
}
