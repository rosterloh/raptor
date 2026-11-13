# telemetry Specification

## Purpose

Logs, traces and metrics: stdout logging always, and optional OpenTelemetry
export in builds with the `otel` feature.

## Requirements

### Requirement: Stdout logging
raptor SHALL log to stdout with verbosity controlled by `RUST_LOG`. Management API writes and authentication failures SHALL be logged.

#### Scenario: Failed device auth
- **WHEN** a DDI request fails authentication
- **THEN** a warning with the request path is logged

### Requirement: Optional OTLP export
In an `otel` build, an `[otel]` section SHALL turn on OTLP export of traces, metrics and logs over gRPC or HTTP, with `service_name`, extra `headers` and an optional `sample_ratio` (0.0–1.0, out of range stops startup). Other builds SHALL parse and ignore the section.

#### Scenario: Non-otel build
- **WHEN** a build without `otel` reads a config with `[otel]`
- **THEN** it starts normally without exporting

### Requirement: Trace propagation
With export on, a request carrying W3C `traceparent` SHALL join the caller's trace, and the caller's sampling decision SHALL win over `sample_ratio`.

#### Scenario: Sampled caller
- **WHEN** a request arrives with a sampled `traceparent`
- **THEN** raptor's spans are part of that trace

### Requirement: Metrics
An `otel` build SHALL record HTTP request counts and durations, actions created, finished, failed and canceled, artifact bytes uploaded and downloaded, auth failures by zone, fleet status gauges, and sweep skips by kind.

#### Scenario: Auth failure metric
- **WHEN** a Management API request fails authentication
- **THEN** `raptor.auth.failures` increments with zone `mgmt`
