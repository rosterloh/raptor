# Spec Delta

## MODIFIED Requirements

### Requirement: Tenant configuration is read-only
`GET /rest/v1/system/configs` SHALL report hawkBit keys derived from the config file, including `pollingTime`, `authentication.gatewaytoken.enabled`, `authentication.targettoken.enabled`, `authentication.header.enabled`, `authentication.header.authority`, `user.confirmation.flow.enabled`, `rollout.approval.enabled`, the cleanup keys and `multi.assignments.enabled` (false). Each SHALL be marked global. Writes to a key SHALL be `403`.

#### Scenario: Write attempt
- **WHEN** a client PUTs `/rest/v1/system/configs/pollingTime`
- **THEN** the response is `403`

#### Scenario: Certificate auth reported
- **WHEN** `[ddi.certificate]` is enabled with trusted issuer hashes `a1,b2`
- **THEN** `authentication.header.enabled` is `true` and `authentication.header.authority` is `a1,b2`
