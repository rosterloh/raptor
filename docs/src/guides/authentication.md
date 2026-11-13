# Authentication

raptor has two independent auth zones: the **device-facing DDI zone** and the
**operator-facing Management zone**.

## DDI zone (devices)

Devices authenticate on every DDI request with one of:

- **Target security token** —
  `Authorization: TargetToken <token>`, matched against the target's stored
  token. Each target has its own token (generated at creation, or supplied).
  A target authenticated this way can list and download only the artifacts of
  modules in distribution sets it has (or had) an action for; any other gets
  `404`.
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

### Certificate authentication (mTLS via a reverse proxy)

Fleets provisioned with X.509 client certificates can skip per-device tokens.
TLS stays at a reverse proxy: the proxy verifies the device's certificate
against your CA and forwards two headers, which raptor trusts in place of a
token. This is hawkBit's *header authentication* and uses the same header
names.

- `X-Controller-Id` — the device id the certificate proves (typically its CN).
  It must equal the `controllerId` in the request path.
- `X-Authority` — who vouched for it, typically a hash of the issuing CA. It
  must match an entry in `trusted_authorities` (case-insensitive). It may be a
  `,`/`;` list, or numbered headers `X-Authority-0`, `X-Authority-1`, … for a
  certificate chain.

```toml
[ddi.header_auth]
enabled = true
trusted_authorities = ["4b5a8e1f"]   # required
trusted_proxies = ["127.0.0.1"]      # only these peers may send the headers
auto_register = true                 # default: unknown devices register on first poll
```

A device authenticated this way behaves like one using its own target token:
it can download only the artifacts of sets it has been assigned. Unlike a
target token, it can register itself on its first poll, because certificate
fleets are provisioned at the factory rather than created in raptor first. Set
`auto_register = false` if your CA also signs certificates for things that are
not devices. Then an unknown device gets `401`, and you create targets yourself.

If the headers are missing or don't check out, raptor falls back to the token
methods, so a gateway behind the same proxy can keep using its `GatewayToken`.

> **Warning: the headers are only as trustworthy as the path to raptor.** Any
> client that can reach raptor directly can send `X-Controller-Id` itself.
> Do all of the following:
>
> 1. Make raptor reachable only through the proxy: `bind = "127.0.0.1:8088"`,
>    or a private interface.
> 2. List the proxy's address in `trusted_proxies`. Headers from any other
>    peer are then ignored.
> 3. Have the proxy **overwrite** these headers on every request, never pass
>    through what the client sent.
>
> raptor logs a warning at startup when header auth is on, `trusted_proxies`
> is empty and `bind` is not a loopback address.

An nginx example that requires a client certificate on the device-facing
server and forwards its identity:

```nginx
# Extract the CN from the verified subject DN (RFC 2253 form, e.g. "CN=dev-1,O=Acme").
map $ssl_client_s_dn $ssl_client_cn {
    default        "";
    ~(^|,)CN=(?<cn>[^,]+) $cn;
}

server {
    listen 443 ssl;
    server_name ota.example.com;

    ssl_certificate         /etc/nginx/tls/server.crt;
    ssl_certificate_key     /etc/nginx/tls/server.key;
    ssl_client_certificate  /etc/nginx/tls/device-ca.pem;
    ssl_verify_client       on;

    location / {
        # Set, never pass through: proxy_set_header replaces any client-sent
        # value, and an empty value drops the header entirely.
        proxy_set_header X-Controller-Id $ssl_client_cn;
        proxy_set_header X-Authority     $ssl_client_i_dn;
        proxy_set_header X-Authority-0   "";
        proxy_set_header X-Authority-1   "";
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_pass http://127.0.0.1:8088;
    }
}
```

Whatever value you forward as `X-Authority` (the issuer DN as above, or a
fingerprint) is what goes in `trusted_authorities`. Because the proxy always
sets the plain `X-Authority` header, raptor never consults numbered ones; the
two empty `X-Authority-N` lines are belt and braces. With
`ssl_verify_client on`, nginx has already rejected certificates that don't
chain to `device-ca.pem` before raptor sees the request.

## Management zone (operators)

The Management API and web console authenticate with HTTP Basic, a session
cookie, or (with [OIDC](#single-sign-on-oidc) configured) a bearer token. The `[mgmt]` account in the config file is always an admin:

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

### Users and roles

Admins can add more operator accounts through `/rest/v1/users`, `raptorctl
user` or the console's **Users** page. Each user has one of two roles:

| Role | May |
|---|---|
| `admin` | everything, including managing users |
| `read_only` | `GET`/`HEAD` on every Management API route (including the event stream) and log out |

Anything else a `read_only` user sends is answered `403` with hawkBit's
`InsufficientPermissionException` body, before any handler runs, so a new
write endpoint is protected without extra code. Read-only users never see a
target's `securityToken` (hawkBit's `READ_TARGET_SECURITY_TOKEN`), since a
token is enough to impersonate the device.

```console
$ raptorctl user create alice --role read-only      # prompts for the password
$ printf 's3cret\n' | raptorctl user create ci --role admin
$ raptorctl user set alice --disable
$ raptorctl user delete alice -y
```

- The `[mgmt]` account is not stored as a user. It cannot be deleted, disabled
  or demoted through the API, so it stays a break-glass admin. Its username is
  reserved.
- With no users created, behaviour is exactly as before.
- Users are stored in the database with argon2id password hashes. Passwords
  are never returned by the API.
- Deleting or disabling a user, or changing their role, ends their open
  sessions at once.
- A user cannot delete, disable or demote their own account.
- `GET /rest/v1/userinfo` returns the caller's username, role and hawkBit
  permission names; the console uses it to hide controls the role cannot use.
  The server enforces the role either way.
- Rollout approvals record the approving user (`approveDecidedBy`).

With only two roles, an admin can approve their own rollout. A separate
approver role would need per-route permissions.

### Single sign-on (OIDC)

With a `[mgmt.oidc]` section, operators can sign in through an OpenID Connect
provider (Keycloak, Authentik, Entra ID, Google, ...), and scripts can call the
Management API with the provider's JWTs. Without the section nothing changes:
the OIDC routes answer `404` and bearer tokens are refused.

```toml
url = "https://raptor.example.com"        # required: the redirect URI is built from it

[mgmt.oidc]
issuer = "https://id.example.com/realms/ops"
client_id = "raptor"
client_secret = "..."                     # or RAPTOR_MGMT__OIDC__CLIENT_SECRET
scopes = ["openid", "profile"]            # the default
role_claim = "groups"
admin_values = ["raptor-admins"]
read_only_values = ["raptor-viewers"]
# audience = "raptor"                     # bearer `aud`; default client_id
# username_claim = "preferred_username"   # the default
```

Register `<url>/rest/v1/oidc/callback` as the client's redirect URI.

- **Console**: the login page shows **Sign in with SSO**. raptor runs the
  authorization-code flow with PKCE, a `state` and a `nonce`, validates the ID
  token, then starts a normal session and returns to `/ui`. A login must be
  completed within 10 minutes; pending logins are lost on restart.
- **Bearer tokens**: `Authorization: Bearer <JWT>` is accepted when the token
  is signed by one of the provider's keys (asymmetric algorithms only), `iss`
  is the configured issuer, `aud` contains `audience`, and `exp`/`nbf` hold
  within 60 s of clock skew. Anything else is `401`. With `raptorctl`, set
  `RAPTOR_TOKEN`. raptor does not fetch tokens itself; get them from the
  provider by its own means.
- **Roles**: `role_claim` may be a string or an array. A value in
  `admin_values` gives `admin`, otherwise one in `read_only_values` gives
  `read_only`. An identity with neither is refused with `403`. The role is
  read from every token, so a group change at the provider takes effect on the
  next request or sign-in.
- **User records**: the first sign-in or bearer request creates a `users` row
  with source `oidc`, named by `username_claim`. Admins see these on the Users
  page and can disable or delete them; a disabled OIDC user is refused even
  with a valid token. OIDC users have no password and can never use Basic auth
  or `POST /rest/v1/login`. A name that belongs to a local user or the `[mgmt]`
  account is refused, so the provider cannot take over those accounts.
- **Provider outages**: raptor fetches the provider's discovery document and
  keys in the background and never refuses to start over them. Until they
  load, SSO answers `503` and bearer tokens `401`, devices and password users
  are unaffected, and each failed attempt is logged at `error` (retrying from
  1 s, backing off to every 5 minutes). When the provider rotates keys, a
  token with an unknown key id triggers one key refresh, at most once a
  minute.
- Startup fails if `[mgmt.oidc]` is present but `url` is unset, the issuer is
  not a URL, `client_id` is empty, or both value lists are empty.

#### Authentik example

1. Create an **OAuth2/OpenID Provider**:
   - **Client type**: Confidential.
   - **Redirect URIs**: `https://raptor.example.com/rest/v1/oidc/callback`
     (strict).
   - **Signing Key**: pick a certificate, for example *authentik Self-signed
     Certificate*. This is required. Without one, Authentik signs tokens with
     the client secret (HS256), which raptor refuses, and logs a warning that
     the provider publishes no keys.
   - Leave **Encryption Key** empty. raptor does not read encrypted (JWE)
     tokens.
   - Scopes `openid` and `profile`. Authentik's `profile` scope already carries
     `groups` (group names) and `preferred_username`.
2. Create an **Application** for the provider, slug `raptor`. If you bind a
   policy to limit who may use it, allow both raptor groups.
3. Create groups `raptor-admins` and `raptor-viewers` and add users.

The issuer is the provider's *OpenID Configuration Issuer*, which ends with a
slash. raptor accepts it with or without one:

```toml
[mgmt.oidc]
issuer = "https://auth.example.com/application/o/raptor/"
client_id = "..."                         # the provider's Client ID
client_secret = "..."
role_claim = "groups"
admin_values = ["raptor-admins"]
read_only_values = ["raptor-viewers"]
```

Authentik puts the client ID in the access token's `aud`, so the default
`audience` works for bearer tokens. For a script, one way is the client
credentials grant with a service account in one of the groups and an app
password. This step has not been tried against a live Authentik:

```console
$ RAPTOR_TOKEN=$(curl -s https://auth.example.com/application/o/token/ \
    -d grant_type=client_credentials -d client_id=<client id> \
    -d username=raptor-ci -d password=<app password> -d scope='openid profile' \
    | jq -r .access_token) raptorctl target list
```

#### Keycloak example

This example has not been tried against a live Keycloak. Treat it as a
starting point.

1. In realm `ops`, create an OpenID Connect client `raptor` with **Client
   authentication** on and the **Standard flow** enabled. Set **Valid redirect
   URIs** to `https://raptor.example.com/rest/v1/oidc/callback`, and copy the
   client secret from the **Credentials** tab.
2. Under the client's dedicated scope, add a **Group Membership** mapper with
   token claim name `groups`, **Full group path** off, and *Add to ID token*
   and *Add to access token* on.
3. For bearer access, add an **Audience** mapper that puts `raptor` in the
   access token's `aud`. Keycloak's access tokens otherwise carry `account`,
   which raptor refuses.
4. Create groups `raptor-admins` and `raptor-viewers` and add users to them.

```toml
[mgmt.oidc]
issuer = "https://keycloak.example.com/realms/ops"
client_id = "raptor"
client_secret = "..."
role_claim = "groups"
admin_values = ["raptor-admins"]
read_only_values = ["raptor-viewers"]
```

## Deployment guidance

- Terminate TLS at a reverse proxy in front of raptor; the DDI tokens and Basic
  credentials are bearer secrets that must not travel in cleartext.
- Set `url` to the externally visible base URL so `_links` are dialable through
  the proxy (see [Configuration](../getting-started/configuration.md)).
- Prefer environment variables or the systemd `raptor.env` file for the gateway
  token and password hash rather than committing them to a shared config file
  (see [Running as a systemd Service](./systemd.md)).
