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
        TOASTS.write().retain(|t| t.id != id);
    });
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
        div { class: "fixed bottom-4 right-4 z-50 flex flex-col gap-2",
            for t in TOASTS() {
                div {
                    key: "{t.id}",
                    class: if t.error {
                        "rounded border border-red-800 bg-red-950 px-4 py-2 text-sm text-red-200 shadow-lg"
                    } else {
                        "rounded border border-emerald-800 bg-emerald-950 px-4 py-2 text-sm text-emerald-200 shadow-lg"
                    },
                    "{t.text}"
                }
            }
        }
    }
}
