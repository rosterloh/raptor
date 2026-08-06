# CLI & TUI (`raptorctl`)

`raptorctl` is a separate binary (crate `raptor-cli`) that drives a running
raptor server over the same Management API the web console uses. It is a pure
HTTP client — install it anywhere that can reach `raptor serve`, no server-side
changes required.

## Login

```console
$ raptorctl login --url http://localhost:8088 --user admin
password for admin: 
logged in as admin
```

This verifies the credentials with a request against the server, then saves
them to `~/.config/raptor/cli.toml` (mode `0600`). Every later command reuses
that file, so `--url`/`--user` are only needed again to switch servers.

`raptorctl logout` deletes the saved file.

### Scripting / CI

Skip the config file entirely with environment variables — useful in CI where
there's no interactive prompt for `login` to read a password from:

```console
$ RAPTOR_URL=http://localhost:8088 RAPTOR_USER=admin RAPTOR_PASS=secret \
    raptorctl target list
```

Precedence is flags > environment > the saved config file.

## Commands

Every command accepts `--json` to print the server's raw response instead of a
table — pipe it into `jq`:

```console
$ raptorctl target list --json | jq '.content[].controllerId'
```

| Command | Does |
|---|---|
| `target list/get/create/set/delete` | target CRUD |
| `target attributes <cid>` | reported device attributes |
| `target tag add\|rm <cid> <tag>` | tag/untag a target |
| `target assign <cid> --ds <id> [--force ...]` | assign a distribution set |
| `target actions <cid>` | a target's action history |
| `module list/create` | software module CRUD |
| `artifact upload/list/delete <moduleId>` | artifact management |
| `ds list/get/create` | distribution set CRUD |
| `action list/status/cancel/force` | deployment action control |
| `status` | fleet-wide statistics |

Run `raptorctl <command> --help` for full flag lists.

### End-to-end example

```console
$ raptorctl module create --name fw --version 1.4.1 --type os
$ raptorctl artifact upload 1 ./firmware.bin
$ raptorctl ds create --name fw --version 1.4.1 --type os --module 1
$ raptorctl target create dev-042
$ raptorctl target assign dev-042 --ds 1 --force forced
$ raptorctl target get dev-042
```

## TUI

`raptorctl tui` opens an interactive dashboard: the fleet on the left, the
selected target's status, assigned/installed set, and action history on the
right, and running rollouts underneath the target list.

```console
$ raptorctl tui [--refresh <seconds>]   # default 5, 0 disables auto-refresh
```

| Key | Does |
|---|---|
| `↑↓` / `j`/`k`, `g`/`G` | move selection |
| `/` | filter targets (sent server-side as a FIQL `q=`) |
| `a` | assign a distribution set to the selected target |
| `t` | tag the selected target |
| `c` / `f` | cancel / force the target's active action (`y` to confirm) |
| `r` | refresh now |
| `?` | help |
| `q` / `Esc` | quit |

It respects `NO_COLOR`, works over SSH and inside tmux, and requires at least
an 80x24 terminal.
