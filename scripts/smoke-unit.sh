#!/usr/bin/env bash
# Deterministic unit smoke for MiceScale using a fake tailscale binary.
# Exercises enroll -> status -> audit -> leave without touching a real tailnet.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo build --workspace >/dev/null

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

cat > "$WORK/fake-tailscale" <<'EOF'
#!/bin/sh
case "$1" in
  status) cat "$FAKE_FIXTURE"; exit 0 ;;
  up|logout) echo "fake: $*" >> "$FAKE_LOG"; exit 0 ;;
  *) echo "unexpected: $*" >&2; exit 2 ;;
esac
EOF
chmod +x "$WORK/fake-tailscale"

cat > "$WORK/status-fixture.json" <<'EOF'
{"BackendState":"Running","Self":{"HostName":"smoke-node","Online":true,"TailscaleIPs":["100.64.0.1"]},"Peer":{"server":{"Online":true}},"Health":[]}
EOF

export MICESCALE_CONFIG="$WORK/config.toml"
export MICESCALE_AUDIT_LOG="$WORK/audit.jsonl"
export MICESCALE_TAILSCALE_BIN="$WORK/fake-tailscale"
export FAKE_FIXTURE="$WORK/status-fixture.json"
export FAKE_LOG="$WORK/fake.log"

run() { "$ROOT/target/debug/micescale" "$@"; }

echo "== version =="
run version

echo "== carrier policy =="
run carrier policy --format json | grep -q '"posture": "carrier-untrusted"'
echo "posture ok"

echo "== carrier profiles =="
run carrier profiles --format json | grep -q '"id": "headscale"'
echo "profiles ok"

echo "== enroll =="
run enroll --server https://headscale.example.com --authkey smoke-secret-key --node-name smoke-node
grep -q 'enroll' "$MICESCALE_AUDIT_LOG"

echo "== status =="
run status --format json | grep -q '"online_peers": 1'

echo "== config =="
run config show --format json | grep -q '"carrier": "headscale"'

echo "== audit =="
run audit tail --format json | grep -q 'enroll'

echo "== leave =="
run leave
grep -q 'leave' "$MICESCALE_AUDIT_LOG"

echo "== secret hygiene =="
if grep -rq 'smoke-secret-key' "$MICESCALE_CONFIG" "$MICESCALE_AUDIT_LOG"; then
  echo "FAIL: secret leaked into config or audit" >&2
  exit 1
fi
echo "no secret leaks"

echo "== doctor (expected partial: control server unreachable) =="
if run doctor --format json; then
  echo "note: control server unexpectedly reachable; acceptable only with network"
fi

echo "SMOKE OK"
