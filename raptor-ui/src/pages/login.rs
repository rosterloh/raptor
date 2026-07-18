use crate::components::ui::{Button, Input};
use crate::Route;
use dioxus::prelude::*;

#[component]
pub fn Login() -> Element {
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let nav = use_navigator();
    rsx! {
        div { class: "flex min-h-screen items-center justify-center bg-zinc-950",
            form {
                class: "w-80 rounded-lg border border-zinc-800 bg-zinc-900 p-8",
                onsubmit: move |e: FormEvent| {
                    e.prevent_default();
                    busy.set(true);
                    error.set(None);
                    spawn(async move {
                        match crate::api::login(&username(), &password()).await {
                            Ok(()) => {
                                nav.push(Route::Dashboard {});
                            }
                            Err(err) => error.set(Some(err.to_string())),
                        }
                        busy.set(false);
                    });
                },
                h1 { class: "mb-6 text-center text-xl font-bold text-emerald-400", "raptor" }
                Input {
                    class: "mb-3",
                    placeholder: "Username",
                    value: "{username}",
                    oninput: move |e: FormEvent| username.set(e.value()),
                }
                Input {
                    class: "mb-3",
                    r#type: "password",
                    placeholder: "Password",
                    value: "{password}",
                    oninput: move |e: FormEvent| password.set(e.value()),
                }
                if let Some(e) = error() {
                    p { class: "mb-3 text-sm text-red-400", "{e}" }
                }
                Button {
                    class: "w-full py-2",
                    disabled: busy(),
                    r#type: "submit",
                    "Sign in"
                }
            }
        }
    }
}
