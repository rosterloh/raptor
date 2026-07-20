# CLI Reference

The `raptor` binary has two subcommands.

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
e.g. `RUST_LOG=raptor=debug,tower_http=info`.

```console
$ RUST_LOG=raptor=debug raptor serve --config /etc/raptor/config.toml
raptor listening bind=0.0.0.0:8080
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

## Version

```console
$ raptor --version
```
