# Architecture

## Role in the LinuxMice stack

MiceScale is the carrier-operations component of the LinuxMice private
transport layer. LinuxMice ships a carrier-untrusted posture: reachability is
never authorization, and LinuxMice identity (service certificates, mTLS),
policy, and encrypted envelopes sit above any packet carrier. MiceScale makes
the self-hosted carrier a first-class option instead of a manual, weakly
documented alternative to Tailscale cloud.

## What MiceScale owns

- Declared carrier posture and profiles (`micescale carrier policy|profiles`).
- Node configuration and enrollment flow around the standard `tailscale`
  client pointed at a customer-owned Headscale server.
- Health checks (`micescale doctor`): client binary, tailscaled backend,
  control-server reachability, WireGuard kernel presence, optional peer check.
- Local audit events for mutating operations (enroll, leave), appended to a
  JSONL log that is never allowed to contain keys or credentials.
- A deployable Headscale server profile: config template, systemd unit, DERP
  example, firewall guidance, backup and rotation procedures.
- The LinuxMice component manifest (decision 0017) that lets LinuxMice
  catalogs discover MiceScale as a provider of `carrier.headscale` and
  `carrier.wireguard`.

## What MiceScale deliberately does not own

- It does not reimplement WireGuard, Headscale, the Tailscale client, DERP,
  or NAT traversal. Those are maintained open-source components with their own
  security processes.
- It does not invent cryptographic primitives or protocols.
- It does not replace LinuxMice service identity. Enrolling a node into the
  tailnet does not grant LinuxMice authorization; LinuxMice certificates and
  policy remain required for LinuxMice services.
- It does not hide what the carrier can observe: timing, endpoint
  reachability, packet sizes, and availability signals.

## Runtime shape

Headscale carrier:

```text
device
  tailscale client (WireGuard)
    -> Headscale control server (operator-owned)
       -> WireGuard mesh between peers
  micescale enroll/status/doctor/leave  (config + audit + health)
  micescale-headscale.service           (control server, server side)
```

The control server holds the node registry and pre-auth keys. The clients hold
WireGuard identities issued by that control server. Nothing on this path
depends on a third-party coordinator.

WireGuard carrier (coordination-free, no Tailscale code):

```text
hub (micescale wg hub-init, hub.conf + hub.key)
  <- client A: micescale wg client-init, client.conf
  <- client B
  micescale wg hub-add-peer / status / up / down / leave
```

There is no registry, no coordinator, and no pre-auth key. The hub holds the
peer list in `hub.json` and re-renders `hub.conf`; clients hold only their own
config. Peer keys travel out-of-band. See
[docs/wireguard-carrier.md](wireguard-carrier.md) for the full workflow.

## Trust model

- Pre-auth keys are fleet onboarding credentials (headscale carrier only);
  short-lived, revocable, never persisted by MiceScale.
- WireGuard private keys (either carrier) live only in 0600 `*.key`/`*.conf`
  files; they are never written to JSON state or audit events.
- The `noise_private.key` and the database are control-plane state; their
  backup is the operator's responsibility (see `deploy/headscale/`).
- The tailnet is a reachability fabric. LinuxMice mTLS, policy, and encrypted
  envelopes remain the authorization and confidentiality boundaries, matching
  the LinuxMice transport doctrine (decision 0012).
