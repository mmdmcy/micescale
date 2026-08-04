# Operations

## WireGuard carrier (coordination-free)

There is no control server to operate; the full hub-and-spoke workflow lives
in [wireguard-carrier.md](wireguard-carrier.md). The essentials:

```sh
# hub host
micescale wg hub-init --address 10.60.0.1/24 --endpoint hub.example.com:51820
sudo wg-quick up ~/.local/state/micescale/wireguard/hub.conf
sudo firewall-cmd --add-port=51820/udp --permanent   # or equivalent

# hub operator, per client (key/address come from `client-init` on the device)
micescale wg hub-add-peer --name laptop --pubkey <client-public-key> --address 10.60.0.2/32
sudo wg-quick syncconf ~/.local/state/micescale/wireguard/hub.conf \
  <(wg-quick strip ~/.local/state/micescale/wireguard/hub.conf)

# device
micescale wg client-init --address 10.60.0.2/24 --endpoint hub.example.com:51820 \
  --hub-pubkey <hub-public-key> --allowed-ips 10.60.0.0/24
sudo wg-quick up ~/.local/state/micescale/wireguard/client.conf
```

## Headscale server bootstrap

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
  health probes. On the wireguard carrier it has no control server to probe.
- `micescale wg status` — wireguard interface state and peer handshakes.
- `micescale audit tail` — recent enroll/leave/wg events.
- `headscale nodes list|delete|expire` — node lifecycle (headscale carrier).
- `headscale preauthkeys list|revoke` — key lifecycle (headscale carrier).
- `micescale leave` — remove a device from the tailnet (either carrier).

## Troubleshooting

| Symptom | Check |
| --- | --- |
| `tailscale up` fails on a device | `micescale doctor`; confirm `MICESCALE_AUTHKEY` set; confirm control URL reachable |
| Control server down | `systemctl status micescale-headscale`; `curl https://<control.example.com>/health` |
| WireGuard peer shows offline | Firewall: hub 51820/UDP (coordination-free) or 41641/UDP (headscale); DERP fallback: 8765/TCP + 3478/UDP |
| Hub has no routes to clients | Clients must `wg-quick up` with `AllowedIPs` covering the fleet prefix; `micescale wg status` shows handshakes |
| Enroll works, no traffic | WireGuard kernel module present? `micescale doctor` reports it |
| Policy applied but no access | Verify the policy file and recheck `headscale policy get` |

## Backups

`deploy/headscale/backup-and-rotation.md` covers database, pre-auth key, node
key, and TLS rotation, including the break-glass re-enroll path.
