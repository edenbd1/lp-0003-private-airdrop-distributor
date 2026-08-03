#!/usr/bin/env bash
# Full LP-0003 lifecycle against a REAL LEZ sequencer running locally in
# standalone mode, with RISC0_DEV_MODE=0.
#
# WHY THIS EXISTS SEPARATELY FROM demo.sh
#
# `demo.sh` is the fast tour: it runs the circuit and the built verifier through
# the sequencer's *executor*, which is the same code the chain runs but linked
# in-process. That is enough to show what is rejected and why, in seconds, with
# no network — but it is not the same claim as "works against a real sequencer".
#
# This script makes that one. It starts the actual `sequencer_service` binary in
# standalone mode, points a throwaway wallet at it, and drives the whole
# lifecycle over JSON-RPC — deploy both programs, commit a distribution, and
# submit a real privacy-preserving claim whose Risc0 receipt the sequencer
# verifies against PRIVACY_PRESERVING_CIRCUIT_ID — then reads the resulting
# marker PDA back off that local chain and checks it is owned by the verifier.
# Nothing is mocked and RISC0_DEV_MODE is 0 throughout.
#
#   ./scripts/e2e-local-sequencer.sh
#   COUNT1=1 COUNT2=0 ./scripts/e2e-local-sequencer.sh   # the CI shape, one proof
#
# Env:
#   LEZ_SRC     checkout of logos-execution-zone (default: ./_external/lez)
#   COUNT1      claims in distribution 1     (default 1)
#   COUNT2      claims in distribution 2     (default 0 = single distribution)
#   PORT        sequencer RPC port           (default: first free from 3141)
#   KEEP        set to 1 to leave the sequencer running for inspection
#
# Budget: one claim is a real proof, measured at ~150 s. COUNT1=1 COUNT2=0 is a
# single proof and is what CI uses; it still exercises every integration point.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

LEZ_SRC="${LEZ_SRC:-$ROOT/_external/lez}"
COUNT1="${COUNT1:-1}"
COUNT2="${COUNT2:-0}"
KEEP="${KEEP:-0}"

# The signing key of the genesis-funded test account, published in clear in
# LEZ's own justfile (`wallet-import-test-accounts`). It is a shared test
# account, not a secret, and it only exists on chains whose genesis lists it.
TEST_SIGNER_KEY=7f273098f25b71e6c005a9519f2678da8d1c7f01f6a27778e2d9948abdf901fb
TEST_SIGNER=CbgR6tj5kWx5oziiFptM7jMvrQeYY3Mzaao6ciuhSr2r

die() { echo "error: $*" >&2; exit 1; }
say() { printf '\n\033[1m%s\033[0m\n' "$*"; }

[ -d "$LEZ_SRC" ] || die "no LEZ checkout at $LEZ_SRC. Clone logos-execution-zone there, or set LEZ_SRC."
CONFIG_SRC="$LEZ_SRC/lez/sequencer/service/configs/debug/sequencer_config.json"
[ -f "$CONFIG_SRC" ] || die "no standalone config at $CONFIG_SRC"

SEQ_BIN="$LEZ_SRC/target/release/sequencer_service"
if [ ! -x "$SEQ_BIN" ]; then
  say "building the standalone sequencer (once; a few minutes)"
  ( cd "$LEZ_SRC" && cargo build --release --features standalone -p sequencer_service ) \
    || die "sequencer build failed"
fi
WALLET_BIN="$LEZ_SRC/target/release/wallet"
[ -x "$WALLET_BIN" ] || ( cd "$LEZ_SRC" && cargo build --release -p wallet ) || die "wallet build failed"
command -v spel >/dev/null 2>&1 || die "spel not on PATH (cargo install --git https://github.com/logos-co/spel --locked spel-cli)"

# The v0.2.0 wallet links Python 3.9 and dies with `Library not loaded:
# @rpath/Python3.framework/...` on some macOS setups. macOS SIP strips DYLD_*
# whenever bash execs another script, so it has to be set on the wallet's own
# exec — same reason deploy-and-claim.sh needs it too, which is why we export a
# name it also reads.
WALLET_ENV=()
if [ "$(uname)" = "Darwin" ]; then
  export DYLD_FALLBACK_FRAMEWORK_PATH="${DYLD_FALLBACK_FRAMEWORK_PATH:-/Library/Developer/CommandLineTools/Library/Frameworks}"
  WALLET_ENV=(env "DYLD_FALLBACK_FRAMEWORK_PATH=$DYLD_FALLBACK_FRAMEWORK_PATH")
fi
wallet_run() { "${WALLET_ENV[@]}" "$WALLET_BIN" "$@"; }

# A free port, so this never fights a sequencer the developer already has up.
PORT="${PORT:-}"
if [ -z "$PORT" ]; then
  for p in $(seq 3141 3200); do
    if ! nc -z localhost "$p" 2>/dev/null; then PORT=$p; break; fi
  done
fi
[ -n "$PORT" ] || die "no free port in 3141-3200"

WORK="$(mktemp -d)"
SEQ_HOME="$WORK/sequencer"; mkdir -p "$SEQ_HOME"
WALLET_HOME="$WORK/wallet";  mkdir -p "$WALLET_HOME"
RPC="http://localhost:$PORT"
SEQ_PID=""

cleanup() {
  if [ -n "$SEQ_PID" ] && [ "$KEEP" != "1" ]; then
    kill "$SEQ_PID" 2>/dev/null
    wait "$SEQ_PID" 2>/dev/null
  fi
  if [ "$KEEP" = "1" ]; then
    echo
    echo "left running: sequencer pid $SEQ_PID on $RPC"
    echo "  wallet home $WALLET_HOME"
    echo "  logs        $WORK/sequencer.log"
  else
    rm -rf "$WORK"
  fi
}
trap cleanup EXIT INT TERM

say "[1/5] starting a real sequencer in standalone mode on $RPC"
python3 - "$CONFIG_SRC" "$SEQ_HOME" <<'PY'
import json, sys
cfg = json.load(open(sys.argv[1]))
cfg["home"] = sys.argv[2]
json.dump(cfg, open(sys.argv[2] + "/sequencer_config.json", "w"), indent=2)
PY
RUST_LOG=info "$SEQ_BIN" --port "$PORT" "$SEQ_HOME/sequencer_config.json" \
  > "$WORK/sequencer.log" 2>&1 &
SEQ_PID=$!

for _ in $(seq 1 60); do
  if curl -s -m 2 -X POST "$RPC" -H 'Content-Type: application/json' \
       -d '{"jsonrpc":"2.0","id":1,"method":"getAccount","params":["'"$TEST_SIGNER"'"]}' \
       | grep -q '"result"'; then
    echo "  up, pid $SEQ_PID"; break
  fi
  kill -0 "$SEQ_PID" 2>/dev/null || { tail -20 "$WORK/sequencer.log"; die "sequencer died on startup"; }
  sleep 1
done
curl -s -m 3 -X POST "$RPC" -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getAccount","params":["'"$TEST_SIGNER"'"]}' \
  | grep -q '"result"' || die "sequencer never answered on $RPC"

say "[2/5] a throwaway wallet pointed at it"
cat > "$WALLET_HOME/wallet_config.json" <<EOF
{ "sequencer_addr": "$RPC/", "seq_poll_timeout": "30s", "seq_tx_poll_max_blocks": 15,
  "seq_poll_max_retries": 10, "seq_block_poll_max_amount": 100 }
EOF
export LEE_WALLET_HOME_DIR="$WALLET_HOME"
export NSSA_WALLET_HOME_DIR="$WALLET_HOME"
printf 'lp0003\n' | wallet_run account import public --private-key "$TEST_SIGNER_KEY" >/dev/null 2>&1 \
  || die "could not import the test signer"
echo "  imported Public/$TEST_SIGNER"

say "[3/5] funding it from the genesis vault"
wallet_run vault claim --account-id "Public/$TEST_SIGNER" --amount 5000 </dev/null >/dev/null 2>&1
for _ in $(seq 1 30); do
  bal=$(curl -s -m 3 -X POST "$RPC" -H 'Content-Type: application/json' \
        -d '{"jsonrpc":"2.0","id":1,"method":"getAccount","params":["'"$TEST_SIGNER"'"]}' \
        | python3 -c 'import json,sys; print(json.load(sys.stdin).get("result",{}).get("balance",0))' 2>/dev/null)
  [ "${bal:-0}" -gt 0 ] 2>/dev/null && { echo "  balance $bal"; break; }
  sleep 2
done
[ "${bal:-0}" -gt 0 ] 2>/dev/null || die "vault claim did not land"

say "[4/5] a private account to sign the claim"
wallet_run account new private </dev/null >/dev/null 2>&1 || die "could not create a private account"
CLAIMANT=$(wallet_run account list </dev/null 2>/dev/null \
  | grep -oE 'Private/[1-9A-HJ-NP-Za-km-z]+' | sed 's|Private/||' | tail -n 1)
[ -n "$CLAIMANT" ] || die "no private account to claim with"
echo "  Private/$CLAIMANT"

say "[5/5] deploy, commit a distribution, and claim against that sequencer"
echo "  RISC0_DEV_MODE=0, $((COUNT1 + COUNT2)) real proof(s), ~150 s apiece"
RISC0_DEV_MODE=0 \
SIGNER="$TEST_SIGNER" \
CLAIMANT="$CLAIMANT" \
COUNT1="$COUNT1" \
COUNT2="$COUNT2" \
SEQUENCER_URL="$RPC" \
WALLET_BIN="$WALLET_BIN" \
LEE_WALLET_HOME_DIR="$WALLET_HOME" \
NSSA_WALLET_HOME_DIR="$WALLET_HOME" \
  ./scripts/deploy-and-claim.sh
rc=$?

MANIFEST=artifacts/e2e/deploy-run.tsv
if [ $rc -ne 0 ] || [ ! -s "$MANIFEST" ]; then
  echo
  echo "lifecycle produced no claim; last sequencer output:" >&2
  tail -30 "$WORK/sequencer.log" >&2
  exit 1
fi

say "verifying the claim on the local chain"
IFS=$'\t' read -r DID CLAIM_TX NULL < <(head -n 1 "$MANIFEST")
echo "  distribution ${DID:0:16}…  claim ${CLAIM_TX:0:16}…"
SEQUENCER_URL="$RPC" \
CLAIM_TX="$CLAIM_TX" \
NULLIFIER="$NULL" \
DISTRIBUTION_ID="$DID" \
  ./scripts/verify-onchain-claim.sh
rc=$?

echo
if [ $rc -eq 0 ]; then
  printf '\033[1me2e against a real local sequencer: PASS\033[0m  (%s, %s claim(s), RISC0_DEV_MODE=0)\n' \
    "$RPC" "$((COUNT1 + COUNT2))"
else
  echo "e2e against a real local sequencer: FAIL" >&2
fi
exit $rc
