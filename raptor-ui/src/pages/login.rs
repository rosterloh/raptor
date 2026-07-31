use crate::Route;
use crate::components::ui::{Button, Input};
use dioxus::prelude::*;

const LOGO: Asset = asset!("/assets/logo/logo-login.png");

#[component]
pub fn Login() -> Element {
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut missing_user = use_signal(|| false);
    let mut missing_pass = use_signal(|| false);
    let mut busy = use_signal(|| false);
    let nav = use_navigator();

    // Whether the API answers at all. Worth knowing *before* typing credentials:
    // a console pointed at the wrong port fails identically to a wrong password
    // from the operator's side, and that misconfiguration is a real one — the dev
    // proxy and the server's default port disagreed until recently.
    let reachable = use_resource(|| async { crate::api::api_reachable().await });

    let submit = move |e: FormEvent| {
        e.prevent_default();
        // Validate at the boundary. An empty field should never cost a request,
        // and "invalid username or password" is a misleading answer to "you did
        // not type anything".
        let (no_user, no_pass) = (username().trim().is_empty(), password().is_empty());
        missing_user.set(no_user);
        missing_pass.set(no_pass);
        error.set(None);
        if no_user || no_pass {
            return;
        }
        busy.set(true);
        spawn(async move {
            match crate::api::login(&username(), &password()).await {
                Ok(()) => {
                    nav.push(Route::Dashboard {});
                }
                Err(err) => error.set(Some(err.message())),
            }
            busy.set(false);
        });
    };

    rsx! {
        // Login is the one route outside `Shell`, which is where the rest of the
        // app inherits its text colour from — without it here the inputs render
        // the browser default (black) on the dark card.
        div { class: "grid min-h-screen grid-cols-1 bg-background text-foreground lg:grid-cols-[1fr_26rem]",

            // Left: what this server is. Deliberately no fleet figures — there is
            // no session yet, so any number here would be invented.
            aside { class: "hidden flex-col justify-between border-r border-border-soft bg-card p-12 lg:flex",
                div {
                    div { class: "flex items-center gap-3",
                        img { src: LOGO, class: "h-10 w-10", alt: "" }
                        span { class: "font-display text-xl font-bold tracking-[0.08em] text-foreground uppercase",
                            "raptor"
                        }
                    }
                    h1 { class: "font-display mt-12 max-w-[16ch] text-[44px] leading-[0.95] font-bold tracking-wide text-foreground uppercase",
                        "Firmware, delivered on purpose"
                    }
                    p { class: "mt-5 max-w-[46ch] text-base text-fg-dim",
                        "A hawkBit-compatible update server. One binary, one config file, and a console that
                         tells you what your fleet is actually doing before you ask it to change."
                    }
                    dl { class: "mt-12 grid max-w-md grid-cols-[10rem_1fr] gap-x-4 gap-y-2 font-mono text-[11px]",
                        dt { class: "text-muted-foreground uppercase", "Device API" }
                        dd { class: "text-fg-dim", "DDI v1 — SWUpdate, RAUC, Zephyr" }
                        dt { class: "text-muted-foreground uppercase", "Management API" }
                        dd { class: "text-fg-dim", "hawkBit-compatible, FIQL filtering" }
                        dt { class: "text-muted-foreground uppercase", "Tenancy" }
                        dd { class: "text-fg-dim", "single tenant — DEFAULT" }
                    }
                }
                p { class: "font-mono text-[11px] text-muted-foreground",
                    "Sessions are held in memory. A server restart signs everyone out."
                }
            }

            // Right: the form.
            div { class: "flex flex-col justify-center gap-6 p-8 lg:p-12",
                div { class: "lg:hidden",
                    img { src: LOGO, class: "h-10 w-10", alt: "" }
                }
                div {
                    h2 { class: "font-display text-2xl font-bold tracking-wider text-foreground uppercase",
                        "Sign in"
                    }
                    p { class: "mt-1 font-mono text-xs text-muted-foreground",
                        "Management credentials from [mgmt] in raptor.toml"
                    }
                }

                form { onsubmit: submit, novalidate: true,
                    // Wrapping the input in its label associates the two without
                    // needing an id, and a placeholder is not a label — a screen
                    // reader gets nothing from a placeholder-only field.
                    label { class: "mb-4 block",
                        span { class: "mb-2 block font-mono text-[11px] tracking-[0.09em] text-muted-foreground uppercase",
                            "Username"
                        }
                        Input {
                            r#type: "text",
                            autocomplete: "username",
                            placeholder: "admin",
                            value: "{username}",
                            invalid: missing_user(),
                            oninput: move |e: FormEvent| {
                                username.set(e.value());
                                missing_user.set(false);
                            },
                        }
                        if missing_user() {
                            span { class: "mt-2 block font-mono text-[11px] text-err",
                                "Username is required"
                            }
                        }
                    }

                    label { class: "mb-5 block",
                        span { class: "mb-2 block font-mono text-[11px] tracking-[0.09em] text-muted-foreground uppercase",
                            "Password"
                        }
                        Input {
                            r#type: "password",
                            autocomplete: "current-password",
                            placeholder: "••••••••",
                            value: "{password}",
                            invalid: missing_pass(),
                            oninput: move |e: FormEvent| {
                                password.set(e.value());
                                missing_pass.set(false);
                            },
                        }
                        if missing_pass() {
                            span { class: "mt-2 block font-mono text-[11px] text-err",
                                "Password is required"
                            }
                        }
                    }

                    if let Some(e) = error() {
                        p {
                            class: "mb-4 border border-err-border bg-err-bg px-3 py-2 font-mono text-[11px] text-err-fg",
                            role: "alert",
                            "{e}"
                        }
                    }

                    Button {
                        class: "h-10 w-full justify-center",
                        disabled: busy(),
                        r#type: "submit",
                        if busy() {
                            "Signing in…"
                        } else {
                            "Sign in"
                        }
                    }
                }

                div { class: "flex items-center gap-2 font-mono text-[11px]",
                    match &*reachable.read_unchecked() {
                        Some(true) => rsx! {
                            span { class: "size-[7px] shrink-0 rounded-full bg-ok" }
                            span { class: "text-muted-foreground", "API reachable" }
                        },
                        Some(false) => rsx! {
                            span { class: "size-[7px] shrink-0 rounded-full bg-err" }
                            span { class: "text-err",
                                "API unreachable — check the server is running and the console points at it"
                            }
                        },
                        None => rsx! {
                            span { class: "size-[7px] shrink-0 rounded-full bg-neutral" }
                            span { class: "text-muted-foreground", "checking API…" }
                        },
                    }
                }

                p { class: "font-mono text-[11px] leading-relaxed text-muted-foreground",
                    "Session is an httpOnly cookie, SameSite=Strict."
                    br {}
                    "Basic auth keeps working for curl and CI."
                }
            }
        }
    }
}
