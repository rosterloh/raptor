# Confirmation Flow

By default an assignment becomes an active deployment immediately. With the
**confirmation flow** enabled, a new assignment first enters a
`wait_for_confirmation` state and does **not** deploy until it is confirmed —
either by the device over DDI, or by an operator activating auto-confirm.

This mirrors hawkBit's confirmation flow (`confirmationBase`), which clients like
the RAUC hawkbit-updater support.

## Enabling the flow

It's a server-wide toggle in the config:

```toml
[ddi]
confirmation_flow = true
```

When off (the default), behavior is exactly as before — assignments go straight
to `running`.

## What a device sees

With the flow on, a target's poll offers a `confirmationBase` link instead of
`deploymentBase`:

```console
$ curl localhost:8080/DEFAULT/controller/v1/device-42
# _links.confirmationBase -> .../confirmationBase/7
```

`GET .../confirmationBase/{actionId}` returns the pending deployment (the same
chunk/artifact shape as `deploymentBase`, under a `confirmation` key) so the
device can decide. The device then confirms or denies:

```bash
# confirm -> action goes to running; next poll offers deploymentBase
curl -X POST localhost:8080/DEFAULT/controller/v1/device-42/confirmationBase/7/feedback \
  -H 'Content-Type: application/json' \
  -d '{"confirmation":"confirmed","details":["operator approved"]}'

# deny -> action stays waiting (a "denied" ActionStatus row is recorded)
curl -X POST localhost:8080/DEFAULT/controller/v1/device-42/confirmationBase/7/feedback \
  -H 'Content-Type: application/json' \
  -d '{"confirmation":"denied","details":["not now"]}'
```

A denied action remains in `wait_for_confirmation`; the device may confirm later,
or an operator can cancel it.

## Auto-confirm

A target can be set to **auto-confirm**, so assignments skip the wait state
entirely. Toggle it from the Management API:

```bash
curl -u admin:pw localhost:8080/rest/v1/targets/device-42/autoConfirm
# {"active": false}

curl -u admin:pw -X POST localhost:8080/rest/v1/targets/device-42/autoConfirm/activate
curl -u admin:pw -X POST localhost:8080/rest/v1/targets/device-42/autoConfirm/deactivate
```

Or by the device itself over DDI:

```bash
curl -X POST localhost:8080/DEFAULT/controller/v1/device-42/confirmationBase/activateAutoConfirm
curl -X POST localhost:8080/DEFAULT/controller/v1/device-42/confirmationBase/deactivateAutoConfirm
```

**Activating auto-confirm releases any already-pending actions** on that target —
they transition straight to `running`. New assignments to an auto-confirm target
never enter the wait state.

## Operator confirm/deny

There is currently no per-action operator confirm/deny over the Management API —
device-driven confirm/deny is DDI-only. The operator path is to activate
auto-confirm on the target (which releases pending actions), or to cancel the
action.
