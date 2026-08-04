# MiceScale

MiceScale is a standalone carrier component for the LinuxMice private transport
layer: it operationalizes a self-hosted Headscale control server and plain
WireGuard so a fleet can run a Tailscale-like tailnet without depending on
Tailscale's cloud coordination. Tailscale becomes an optional carrier, never
the trust root.

It is a deployment and operations layer, not a reimplementation: the standard
open-source `tailscale` client connects to your own Headscale server, WireGuard
is the carrier protocol, and MiceScale owns configuration, enrollment, health
checks, audit events, and the LinuxMice component contract around them.

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
micescale status [--format json|yaml]
micescale doctor [--peer <name>] [--format json|yaml]
micescale leave
micescale audit tail [--limit N] [--format json|yaml]
```

All output supports structured JSON/YAML. Secrets are never stored in config,
audit logs, or tracked files; auth keys exist only in the process environment
and the underlying `tailscale` call.

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

## How it fits

```text
browser / service
  -> LinuxMice mTLS / encrypted envelope   (identity, policy, audit)
  -> MiceScale tailnet                     (reachability only)
  -> Headscale coordination + WireGuard     (carrier)
```

Reachability is a carrier concern. Authorization stays LinuxMice-owned.

## Repository layout

- `crates/` — Rust workspace: `micescale` CLI and `micescale-core` library.
- `deploy/headscale/` — Headscale server profile: config template, systemd
  unit, DERP example, firewall and backup/rotation procedures.
- `manifests/` — LinuxMice component contract (decision 0017).
- `docs/` — architecture, operations, and migration documentation.
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
