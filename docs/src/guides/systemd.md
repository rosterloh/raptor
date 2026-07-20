# Running as a systemd Service

The Debian package installs a hardened systemd unit at
`/usr/lib/systemd/system/raptor.service`. This guide covers how it's wired and
how to operate it.

## Layout

| Path | Purpose |
|---|---|
| `/usr/bin/raptor` | the binary (web console embedded) |
| `/etc/raptor/config.toml` | config, a dpkg *conffile* (edits survive upgrades) |
| `/var/lib/raptor/` | state: SQLite DB and artifact blobs |
| `/etc/raptor/raptor.env` | optional, root-only secrets (not shipped) |

## First start

The service is **enabled but not started** on install — you must set an admin
password first:

```console
$ raptor hash-password                 # type a password, copy the hash
$ sudoedit /etc/raptor/config.toml     # set password_hash, choose DDI auth
$ sudo systemctl start raptor
$ systemctl status raptor
```

## State directory

The unit uses systemd's `DynamicUser=yes` with `StateDirectory=raptor`. That
means:

- raptor runs as a **transient, unprivileged user** allocated at runtime — there
  is no `raptor` user to manage and no `postinst` `chown`.
- `/var/lib/raptor` is created and owned by that user automatically, mode `0750`.

The default config points both the database and the artifact store there:

```toml
database_url = "sqlite:///var/lib/raptor/raptor.db?mode=rwc"
artifact_dir = "/var/lib/raptor/artifacts"
```

`/var/lib` is the FHS location for persistent, service-owned state — the
content-addressed artifact blobs are the source of truth (not regenerable
cache), which is why they live here rather than under `/var/cache`.

## Secrets

The config file is **world-readable** because the DynamicUser must read it. Keep
plaintext secrets out of it. Put them in a root-only environment file, which
systemd reads *before* dropping privileges:

```console
$ sudo install -m 600 /dev/null /etc/raptor/raptor.env
$ echo 'RAPTOR_DDI__GATEWAY_TOKEN=super-secret' | sudo tee -a /etc/raptor/raptor.env
$ sudo systemctl restart raptor
```

The unit loads it via `EnvironmentFile=-/etc/raptor/raptor.env` (the leading `-`
makes it optional). The mandatory `password_hash` is an argon2 hash and is safe
to keep in the config.

## Hardening

The unit ships with a broad systemd sandbox: `ProtectSystem=strict`,
`ProtectHome=yes`, `PrivateTmp=yes`, `NoNewPrivileges=yes`,
`MemoryDenyWriteExecute=yes`, a restricted `SystemCallFilter=@system-service`, an
empty `CapabilityBoundingSet`, and address-family restrictions. raptor needs no
Linux capabilities because it binds a high port (8080) by default.

> **Note:** If you change `bind` to a privileged port (< 1024), grant the
> capability explicitly with `AmbientCapabilities=CAP_NET_BIND_SERVICE` via a
> drop-in (`systemctl edit raptor`), or bind a high port and let the reverse
> proxy front it.

## Logs

raptor logs to stdout/stderr, captured by the journal:

```console
$ journalctl -u raptor -f
```

Adjust verbosity with the `RUST_LOG` env var (e.g. `RUST_LOG=raptor=debug`) via a
`systemctl edit` drop-in or `raptor.env`.
