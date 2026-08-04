# Security

## Reporting a vulnerability

Do not open a public issue for a security problem. Report privately to the
project owner (Rei, Founder & CEO of Katteke) through the LinuxMice private
channel, and include:

- affected version or commit,
- a minimal reproduction,
- what you expected and what happened,
- impact assessment if you have one.

## Threat model summary

MiceScale is a deployment and operations layer around Headscale and WireGuard.
It does not invent cryptographic primitives or protocols; it configures
maintained open-source carriers and audits the operations around them.

- The tailnet provides reachability only. It must never be treated as
  authorization for LinuxMice services; LinuxMice identity and policy remain
  the authorization boundary.
- A shared pre-auth key is a fleet onboarding credential, roughly a fleet
  root token. Treat it as a secret, rotate it, and prefer short-lived keys.
  MiceScale never writes auth keys to config, audit logs, or any tracked file.
- The Headscale control server holds the node registry and pre-auth keys.
  Back it up, protect it, and restrict administrative access.
- The carrier can still observe timing, source/destination reachability,
  packet sizes, and availability. Payload confidentiality above the carrier is
  the LinuxMice encrypted envelope layer, not MiceScale.
- Local audit events record what MiceScale commanded, not secrets: no keys,
  no credentials, no private hostnames beyond the configured control server
  URL.

## Data rules

- Never commit private IPs, private hostnames, tokens, keys, credentials, or
  machine-specific state.
- `~/.config/micescale` and `~/.local/state/micescale` are local runtime state
  and are not part of this repository.
- `MICESCALE_AUTHKEY` exists only in the environment of the enrolling process.
