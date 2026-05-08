# Adapting the Upstream SPEC

This implementation follows OpenAI Symphony Draft v1 and documents the implementation-defined choices required by the spec.

## GitHub Tracker

Upstream Draft v1 specifies Linear. This implementation adds:

```yaml
tracker:
  kind: github
  mode: labels # or projects_v2
```

The normalized `Issue` shape remains aligned with SPEC Section 4 and Section 11.

## Labels Mode

GitHub issue state is represented by one `symphony:<state>` label. Priority is parsed from `priority:1` through `priority:4`. Non-state and non-priority labels are passed through lowercased.

## Projects v2 Mode

Projects v2 reads organization project items and maps the configured single-select field, default `Status`, to Symphony issue state.

v0.1 limitation: Projects v2 write operations return `unsupported_tracker_write` until field-option mutation lookup is implemented and tested.

## Codex Trust Policy

Default policy:

- `approval_policy: never`
- `thread_sandbox: workspace-write`
- user-input-required is treated as run failure
- unsupported dynamic tools return failure instead of blocking the run

This is a high-trust local daemon posture. Operators should run it only where automatic workspace-scoped changes are acceptable.

## Retry Formula

The implementation follows SPEC Section 8.4: failure retry starts from 10 seconds and doubles by attempt, capped by `agent.max_retry_backoff_ms`, with jitter subtracting up to 25 percent. The local implementation plan had a 1 second example; SPEC wins.

Normal worker exit still schedules a continuation retry after roughly 1 second.

## HTTP Surface

The daemon exposes convenience aliases:

- `/health`
- `/status`

It also implements the SPEC HTTP extension baseline:

- `GET /api/v1/state`
- `GET /api/v1/<issue_identifier>`
- `POST /api/v1/refresh`

Issue identifiers containing `/` must be URL-encoded when used in the path.

## Out of Scope

Appendix A SSH worker scheduling is not implemented in v0.1.
