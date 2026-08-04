# Operations

## Server bootstrap

Follow `deploy/headscale/README.md` to install the control server. Decide up
front:

- TLS termination: reverse proxy in front of `127.0.0.1:8080` (recommended)
  or headscale's embedded ACME.
- DERP: embedded relay for NAT fallback, or none for a single-LAN fleet.
- Policy storage: database mode initially, then a tracked policy file.

## Policy

Start with headscale's database ACL mode and move to a policy file once the
fleet shape is known:

```sh
headscale policy set --file /etc/headscale/policy.hujson
```

A minimal `policy.hujson` that allows the `mice` user group to reach each
other's tailnet addresses:

```hujson
{
    "groups": {
        "group:mice": ["user1", "user2"],
    },
    "acls": [
        { "action": "accept", "src": ["group:mice"], "dst": ["group:mice:*"] },
    ],
}
```

Keep the policy file in the operator's version control; it is fleet policy,
not machine state.

## Fleet enrollment

```sh
headscale users create <user>
headscale preauthkeys create --user <user> --reusable --expiration 24h
```

On each device:

```sh
export MICESCALE_AUTHKEY="<short-lived-key>"
micescale enroll --server https://<control.example.com> --node-name <hostname>
micescale status
micescale doctor
```

Devices switching from Tailscale cloud are reauthenticated automatically
(`enroll` passes `--force-reauth`).

## Day-to-day

- `micescale status` — who am I, which peers are online, backend health.
- `micescale doctor` — full carrier health; use it in cron or component
  health probes.
- `micescale audit tail` — recent enroll/leave events.
- `headscale nodes list|delete|expire` — node lifecycle.
- `headscale preauthkeys list|revoke` — key lifecycle.
- `micescale leave` — remove a device from the tailnet.

## Troubleshooting

| Symptom | Check |
| --- | --- |
| `tailscale up` fails on a device | `micescale doctor`; confirm `MICESCALE_AUTHKEY` set; confirm control URL reachable |
| Control server down | `systemctl status micescale-headscale`; `curl https://<control.example.com>/health` |
| Peer shows offline | Firewall: 41641/UDP; DERP fallback: 8765/TCP + 3478/UDP |
| Enroll works, no traffic | WireGuard kernel module present? `micescale doctor` reports it |
| Policy applied but no access | Verify the policy file and recheck `headscale policy get` |

## Backups

`deploy/headscale/backup-and-rotation.md` covers database, pre-auth key, node
key, and TLS rotation, including the break-glass re-enroll path.
