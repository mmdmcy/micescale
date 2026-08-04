# deploy/headscale

MiceScale server-side profile for a self-hosted Headscale coordination
server.

| File | Purpose |
| --- | --- |
| `headscale.example.yaml` | Headscale server config template; copy to `/etc/headscale/config.yaml` and replace placeholders |
| `micescale-headscale.service` | systemd unit for `headscale serve` |
| `derp.example.yaml` | Optional standalone DERP relay map |
| `firewall.md` | Minimal port exposure |
| `backup-and-rotation.md` | Database, key, and certificate backup and rotation procedures |

## Install (Debian/Ubuntu)

1. Install the Headscale package from its official repository and create the
   service user:

   ```sh
   adduser --system --group --home /var/lib/headscale headscale
   ```

2. Install the config template, replace `<control.example.com>`, and move the
   embedded DERP and Noise key paths into place:

   ```sh
   sudo install -m 0640 deploy/headscale/headscale.example.yaml /etc/headscale/config.yaml
   sudo install -m 0644 deploy/headscale/micescale-headscale.service /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable --now micescale-headscale
   ```

3. Put a reverse proxy (Caddy/nginx) with TLS in front of `127.0.0.1:8080`
   (see `firewall.md`), then verify: `curl https://<control.example.com>/health`.

4. Create a user and a pre-auth key:

   ```sh
   headscale users create youruser
   headscale preauthkeys create --user youruser --reusable --expiration 24h
   ```

5. Enroll clients with `micescale enroll` (see `docs/operations.md`).

The embedded DERP server (UDP 3478, region 999) is enabled in the template.
On a single-LAN fleet you can disable it in the config.

This profile is dogfood guidance, not a managed service.
