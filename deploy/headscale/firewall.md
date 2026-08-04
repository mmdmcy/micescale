# MiceScale Headscale firewall

Minimal ports for a self-hosted Headscale control server on Debian/Ubuntu.
Expose only what the fleet actually needs. The defaults below assume the
control HTTPS endpoint is terminated by a reverse proxy on the same host.

## Control plane

| Port | Protocol | Purpose |
| --- | --- | --- |
| 443 | TCP | HTTPS control plane (reverse proxy to `127.0.0.1:8080`) |
| 8080 | TCP | Headscale HTTP API and control; keep bound to the proxy only |
| 41641 | UDP | WireGuard/Noise traffic between the server and clients |

Example with `nftables`:

```sh
nft add rule inet filter input tcp dport 443 accept
nft add rule inet filter input udp dport 41641 accept
```

Only open 41641/UDP if the control host also peers directly with clients. If
clients connect through a DERP relay instead, the relay host needs 8765/TCP
(and 3478/UDP for STUN) instead.

## Optional DERP / STUN

| Port | Protocol | Purpose |
| --- | --- | --- |
| 8765 | TCP | DERP relay fallback |
| 3478 | UDP | STUN for NAT traversal (embedded DERP server) |

## What not to open

- `127.0.0.1:9090` (metrics) and `127.0.0.1:50443` (gRPC API) stay loopback.
- Do not expose the Headscale database, config, or admin CLI over the network.

## Verification

```sh
micescale doctor
```

from a client reports control-server reachability and WireGuard state.
