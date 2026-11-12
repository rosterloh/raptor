use dioxus::prelude::*;

use super::ui::Input;

/// An optional hex colour: a native colour swatch beside the hex text field,
/// both bound to `value`. The text field stays because the API takes free
/// text and an empty value means "no colour".
#[component]
pub fn ColourField(value: Signal<String>) -> Element {
    let mut value = value;
    let swatch = crate::logic::colour_picker_value(&value());
    rsx! {
        input {
            r#type: "color",
            class: "size-9 shrink-0 cursor-pointer rounded border border-input bg-transparent px-1 py-1",
            aria_label: "Pick colour",
            value: "{swatch}",
            oninput: move |e: FormEvent| value.set(e.value()),
        }
        Input {
            class: "mb-0 font-mono text-sm",
            placeholder: "Colour, e.g. #00ff00 (optional)",
            value: "{value}",
            oninput: move |e: FormEvent| value.set(e.value()),
        }
    }
}
