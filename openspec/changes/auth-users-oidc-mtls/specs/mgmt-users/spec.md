# Spec Delta

## Purpose

Operator accounts beyond the `[mgmt]` config admin: who they are, which role
they hold, how that role limits the Management API, and how admins manage them.

## ADDED Requirements

### Requirement: User accounts
A user SHALL have a username unique within its tenant, a role, an `enabled` flag and a source: `local` (with an argon2id password hash) or `oidc` (no password). Users SHALL be stored with a `tenant` column and a named unique index on `(tenant, username)`.

#### Scenario: Duplicate username
- **WHEN** an admin creates user `alice` twice
- **THEN** the second create is `409`

### Requirement: Config account is a permanent admin
The `[mgmt]` config account SHALL always authenticate as `admin` and SHALL not be stored as a user. Its username SHALL be reserved: creating a user with that name SHALL be `409`.

#### Scenario: No users configured
- **WHEN** the `users` table is empty
- **THEN** the config account works exactly as before

### Requirement: Roles
The roles SHALL be `admin` (all operations) and `read_only` (reads only). Any other role value SHALL be `400`.

#### Scenario: Unknown role
- **WHEN** an admin creates a user with role `operator`
- **THEN** the response is `400`

### Requirement: Read-only enforcement
A `read_only` user SHALL be allowed `GET` and `HEAD` on every Management API route, including the event stream, plus `POST /rest/v1/logout`. Any other request SHALL be `403` with a hawkBit-shaped error body and change nothing.

#### Scenario: Read-only assign
- **WHEN** a `read_only` user posts to `/rest/v1/targets/dev-1/assignedDS`
- **THEN** the response is `403` and no action is created

#### Scenario: Read-only list
- **WHEN** a `read_only` user lists targets
- **THEN** the list is returned

### Requirement: User administration API
`/rest/v1/users` (raptor extension) SHALL let admins list, create, get, update (role, enabled, password) and delete users, with paging, `sort` and `q` over `username`, `role`, `enabled` and `source`. Password hashes SHALL never be returned. Non-admins SHALL get `403`.

#### Scenario: Create local user
- **WHEN** an admin POSTs `{"username":"alice","password":"…","role":"read_only"}`
- **THEN** `alice` exists and can log in, and the response has no password or hash

#### Scenario: Password for OIDC user
- **WHEN** an admin sets a password on an `oidc` user
- **THEN** the response is `400`

### Requirement: Admins cannot lock themselves out
A user SHALL not be able to delete, disable or demote their own account through the API.

#### Scenario: Self-demotion
- **WHEN** admin `alice` changes her own role to `read_only`
- **THEN** the response is `400`

### Requirement: User info
`GET /rest/v1/userinfo` SHALL return the caller's `username`, `role` and `tenant`, for any authenticated caller.

#### Scenario: Console role check
- **WHEN** a `read_only` user's console calls `/rest/v1/userinfo`
- **THEN** it gets `{"username":"bob","role":"read_only","tenant":"DEFAULT"}`
