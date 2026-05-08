# Retry and Recovery

Symphony keeps runtime scheduler state in memory. Restart recovery is driven by the tracker and workspace filesystem, matching the upstream SPEC non-database model.

Failure retry:

```text
delay = min(10000 * 2^(attempt - 1), max_retry_backoff_ms)
jitter = 0..delay/4
effective_delay = delay - jitter
```

Normal worker exit:

- record issue as completed for bookkeeping
- keep claim
- schedule continuation retry after 1 second

Startup cleanup:

- fetch terminal-state issues
- run `before_remove` best effort
- remove matching workspaces best effort
- log warnings but do not fail startup on cleanup errors
