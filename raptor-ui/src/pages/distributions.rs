use crate::components::*;
use crate::{api, logic, Route};
use dioxus::prelude::*;
use raptor_api_types::DsCreate;

const LIMIT: u64 = 25;

#[component]
pub fn Distributions() -> Element {
    let mut offset = use_signal(|| 0u64);
    let mut query = use_signal(String::new);
    let mut sets = use_resource(move || async move {
        let q = logic::fiql_contains(&["name", "version"], &query());
        api::list_ds(offset(), LIMIT, q.as_deref()).await
    });
    let mut show_create = use_signal(|| false);
    let nav = use_navigator();
    rsx! {
        div { class: "mb-4 flex items-center justify-between",
            h1 { class: "text-xl font-bold text-zinc-100", "Distributions" }
            button { class: BTN, onclick: move |_| show_create.set(true), "New distribution set" }
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
        match &*sets.read_unchecked() {
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Name" }
                            th { class: TH, "Version" }
                            th { class: TH, "Type" }
                            th { class: TH, "Complete" }
                            th { class: TH, "Created" }
                        }
                    }
                    tbody {
                        for ds in page.content.clone() {
                            tr {
                                key: "{ds.id}",
                                class: ROW,
                                onclick: move |_| {
                                    nav.push(Route::DsDetail { id: ds.id });
                                },
                                td { class: TD, "{ds.name}" }
                                td { class: TD, "{ds.version}" }
                                td { class: TD, "{ds.ds_type}" }
                                td { class: TD, if ds.complete { "yes" } else { "no" } }
                                td { class: TD, {logic::format_ts(ds.created_at)} }
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
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| sets.restart() } },
            None => rsx! { p { class: "text-zinc-500", "Loading…" } },
        }
        CreateDsDialog { open: show_create, on_created: move |_| sets.restart() }
    }
}

#[component]
fn CreateDsDialog(open: Signal<bool>, on_created: EventHandler<()>) -> Element {
    let mut name = use_signal(String::new);
    let mut version = use_signal(String::new);
    let mut ds_type = use_signal(|| "os".to_string());
    rsx! {
        if open() {
            div { class: "fixed inset-0 z-40 flex items-center justify-center bg-black/60",
                form {
                    class: "w-96 rounded-lg border border-zinc-800 bg-zinc-900 p-6",
                    onsubmit: move |e: FormEvent| {
                        e.prevent_default();
                        let ds = DsCreate {
                            name: name(),
                            version: version(),
                            ds_type: ds_type(),
                            description: None,
                            required_migration_step: false,
                            modules: vec![],
                        };
                        spawn(async move {
                            match api::create_ds(&ds).await {
                                Ok(_) => {
                                    toast_ok("distribution set created");
                                    open.set(false);
                                    name.set(String::new());
                                    version.set(String::new());
                                    on_created.call(());
                                }
                                Err(e) => toast_error(e.to_string()),
                            }
                        });
                    },
                    h3 { class: "mb-3 text-lg font-semibold text-zinc-100", "New distribution set" }
                    input { class: INPUT, placeholder: "Name", required: true, value: "{name}",
                        oninput: move |e| name.set(e.value()) }
                    input { class: INPUT, placeholder: "Version", required: true, value: "{version}",
                        oninput: move |e| version.set(e.value()) }
                    select {
                        class: INPUT,
                        value: "{ds_type}",
                        onchange: move |e| ds_type.set(e.value()),
                        option { value: "os", "os" }
                        option { value: "app", "app" }
                        option { value: "os_app", "os_app" }
                    }
                    div { class: "flex justify-end gap-2",
                        button {
                            class: "rounded px-3 py-1.5 text-sm text-zinc-300 hover:bg-zinc-800",
                            r#type: "button",
                            onclick: move |_| open.set(false),
                            "Cancel"
                        }
                        button { class: BTN, r#type: "submit", "Create" }
                    }
                }
            }
        }
    }
}
