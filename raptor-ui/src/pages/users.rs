//! Operator users (admins only; the nav link is hidden from everyone else and
//! the server answers them 403). OIDC users appear here after their first
//! sign-in and have no password to set.

use crate::api;
use crate::components::ui::{Button, Dialog, Input};
use crate::components::*;
use dioxus::prelude::*;
use raptor_api_types::{UserCreate, UserRest, UserUpdate};

const LIMIT: u64 = 25;

#[component]
pub fn Users() -> Element {
    let mut offset = use_signal(|| 0u64);
    let mut users = use_resource(move || async move { api::list_users(offset(), LIMIT).await });
    let mut show_form = use_signal(|| false);
    let mut form_for = use_signal(|| None::<UserRest>);
    let mut confirm_delete = use_signal(|| false);
    let mut delete_for = use_signal(|| None::<UserRest>);

    rsx! {
        document::Title { "Users — raptor" }
        div { class: "mb-4 flex items-center justify-between",
            h1 { class: PAGE_TITLE, "Users" }
            Button {
                onclick: move |_| {
                    form_for.set(None);
                    show_form.set(true);
                },
                "New user"
            }
        }
        match &*users.read_unchecked() {
            Some(Ok(page)) if page.content.is_empty() => rsx! {
                p { class: "text-sm text-muted-foreground",
                    "No users yet. The config account always signs in as admin."
                }
            },
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Username" }
                            th { class: TH, "Role" }
                            th { class: TH, "Source" }
                            th { class: TH, "Enabled" }
                            th { class: TH, "" }
                        }
                    }
                    tbody {
                        for u in page.content.clone() {
                            tr { key: "{u.id}", class: ROW,
                                td { class: TD, "{u.username}" }
                                td { class: "{TD} font-mono text-xs", "{u.role}" }
                                td { class: "{TD} text-fg-dim", "{u.source}" }
                                td { class: "{TD} text-fg-dim", if u.enabled { "yes" } else { "no" } }
                                td { class: TD,
                                    div { class: "flex justify-end gap-2",
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                                            onclick: {
                                                let u = u.clone();
                                                move |_| {
                                                    form_for.set(Some(u.clone()));
                                                    show_form.set(true);
                                                }
                                            },
                                            "Edit"
                                        }
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-err hover:bg-accent",
                                            onclick: move |_| {
                                                delete_for.set(Some(u.clone()));
                                                confirm_delete.set(true);
                                            },
                                            "Delete"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                Paginator { offset: offset(), limit: LIMIT, total: page.total, on_change: move |o| offset.set(o) }
            },
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| users.restart() } },
            None => rsx! { p { class: "text-muted-foreground", "Loading…" } },
        }
        if show_form() {
            UserFormDialog { open: show_form, existing: form_for(), on_saved: move |_| users.restart() }
        }
        ConfirmDialog {
            title: "Delete user".to_string(),
            message: delete_for()
                .map(|u| format!("Delete user \"{}\"? Their sessions end at once.", u.username))
                .unwrap_or_default(),
            open: confirm_delete,
            on_confirm: move |_| {
                let Some(u) = delete_for() else { return };
                spawn(async move {
                    match api::delete_user(u.id).await {
                        Ok(()) => {
                            toast_ok(format!("deleted {}", u.username));
                            users.restart();
                        }
                        Err(e) => toast_error(e.message()),
                    }
                });
            },
        }
    }
}

#[component]
fn UserFormDialog(
    open: Signal<bool>,
    existing: Option<UserRest>,
    on_saved: EventHandler<()>,
) -> Element {
    let editing_id = existing.as_ref().map(|u| u.id);
    let is_local = existing.as_ref().is_none_or(|u| u.source == "local");
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut role = use_signal(|| {
        existing
            .as_ref()
            .map(|u| u.role.clone())
            .unwrap_or_else(|| "read_only".into())
    });
    let mut enabled = use_signal(|| existing.as_ref().is_none_or(|u| u.enabled));
    let mut error = use_signal(|| None::<String>);
    let title = existing
        .as_ref()
        .map(|u| u.username.clone())
        .unwrap_or_else(|| "New user".into());

    rsx! {
        Dialog { open, class: "w-[26rem]",
            form {
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    error.set(None);
                    let pw = password();
                    spawn(async move {
                        let saved = match editing_id {
                            Some(id) => {
                                api::update_user(id, &UserUpdate {
                                        role: Some(role()),
                                        enabled: Some(enabled()),
                                        password: (!pw.is_empty()).then_some(pw),
                                    })
                                    .await
                            }
                            None => {
                                api::create_user(&UserCreate {
                                        username: username().trim().to_string(),
                                        password: pw,
                                        role: role(),
                                        enabled: None,
                                    })
                                    .await
                            }
                        };
                        match saved {
                            Ok(u) => {
                                toast_ok(format!("user {} saved", u.username));
                                open.set(false);
                                on_saved.call(());
                            }
                            Err(e) => error.set(Some(e.message())),
                        }
                    });
                },
                h3 { class: "mb-3 text-lg font-semibold text-foreground", "{title}" }
                if editing_id.is_none() {
                    Input { class: "mb-3", placeholder: "Username", required: true, value: "{username}",
                        oninput: move |e: FormEvent| username.set(e.value()) }
                }
                if is_local {
                    Input {
                        class: "mb-3",
                        r#type: "password",
                        placeholder: if editing_id.is_some() { "New password (leave empty to keep)" } else { "Password" },
                        required: editing_id.is_none(),
                        value: "{password}",
                        oninput: move |e: FormEvent| password.set(e.value()),
                    }
                }
                select {
                    class: SELECT,
                    value: "{role}",
                    onchange: move |e| role.set(e.value()),
                    option { value: "read_only", "read only" }
                    option { value: "admin", "admin" }
                }
                if editing_id.is_some() {
                    label { class: "mb-4 flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: enabled(),
                            onchange: move |e| enabled.set(e.checked()),
                        }
                        "Enabled"
                    }
                }
                if let Some(m) = error() {
                    p { class: "mb-3 text-xs text-err", "{m}" }
                }
                div { class: "flex justify-end gap-2",
                    button {
                        class: "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent",
                        r#type: "button",
                        onclick: move |_| open.set(false),
                        "Cancel"
                    }
                    Button { r#type: "submit", "Save" }
                }
            }
        }
    }
}
