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
> roles and OIDC are not yet implemented.

## Deployment guidance

- Terminate TLS at a reverse proxy in front of raptor; the DDI tokens and Basic
  credentials are bearer secrets that must not travel in cleartext.
- Set `url` to the externally visible base URL so `_links` are dialable through
  the proxy (see [Configuration](../getting-started/configuration.md)).
- Prefer environment variables or the systemd `raptor.env` file for the gateway
  token and password hash rather than committing them to a shared config file
  (see [Running as a systemd Service](./systemd.md)).
