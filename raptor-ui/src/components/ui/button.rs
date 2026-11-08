// Vendored from https://github.com/rust-ui/dioxus-ui (app_crates/registry/src/ui/button.rs)
// Community shadcn-style port for Dioxus. Copy-paste per their registry model, not a
// live dependency — see docs/shadcn-pilot notes.

use dioxus::prelude::*;
use tw_merge::tw_merge;

#[allow(dead_code)]
#[derive(Default, Clone, PartialEq)]
pub enum ButtonVariant {
    #[default]
    Default,
    Destructive,
    Outline,
    Secondary,
    Ghost,
    Link,
}

#[allow(dead_code)]
#[derive(Default, Clone, PartialEq)]
pub enum ButtonSize {
    #[default]
    Default,
    Sm,
    Lg,
    Icon,
    /// 44px minimum for controls used on touch screens (paginator, shell).
    Touch,
}

impl ButtonVariant {
    fn as_str(&self) -> &'static str {
        match self {
            ButtonVariant::Default => {
                "bg-primary text-primary-foreground shadow-xs hover:bg-primary/90"
            }
            ButtonVariant::Destructive => {
                "bg-destructive text-destructive-foreground shadow-xs hover:bg-destructive/90"
            }
            ButtonVariant::Outline => {
                "border border-input bg-background shadow-xs hover:bg-accent hover:text-accent-foreground"
            }
            ButtonVariant::Secondary => {
                "bg-secondary text-secondary-foreground shadow-xs hover:bg-secondary/80"
            }
            ButtonVariant::Ghost => "hover:bg-accent hover:text-accent-foreground",
            ButtonVariant::Link => "text-primary underline-offset-4 hover:underline",
        }
    }
}

impl ButtonSize {
    fn as_str(&self) -> &'static str {
        match self {
            ButtonSize::Default => "h-9 px-4 py-2",
            ButtonSize::Sm => "h-8 px-3 text-xs",
            ButtonSize::Lg => "h-10 px-6",
            ButtonSize::Icon => "size-9",
            ButtonSize::Touch => "min-h-11 min-w-11 px-3",
        }
    }
}

#[component]
pub fn Button(
    #[props(into, optional)] class: Option<String>,
    #[props(default = ButtonVariant::default())] variant: ButtonVariant,
    #[props(default = ButtonSize::default())] size: ButtonSize,
    #[props(into, optional, default = "button".to_string())] r#type: String,
    #[props(optional)] disabled: bool,
    // Added to the vendored props: icon-like labels ("Prev", "Menu") need a fuller name.
    #[props(into, optional)] aria_label: Option<String>,
    #[props(optional)] onclick: Option<EventHandler<MouseEvent>>,
    children: Element,
) -> Element {
    let merged_class = tw_merge!(
        "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded text-sm font-medium transition-all disabled:pointer-events-none disabled:opacity-50 outline-none focus-visible:ring-2 focus-visible:ring-ring hover:cursor-pointer w-fit",
        variant.as_str(),
        size.as_str(),
        class.as_deref().unwrap_or("")
    );

    rsx! {
        button {
            class: "{merged_class}",
            r#type,
            disabled,
            aria_label,
            onclick: move |e| {
                if let Some(handler) = &onclick {
                    handler.call(e);
                }
            },
            {children}
        }
    }
}
