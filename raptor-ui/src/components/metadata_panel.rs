//! Metadata panel shared by the target, software-module and distribution-set
//! detail pages: list + add/edit/delete against `.../metadata`. Software
//! modules alone carry `targetVisible` (it surfaces the entry to devices in
//! DDI `deploymentBase` chunks); target and set metadata omit the toggle by
//! passing `show_visible: false`.

use crate::api;
use crate::components::ui::{Button, Dialog, Input};
use crate::components::*;
use dioxus::prelude::*;
use raptor_api_types::{MetadataCreate, MetadataRest, MetadataUpdate};

#[component]
pub fn MetadataPanel(prefix: String, #[props(default = false)] show_visible: bool) -> Element {
    let prefix_s = use_signal(|| prefix.clone());
    let mut entries = use_resource(move || async move { api::list_metadata(&prefix_s()).await });

    let mut show_form = use_signal(|| false);
    // None = create, Some = edit that entry.
    let mut form_for = use_signal(|| None::<MetadataRest>);

    rsx! {
        div { class: "mb-2 flex items-center justify-between",
            h2 { class: "font-semibold text-foreground", "Metadata" }
            button {
                class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                onclick: move |_| {
                    form_for.set(None);
                    show_form.set(true);
                },
                "Add…"
            }
        }
        match &*entries.read_unchecked() {
            Some(Ok(list)) if list.is_empty() => rsx! {
                p { class: "text-sm text-muted-foreground", "No metadata yet." }
            },
            Some(Ok(list)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Key" }
                            th { class: TH, "Value" }
                            if show_visible {
                                th { class: TH, "Device-visible" }
                            }
                            th { class: TH, "" }
                        }
                    }
                    tbody {
                        for m in list.clone() {
                            tr { key: "{m.key}",
                                td { class: "{TD} font-mono text-xs", "{m.key}" }
                                td { class: "{TD} break-all", "{m.value}" }
                                if show_visible {
                                    td { class: TD,
                                        if m.target_visible.unwrap_or(false) {
                                            "yes"
                                        } else {
                                            "no"
                                        }
                                    }
                                }
                                td { class: TD,
                                    div { class: "flex justify-end gap-2",
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                                            onclick: {
                                                let m = m.clone();
                                                move |_| {
                                                    form_for.set(Some(m.clone()));
                                                    show_form.set(true);
                                                }
                                            },
                                            "Edit"
                                        }
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-err hover:bg-accent",
                                            onclick: {
                                                let key = m.key.clone();
                                                let prefix = prefix_s();
                                                move |_| {
                                                    let key = key.clone();
                                                    let prefix = prefix.clone();
                                                    spawn(async move {
                                                        match api::delete_metadata(&prefix, &key).await {
                                                            Ok(()) => {
                                                                toast_ok(format!("deleted {key}"));
                                                                entries.restart();
                                                            }
                                                            Err(e) => toast_error(e.to_string()),
                                                        }
                                                    });
                                                }
                                            },
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! {
                ErrorPane { message: e.to_string(), on_retry: move |_| entries.restart() }
            },
            None => rsx! {
                p { class: "text-muted-foreground", "Loading…" }
            },
        }
        if show_form() {
            MetadataFormDialog {
                open: show_form,
                prefix: prefix_s(),
                show_visible,
                existing: form_for(),
                on_saved: move |_| entries.restart(),
            }
        }
    }
}

#[component]
fn FieldError(message: Option<String>) -> Element {
    match message {
        Some(m) => rsx! {
            p { class: "mb-3 text-xs text-err", "{m}" }
        },
        None => rsx! {},
    }
}

/// Create/edit form. The key is only settable on create — `PUT .../metadata/{key}`
/// addresses the entry by key, so renaming isn't a thing this dialog offers.
/// A duplicate key on create comes back as 409 and lands on the key field.
#[component]
fn MetadataFormDialog(
    open: Signal<bool>,
    prefix: String,
    show_visible: bool,
    existing: Option<MetadataRest>,
    on_saved: EventHandler<()>,
) -> Element {
    let editing_key = existing.as_ref().map(|m| m.key.clone());
    let mut key = use_signal(|| existing.as_ref().map(|m| m.key.clone()).unwrap_or_default());
    let mut value = use_signal(|| {
        existing
            .as_ref()
            .map(|m| m.value.clone())
            .unwrap_or_default()
    });
    let mut visible = use_signal(|| {
        existing
            .as_ref()
            .and_then(|m| m.target_visible)
            .unwrap_or(false)
    });
    let mut key_error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);

    rsx! {
        Dialog { open, class: "w-[28rem]",
            form {
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    let k = key().trim().to_string();
                    if k.is_empty() {
                        return;
                    }
                    let v = value();
                    key_error.set(None);
                    saving.set(true);
                    let prefix = prefix.clone();
                    let editing_key = editing_key.clone();
                    spawn(async move {
                        let saved = match editing_key {
                            Some(existing_key) => {
                                api::update_metadata(
                                        &prefix,
                                        &existing_key,
                                        &MetadataUpdate {
                                            value: v,
                                            target_visible: show_visible.then_some(visible()),
                                        },
                                    )
                                    .await
                            }
                            None => {
                                api::create_metadata(
                                        &prefix,
                                        &MetadataCreate {
                                            key: k,
                                            value: v,
                                            target_visible: visible(),
                                        },
                                    )
                                    .await
                            }
                        };
                        saving.set(false);
                        match saved {
                            Ok(m) => {
                                toast_ok(format!("metadata {} saved", m.key));
                                open.set(false);
                                on_saved.call(());
                            }
                            Err(api::ApiError::Server { status: 409, message }) => {
                                key_error.set(Some(message))
                            }
                            Err(e) => toast_error(e.to_string()),
                        }
                    });
                },
                h3 { class: "mb-3 text-lg font-semibold text-foreground",
                    if editing_key.is_some() {
                        "Edit metadata"
                    } else {
                        "New metadata"
                    }
                }
                Input {
                    class: "mb-3 font-mono text-sm",
                    placeholder: "Key",
                    required: true,
                    disabled: editing_key.is_some(),
                    value: "{key}",
                    oninput: move |e: FormEvent| {
                        key.set(e.value());
                        key_error.set(None);
                    },
                }
                FieldError { message: key_error() }
                Input {
                    class: "mb-3",
                    placeholder: "Value",
                    required: true,
                    value: "{value}",
                    oninput: move |e: FormEvent| value.set(e.value()),
                }
                if show_visible {
                    label { class: "mb-4 flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: visible(),
                            onchange: move |e| visible.set(e.checked()),
                        }
                        "Device-visible (sent to the target in deploymentBase)"
                    }
                }
                div { class: "flex justify-end gap-2",
                    button {
                        class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                        r#type: "button",
                        onclick: move |_| open.set(false),
                        "Cancel"
                    }
                    Button { r#type: "submit", disabled: saving(), "Save" }
                }
            }
        }
    }
}
