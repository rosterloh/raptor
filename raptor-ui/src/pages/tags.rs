use crate::components::ui::{Button, Dialog, Input};
use crate::components::*;
use crate::{Route, api, logic};
use dioxus::prelude::*;
use raptor_api_types::{TagCreate, TagRest, TagUpdate};

const LIMIT: u64 = 25;

/// Target tags and distribution-set tags are the same resource behind two
/// prefixes, so the page carries the kind and both tabs share every component.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TagKind {
    Target,
    Ds,
}

impl TagKind {
    pub fn prefix(self) -> &'static str {
        match self {
            TagKind::Target => "/rest/v1/targettags",
            TagKind::Ds => "/rest/v1/distributionsettags",
        }
    }

    fn tab(self) -> &'static str {
        match self {
            TagKind::Target => "Target tags",
            TagKind::Ds => "Distribution set tags",
        }
    }

    /// Header for the "how many things carry this tag" column.
    fn tagged(self) -> &'static str {
        match self {
            TagKind::Target => "Targets",
            TagKind::Ds => "Sets",
        }
    }

    fn empty_hint(self) -> &'static str {
        match self {
            TagKind::Target => {
                "No target tags yet. Tags label targets for fleet organisation, and can be used as a filter term (tag==beta) on the targets list and in target filters."
            }
            TagKind::Ds => {
                "No distribution set tags yet. Tags label sets — for example by release channel — and filter the distributions list (tag==qa)."
            }
        }
    }

    /// URL-safe form of the kind, for the `?kind=` query segment. Anything
    /// other than "ds" — including a missing/old-bookmarked param — falls
    /// back to `Target`.
    fn slug(self) -> &'static str {
        match self {
            TagKind::Target => "target",
            TagKind::Ds => "ds",
        }
    }

    fn from_slug(s: &str) -> Self {
        if s == "ds" {
            TagKind::Ds
        } else {
            TagKind::Target
        }
    }
}

#[component]
pub fn Tags(kind: String, query: String, offset: u64) -> Element {
    let tag_kind = TagKind::from_slug(&kind);
    let nav = use_navigator();
    let goto = move |kind: TagKind, query: String, offset: u64| {
        nav.replace(Route::Tags {
            kind: kind.slug().to_string(),
            query,
            offset,
        });
    };

    let mut tags = use_resource(use_reactive!(|tag_kind, query, offset| async move {
        let q = logic::fiql_contains(&["name"], &query);
        api::list_tags(tag_kind.prefix(), offset, LIMIT, q.as_deref()).await
    }));

    let mut show_form = use_signal(|| false);
    // None = create, Some = edit that tag.
    let mut form_for = use_signal(|| None::<TagRest>);
    let mut confirm_delete = use_signal(|| false);
    let mut delete_for = use_signal(|| None::<TagRest>);

    let mut search_key = use_signal(|| 0u32);
    let active = !query.is_empty();
    use_filter_clear(use_reactive!(|active| active), move || {
        goto(tag_kind, String::new(), 0);
        search_key += 1;
    });

    rsx! {
        div { class: "mb-4 flex items-center justify-between",
            h1 { class: "text-xl font-bold text-foreground", "Tags" }
            Button {
                onclick: move |_| {
                    form_for.set(None);
                    show_form.set(true);
                },
                "New tag"
            }
        }
        div { class: "mb-3 flex gap-2",
            for k in [TagKind::Target, TagKind::Ds] {
                button {
                    key: "{k.tab()}",
                    class: if tag_kind == k { "rounded px-3 py-1.5 text-sm bg-accent text-foreground border-b-2 border-primary" } else { "rounded px-3 py-1.5 text-sm text-fg-dim hover:bg-accent" },
                    onclick: {
                        let query = query.clone();
                        move |_| goto(k, query.clone(), 0)
                    },
                    "{k.tab()}"
                }
            }
        }
        div { class: "mb-3",
            SearchBox {
                key: "{search_key}",
                placeholder: "Search name…",
                initial: query.clone(),
                on_search: move |s| goto(tag_kind, s, 0),
            }
        }
        match &*tags.read_unchecked() {
            Some(Ok(page)) if page.content.is_empty() => rsx! {
                p { class: "text-sm text-muted-foreground", "{tag_kind.empty_hint()}" }
            },
            Some(Ok(page)) => rsx! {
                table { class: TABLE,
                    thead {
                        tr {
                            th { class: TH, "Name" }
                            th { class: TH, "Description" }
                            th { class: TH, "{tag_kind.tagged()}" }
                            th { class: TH, "Modified" }
                            th { class: TH, "" }
                        }
                    }
                    tbody {
                        for t in page.content.clone() {
                            tr { key: "{t.id}", class: "hover:bg-card",
                                td { class: TD,
                                    TagChip { name: t.name.clone(), colour: t.colour.clone() }
                                }
                                td { class: "{TD} text-fg-dim",
                                    {t.description.clone().unwrap_or_else(|| "-".into())}
                                }
                                td { class: "{TD} tabular-nums text-fg-dim", "{t.assigned_count}" }
                                td { class: TD, {logic::format_ts(t.last_modified_at)} }
                                td { class: TD,
                                    div { class: "flex justify-end gap-2",
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    form_for.set(Some(t.clone()));
                                                    show_form.set(true);
                                                }
                                            },
                                            "Edit"
                                        }
                                        button {
                                            class: "rounded px-2 py-1 text-xs text-err hover:bg-accent",
                                            onclick: {
                                                let t = t.clone();
                                                move |_| {
                                                    delete_for.set(Some(t.clone()));
                                                    confirm_delete.set(true);
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
                Paginator {
                    offset,
                    limit: LIMIT,
                    total: page.total,
                    on_change: move |o| goto(tag_kind, query.clone(), o),
                }
            },
            Some(Err(e)) => rsx! {
                ErrorPane { message: e.to_string(), on_retry: move |_| tags.restart() }
            },
            None => rsx! {
                p { class: "text-muted-foreground", "Loading…" }
            },
        }
        if show_form() {
            TagFormDialog {
                open: show_form,
                kind: tag_kind,
                existing: form_for(),
                on_saved: move |_| tags.restart(),
            }
        }
        ConfirmDialog {
            title: "Delete tag".to_string(),
            message: delete_for()
                .map(|t| format!(
                    "Delete tag \"{}\"? It is removed from everything it is assigned to; the tagged entities themselves are not touched.",
                    t.name,
                ))
                .unwrap_or_default(),
            open: confirm_delete,
            on_confirm: move |_| {
                let Some(t) = delete_for() else { return };
                let prefix = tag_kind.prefix();
                spawn(async move {
                    match api::delete_tag(prefix, t.id).await {
                        Ok(()) => {
                            toast_ok(format!("deleted {}", t.name));
                            tags.restart();
                        }
                        Err(e) => toast_error(e.to_string()),
                    }
                });
            },
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

/// Create/edit form. A duplicate name comes back as 409 and lands on the name
/// field; the colour is free text with a live chip preview, since the API
/// accepts any string and only hex values render as a dot.
#[component]
fn TagFormDialog(
    open: Signal<bool>,
    kind: TagKind,
    existing: Option<TagRest>,
    on_saved: EventHandler<()>,
) -> Element {
    let editing_id = existing.as_ref().map(|t| t.id);
    let mut name = use_signal(|| {
        existing
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_default()
    });
    let mut description = use_signal(|| {
        existing
            .as_ref()
            .and_then(|t| t.description.clone())
            .unwrap_or_default()
    });
    let mut colour = use_signal(|| {
        existing
            .as_ref()
            .and_then(|t| t.colour.clone())
            .unwrap_or_default()
    });
    let mut name_error = use_signal(|| None::<String>);
    let mut saving = use_signal(|| false);

    rsx! {
        Dialog { open, class: "w-[28rem]",
            form {
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    let n = name().trim().to_string();
                    if n.is_empty() {
                        return;
                    }
                    let d = description().trim().to_string();
                    let c = colour().trim().to_string();
                    let (desc, col) = (
                        (!d.is_empty()).then_some(d),
                        (!c.is_empty()).then_some(c),
                    );
                    name_error.set(None);
                    saving.set(true);
                    spawn(async move {
                        let prefix = kind.prefix();
                        let saved = match editing_id {
                            Some(id) => {
                                api::update_tag(
                                        prefix,
                                        id,
                                        &TagUpdate {
                                            name: Some(n),
                                            description: desc,
                                            colour: col,
                                        },
                                    )
                                    .await
                            }
                            None => {
                                api::create_tag(
                                        prefix,
                                        &TagCreate {
                                            name: n,
                                            description: desc,
                                            colour: col,
                                        },
                                    )
                                    .await
                                    .and_then(|mut v| {
                                        v.pop()
                                            .ok_or_else(|| api::ApiError::Network(
                                                "empty response".into(),
                                            ))
                                    })
                            }
                        };
                        saving.set(false);
                        match saved {
                            Ok(t) => {
                                toast_ok(format!("tag {} saved", t.name));
                                open.set(false);
                                on_saved.call(());
                            }
                            Err(api::ApiError::Server { status: 409, message }) => {
                                name_error.set(Some(message))
                            }
                            Err(e) => toast_error(e.to_string()),
                        }
                    });
                },
                h3 { class: "mb-3 text-lg font-semibold text-foreground",
                    if editing_id.is_some() {
                        "Edit tag"
                    } else {
                        "New tag"
                    }
                }
                Input {
                    class: "mb-3",
                    placeholder: "Name",
                    required: true,
                    value: "{name}",
                    oninput: move |e: FormEvent| {
                        name.set(e.value());
                        name_error.set(None);
                    },
                }
                FieldError { message: name_error() }
                Input {
                    class: "mb-3",
                    placeholder: "Description (optional)",
                    value: "{description}",
                    oninput: move |e: FormEvent| description.set(e.value()),
                }
                div { class: "mb-3 flex items-center gap-3",
                    Input {
                        class: "mb-0 font-mono text-sm",
                        placeholder: "Colour, e.g. #00ff00 (optional)",
                        value: "{colour}",
                        oninput: move |e: FormEvent| colour.set(e.value()),
                    }
                    TagChip {
                        name: if name().trim().is_empty() { "preview".to_string() } else { name().trim().to_string() },
                        colour: Some(colour()),
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

/// "filter by tag" dropdown for the target and distribution-set lists. Empty
/// means no tag term; the caller ANDs the selection into its FIQL query.
#[component]
pub fn TagFilter(kind: TagKind, selected: Signal<String>, on_change: EventHandler<()>) -> Element {
    let all =
        use_resource(move || async move { api::list_tags(kind.prefix(), 0, 100, None).await });
    let mut selected = selected;
    rsx! {
        match &*all.read_unchecked() {
            // Nothing to filter by yet — don't show a dropdown with one option.
            Some(Ok(page)) if page.content.is_empty() => rsx! {},
            Some(Ok(page)) => rsx! {
                select {
                    class: "rounded border border-border bg-background px-3 py-2 text-sm text-foreground focus:border-ring focus:outline-none",
                    value: "{selected}",
                    onchange: move |e| {
                        selected.set(e.value());
                        on_change.call(());
                    },
                    option { value: "", "All tags" }
                    for t in page.content.clone() {
                        option { key: "{t.id}", value: "{t.name}", "{t.name}" }
                    }
                }
            },
            _ => rsx! {},
        }
    }
}

/// The tag panel shared by target and distribution-set detail pages: the
/// entity's tags as chips, plus a dialog to toggle any tag on or off. `key` is
/// the entity's path segment — a controller id for targets, the id for sets.
#[component]
pub fn EntityTags(
    kind: TagKind,
    entity_key: String,
    tags: Resource<api::ApiResult<Vec<TagRest>>>,
) -> Element {
    let mut show_manage = use_signal(|| false);
    rsx! {
        div { class: "mb-2 flex items-center justify-between",
            h2 { class: "font-semibold text-foreground", "Tags" }
            button {
                class: "rounded px-2 py-1 text-xs text-fg-dim hover:bg-accent",
                onclick: move |_| show_manage.set(true),
                "Manage…"
            }
        }
        match &*tags.read_unchecked() {
            Some(Ok(list)) if list.is_empty() => rsx! {
                p { class: "text-sm text-muted-foreground", "none" }
            },
            Some(Ok(list)) => rsx! {
                div { class: "flex flex-wrap gap-1.5",
                    for t in list.clone() {
                        TagChip { key: "{t.id}", name: t.name.clone(), colour: t.colour.clone() }
                    }
                }
            },
            Some(Err(e)) => rsx! {
                p { class: "text-sm text-err", "{e}" }
            },
            None => rsx! {
                p { class: "text-muted-foreground", "Loading…" }
            },
        }
        if show_manage() {
            ManageTagsDialog {
                open: show_manage,
                kind,
                entity_key,
                assigned: match &*tags.read_unchecked() {
                    Some(Ok(list)) => list.iter().map(|t| t.id).collect(),
                    _ => Vec::new(),
                },
                on_changed: move |_| tags.restart(),
            }
        }
    }
}

/// Every tag with a checkbox; ticking assigns and unticking unassigns straight
/// away, so the dialog has no save step to get out of sync with the server.
#[component]
fn ManageTagsDialog(
    open: Signal<bool>,
    kind: TagKind,
    entity_key: String,
    assigned: Vec<i64>,
    on_changed: EventHandler<()>,
) -> Element {
    let all =
        use_resource(move || async move { api::list_tags(kind.prefix(), 0, 100, None).await });
    let mut current = use_signal(|| assigned.clone());
    let mut busy = use_signal(|| false);
    let key = use_signal(|| entity_key.clone());

    rsx! {
        Dialog { open, class: "max-h-[80vh] w-[28rem] overflow-y-auto",
            h3 { class: "mb-3 text-lg font-semibold text-foreground", "Tags" }
            match &*all.read_unchecked() {
                Some(Ok(page)) if page.content.is_empty() => rsx! {
                    p { class: "mb-3 text-sm text-muted-foreground",
                        "No tags defined yet — create one on the Tags page first."
                    }
                },
                Some(Ok(page)) => rsx! {
                    ul { class: "mb-4 space-y-1",
                        for t in page.content.clone() {
                            li { key: "{t.id}",
                                label { class: "flex cursor-pointer items-center gap-2 rounded px-2 py-1 text-sm hover:bg-accent",
                                    input {
                                        r#type: "checkbox",
                                        checked: current().contains(&t.id),
                                        disabled: busy(),
                                        onchange: {
                                            let name = t.name.clone();
                                            move |e: FormEvent| {
                                                let (tag_id, on, name) = (t.id, e.checked(), name.clone());
                                                busy.set(true);
                                                spawn(async move {
                                                    let prefix = kind.prefix();
                                                    let k = key();
                                                    let res = if on {
                                                        api::assign_tag(prefix, tag_id, &k).await
                                                    } else {
                                                        api::unassign_tag(prefix, tag_id, &k).await
                                                    };
                                                    busy.set(false);
                                                    match res {
                                                        Ok(()) => {
                                                            let mut c = current();
                                                            if on { c.push(tag_id) } else { c.retain(|&x| x != tag_id) }
                                                            current.set(c);
                                                            toast_ok(
                                                                format!("{} {name}", if on { "tagged" } else { "untagged" }),
                                                            );
                                                            on_changed.call(());
                                                        }
                                                        Err(e) => toast_error(e.to_string()),
                                                    }
                                                });
                                            }
                                        },
                                    }
                                    TagChip { name: t.name.clone(), colour: t.colour.clone() }
                                    if let Some(d) = t.description.clone() {
                                        span { class: "text-xs text-muted-foreground", "{d}" }
                                    }
                                }
                            }
                        }
                    }
                },
                Some(Err(e)) => rsx! {
                    p { class: "mb-3 text-sm text-err", "{e}" }
                },
                None => rsx! {
                    p { class: "text-muted-foreground", "Loading…" }
                },
            }
            div { class: "flex justify-end",
                Button { onclick: move |_| open.set(false), "Done" }
            }
        }
    }
}
