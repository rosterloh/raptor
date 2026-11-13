# Spec Delta

## ADDED Requirements

### Requirement: Single sign-on button
When OIDC is configured, the login page SHALL offer a sign-in button that starts `/rest/v1/oidc/login`, alongside the username and password form.

#### Scenario: OIDC configured
- **WHEN** an operator opens `/ui` on a server with `[mgmt.oidc]`
- **THEN** the login page shows the single sign-on button

### Requirement: Role-aware controls
The console SHALL read the caller's role from `/rest/v1/userinfo` and hide or disable write controls for `read_only` users. A `403` from the API SHALL be shown as a permissions message, not a generic failure.

#### Scenario: Read-only operator
- **WHEN** a `read_only` user opens a rollout
- **THEN** no Start, Pause, Stop, Approve or Delete buttons are offered

### Requirement: Users page
Admins SHALL have a Users page to list, create, change role of, enable, disable and delete users, and reset local users' passwords. It SHALL not be shown to `read_only` users.

#### Scenario: Add a viewer
- **WHEN** an admin creates a `read_only` user on the Users page
- **THEN** the new user appears in the list and can log in
