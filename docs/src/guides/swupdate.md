# Using raptor with SWUpdate (suricatta)

SWUpdate's suricatta module speaks hawkBit DDI v1 and works against raptor
unchanged. Unlike the [Zephyr guide](./zephyr.md), there is no wiring to
describe here beyond the server URL and a token — suricatta's hawkBit server
configuration is ordinary.

What this page covers is one symptom, because it is the one that brings
operators to the server looking for a bug that isn't there:

> **raptor keeps offering the same bundle. The device downloads it on every
> poll, the action never closes, and the target never reaches `in_sync`.**

There are four independent causes. **All four are client-side** — every one of
them lives on the device, in suricatta's state persistence. None of them are
visible from raptor, which is exactly why they're documented here. They were
found and fixed on real hardware (Raspberry Pi 5 with U-Boot, and Jetson AGX
Thor).

They also stack. Each one hides the next, so fixing one and seeing no change
does not mean the fix was wrong.

## Confirming it is client-side

Before reading further, check the action's status history:

```bash
curl -u admin:pw localhost:8088/rest/v1/targets/device-42/actions/1/status
```

If the loop is one of the four below, you will see:

- the action stuck `running` and `active`, never `finished`;
- a status history with the initial entries and **nothing since** — no
  `proceeding`, no `success`, no `failure`;
- in the server log, repeated `GET .../deploymentBase/{id}` and artifact GETs
  with **no `POST .../feedback` between them**.

Repeated downloads plus zero feedback is the signature. A slow-but-healthy
install looks different: it reports `proceeding` at least once. If you are
seeing feedback and the action still won't close, this page is not your
problem.

## Cause 1: suricatta cannot persist `action_id`

**Symptom:** every poll looks like a brand-new deployment to the device.

suricatta stores the action id it is working on via swupdate's vars store.
`swupdate_vars_initialize()` (`core/swupdate_vars.c`) returns `-EINVAL` when no
namespace is configured, so the id is never written. On the next poll the
device has no memory of the action in flight and starts over.

The trap is that the classic two-line `fw_env.config`:

```
/dev/mmcblk0  0x400000  0x4000
```

yields a single unnamed context with `nelem = 0`, so **any** namespace lookup
fails. It looks like a working environment — `fw_printenv` is fine — but
suricatta cannot use it.

libubootenv's YAML config form is required instead, along with two swupdate
settings: `fwenv-config-location` and `namespace-vars`.

```yaml
# /etc/fw_env.config — offsets, paths and sizes are device-specific
u-boot:
  size: 0x4000
  lockfile: /var/lock/fw_printenv.lock
  devices:
    - path: /dev/mmcblk0
      offset: 0x400000
      sectorsize: 0x1000

swupdate:
  size: 0x4000
  lockfile: /var/lock/swupdate_vars.lock
  devices:
    - path: /dev/mmcblk0
      offset: 0x404000
      sectorsize: 0x1000
```

```
# /etc/swupdate.cfg
globals: {
    fwenv-config-location = "/etc/fw_env.config";
    namespace-vars = "swupdate";
}
```

### Namespace order matters

libubootenv uses the **first** namespace in the file when the device tree names
none. Put the boot environment first, as above. Get this backwards and a
bootloader slot switch silently writes into the swupdate vars store, corrupting
the state that suricatta depends on — while appearing to work.

## Cause 2: only `ustate=2` closes the action

**Symptom:** the install succeeds, the device boots the new image, and the
action still reports "Testing Pending" forever.

Committing an update with just `upgrade_available=0` and `bootcount=0` leaves
`ustate` at `1` (INSTALLED). That is not enough.

Per `server_handle_initial_state` in `suricatta/server_hawkbit.c`:

| `ustate` after boot | suricatta reports | action |
|---|---|---|
| `1` INSTALLED | `proceeding` ("Testing Pending") | stays open |
| `2` TESTING | `success` | **closes** |
| `3` FAILED | `failure` | closes as failed |

Worse, the INSTALLED path then calls `save_state(STATE_OK)`, which destroys the
very state its own "an already-installed update is pending testing" guard
depends on. The next boot has nothing to report.

So a post-update health-check unit must set `ustate` explicitly:

```ini
# /etc/systemd/system/health-check.service
[Service]
Type=oneshot
ExecStart=/usr/bin/health-check.sh
ExecStart=/usr/bin/fw_setenv ustate 2
ExecStopPost=/bin/sh -c '[ "$EXIT_STATUS" = 0 ] || fw_setenv ustate 3'
```

Set `2` on pass and `3` on failure. Clearing `upgrade_available` alone will not
close the action.

## Cause 3: `bootloader="none"` has no state store at all

**Symptom:** the same as cause 1, on a platform that has no U-Boot — Tegra, for
instance — where the two-line `fw_env.config` was never an option to begin
with.

swupdate's `save_state` / `read_state` do **not** go through `swupdate_vars`.
They go through `bootloader_env_set` / `bootloader_env_get`, and the `none`
backend (`bootloader/none.c`) is a process-local dictionary. State dies with the
process. After a reboot `get_state()` returns `STATE_NOT_AVAILABLE`, suricatta
reports nothing, and a forced deployment reinstalls on every poll.

Configuring the vars namespace from cause 1 does not fix this — it is a
different store.

The fix that works: use the **`uboot` backend even with no U-Boot present**,
pointed at a plain file on persistent storage. Despite the name, that backend is
only libubootenv over whatever the fw_env config names:

```yaml
# /etc/fw_env.config
swupdate-state:
  size: 0x4000
  lockfile: /var/lock/swupdate_state.lock
  devices:
    - path: /var/lib/swupdate/state.env
```

The file must exist and survive reboots. This is safe wherever the
`sw-description` has no `bootenv` entries, since swupdate's own state is then
that environment's only consumer.

## Cause 4: the first update onto a device with no prior state

**Symptom:** looks intermittent across a fleet — some devices close their
actions, some never do, with no apparent pattern.

The image being *replaced* is the one that has to record the state. A device
whose current image has no usable state store therefore **cannot close its
first action**, no matter how correct the incoming image is. Every update after
that one closes normally.

There is no server-side fix and nothing to change on the device. Cancel that one
action by hand:

```bash
curl -u admin:pw -X DELETE \
  'localhost:8088/rest/v1/targets/device-42/actions/1?force=true'
```

See [Assignments & Actions](./actions.md#cancelling). Devices whose previous
image already had a working environment are unaffected, which is what makes the
pattern look random across a mixed fleet.

## Checklist

Work down it in order; each item can mask the ones below.

- [ ] `fw_env.config` is in libubootenv **YAML** form, not the two-line form
- [ ] the boot environment namespace is listed **first**
- [ ] `swupdate.cfg` sets both `fwenv-config-location` and `namespace-vars`
- [ ] the bootloader backend is not `none` — use `uboot` over a file if there is
      no real U-Boot
- [ ] a health check sets `ustate=2` on pass and `3` on failure
- [ ] the very first action on a previously stateless device was cancelled by
      hand
