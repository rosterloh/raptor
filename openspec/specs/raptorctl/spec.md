# raptorctl Specification

## Purpose

`raptorctl`, a separate command-line and terminal UI client that drives a
running raptor over the public Management API.

## Requirements

### Requirement: Pure API client
`raptorctl` SHALL use only the public Management API and need no server-side changes.

#### Scenario: Remote server
- **WHEN** an operator points `raptorctl` at any reachable raptor
- **THEN** every command works over HTTP

### Requirement: Credentials
`raptorctl login` SHALL verify credentials against the server, then save them to `~/.config/raptor/cli.toml` with mode `0600`. `logout` SHALL delete that file. `RAPTOR_URL`, `RAPTOR_USER` and `RAPTOR_PASS` SHALL override it, and flags SHALL override both.

#### Scenario: CI use
- **WHEN** `RAPTOR_URL`, `RAPTOR_USER` and `RAPTOR_PASS` are set
- **THEN** commands run without a saved config

### Requirement: Machine-readable output
Every command SHALL accept `--json` to print the server's raw response instead of a table.

#### Scenario: Pipe to jq
- **WHEN** an operator runs `raptorctl target list --json`
- **THEN** stdout is the paged JSON envelope

### Requirement: Safe destructive commands
Destructive commands (`ds invalidate`, `rollout stop`) SHALL ask for confirmation when stdin is a TTY unless `-y` is given. `ds invalidate` SHALL leave actions and rollouts alone unless asked otherwise.

#### Scenario: Scripted stop
- **WHEN** `rollout stop` runs with stdin not a TTY
- **THEN** it proceeds without prompting

### Requirement: Publish is atomic from the user's view
`publish` SHALL validate types and the file before writing anything, and delete the module it created if a later step fails.

#### Scenario: Upload fails
- **WHEN** the artifact upload in `publish` fails
- **THEN** the module created for it is deleted

### Requirement: Terminal UI
`raptorctl tui` SHALL show the fleet, the selected target's status and actions, and running rollouts, refresh on an interval (default 5 s, `0` off), honour `NO_COLOR`, and mark data stale after a failed refresh.

#### Scenario: Server unreachable
- **WHEN** a refresh fails or times out
- **THEN** the header shows `⚠ stale` until a fetch succeeds
