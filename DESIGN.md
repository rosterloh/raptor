# raptor design system

Rules for the two operator surfaces: the web console (`raptor-ui`) and the
terminal UI (`raptorctl tui`). The code is the source of truth. Colours and
type are defined in `raptor-ui/tailwind.css`; terminal colours are in
`raptor-cli/src/tui/theme.rs`. This file says how to use them.

## Principles

- **Status is the only loud thing.** Colour goes on device and rollout state
  (`ok`, `pend`, `err`, `info`, `neutral`). Chrome stays in the navy neutrals.
- **One accent, rationed.** `primary` marks the primary action, about two
  visible uses per screen: the default button, the active nav bar, the active
  tab, and first-cell links. It never means a status or a number.
- **Never colour alone.** Every state is shown as a dot or glyph plus a word, so
  it still reads in a screenshot in a ticket, for a red/green colour-blind
  operator, and under `NO_COLOR`.
- **Machine text looks like machine text.** Controller IDs, versions, action
  IDs, timestamps and FIQL use `font-mono`. Human names use the sans face.
- **Works offline.** Fonts and the logo are vendored, because an OTA server
  often has no internet. Never link a CDN asset from the console.
- **hawkBit's nouns.** Use target, distribution set, software module, rollout,
  action and target filter exactly as the API names them.

## Copy

- Terse and factual, like an instrument readout.
- Status words are the API value in lowercase with spaces: "in sync",
  "waiting for approval", "cancel rejected". Add a new state in
  `logic::status_style` (and its test), never as a class at the call site.
- Page titles are short nouns. The style makes them uppercase, so don't type
  them in caps: "Fleet overview", "Rollouts".
- Captions state counts and join facts with ` · `: "1,284 registered targets ·
  12 actions in flight".
- Success toasts are lowercase fragments: "distribution set created", "filter
  line-3 saved". Error toasts show the server's message.
- Empty states are one sentence: "No actions yet." Loading is "Loading…".
- A link that navigates ends with an arrow: "Inspect all targets →". Ranges use
  an en dash: "51–100 of 1,284".
- No emoji.

## Colour tokens

Use these as Tailwind classes (`bg-card`, `text-fg-dim`, `border-border-soft`…).
Never use a palette shade (`zinc-800`) or a literal colour. Dark is the primary
theme. Light follows `prefers-color-scheme`, and the `.theme-light` /
`.theme-dark` class on `:root` overrides it. Each theme is declared twice.
`theme_tests.rs` fails if the two copies drift apart, so edit both.

| Role | Tokens | Use |
|---|---|---|
| Surfaces | `background` → `card` → `accent` → `secondary` | Canvas; panels, tiles, sidebar, dialogs; hover and pressed fills; secondary button. `accent` is a surface despite its shadcn name. |
| Text | `foreground`, `fg-dim`, `muted-foreground` | Headings and key values; body (the default); headers, captions, labels, timestamps. |
| Rules | `border-soft`, `border-subtle` | Container edges and header rules; row separators. Decorative. |
| Control edges | `input`, `border` | `input` for anything you type into or press (inputs, selects, outline buttons) and must reach 3:1. `border` for chips and tag chips. |
| Action | `primary`, `primary-foreground`, `primary-hover`, `ring` | Default button, active nav and tab, list links, focus ring. |
| Destructive | `destructive`, `destructive-foreground` | Destructive button fill and label. Always behind `ConfirmDialog`. |
| Status fill | `ok`, `pend`, `err`, `info`, `neutral` | Dots, progress segments, dashboard counters. |
| Status tint | `{ok,pend,err,info,neutral}-{bg,fg,border}` | Badge and toast ground, label and edge. |

A tone becomes a class only in `tone_badge`, `tone_fill` and `tone_text`
(`components/mod.rs`). They return literal strings because Tailwind only
generates classes it finds written out in the source. Tag colours are operator
data: they fill an 8px dot after `logic::tag_colour` validates them, and are
never used for text or backgrounds.

| Tone | States |
|---|---|
| Ok | in sync, finished |
| Pending | pending, running, paused, downloading, cancelling, stopping, cancel rejected |
| Error | error, stopped, approval denied |
| Info | registered, scheduled, waiting for approval |
| Neutral | ready, canceled, cancel closed, not started, unknown |

Contrast: in both themes, text pairs meet WCAG AA (4.5:1) and control edges
and the focus ring meet 3:1. Check new pairs before you commit them. One known
exception: `text-neutral` is 3.2:1 on `card` in dark, so use it only for figures
24px and larger.

## Type

Three vendored faces (Latin subsets, `tailwind-fonts.css`):

- **Barlow Condensed** 600/700 (`font-display`): page titles and big numerals.
- **Barlow** 400–700 (`font-sans`, the default): UI text.
- **IBM Plex Mono** 400/500 (`font-mono`): machine data and labels.

| Style | Classes | Use |
|---|---|---|
| Page title | `PAGE_TITLE` | Every page's h1, one per page |
| Page caption | `PAGE_CAPTION` | Mono counts under the h1 |
| Counter | `font-display text-[40px] leading-none font-bold tabular-nums` | Dashboard tiles |
| Figure | `font-display text-[28px] leading-none font-bold` | Detail-page figures |
| Body | `text-sm` (14px) | Default UI text |
| Small | `text-xs` (12px) | Badges, legends |
| Input | `text-base` (16px) | Inputs, so iOS doesn't zoom |
| Data | `font-mono text-xs` | IDs, versions, timestamps |
| Label | `mono-label` | Section rules, tile labels, chips, nav groups (11px, 0.08em, uppercase) |

Anything that can change under a poll uses tabular numerals. `th` and `td` do
this already; add `tabular` elsewhere.

## Shape and space

- 4px grid. Panels and tiles have `p-4`. Table cells are `px-3 py-2`. Pages
  have `p-4`, and `sm:p-6` from the `sm` breakpoint up.
- One radius: `rounded` (4px) for every control, badge, chip, toast, dialog and
  the command palette. Panels, cards and tile grids are square. `rounded-full`
  is only for status and tag dots.
- Flat elevation. Surfaces are separated by fill and a hairline. `shadow-lg`
  is only for elements floating over content (toasts, the mobile drawer,
  the palette).
- Stat tiles sit in a `gap-px` grid over a `border-soft` ground, so the gaps
  draw the rules. `tick-scale` is the one ornament: put it under a figure and
  nowhere else.
- Separate the regions of a page with `SectionRule`, not a card for each
  region.

## Interaction states

- **Focus:** a solid 2px ring on everything interactive,
  `focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none`.
  Don't use a translucent ring (`ring-ring/50`): in light it is 2.2:1.
- **Hover:** `hover:bg-accent` on rows, nav items and ghost or outline
  buttons; `primary-hover` on the default button; underline on links.
- **Disabled:** 50% opacity, with no pointer events.
- **Selected:** a chip gets `aria-pressed` with an `accent` fill; a tab gets a
  2px `primary` underline; nav gets a 2px `primary` inset bar.
- **Touch:** controls used on phones (paginator, shell, error retry) are at
  least 44px (`ButtonSize::Touch`).

## Components

Reuse these before writing a class list. Use `components::ui` (the vendored
shadcn ports, merged with `tw_merge`) for primitives, and `components` for
app-specific pieces.

| Component | Use |
|---|---|
| `ui::Button` | Every button. Variants: `Default` (one per view), `Outline`, `Secondary`, `Ghost`, `Link`, `Destructive`. Sizes: `Sm`, `Default`, `Lg`, `Icon`, `Touch`. |
| `ui::Input`, `SELECT` | Text fields and selects: 36px, `input` edge |
| `ui::Card` | A square `card` panel with padding |
| `ui::Dialog`, `ConfirmDialog` | Modals, which trap focus and close on Esc |
| `StatusBadge` | A state as a dot plus a tinted word. Pass the raw API status. |
| `Chip` | A toggle filter, `aria-pressed`, with an optional tone or tag dot |
| `TagChip` | A read-only tag: its name plus a colour dot |
| `ProgressBar`, `ProgressLegend` | Stacked rollout progress, always with its legend |
| `SectionRule` | A labelled hairline between regions |
| `Tabs`, `TabPanel` | Detail-page tabs, moved between with the arrow keys |
| `Paginator` | First/Prev/Next/Last under a list |
| `SearchBox` | A FIQL search box. Enter searches. |
| `ToastStack`, `toast_ok`, `toast_error` | Transient results, bottom right, announced politely. A toast is never the only report of a failure. |
| `ErrorPane` | Takes the place of content that failed to load, with a Retry button |
| `CommandPalette` | ⌘K navigation and actions |
| `TABLE`, `TH`, `TD`, `ROW`, `LINK_CELL` | List tables. Rows are not click targets; the first cell holds a real link. Wrap the table in `overflow-x-auto`. |

## Iconography and logo

- There is no icon library. Use Unicode glyphs in the text colour: `→` for
  links, `×` to dismiss, and `⌘K` inside a `kbd`.
- A status dot is `size-[7px] rounded-full` plus a `tone_fill` class. In text,
  such as the `title` of a progress segment, use `logic::tone_glyph`, which
  matches the TUI.
- The logo is the raster medallion in `raptor-ui/assets/logo/`:
  `logo-sidebar.png` (64px source shown at 32px) and `logo-login.png` (96px
  source shown at 40px). Don't recolour it. Next to the logo, the wordmark is
  lowercase "raptor" in `font-display text-lg font-bold tracking-wide
  text-primary`, and the logo has `alt=""`.

## Terminal (`raptorctl tui`)

Use the same principles with the 16 ANSI colours, so the UI inherits the
operator's terminal theme. Only `theme.rs` names a colour.

| Slot | ANSI | Console equivalent |
|---|---|---|
| `fg` | Reset | `fg-dim` |
| `fg_muted` | DarkGray | `muted-foreground` |
| `fg_emphasis` | Reset + BOLD (never White) | `foreground` |
| `selection` | Reverse video (+BOLD in tables) | selected row |
| `accent` | Cyan | `primary` |
| `success` / `warning` / `error` / `info` | Green / Yellow / Red / Blue | `ok` / `pend` / `err` / `info` |

- Status glyphs: `●` in sync, `◐` pending, `✕` error, `○` registered (`info`
  colour) or unknown (`fg_muted`).
- `NO_COLOR` sets every slot to `Reset`. Selection is already reverse video, so
  nothing else changes.
- Every bordered panel goes through `panel()`. The panel holding the selection
  gets an `accent` border; the others get `fg_muted`. Confirm dialogs get a
  `warning` border.
- Header counts are label/value chips, with the value in reverse video.
- The footer shows the current mode's keys as `[k]ey` hints, with any status
  message first. `Esc` backs out of a mode and never quits; `Ctrl+C` quits from
  any mode.
- The UI works down to 80 × 24. Below that it shows "terminal too small" and
  doesn't try to lay anything out.
