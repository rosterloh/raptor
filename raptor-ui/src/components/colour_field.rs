// Adapted from DioxusLabs/components' styled `color_picker` (preview/src/
// components/color_picker, rev 9a75825, pinned in Cargo.toml): same
// dioxus-primitives parts, restyled with this app's tokens instead of its
// css_module theme, and bound to the hex string the API stores.

use dioxus::prelude::*;
use dioxus_primitives::color_picker::{
    AreaThumb, AreaThumbSaturationInput, AreaThumbValueInput, AreaTrack, Color, ColorArea,
    ColorPicker,
};
use dioxus_primitives::popover::{PopoverContent, PopoverRoot, PopoverTrigger};
use dioxus_primitives::slider::{Slider, SliderThumb, SliderTrack};
use palette::{FromColor, Hsv, RgbHue, Srgb, encoding};

use super::ui::Input;

type Hsv64 = Hsv<encoding::Srgb, f64>;

fn to_hex(hsv: Hsv64) -> String {
    let rgb: Color = Srgb::<f64>::from_color(hsv).into_format();
    format!("#{rgb:x}")
}

/// Only what `TagChip` would render as a dot moves the picker.
fn parse(text: &str) -> Option<Hsv64> {
    let rgb: Color = crate::logic::tag_colour(Some(text))?.parse().ok()?;
    Some(Hsv64::from_color(rgb.into_format::<f64>()))
}

// The gradients are the picker itself, not theme colours.
const AREA_TRACK: &str = "grow rounded bg-(color:--area-color) bg-[linear-gradient(to_top,black,transparent),linear-gradient(to_right,white,transparent)]";
const HUE_TRACK: &str =
    "relative h-4 grow rounded bg-[linear-gradient(to_right,#f00,#ff0,#0f0,#0ff,#00f,#f0f,#f00)]";
const THUMB: &str = "absolute block size-4 -translate-x-1/2 -translate-y-1/2 cursor-pointer rounded-full border-2 border-white shadow-[0_0_0_1px_black] outline-none focus-visible:ring-2 focus-visible:ring-ring";

/// An optional hex colour: a swatch that opens a colour picker (drag area plus
/// hue slider), beside the hex text field. Both edit `value`; the text field
/// stays because the API takes free text and empty means "no colour".
#[component]
pub fn ColourField(value: Signal<String>) -> Element {
    let mut value = value;
    let mut hsv = use_signal(|| parse(&value.peek()).unwrap_or_default());
    // Kept apart from `hsv` so dragging to 360° doesn't snap the thumb to 0°.
    let mut hue = use_signal(|| hsv.peek().hue.into_positive_degrees());
    let mut open = use_signal(|| false);

    // Typed hex moves the picker. Compared as hex so a grey, where hex → HSV
    // loses the hue, doesn't reset the hue the operator just dragged.
    use_effect(move || {
        if let Some(c) = parse(&value())
            && to_hex(c) != to_hex(*hsv.peek())
        {
            hsv.set(c);
            hue.set(c.hue.into_positive_degrees());
        }
    });

    // The primitives re-emit the current colour from inside their own drag
    // effects, so an unconditional write here re-runs them forever.
    let mut pick = move |c: Hsv64| {
        if *hsv.peek() != c {
            hsv.set(c);
            value.set(to_hex(c));
        }
    };

    let swatch = crate::logic::tag_colour(Some(&value()));
    rsx! {
        // The dialog closes on Escape too; with the picker open, close only it.
        div {
            class: "shrink-0",
            onkeydown: move |e: KeyboardEvent| {
                if open() && e.key() == Key::Escape {
                    e.stop_propagation();
                }
            },
        ColorPicker {
            color: hsv,
            on_color_change: move |c: Hsv64| {
                pick(c);
            },
            PopoverRoot {
                class: "relative",
                is_modal: false,
                open: Some(open()),
                on_open_change: move |v| open.set(v),
                PopoverTrigger {
                    class: "flex size-9 cursor-pointer items-center justify-center rounded border border-input bg-transparent outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    aria_label: "Pick colour",
                    aria_expanded: open(),
                    span {
                        class: "block size-6 rounded-sm border border-border",
                        style: if let Some(c) = &swatch { "background-color: {c}" },
                    }
                }
                PopoverContent {
                    class: "absolute left-0 top-full z-50 mt-1 flex w-60 flex-col gap-3 rounded border border-border bg-card p-3 shadow-lg",
                    ColorArea { class: "relative flex aspect-square w-full touch-none",
                        AreaTrack { class: AREA_TRACK,
                            AreaThumb { class: THUMB,
                                AreaThumbSaturationInput { class: "sr-only" }
                                AreaThumbValueInput { class: "sr-only" }
                            }
                        }
                    }
                    Slider {
                        class: "relative flex touch-none items-center",
                        label: "Hue",
                        max: 360.0,
                        value: Some(hue()),
                        on_value_change: move |h: f64| {
                            if *hue.peek() != h {
                                hue.set(h);
                            }
                            let c = *hsv.peek();
                            pick(Hsv64::new(RgbHue::new(h), c.saturation, c.value));
                        },
                        SliderTrack { class: HUE_TRACK,
                            SliderThumb { class: "{THUMB} top-1/2", aria_label: "Hue" }
                        }
                    }
                }
            }
        }
        }
        Input {
            class: "mb-0 font-mono text-sm",
            placeholder: "Colour, e.g. #00ff00 (optional)",
            value: "{value}",
            oninput: move |e: FormEvent| value.set(e.value()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips_through_hsv() {
        let c = parse(" #3366CC ").expect("valid hex");
        assert_eq!(to_hex(c), "#3366cc");
        assert_eq!(parse("#fa0").map(to_hex).as_deref(), Some("#ffaa00"));
        assert!(parse("3366cc").is_none()); // TagChip needs the '#'
        assert!(parse("red").is_none());
    }
}
