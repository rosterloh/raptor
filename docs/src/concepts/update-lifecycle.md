# Update Lifecycle

This is the heart of raptor: how an assignment becomes an installed update, and
how each party's actions move the state machine.

## The happy path

```text
operator                 server                         device
────────                 ──────                         ──────
assignedDS  ──────────▶  Action(running), target=pending
                                                 ◀──────  poll  (sees deploymentBase)
                                                 ◀──────  GET deploymentBase
                         (streams artifacts)     ◀──────  download artifacts
                                                 ◀──────  feedback: proceeding
                         ActionStatus += proceeding
                                                 ◀──────  feedback: closed/success
                         Action(finished), target=in_sync
                                                 ◀──────  poll  (no deploymentBase;
                                                                 installedBase set)
```

1. **Assign.** `POST /rest/v1/targets/{id}/assignedDS` creates an Action in
   `running` (or `wait_for_confirmation` if the confirmation flow is on), and sets
   the target to `pending`. Any prior active action is cancelled.
2. **Poll.** The device polls the DDI root and sees `_links.deploymentBase`.
3. **Fetch & download.** The device gets `deploymentBase/{actionId}` and
   downloads the listed artifacts (with HTTP Range resume).
4. **Feedback.** The device reports `proceeding`, then a final result. Each report
   appends an ActionStatus row.
5. **Complete.** On `closed`/`success` the Action becomes `finished`, the target
   becomes `in_sync`, and `installedBase` reflects the deployment. On
   `closed`/`failure` the Action becomes `error` and the target becomes `error`.

## Feedback vocabulary

Device feedback is `{"status":{"execution": ..., "result":{"finished": ...}}}`.

- `execution` ∈ `proceeding`, `scheduled`, `resumed`, `downloading`,
  `downloaded`, `canceled`, `rejected`, `closed`.
- `result.finished` ∈ `none`, `success`, `failure`.

Only `closed` (with `success` or `failure`) is terminal; the others are recorded
as history and leave the action active.

## Confirmation flow

With the [confirmation flow](../guides/confirmation-flow.md) enabled, a new
assignment lands in `wait_for_confirmation` and the poll offers `confirmationBase`
instead of `deploymentBase`. A `confirmed` feedback moves it to `running`; the
next poll then follows the happy path above.

## Cancellation

```text
operator                 server                         device
────────                 ──────                         ──────
DELETE action ────────▶  Action(canceling)
                                                 ◀──────  poll  (sees cancelAction)
                                                 ◀──────  GET cancelAction
                                                 ◀──────  cancel feedback: closed
                         Action(canceled); assigned reverts to installed
```

A **normal** cancel sets the action to `canceling` and offers the device a
`cancelAction` link; the device confirms and the action becomes `canceled`. A
**forced** cancel (`?force=true`) closes it immediately, without waiting for the
device.

## Auto-registration

An unknown `controllerId` that polls with a valid gateway token (or in anonymous
mode) is created on the spot as `registered`. If an
[auto-assignment](../guides/target-filters.md) filter matches it, the DS is
assigned during that same poll — so a brand-new device can receive a
`deploymentBase` on its very first request.
