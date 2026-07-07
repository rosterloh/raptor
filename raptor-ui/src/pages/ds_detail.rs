use crate::components::*;
use crate::{api, logic, Route};
use dioxus::prelude::*;

#[component]
pub fn DsDetail(id: i64) -> Element {
    let mut ds = use_resource(move || async move { api::get_ds(id).await });
    let mut show_modules = use_signal(|| false);
    let mut show_deploy = use_signal(|| false);
    let mut confirm_delete = use_signal(|| false);
    let nav = use_navigator();
    rsx! {
        match &*ds.read_unchecked() {
            Some(Ok(d)) => rsx! {
                h1 { class: HEADING, "{d.name} {d.version}" }
                div { class: "mb-4 flex gap-2",
                    button { class: BTN, onclick: move |_| show_deploy.set(true), "Deploy…" }
                    button { class: BTN, onclick: move |_| show_modules.set(true), "Assign modules" }
                    button { class: BTN_DANGER, onclick: move |_| confirm_delete.set(true), "Delete" }
                }
                div { class: CARD,
                    p { class: "mb-2 text-sm text-zinc-400",
                        "type {d.ds_type} · "
                        if d.complete { "complete" } else { "incomplete" }
                        " · created {logic::format_ts(d.created_at)}"
                    }
                    h2 { class: "mb-2 font-semibold text-zinc-100", "Modules" }
                    if d.modules.is_empty() {
                        p { class: "text-sm text-zinc-500", "No modules assigned — the set is not deployable yet." }
                    } else {
                        ul { class: "space-y-1 text-sm",
                            for m in d.modules.clone() {
                                li { key: "{m.id}",
                                    Link { to: Route::ModuleDetail { id: m.id }, class: "text-emerald-400 hover:underline",
                                        "{m.name} {m.version} ({m.module_type})"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            Some(Err(e)) => rsx! { ErrorPane { message: e.to_string(), on_retry: move |_| ds.restart() } },
            None => rsx! { p { class: "text-zinc-500", "Loading…" } },
        }
        AssignModulesDialog { open: show_modules, ds_id: id, on_done: move |_| ds.restart() }
        DeployDialog { open: show_deploy, ds_id: id }
        ConfirmDialog {
            title: "Delete distribution set".to_string(),
            message: "Delete this distribution set? Sets referenced by actions cannot be deleted.".to_string(),
            open: confirm_delete,
            on_confirm: move |_| {
                spawn(async move {
                    match api::delete_ds(id).await {
                        Ok(()) => {
                            toast_ok("deleted");
                            nav.push(Route::Distributions {});
                        }
                        Err(e) => toast_error(e.to_string()),
                    }
                });
            },
        }
    }
}

#[component]
fn AssignModulesDialog(open: Signal<bool>, ds_id: i64, on_done: EventHandler<()>) -> Element {
    let modules = use_resource(move || async move {
        if open() {
            api::list_modules(0, 100, None).await
        } else {
            Ok(raptor_api_types::PagedList::new(vec![], 0))
        }
    });
    let mut selected = use_signal(Vec::<i64>::new);
    rsx! {
        if open() {
            div { class: "fixed inset-0 z-40 flex items-center justify-center bg-black/60",
                div { class: "max-h-[80vh] w-[28rem] overflow-y-auto rounded-lg border border-zinc-800 bg-zinc-900 p-6",
                    h3 { class: "mb-3 text-lg font-semibold text-zinc-100", "Assign software modules" }
                    match &*modules.read_unchecked() {
                        Some(Ok(page)) => rsx! {
                            ul { class: "mb-4 space-y-1",
                                for m in page.content.clone() {
                                    li { key: "{m.id}",
                                        label { class: "flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm hover:bg-zinc-800",
                                            input {
                                                r#type: "checkbox",
                                                checked: selected().contains(&m.id),
                                                onchange: move |e| {
                                                    let mut s = selected();
                                                    if e.checked() { s.push(m.id) } else { s.retain(|&x| x != m.id) }
                                                    selected.set(s);
                                                },
                                            }
                                            span { "{m.name} {m.version} ({m.module_type})" }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! { p { class: "text-sm text-red-400", "{e}" } },
                        None => rsx! { p { class: "text-zinc-500", "Loading…" } },
                    }
                    div { class: "flex justify-end gap-2",
                        button {
                            class: "rounded px-3 py-1.5 text-sm text-zinc-300 hover:bg-zinc-800",
                            onclick: move |_| open.set(false),
                            "Cancel"
                        }
                        button {
                            class: BTN,
                            disabled: selected().is_empty(),
                            onclick: move |_| {
                                let ids = selected();
                                spawn(async move {
                                    match api::ds_assign_modules(ds_id, &ids).await {
                                        Ok(()) => {
                                            toast_ok("modules assigned");
                                            open.set(false);
                                            on_done.call(());
                                        }
                                        Err(e) => toast_error(e.to_string()),
                                    }
                                });
                            },
                            "Assign"
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DeployDialog(open: Signal<bool>, ds_id: i64) -> Element {
    let mut query = use_signal(String::new);
    let targets = use_resource(move || async move {
        if open() {
            let q = crate::logic::fiql_contains(&["name", "controllerId"], &query());
            api::list_targets(0, 50, q.as_deref()).await
        } else {
            Ok(raptor_api_types::PagedList::new(vec![], 0))
        }
    });
    let mut selected = use_signal(|| None::<String>);
    let mut forced = use_signal(|| true);
    rsx! {
        if open() {
            div { class: "fixed inset-0 z-40 flex items-center justify-center bg-black/60",
                div { class: "max-h-[80vh] w-[28rem] overflow-y-auto rounded-lg border border-zinc-800 bg-zinc-900 p-6",
                    h3 { class: "mb-3 text-lg font-semibold text-zinc-100", "Deploy to target" }
                    div { class: "mb-3",
                        SearchBox { placeholder: "Search targets…", on_search: move |s| query.set(s) }
                    }
                    match &*targets.read_unchecked() {
                        Some(Ok(page)) => rsx! {
                            ul { class: "mb-3 space-y-1",
                                for t in page.content.clone() {
                                    li { key: "{t.controller_id}",
                                        label { class: "flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm hover:bg-zinc-800",
                                            input {
                                                r#type: "radio",
                                                name: "deploy-target",
                                                checked: selected() == Some(t.controller_id.clone()),
                                                onchange: {
                                                    let cid = t.controller_id.clone();
                                                    move |_| selected.set(Some(cid.clone()))
                                                },
                                            }
                                            span { "{t.name} " }
                                            StatusBadge { status: t.update_status.clone() }
                                        }
                                    }
                                }
                            }
                        },
                        Some(Err(e)) => rsx! { p { class: "text-sm text-red-400", "{e}" } },
                        None => rsx! { p { class: "text-zinc-500", "Loading…" } },
                    }
                    label { class: "mb-4 flex items-center gap-2 text-sm",
                        input {
                            r#type: "checkbox",
                            checked: forced(),
                            onchange: move |e| forced.set(e.checked()),
                        }
                        "Forced"
                    }
                    div { class: "flex justify-end gap-2",
                        button {
                            class: "rounded px-3 py-1.5 text-sm text-zinc-300 hover:bg-zinc-800",
                            onclick: move |_| open.set(false),
                            "Cancel"
                        }
                        button {
                            class: BTN,
                            disabled: selected().is_none(),
                            onclick: move |_| {
                                let (cid, is_forced) = (selected().unwrap(), forced());
                                spawn(async move {
                                    match api::assign_ds(&cid, ds_id, is_forced).await {
                                        Ok(r) if r.assigned > 0 => toast_ok(format!("deploying to {cid}")),
                                        Ok(_) => toast_ok("already assigned"),
                                        Err(e) => {
                                            toast_error(e.to_string());
                                            return;
                                        }
                                    }
                                    open.set(false);
                                });
                            },
                            "Deploy"
                        }
                    }
                }
            }
        }
    }
}
