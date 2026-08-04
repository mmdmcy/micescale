# WireGuard carrier (coordination-free)

The `wireguard` carrier runs plain WireGuard with no coordination server and
no Tailscale code anywhere on the path. It is a hub-and-spoke layout: one hub
(usually a VPS with a public endpoint) and any number of clients that reach
each other through the hub. Peer keys are exchanged out-of-band via the hub
operator; there is no third-party coordinator, no pre-auth key, and no account.

Use this carrier when you want the smallest possible dependency surface and
the fleet fits a hub-and-spoke shape.

## Layout

```text
hub (lmice0)
  <- client A: WireGuard tunnel through the hub
  <- client B
  peer config exchanged out-of-band (micescale wg hub-add-peer / client-init)
```

Only the hub needs a public endpoint. Clients use `AllowedIPs` to send traffic
to each other through the hub (routing is the hub's job, not MiceScale's).

## Hub setup (one-time, on the hub host)

```bash
micescale wg hub-init \
  --address 10.60.0.1/24 \
  --endpoint hub.example.com:51820
```

This creates the hub identity under the state directory and renders
`hub.conf` ready for `wg-quick` (or your distro's WireGuard unit). The private
key lives only in `hub.key` / `hub.conf` (0600); it is never written to JSON
state or the audit log.

Bring the interface up:

```bash
sudo wg-quick up ~/.local/state/micescale/wireguard/hub.conf
```

Open the UDP listen port in the firewall (default 51820/UDP).

## Registering a client (hub operator)

The client runs `micescale wg client-init` and gives you:

- the client's WireGuard public key,
- the client's tunnel address.

Register it on the hub:

```bash
micescale wg hub-add-peer \
  --name laptop \
  --pubkey <client-public-key> \
  --address 10.60.0.2/32
```

`hub-add-peer` re-renders `hub.conf`; apply it on the hub with:

```bash
sudo wg-quick syncconf ~/.local/state/micescale/wireguard/hub.conf \
  <(wg-quick strip ~/.local/state/micescale/wireguard/hub.conf)
```

(`syncconf` applies changes without dropping the interface; use `wg-quick up`
on first run.)

## Client setup (each device)

```bash
micescale wg client-init \
  --address 10.60.0.2/24 \
  --endpoint hub.example.com:51820 \
  --hub-pubkey <hub-public-key> \
  --allowed-ips 10.60.0.0/24
```

This writes `client.conf` and prints the exact `hub-add-peer` command to hand
to the hub operator. Bring the tunnel up:

```bash
sudo wg-quick up ~/.local/state/micescale/wireguard/client.conf
# or, through the CLI (wraps wg-quick):
micescale wg up
micescale wg down
```

## Day-to-day

- `micescale wg status` — interface state, latest handshakes, transfer
  counters, and online/offline peers (text, `--format json|yaml`).
- `micescale wg hub-render` — print the hub config without touching state.
- `micescale wg hub-remove-peer --name <name>` — remove a peer and re-render.
- `micescale leave` — bring the tunnel down and delete the local client state.
- `micescale doctor` — reports the `wireguard-kernel` check and the client
  state; there is no control server to probe on this carrier.

## State layout

Under `MICESCALE_WG_DIR` (default `~/.local/state/micescale/wireguard`):

| File | Content | Permissions |
| --- | --- | --- |
| `hub.json` / `client.json` | public registry: keys, addresses, peers (no secrets) | 0644 |
| `hub.key` | hub private key | 0600 |
| `hub.conf` / `client.conf` | full `wg-quick` config incl. private key | 0600 |

Audit events (`wg-hub-init`, `wg-hub-add-peer`, `wg-enroll`, `wg-up`,
`wg-down`, `wg-leave`) never contain keys.

## Compared with the headscale carrier

| | headscale | wireguard |
| --- | --- | --- |
| Coordination | headscale server (operator-owned) | none |
| Peer exchange | automatic via control server | out-of-band |
| Pre-auth keys | yes (short-lived) | not applicable |
| NAT traversal / DERP | yes | no (hub needs public endpoint) |
| Tailscale code | client binary only | none |
