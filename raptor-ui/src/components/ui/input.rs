// Vendored from https://github.com/rust-ui/dioxus-ui (app_crates/registry/src/ui/input.rs)
// Community shadcn-style port for Dioxus. Copy-paste per their registry model, not a
// live dependency — see docs/shadcn-pilot notes.

use dioxus::prelude::*;
use tw_merge::tw_merge;

#[component]
pub fn Input(
    #[props(into, optional)] class: Option<String>,
    #[props(into, optional, default = "text".to_string())] r#type: String,
    #[props(into, optional)] placeholder: Option<String>,
    #[props(into, optional)] value: Option<String>,
    #[props(optional)] required: bool,
    #[props(optional)] disabled: bool,
    #[props(optional)] oninput: Option<EventHandler<FormEvent>>,
) -> Element {
    let merged_class = tw_merge!(
        "placeholder:text-muted-foreground border-input flex h-9 w-full min-w-0 rounded-md border bg-transparent px-3 py-1 text-base shadow-xs outline-none transition-[color,box-shadow]",
        "focus-visible:border-ring focus-visible:ring-ring/50 focus-visible:ring-2",
        "disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50",
        class.as_deref().unwrap_or("")
    );

    rsx! {
        input {
            r#type,
            class: "{merged_class}",
            placeholder,
            value,
            required,
            disabled,
            oninput: move |e| {
                if let Some(handler) = &oninput {
                    handler.call(e);
                }
            },
        }
    }
}
