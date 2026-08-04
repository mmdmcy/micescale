# Backup and rotation

## What to back up

The Headscale state directory is the source of truth for the fleet:

```text
/var/lib/headscale/
  db.sqlite                  node registry, pre-auth keys, policy, users
  noise_private.key          control server WireGuard identity
  derp_server_private.key    embedded DERP identity (if enabled)
  cache/
```

plus `/etc/headscale/config.yaml` and any tracked policy file.

Back up the SQLite database with the SQLite online backup, not a plain file
copy while the server is running:

```sh
sqlite3 /var/lib/headscale/db.sqlite ".backup /var/lib/headscale/backup-$(date +%F).sqlite"
```

Then move the backup off the host (encrypted object storage or an operator
machine) and retain it according to the fleet retention policy. Test restore
regularly on a disposable host.

## Pre-auth key lifecycle

- Pre-auth keys are onboarding credentials. Create them with a short expiry
  and the narrowest scope, and revoke them when enrollment is done:
  `headscale preauthkeys list` / `headscale preauthkeys revoke`.
- Use single-use keys (`--user <name>`) for interactive onboarding. For
  scripted fleet enrollment, generate a short-lived reusable key, use it, then
  revoke it.
- Keys never belong in tracked files, config, or audit logs. MiceScale reads
  them from `MICESCALE_AUTHKEY` only.

## Node and control identity

- Node keys are long-lived by default in Headscale. Set a sane key expiry and
  document rotation: `headscale nodes list`, `headscale nodes expire`.
- The `noise_private.key` identifies the control server itself. Back it up
  with the database, and treat loss as control-plane break-glass: clients must
  re-enroll.
- Rotating a client node key is just `tailscale logout` followed by
  `micescale enroll` with a fresh pre-auth key. This is the break-glass path:
  it requires only the pre-auth key, not the old node state.

## TLS certificates

- The control endpoint uses ACME (embedded) or reverse-proxy certificates.
  Rotation is automatic for Let's Encrypt; for customer CAs, follow the
  issuing CA's rotation procedure and verify with `micescale doctor` after.
- Check expiry with `openssl s_client -connect <control.example.com>:443`

## Recovery drill

1. Restore `db.sqlite` + `noise_private.key` on a fresh host from backup.
2. Restart `micescale-headscale.service`.
3. Enroll one fresh client and confirm it reaches the rest of the fleet.
4. Record the drill result in fleet operations notes.

No recovery procedure replaces regular, tested backups. Document the drill
owner in the fleet support plan.
