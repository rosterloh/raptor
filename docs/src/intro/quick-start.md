# Quick Start

This gets a raptor server running and deploys an update to a (virtual) device in
a few commands. For production installs, see
[Installation](../getting-started/installation.md).

## 1. Build and configure

```console
$ cargo build --release
$ printf 'yourpassword\n' | ./target/release/raptor hash-password
$argon2id$v=19$m=19456,t=2,p=1$...
```

Put that hash into a `raptor.toml`:

```toml
bind = "0.0.0.0:8080"
database_url = "sqlite://raptor.db?mode=rwc"   # or postgres://user:pass@host/db
artifact_dir = "./artifacts"

[ddi]
anonymous = true          # dev only — no device auth

[mgmt]
username = "admin"
password_hash = "$argon2id$v=19$m=19456,t=2,p=1$..."
```

> **Warning:** `anonymous = true` disables all device authentication. Use it for
> local experiments only. See [Authentication](../guides/authentication.md) for
> production setups.

## 2. Run the server

```console
$ ./target/release/raptor serve --config raptor.toml
raptor listening bind=0.0.0.0:8080
```

The database is created and migrated automatically on first start.

## 3. Deploy an update

Using the Management API (HTTP Basic with the admin credentials):

```bash
# 1. a software module
curl -u admin:yourpassword -X POST localhost:8080/rest/v1/softwaremodules \
  -H 'Content-Type: application/json' \
  -d '[{"name":"rootfs","version":"1.0","type":"os"}]'

# 2. an artifact on module 1
curl -u admin:yourpassword -X POST localhost:8080/rest/v1/softwaremodules/1/artifacts \
  -F 'file=@rootfs.img'

# 3. a distribution set bundling module 1
curl -u admin:yourpassword -X POST localhost:8080/rest/v1/distributionsets \
  -H 'Content-Type: application/json' \
  -d '[{"name":"release","version":"1.0","type":"os","modules":[{"id":1}]}]'

# 4. assign DS 1 to a device (auto-registered on first poll)
curl -u admin:yourpassword -X POST localhost:8080/rest/v1/targets/my-device/assignedDS \
  -H 'Content-Type: application/json' -d '{"id":1,"type":"forced"}'
```

## 4. Poll as the device

```console
$ curl localhost:8080/DEFAULT/controller/v1/my-device
{"config":{"polling":{"sleep":"00:05:00"}},
 "_links":{"deploymentBase":{"href":".../deploymentBase/1"},
           "configData":{"href":".../configData"}}}
```

The `deploymentBase` link tells the device an update is waiting. From here a real
hawkBit client downloads the artifacts and posts feedback. Walk through the full
cycle in [Your First Deployment](../getting-started/first-deployment.md).
