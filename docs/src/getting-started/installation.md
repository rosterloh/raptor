# Installation

raptor is a single binary. You can build it from source or install the Debian
package.

## Debian / Ubuntu package

Prebuilt `.deb` packages are attached to each
[GitHub Release](https://github.com/rosterloh/raptor/releases):

```console
$ sudo dpkg -i raptor_*.deb
```

The package installs:

- `/usr/bin/raptor` — the server, with the web console embedded
- `/etc/raptor/config.toml` — default config, registered as a dpkg *conffile*
  (your edits survive upgrades)
- `/usr/lib/systemd/system/raptor.service` — a hardened systemd unit running as a
  transient `DynamicUser`, with state (SQLite DB + artifacts) under
  `/var/lib/raptor`

The service is **enabled but not started** on install, because you must set an
admin password first:

```console
$ raptor hash-password                 # type a password, copy the hash
$ sudoedit /etc/raptor/config.toml     # paste into password_hash, pick DDI auth
$ sudo systemctl start raptor
```

See [Running as a systemd Service](../guides/systemd.md) for the unit's
hardening details and how to supply secrets safely.

## From source

You need Rust 1.98.1 or newer.

```console
$ git clone https://github.com/rosterloh/raptor
$ cd raptor
$ cargo build --release
$ ./target/release/raptor --version
```

The binary lands at `target/release/raptor`.

### Building with the web console

The embedded web console is behind the `embed-ui` Cargo feature. It requires the
[Dioxus CLI](https://dioxuslabs.com/) (`dx`) to build the WASM bundle first:

```console
$ mise install          # pinned dx (and the other dev tools) from mise.toml
$ mise run ui           # clean release build of the console bundle
$ cargo build --release --features embed-ui
```

Without `embed-ui`, the server runs identically but does not serve `/ui`.

### Building the Debian package yourself

```console
$ mise run deb      # builds the console, then -> target/debian/raptor_*.deb
```

The package is built with the `embed-ui` and `otel` features.

## Database

raptor supports **SQLite** and **Postgres**, selected by the `database_url`
scheme. Migrations run automatically at startup, so there is no separate migrate
step.

- SQLite: `sqlite://raptor.db?mode=rwc` (the `mode=rwc` creates the file)
- Postgres: `postgres://user:pass@host/dbname`

## Next steps

- [Configuration](./configuration.md) — the `raptor.toml` file
- [Your First Deployment](./first-deployment.md) — an end-to-end walkthrough
