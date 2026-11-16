# Tasks

## 1. CLI

- [x] 1.1 `api::distribution_sets::delete` and `ds delete <id>`
- [x] 1.2 `api::rollouts::delete` and `rollout delete <id> [-y]`, fetching first so the prompt names the rollout; parse test that a bare delete asks
- [x] 1.3 CLI guide command table and `CHANGELOG.md` Added bullets

## 2. TUI

- [x] 2.1 `Del` in the assign picker → `ConfirmDeleteDs`, `y` deletes; test that it names the filtered, highlighted set and `n` sends nothing
- [x] 2.2 `Focus` (targets | rollouts) toggled by `Tab`; rollout selection kept by id across refreshes; target keys ignored while rollouts are focused
- [x] 2.3 `Del` with rollouts focused → `ConfirmDeleteRollout`, `y` deletes
- [x] 2.4 Focused panel gets the accent border; footer hints and `?` help per focus; still fits 80×24
- [x] 2.5 Tests: Tab toggles focus, Del confirms the selected rollout and `n` backs out, selection follows the id across a refresh, `a` does nothing while rollouts are focused
- [x] 2.6 TUI key table in the CLI guide; CHANGELOG
