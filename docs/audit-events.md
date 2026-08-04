# Audit events

MiceScale appends one JSON object per line to a local JSONL audit log.

Default path: `~/.local/state/micescale/audit.jsonl` (or `MICESCALE_AUDIT_LOG`).

## Events

| Event | Trigger | Status |
| --- | --- | --- |
| `enroll` | `micescale enroll` succeeds | `ok` |
| `leave` | `micescale leave` succeeds | `ok` |

## Schema

```json
{
  "ts": 1780000000,
  "event": "enroll",
  "carrier": "headscale",
  "control_server": "https://control.example.com",
  "status": "ok",
  "node_name": "workstation-1",
  "detail": "joined via pre-auth key; key never persisted"
}
```

## Guarantees

- No event contains keys, tokens, passwords, or credentials. The pre-auth key
  is only ever passed through the process environment to the `tailscale`
  client.
- `control_server` is the operator-configured URL; it is fleet policy, not a
  secret, but treat the log as sensitive fleet state.
- Events are append-only. Rotation is the operator's job (for example
  `logrotate` with `copytruncate`).

## Inspection

```sh
micescale audit tail --limit 20 --format json
```
