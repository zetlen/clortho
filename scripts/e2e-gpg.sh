#!/usr/bin/env bash
# End-to-end check against a real gpg-agent in a throwaway GNUPGHOME.
# Usage: cargo build && scripts/e2e-gpg.sh
set -euo pipefail
BIN="$(cd "$(dirname "$0")/.." && pwd)/target/debug"
T="$(mktemp -d)"
export CLORTHO_SOCKET="$T/run/clortho.sock" CLORTHO_CONFIG="$T/config.toml" CLORTHO_STATE_DIR="$T/state"
export GNUPGHOME="$T/gnupg" PATH="$BIN:$PATH"
printf 'store = "confirm"\nstore_backend = "memory"\nnotify_command = []\n' > "$CLORTHO_CONFIG"
mkdir -m 700 "$GNUPGHOME"
echo "pinentry-program $BIN/pinentry-clortho" > "$GNUPGHOME/gpg-agent.conf"
PASS='gate keeper%1'
cleanup() { gpgconf --kill gpg-agent || true; kill "$(clortho status | sed -n 's/^daemon pid \([0-9]*\).*/\1/p')" 2>/dev/null || true; }
trap cleanup EXIT

gpg -q --batch --pinentry-mode loopback --passphrase "$PASS" \
  --quick-generate-key 'Vinz Clortho <vinz@gozer.test>' ed25519 sign never 2>/dev/null
gpgconf --kill gpg-agent

pending_id() { { clortho status 2>/dev/null || true; } | awk '/^recent:/{exit} /^  #/{sub("#","",$1); print $1; exit}'; }
wait_pending() { for _ in $(seq 100); do id="$(pending_id)"; if [ -n "$id" ]; then echo "$id"; return; fi; sleep 0.1; done; echo "no pending request" >&2; exit 1; }
sign() { echo "$1" | gpg --clearsign > "$T/$1.asc" 2> "$T/$1.err"; }

echo "1. wrong passphrase, then right one with remember (daemon autostarts)"
sign one & job=$!
echo nope | clortho answer "$(wait_pending)"
sleep 0.5
echo "$PASS" | clortho answer "$(wait_pending)" --remember
wait $job && gpg -q --verify "$T/one.asc" 2>/dev/null

echo "2. release the remembered secret on approval"
gpgconf --reload gpg-agent
sign two & job=$!
clortho approve "$(wait_pending)"
wait $job && gpg -q --verify "$T/two.asc" 2>/dev/null

echo "3. cancel"
gpgconf --reload gpg-agent
sign three & job=$!
clortho cancel "$(wait_pending)"
if wait $job; then echo "signing should have failed" >&2; exit 1; fi
grep -q 'cancelled' "$T/three.err"

echo "4. three askpass callers for one key, one answer"
for i in 1 2 3; do clortho-askpass "Enter passphrase for key '/k/id_ed25519': " > "$T/ap$i" & done
sleep 0.5
echo shared | clortho answer "$(wait_pending)"
wait
[ "$(cat "$T"/ap1 "$T"/ap2 "$T"/ap3 | tr '\n' ' ')" = "shared shared shared " ]

echo "ok"
