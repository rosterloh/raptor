# Spec Delta

## MODIFIED Requirements

### Requirement: Credentials
`raptorctl login` SHALL verify credentials against the server, then save them to `~/.config/raptor/cli.toml` with mode `0600`. `logout` SHALL delete that file. `RAPTOR_URL`, `RAPTOR_USER` and `RAPTOR_PASS` SHALL override it, and flags SHALL override both. When `RAPTOR_TOKEN` is set, requests SHALL send it as a bearer token instead of Basic credentials.

#### Scenario: CI use
- **WHEN** `RAPTOR_URL`, `RAPTOR_USER` and `RAPTOR_PASS` are set
- **THEN** commands run without a saved config

#### Scenario: OIDC token in CI
- **WHEN** `RAPTOR_URL` and `RAPTOR_TOKEN` are set
- **THEN** commands authenticate with `Authorization: Bearer <token>`

## ADDED Requirements

### Requirement: User commands
`raptorctl user list|create|set|delete` SHALL manage users through `/rest/v1/users`. `create` and `set --password` SHALL prompt for the password on a TTY and read it from stdin otherwise, never from a flag. `delete` SHALL confirm on a TTY unless `-y` is given.

#### Scenario: Create a read-only user
- **WHEN** an admin runs `raptorctl user create bob --role read_only` and types a password
- **THEN** `bob` is created with role `read_only`
