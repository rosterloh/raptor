use crate::components::ui::{Button, Dialog, Input};
use crate::components::*;
use crate::{api, logic, Route};
use dioxus::prelude::*;
use raptor_api_types::SmCreate;

const LIMIT: u64 = 25;

#[component]
pub fn Modules() -> Element {
    let mut offset = use_signal(|| 0u64);
    let mut query = use_signal(String::new);
    let mut modules = use_resource(move || async move {
        let q = logic::fiql_contains(&["name", "version"], &query());
        api::list_modules(offset(), LIMIT, q.as_deref()).await
    });
    let mut show_create = use_signal(|| false);
    let nav = use_navigator();
    rsx! {
        div { class: "mb-4 flex items-center justify-between",
            h1 { class: "text-xl font-bold text-zinc-100", "Modules" }
            Button { onclick: move |_| show_create.set(true), "New module" }
        }
        div { class: "mb-3",
            SearchBox {
                placeholder: "Search name or version…",
                on_search: move |s| {
                    query.set(s);
                    offset.set(0);
                },
            }
        }
        match &*modules.read_unchecked() {
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Name" }
                            th { class: TH, "Version" }
                            th { class: TH, "Type" }
                            th { class: TH, "Vendor" }
                            th { class: TH, "Created" }
                        }
                    }
                    tbody {
                        for m in page.content.clone() {
                            tr {
                                key: "{m.id}",
                                class: ROW,
                                onclick: move |_| {
                                    nav.push(Route::ModuleDetail { id: m.id });
                                },
                                td { class: TD, "{m.name}" }
                                td { class: TD, "{m.version}" }
                                td { class: TD, "{m.module_type}" }
                                td { class: TD, {m.vendor.clone().unwrap_or_else(|| "-".into())} }
                                td { class: TD, {logic::format_ts(m.created_at)} }
                            }
                        }
                    }
                }
                Paginator {
                    offset: offset(),
                    limit: LIMIT,
                    total: page.total,
                    on_change: move |o| offset.set(o),
                }
            },
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| modules.restart() } },
            None => rsx! { p { class: "text-zinc-500", "Loading…" } },
        }
        CreateModuleDialog { open: show_create, on_created: move |_| modules.restart() }
    }
}

#[component]
fn CreateModuleDialog(open: Signal<bool>, on_created: EventHandler<()>) -> Element {
    let mut name = use_signal(String::new);
    let mut version = use_signal(String::new);
    let mut module_type = use_signal(|| "os".to_string());
    rsx! {
        Dialog { open,
            form {
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    let m = SmCreate {
                        name: name(),
                        version: version(),
                        module_type: module_type(),
                        vendor: None,
                        description: None,
                    };
                    spawn(async move {
                        match api::create_module(&m).await {
                            Ok(_) => {
                                toast_ok("module created");
                                open.set(false);
                                name.set(String::new());
                                version.set(String::new());
                                on_created.call(());
                            }
                            Err(e) => toast_error(e.to_string()),
                        }
                    });
                },
                h3 { class: "mb-3 text-lg font-semibold text-zinc-100", "New software module" }
                Input { class: "mb-3", placeholder: "Name", required: true, value: "{name}",
                    oninput: move |e: FormEvent| name.set(e.value()) }
                Input { class: "mb-3", placeholder: "Version", required: true, value: "{version}",
                    oninput: move |e: FormEvent| version.set(e.value()) }
                select {
                    class: INPUT,
                    value: "{module_type}",
                    onchange: move |e| module_type.set(e.value()),
                    option { value: "os", "os" }
                    option { value: "firmware", "firmware" }
                    option { value: "runtime", "runtime" }
                    option { value: "application", "application" }
                }
                div { class: "flex justify-end gap-2",
                    button {
                        class: "rounded px-3 py-1.5 text-sm text-zinc-300 hover:bg-zinc-800",
                        r#type: "button",
                        onclick: move |_| open.set(false),
                        "Cancel"
                    }
                    Button { r#type: "submit", "Create" }
                }
            }
        }
    }
}
