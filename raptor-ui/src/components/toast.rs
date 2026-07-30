use dioxus::prelude::*;

#[derive(Clone, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub text: String,
    pub error: bool,
}

pub static TOASTS: GlobalSignal<Vec<Toast>> = Signal::global(Vec::new);
static NEXT_ID: GlobalSignal<u64> = Signal::global(|| 0u64);

fn push(text: String, error: bool) {
    let id = {
        let mut n = NEXT_ID.write();
        *n += 1;
        *n
    };
    TOASTS.write().push(Toast { id, text, error });
    spawn(async move {
        gloo_timers::future::TimeoutFuture::new(6_000).await;
        dismiss(id);
    });
}

/// Drop one toast, whether the 6s timer or the user's close button got there
/// first — dismissing early just makes the pending timer a no-op.
fn dismiss(id: u64) {
    TOASTS.write().retain(|t| t.id != id);
}

pub fn toast_error(text: impl Into<String>) {
    push(text.into(), true);
}

pub fn toast_ok(text: impl Into<String>) {
    push(text.into(), false);
}

#[component]
pub fn ToastStack() -> Element {
    rsx! {
        // A live region has to be in the DOM *before* its content changes for a
        // screen reader to announce the change, so this container renders even
        // when there are no toasts. Successes and errors share one polite
        // region: `assertive` would cut the reader off mid-sentence on every
        // failed request, and toasts here are never the only report of a failure.
        div {
            class: "fixed bottom-4 right-4 z-50 flex flex-col gap-2",
            role: "status",
            aria_live: "polite",
            for t in TOASTS() {
                div {
                    key: "{t.id}",
                    class: if t.error {
                        "flex items-start gap-3 rounded border border-err-border bg-err-bg px-4 py-2 text-sm text-err-fg shadow-lg"
                    } else {
                        "flex items-start gap-3 rounded border border-ok-border bg-ok-bg px-4 py-2 text-sm text-ok-fg shadow-lg"
                    },
                    span { "{t.text}" }
                    button {
                        class: "shrink-0 rounded px-1 leading-none hover:opacity-70",
                        aria_label: "Dismiss notification",
                        onclick: move |_| dismiss(t.id),
                        "×"
                    }
                }
            }
        }
    }
}
