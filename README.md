# MiceScale

MiceScale is a standalone carrier component for the LinuxMice private transport
layer: it operationalizes plain WireGuard with no coordination server and, as
an alternative, a self-hosted Headscale control server. Either way a fleet runs
a Tailscale-like tailnet without depending on Tailscale's cloud coordination.
Tailscale becomes an optional carrier, never the trust root.

It is a deployment and operations layer, not a reimplementation: the standard
open-source `tailscale` client can connect to your own Headscale server, or
MiceScale drives plain `wg`/`wg-quick` directly with zero Tailscale code.
MiceScale owns configuration, enrollment, health checks, audit events, and the
LinuxMice component contract around them.

MiceScale follows the LinuxMice transport doctrine
(linuxmice decision 0012): network reachability is never authorization.
LinuxMice service identity, mTLS, and encrypted envelopes remain the
authorization and confidentiality layer above whatever carrier moves packets.

Project owner: Rei, Founder & CEO of Katteke.

## Status

Early dogfood. The CLI, carrier profile model, audit events, and Headscale
deployment profile work locally. Nothing here is a production VPN, a
certification, or a managed service yet.

## Why

LinuxMice must not require Tailscale, Headscale, or any single carrier to
operate. Customers should be able to choose a customer-owned coordination
server without changing LinuxMice service authorization. MiceScale is the
operational layer that makes the self-hosted option a first-class citizen.

## Quick start

### WireGuard carrier (no coordination server, no Tailscale code)

Hub host (one VPS):

```bash
micescale wg hub-init --address 10.60.0.1/24 --endpoint hub.example.com:51820
sudo wg-quick up ~/.local/state/micescale/wireguard/hub.conf
```

Each device:

```bash
micescale wg client-init \
  --address 10.60.0.2/24 \
  --endpoint hub.example.com:51820 \
  --hub-pubkey <hub-public-key> \
  --allowed-ips 10.60.0.0/24
sudo wg-quick up ~/.local/state/micescale/wireguard/client.conf
micescale wg status
```

The hub operator registers each client with `micescale wg hub-add-peer`.
Peer keys are exchanged out-of-band; see [docs/wireguard-carrier.md](docs/wireguard-carrier.md).

### Headscale carrier (coordinated tailnet)

Server (one host):

```bash
sudo ./bin/headscale deploy --profile deploy/headscale/   # documented in docs/operations.md
```

Client (each device):

```bash
export MICESCALE_AUTHKEY=$(headscale preauthkeys create --user youruser)
micescale enroll --server https://headscale.example.com --authkey "$MICESCALE_AUTHKEY"
micescale status
micescale doctor
```

Leave a fleet:

```bash
micescale leave
```

Inspect policy and local audit events:

```bash
micescale carrier policy
micescale carrier profiles
micescale audit tail --format json
```

## Commands

```text
micescale version
micescale carrier policy [--format json|yaml]
micescale carrier profiles [--format json|yaml]
micescale config show [--format json|yaml]
micescale enroll --server <url> --authkey <key> [--node-name <name>] [--carrier headscale|wireguard]
micescale wg hub-init --address <addr/prefix> --endpoint <host:port> [--listen-port N]
micescale wg hub-add-peer --name <name> --pubkey <key> --address <addr>
micescale wg hub-remove-peer --name <name>
micescale wg hub-render
micescale wg client-init --address <addr/prefix> --endpoint <host:port> --hub-pubkey <key> --allowed-ips <prefix>
micescale wg up|down
micescale wg status [--format json|yaml]
micescale status [--format json|yaml]
micescale doctor [--peer <name>] [--format json|yaml]
micescale leave
micescale audit tail [--limit N] [--format json|yaml]
```

All output supports structured JSON/YAML. Secrets are never stored in config,
audit logs, or tracked files; auth keys exist only in the process environment
and the underlying `tailscale` call, and WireGuard private keys only in 0600
`*.key`/`*.conf` files under the WireGuard state directory.

## Configuration

MiceScale reads `MICESCALE_CONFIG` (default `~/.config/micescale/config.toml`)
for non-secret settings: control server URL, carrier, node name, and audit log
path. The audit log defaults to `~/.local/state/micescale/audit.jsonl`.

Environment overrides:

| Variable | Meaning |
| --- | --- |
| `MICESCALE_CONFIG` | Config file path |
| `MICESCALE_AUDIT_LOG` | Audit log path |
| `MICESCALE_AUTHKEY` | Pre-auth key for `enroll` (never persisted) |
| `MICESCALE_TAILSCALE_BIN` | `tailscale` binary path (default: `tailscale`) |
| `MICESCALE_WG_BIN` | `wg` binary path (default: `wg`) |
| `MICESCALE_WGQUICK_BIN` | `wg-quick` binary path (default: `wg-quick`) |
| `MICESCALE_WG_DIR` | WireGuard state directory (default: `~/.local/state/micescale/wireguard`) |

## How it fits

```text
browser / service
  -> LinuxMice mTLS / encrypted envelope   (identity, policy, audit)
  -> MiceScale tailnet                     (reachability only)
  -> WireGuard carrier                     (no coordinator, or Headscale)
```

Reachability is a carrier concern. Authorization stays LinuxMice-owned.

## Repository layout

- `crates/` — Rust workspace: `micescale` CLI and `micescale-core` library.
- `deploy/headscale/` — Headscale server profile: config template, systemd
  unit, DERP example, firewall and backup/rotation procedures.
- `manifests/` — LinuxMice component contract (decision 0017).
- `docs/` — architecture, operations, and migration documentation, including
  the coordination-free [WireGuard carrier](docs/wireguard-carrier.md).
- `scripts/` — deterministic local smoke checks.

## Development

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/smoke-unit.sh
```

## Related projects

- [LinuxMice](https://github.com/mmdmcy/linuxmice) — the private transport
  layer contract this component implements.
- [Homefleet](https://github.com/mmdmcy/homefleet) — LAN fleet control that
  needs no VPN at all.

## License

Apache License 2.0. See [LICENSE](LICENSE).
