# Contributing

## Scope

MiceScale stays a carrier operations layer. Contributions that reimplement
WireGuard, Headscale, the Tailscale client, NAT traversal, or cryptographic
protocols are out of scope; propose them in an issue first.

## Rules

- Rust-first, CLI-first: every workflow must be scriptable and emit structured
  JSON/YAML where useful.
- No secrets in config, audit events, tests, or tracked files. Fixtures use
  synthetic domains and test keys only.
- No private hostnames, private IPs, or machine-specific state in the
  repository.
- Do not add dependencies without a short justification in the PR description.
  Cryptographic and protocol dependencies must be maintained, standards-based
  crates.
- Keep the LinuxMice component contract (decision 0017) satisfied: the
  manifest in `manifests/` must stay valid for the schema in
  `linuxmice-component` and the component must remain independently usable.

## Checks

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/smoke-unit.sh
```

## Documentation

User-facing behavior changes update `README.md` and the relevant `docs/` file
in the same change. Raw thinking belongs in `docs/discussions/`, durable
direction in `docs/`.
