# Proposal: delete distribution sets and rollouts from raptorctl

## Why

The server already deletes distribution sets and rollouts
(`DELETE /rest/v1/distributionsets/{id}`, `DELETE /rest/v1/rollouts/{id}`), but
`raptorctl` can do neither. A test release published by mistake stays in every
`ds list` and in the TUI's assign picker, and a denied rollout, which `rollout
deny` says "can only be deleted", cannot be cleared without hand-written HTTP.

## What Changes

- `raptorctl ds delete <id>`: deletes a set. The server still refuses one any
  action references; its modules stay for `module delete`. No prompt, like
  `module delete`, because the server's refusal is the guard.
- `raptorctl rollout delete <id> [-y]`: deletes a rollout in any state. The
  server cancels its in-flight actions at once, without device confirmation,
  so it confirms on a TTY like `rollout stop`.
- `raptorctl tui`: `Del` in the assign picker deletes the highlighted set after
  a `y/n` confirm. `Tab` moves focus between the targets and rollouts panels;
  with rollouts focused, `↑↓`/`j k`/`g G` select a rollout and `Del` deletes it
  after a confirm. Target keys act only while targets are focused.

No server or wire-format change: `raptorctl` stays a pure Management API client.

## Impact

- `raptor-cli` only (`commands.rs`, `api/`, `tui/`), the CLI guide and
  `CHANGELOG.md`.
- Spec: `raptorctl` (Safe destructive commands, Terminal UI).
