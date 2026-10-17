# CLI Reference

The `raptor` binary has three subcommands.

## `raptor serve`

Runs the server: loads config, connects to the database, applies migrations,
starts the background evaluator, and serves HTTP.

```console
$ raptor serve [--config <path>]
```

| Flag | Default | Description |
|---|---|---|
| `--config <path>` | `raptor.toml` | path to the TOML config file |

Migrations are applied automatically before the listener starts. The server logs
to stdout/stderr; control verbosity with the `RUST_LOG` environment variable,
e.g. `RUST_LOG=raptor=debug,tower_http=info`. In an `otel` build with
[`[otel]`](./configuration.md#otel--opentelemetry-export) configured, the same
filter decides what is exported, and the OpenTelemetry SDK's own warnings and
errors are added to stdout unless `RUST_LOG` already names an `opentelemetry`
target.

```console
$ RUST_LOG=raptor=debug raptor serve --config /etc/raptor/config.toml
raptor listening bind=0.0.0.0:8088
```

## `raptor hash-password`

Reads a password from stdin and prints its argon2id hash for use in
`mgmt.password_hash`.

```console
$ printf 'yourpassword\n' | raptor hash-password
$argon2id$v=19$m=19456,t=2,p=1$...
```

Pipe from a file or a secrets manager rather than typing interactively in
scripts. The output is safe to store in the config file (it's a hash, not the
password).

## `raptor storage-backfill`

Copies an existing on-disk artifact store into the `[storage.s3]` bucket, for
moving a deployment from `backend = "fs"` to `"s3"`.

```console
$ raptor storage-backfill [--config <path>]
uploaded 42 blob(s) (3221225472 bytes); 0 already in the bucket
```

| Flag | Default | Description |
|---|---|---|
| `--config <path>` | `raptor.toml` | path to the TOML config file |

It walks `artifact_dir` and uploads every blob the bucket doesn't already hold.
Blobs are keyed by their SHA-256, so a re-run skips what is already there —
safe to run while the server is still on `fs`, again after an interruption, and
once more after stopping the server to catch late uploads. Local files are
never modified or deleted. The database stores only hashes, so no database
change is involved.

A typical switch:

1. Add `[storage.s3]` (leave `backend` unset) and run `raptor storage-backfill`.
2. Stop raptor, run `raptor storage-backfill` again, set `[storage] backend = "s3"`.
3. Start raptor. Once satisfied, remove the old blobs under `artifact_dir`
   (keep the directory itself — it is still the upload spool).

## Version

```console
$ raptor --version
```
