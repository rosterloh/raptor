<p align="center">
  <img src="./logo.png" alt="raptor logo" width="160">
</p>

# raptor

**raptor** is a [hawkBit](https://eclipse.dev/hawkbit/)-compatible over-the-air
(OTA) update server, written in Rust. It speaks hawkBit's **DDI v1** device API —
so SWUpdate, the RAUC hawkbit-updater, and other hawkBit clients work
unchanged — and the core hawkBit **Management API** workflow, all from a single
static binary and one config file.

Where a stock hawkBit deployment is Java + a relational database + RabbitMQ,
raptor is one process backed by SQLite or Postgres. No broker, no JVM, no
servlet container.

## Highlights

- **Drop-in DDI v1** — devices poll, download, and report feedback exactly as
  they do against hawkBit.
- **Core Management API** — targets, software modules, artifacts, distribution
  sets, assignments, actions, rollouts, and target filters, with hawkBit-shaped
  JSON, paging, sorting, and FIQL `q=` filtering.
- **Rollouts** — staged, threshold-driven group deployments.
- **Target filters + auto-assignment** — saved FIQL queries that assign a
  distribution set to matching devices automatically.
- **Confirmation flow** — optional device/operator confirmation before a
  deployment starts.
- **One binary** — an embedded web console ships inside the executable; SQLite
  by default, Postgres when you need it.
- **Packaged** — a Debian package with a hardened systemd unit.

## Where to go next

- New here? Start with [What is raptor?](./intro/what-is-raptor.md) and the
  [Quick Start](./intro/quick-start.md).
- Ready to run it? See [Installation](./getting-started/installation.md) and
  [Your First Deployment](./getting-started/first-deployment.md).
- Coming from hawkBit? Read [hawkBit Compatibility](./intro/compatibility.md) to
  see what is and isn't implemented.

> **Note:** raptor is young and its surface is a subset of hawkBit's. The
> [compatibility matrix](./intro/compatibility.md) is the source of truth for
> what exists today.
