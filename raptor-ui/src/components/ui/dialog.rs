// Adapted from https://github.com/rust-ui/dioxus-ui's dialog.rs concept, but not
// vendored verbatim: upstream drives open/close with a <script>-injected vanilla-JS
// DOM patch and an undefined `window.ScrollLock` global, which doesn't fit this
// codebase's Dioxus-signal-driven components (see ConfirmDialog/CreateModuleDialog).
// This rewrite keeps the same visual language but is driven entirely by a
// `Signal<bool>`, matching the reactive pattern already used elsewhere.

use dioxus::prelude::*;
use tw_merge::tw_merge;

// Focus trap + restore-on-close, run via `document::eval` the same way
// `use_theme`'s persistence does (components/mod.rs) — Dioxus's own supported
// JS-interop primitive, not the ad-hoc `<script>`/`window.ScrollLock` glue
// this file's rewrite steered away from (see header comment above). Only one
// dialog is ever open at a time in this app, so scoping by `[role="dialog"]`
// is enough.
#[cfg(target_arch = "wasm32")]
const TRAP_OPEN_JS: &str = r#"(() => {
    const panel = document.querySelector('[role="dialog"]');
    if (!panel) return;
    window.__raptorDialogPrevFocus = document.activeElement;
    const focusable = () => Array.from(panel.querySelectorAll(
        'a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex="-1"])'
    ));
    const handler = (e) => {
        if (e.key !== 'Tab') return;
        const items = focusable();
        if (items.length === 0) return;
        const first = items[0];
        const last = items[items.length - 1];
        if (e.shiftKey && document.activeElement === first) {
            e.preventDefault();
            last.focus();
        } else if (!e.shiftKey && document.activeElement === last) {
            e.preventDefault();
            first.focus();
        }
    };
    panel.addEventListener('keydown', handler);
    window.__raptorDialogTrapHandler = handler;
    window.__raptorDialogPanel = panel;
    const items = focusable();
    if (items.length > 0) items[0].focus();
})();"#;

#[cfg(target_arch = "wasm32")]
const TRAP_CLOSE_JS: &str = r#"(() => {
    if (window.__raptorDialogPanel && window.__raptorDialogTrapHandler) {
        window.__raptorDialogPanel.removeEventListener('keydown', window.__raptorDialogTrapHandler);
    }
    window.__raptorDialogPanel = undefined;
    window.__raptorDialogTrapHandler = undefined;
    const prev = window.__raptorDialogPrevFocus;
    if (prev && document.body.contains(prev)) prev.focus();
    window.__raptorDialogPrevFocus = undefined;
})();"#;

#[component]
pub fn Dialog(
    open: Signal<bool>,
    #[props(into, optional)] class: Option<String>,
    children: Element,
) -> Element {
    let merged_class = tw_merge!(
        "w-96 rounded-lg border border-border-soft bg-card p-6",
        class.as_deref().unwrap_or("")
    );

    // Fires the trap/restore JS only on the open<->closed edges (not on every
    // unrelated re-render) by comparing against the previous value.
    let mut was_open = use_signal(|| false);
    use_effect(move || {
        let now_open = open();
        if now_open != was_open() {
            #[cfg(target_arch = "wasm32")]
            document::eval(if now_open {
                TRAP_OPEN_JS
            } else {
                TRAP_CLOSE_JS
            });
            was_open.set(now_open);
        }
    });

    rsx! {
        if open() {
            // The backdrop takes focus itself (tabindex -1 + autofocus) so the
            // Escape handler has somewhere to land: opening a dialog otherwise
            // leaves focus on the trigger outside it, and keydown never arrives.
            // Landing here also means Tab walks forwards into the panel.
            div {
                class: "fixed inset-0 z-40 flex items-center justify-center bg-black/60",
                tabindex: "-1",
                autofocus: true,
                onkeydown: move |e| {
                    if e.key() == Key::Escape {
                        open.set(false);
                    }
                },
                onclick: move |_| open.set(false),
                div {
                    class: "{merged_class}",
                    role: "dialog",
                    aria_modal: "true",
                    // Clicks on the panel must not bubble to the backdrop's
                    // close handler, or every interaction would dismiss it.
                    onclick: move |e| e.stop_propagation(),
                    {children}
                }
            }
        }
    }
}
