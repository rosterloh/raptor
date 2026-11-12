# ddi-polling Specification

## Purpose

The DDI poll root and device bookkeeping: what a polling device is told to do
next, how unknown devices register, how often devices are asked to poll, and how
devices report their attributes.

## Requirements

### Requirement: Poll root response
`GET /{tenant}/controller/v1/{controllerId}` SHALL return hawkBit's poll-root JSON: `config.polling.sleep` as `HH:MM:SS` and `_links` for the work currently offered. It SHALL record the poll time and the device's address on the target.

#### Scenario: Idle target
- **WHEN** a registered target with nothing assigned and no attribute request polls
- **THEN** the response has `config.polling.sleep` and no `deploymentBase`, `confirmationBase` or `cancelAction` link

### Requirement: Links follow action state
The poll root SHALL offer `deploymentBase` when the target's active action is `running`, `confirmationBase` when it is `wait_for_confirmation`, `cancelAction` when it is `canceling`, and `installedBase` once anything has been installed.

#### Scenario: Running action
- **WHEN** a target's active action 7 is `running`
- **THEN** `_links.deploymentBase.href` ends in `/deploymentBase/7`

#### Scenario: Canceling action
- **WHEN** an operator has requested a normal cancel of action 7
- **THEN** the poll offers `cancelAction/7` and no `deploymentBase`

### Requirement: Auto-registration
An unknown `controllerId` polling with gateway-token or anonymous auth SHALL be created with status `registered`, name equal to its controller id, a generated security token, and `autoConfirm` from `[ddi] auto_confirm_default`. Matching auto-assign target filters SHALL be applied in the same poll.

#### Scenario: First poll gets a deployment
- **WHEN** an unknown device matching an auto-assign filter polls with the gateway token
- **THEN** it is registered and the same response already offers `deploymentBase`

### Requirement: configData link only when attributes are wanted
The poll SHALL offer `configData` only while the target's `requestAttributes` flag is set. The flag SHALL be set on creation, cleared by any `configData` upload, and re-armable by an operator.

#### Scenario: After upload
- **WHEN** a device PUTs `configData` and polls again
- **THEN** the poll has no `configData` link

### Requirement: Attribute upload modes
`PUT .../configData` SHALL accept `{"mode","data"}` with mode `merge` (default; upsert keys), `replace` (drop all, then set) or `remove` (delete the listed keys). Unknown body fields SHALL be ignored. An attribute change SHALL re-evaluate auto-assign filters.

#### Scenario: Replace
- **WHEN** a target with attributes `{a,b}` PUTs `{"mode":"replace","data":{"c":"1"}}`
- **THEN** its attributes are exactly `{c: "1"}`

### Requirement: Polling interval and override rules
`[ddi] polling_interval` SHALL accept hawkBit's `pollingTime` grammar, `<default>[, <FIQL> -> <interval>]*`. The first rule whose filter matches the target wins. An interval MAY add `~NN%` jitter, drawn fresh per response. A malformed value or unknown filter field SHALL stop `raptor serve` at startup.

#### Scenario: Group override
- **WHEN** `polling_interval = "00:05:00, group==eu -> 00:00:30"` and a target in group `eu` polls
- **THEN** `config.polling.sleep` is `00:00:30`

#### Scenario: Bad rule
- **WHEN** a rule references an unknown field
- **THEN** `raptor serve` exits with an error before listening

### Requirement: Device address behind a proxy
The recorded address SHALL be the socket peer unless `[ddi] trusted_proxy_header` names a header. Then its rightmost valid IP SHALL be used. Both `X-Forwarded-For` and RFC 7239 `Forwarded` syntax, with or without a port, SHALL be understood.

#### Scenario: Forwarded chain
- **WHEN** `trusted_proxy_header = "x-forwarded-for"` and the header is `1.2.3.4, 10.0.0.5`
- **THEN** the address recorded is `10.0.0.5`
