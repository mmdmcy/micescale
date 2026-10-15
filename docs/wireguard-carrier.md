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
to each other through the hub, and the rendered hub config forwards it (see
[Spoke-to-spoke forwarding](#spoke-to-spoke-forwarding)).

## Hub setup (one-time, on the hub host)

```bash
micescale wg hub-init \
  --address 10.60.0.1/24 \
  --endpoint hub.example.com:51820
```

This creates the hub identity under the state directory and renders
`lmice0.conf` ready for `wg-quick` (or your distro's WireGuard unit). The
config is always named `<interface>.conf` (`--interface`, default `lmice0`)
because `wg-quick` names the interface after the file stem; `status` and
`doctor` look for that same name. The private key lives only in `hub.key` /
`lmice0.conf` (0600, written atomically); it is never written to JSON state or
the audit log.

`hub-init` refuses to run when hub state already exists, since a new key
invalidates every client and drops the peer list. `--force` starts over, after
copying `hub.json`, `hub.key` and the old config to `<file>.bak-<unix-time>`
(0600); nothing is deleted.

Bring the interface up:

```bash
sudo micescale wg up
# equivalent to: sudo wg-quick up ~/.local/state/micescale/wireguard/lmice0.conf
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
  --address 10.60.0.2
```

The address is a plain IP that must parse, lie inside the hub subnet, and not
be the hub's own, the network or the broadcast address, or another peer's. The
key must be a canonical 44-character base64 WireGuard key and not the hub's own.

`hub-add-peer` (and `hub-remove-peer`, `hub-render`) re-renders the config and
prints the command that applies it live, with the real path filled in:

```bash
sudo bash -c 'wg syncconf lmice0 <(wg-quick strip /home/<you>/.local/state/micescale/wireguard/lmice0.conf)'
```

`wg syncconf` applies peer changes without dropping the interface. `wg-quick
strip` is given the full path because a bare interface name only resolves
under `/etc/wireguard`, and both tools need root, hence one `sudo bash -c`.
If the interface is down, `sudo micescale wg up` instead.

## Spoke-to-spoke forwarding

Clients reach each other through the hub, so the hub config carries:

```ini
PostUp = sysctl -w net.ipv4.ip_forward=1; iptables -I FORWARD -i %i -o %i -j ACCEPT
PostDown = iptables -D FORWARD -i %i -o %i -j ACCEPT
```

(`net.ipv6.conf.all.forwarding` / `ip6tables` for an IPv6 hub address.) This
forwards only between peers on the WireGuard interface, not to the hub's LAN or
the internet. `PostDown` removes the rule but leaves `ip_forward` on, since
other services may rely on it. Hosts whose firewall (nftables-only, ufw, a
custom chain) does not honour `iptables -I FORWARD` need an equivalent rule.

## Client setup (each device)

```bash
micescale wg client-init \
  --address 10.60.0.2/24 \
  --endpoint hub.example.com:51820 \
  --hub-pubkey <hub-public-key> \
  --allowed-ips 10.60.0.0/24
```

`--allowed-ips` takes one or more comma-separated prefixes. This writes
`lmice0.conf` (0600) and prints the exact `hub-add-peer` command to hand to the
hub operator. Bring the tunnel up:

```bash
sudo micescale wg up      # wraps wg-quick up .../lmice0.conf
sudo micescale wg down
```

Under `sudo`, MiceScale resolves `~` to `SUDO_USER`'s home (from
`/etc/passwd`), so it finds the state written without sudo. To keep state
elsewhere, pass the directory explicitly (sudo strips the environment):
`sudo MICESCALE_WG_DIR=/srv/micescale/wg micescale wg up`.

## Day-to-day

- `micescale wg status` — interface state, latest handshakes, transfer
  counters, and online/offline peers (text, `--format json|yaml`).
- `micescale wg hub-render` — re-render the hub config from `hub.json`.
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
| `<interface>.conf` (default `lmice0.conf`) | full `wg-quick` config incl. private key | 0600 |
| `*.bak-<unix-time>` | copies kept by `hub-init --force` | 0600 |

State from older versions used `hub.conf` / `client.conf`: run
`micescale wg hub-render` on the hub, and on a client rename `client.conf` to
`lmice0.conf` (or `<interface>.conf`).

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
