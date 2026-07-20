# Authentication

raptor has two independent auth zones: the **device-facing DDI zone** and the
**operator-facing Management zone**.

## DDI zone (devices)

Devices authenticate on every DDI request with one of:

- **Target security token** —
  `Authorization: TargetToken <token>`, matched against the target's stored
  token. Each target has its own token (generated at creation, or supplied).
- **Gateway token** —
  `Authorization: GatewayToken <token>`, a single shared token from config. A
  valid gateway token also enables **auto-registration** of unknown controller
  IDs.

Configure them under `[ddi]`:

```toml
[ddi]
gateway_token = "shared-secret-for-registration"
```

### Anonymous mode

```toml
[ddi]
anonymous = true
```

This disables DDI authentication entirely — any controller ID can poll and
register. It's convenient for local development and **should not be used in
production**. It is off by default.

## Management zone (operators)

The Management API and web console authenticate against a **single admin
credential** using HTTP Basic:

```toml
[mgmt]
username = "admin"
password_hash = "$argon2id$..."     # from `raptor hash-password`
```

The password is stored as an argon2id hash, never in plaintext. Generate it with:

```console
$ printf 'yourpassword\n' | raptor hash-password
```

The web console also issues a **session cookie** (via `POST /rest/v1/login`) so
browser users don't send Basic credentials on every request.

> **Note:** raptor has a single admin user in this version. Multiple users,
> roles, OIDC, and certificate-based (mTLS) DDI auth are not yet implemented.

## Deployment guidance

- Terminate TLS at a reverse proxy in front of raptor; the DDI tokens and Basic
  credentials are bearer secrets that must not travel in cleartext.
- Set `url` to the externally visible base URL so `_links` are dialable through
  the proxy (see [Configuration](../getting-started/configuration.md)).
- Prefer environment variables or the systemd `raptor.env` file for the gateway
  token and password hash rather than committing them to a shared config file
  (see [Running as a systemd Service](./systemd.md)).
