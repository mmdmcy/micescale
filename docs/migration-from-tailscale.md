# Migrating from Tailscale cloud

Goal: keep the tailnet, drop the dependency on Tailscale's third-party
coordination. LinuxMice services are unaffected by the switch: their
authorization never depended on the carrier.

## Before you start

- Confirm `micescale doctor` passes on a client and that the control server
  (`deploy/headscale/README.md`) is up with a valid TLS endpoint.
- Create a fleet user and a short-lived, reusable pre-auth key.
- Verify the current Tailscale node state is not the only recovery path for
  the machines being migrated (see the fleet's off-device backup plan).

## Switch one device

```sh
export MICESCALE_AUTHKEY="<short-lived-key>"
micescale enroll --server https://<control.example.com> --node-name <hostname>
micescale status      # node appears under the self-hosted control server
micescale doctor
```

The enroll flow uses `--force-reauth` automatically, which is required when a
device is already logged in to another control server (for example Tailscale
cloud).

## Verify the migration

- `micescale status` shows the expected peers with `online_peers` matching the
  migrated fleet.
- `micescale doctor --peer <peer>` reports the peer reachable.
- Reach a service that previously went through the tailnet (for example a
  loopback service exposed through `lmd gateway` over the carrier).
- `micescale audit tail` shows the enroll events with timestamps.

## Decommission the old control plane

Only after every device is migrated and verified:

1. `tailscale logout` (or `micescale leave`) on each migrated device to revoke
   its old node identity.
2. Delete the Tailscale cloud nodes from the old tailnet admin console.
3. Remove the old `tailscale` login state from devices.
4. Drop any Tailscale-specific firewall rules on the migrated hosts.

Keep the old tailnet active for one device as a fallback during a
short window, or decommission fully per fleet policy.

## Rollback

A migrated device returns to Tailscale cloud with:

```sh
tailscale up --force-reauth
```

`micescale` config and audit logs remain valid; enrollment state is separate
from the carrier choice.
