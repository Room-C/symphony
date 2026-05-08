# Observability

Logging uses `tracing`.

By default logs are JSON on stdout. If `observability.log_dir` is configured, daily JSONL files are written there.

HTTP status surfaces:

- `/` human-readable dashboard
- `/health`
- `/status`
- `/api/v1/state`
- `/api/v1/<issue_identifier>`
- `/api/v1/refresh`

The HTTP API is an observability/control surface only. It is not required for orchestrator correctness.
