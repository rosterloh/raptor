use dioxus::prelude::*;

/// A tab strip following the ARIA tabs pattern: arrow keys move between tabs,
/// only the selected tab is in the tab order, and each panel is labelled by its
/// tab. Without the roving tabindex a keyboard user has to Tab through every tab
/// to reach the panel.
///
/// Panels stay mounted and are hidden rather than swapped out, so a tab that
/// fetched something does not refetch on every visit.
#[component]
pub fn Tabs(
    /// Tab labels, in order. The index is the identity.
    labels: Vec<String>,
    selected: Signal<usize>,
    /// One panel per label, same order.
    children: Element,
) -> Element {
    let mut selected = selected;
    let count = labels.len();
    rsx! {
        div {
            class: "flex gap-0.5 border-b border-border-soft",
            role: "tablist",
            aria_label: "Detail sections",
            for (i , label) in labels.iter().enumerate() {
                button {
                    key: "{i}",
                    r#type: "button",
                    role: "tab",
                    id: "tab-{i}",
                    aria_selected: selected() == i,
                    aria_controls: "tabpanel-{i}",
                    tabindex: if selected() == i { "0" } else { "-1" },
                    class: if selected() == i {
                        "border-b-2 border-primary px-4 py-2 text-sm font-medium text-foreground"
                    } else {
                        "border-b-2 border-transparent px-4 py-2 text-sm font-medium text-muted-foreground hover:text-fg-dim"
                    },
                    onclick: move |_| selected.set(i),
                    onkeydown: move |e| {
                        let delta = match e.key() {
                            Key::ArrowRight => 1,
                            Key::ArrowLeft => -1,
                            _ => 0,
                        };
                        if delta != 0 {
                            e.prevent_default();
                            let next = (selected() as i32 + delta).rem_euclid(count as i32) as usize;
                            selected.set(next);
                        }
                    },
                    "{label}"
                }
            }
        }
        {children}
    }
}

/// One tab's panel. Hidden rather than unmounted when its tab is not selected.
#[component]
pub fn TabPanel(index: usize, selected: Signal<usize>, children: Element) -> Element {
    rsx! {
        div {
            role: "tabpanel",
            id: "tabpanel-{index}",
            aria_labelledby: "tab-{index}",
            hidden: selected() != index,
            class: "pt-4",
            {children}
        }
    }
}
