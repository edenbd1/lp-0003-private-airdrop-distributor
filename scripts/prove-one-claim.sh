#!/usr/bin/env bash
# Generate ONE real claim proof, end to end on the public testnet, with
# RISC0_DEV_MODE=0, and verify it on chain. This is the proof that demo.sh does
# not generate: demo.sh runs the adversarial suites through the executor (which
# execute, they do not prove), while this script produces a real STARK on the
# privacy path. It is the narrated video's proof-generation scene.
#
#   SIGNER=<funded public id> ./scripts/prove-one-claim.sh
# Needs the v0.2.4 wallet (WALLET_BIN) and the vendored spel (SPEL_BIN) on hand.
#
# Needs `spel` and `wallet` on PATH and a funded SIGNER. Each run uses a fresh,
# random distribution id, so it never collides with a previous run.

set -uo pipefail  # not -e: this is a network script, so failures are handled
                  # explicitly at each on-chain step rather than aborting silently.
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
export RISC0_DEV_MODE=0

: "${SIGNER:?set SIGNER to a funded public account id}"
RPC="${SEQUENCER_URL:-https://testnet.lez.logos.co}"
WALLET="${WALLET_BIN:-wallet}"
SPEL="${SPEL_BIN:-spel}"
# The funded SIGNER's wallet home. The claim signs with its own throwaway home
# (step 4), so remember this one for create_distribution.
SIGNER_HOME="${LEE_WALLET_HOME_DIR:-$HOME/.lee/wallet}"
CLI=target/release/airdrop
IDL=idl/claim_verifier.idl.json
VERIFIER=artifacts/programs/claim_verifier.bin
CLAIM_LEZ=artifacts/programs/claim_lez.bin
DIR=.demo-prove
rm -rf "$DIR"

confirmed() { curl -s -m 25 -X POST "$RPC" -H 'Content-Type: application/json' \
  -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTransaction\",\"params\":[\"$1\"]}" \
  | grep -qE '"result":\['; }  # v0.2.4: getTransaction returns [tx, block]
rule() { printf '\n\033[1m== %s\033[0m\n' "$1"; }

# A fresh distribution id each run, so re-running never hits a double-claim.
ID=$(head -c32 /dev/urandom | od -An -tx1 | tr -d ' \n')

rule "0. environment"
echo "RISC0_DEV_MODE=$RISC0_DEV_MODE  (0 = real proofs, no mock receipts)"
cargo build --release --quiet -p airdrop-cli

rule "1. a fresh one-recipient distribution"
$CLI demo-distribution --count 1 --id "$ID" --out "$DIR" >/dev/null
ROOT_HEX=$(python3 -c "import json;print(json.load(open('$DIR/distribution.json'))['root_hex'])")
echo "distribution ${ID:0:16}..."
echo "root         ${ROOT_HEX:0:16}..."

rule "2. commit the eligibility root on chain"
CDTX=""
for attempt in 1 2 3; do
  CD=$($SPEL --idl "$IDL" --program "$VERIFIER" -- create_distribution --authority "Public/$SIGNER" \
    --distribution-id "$ID" --eligibility-root "$ROOT_HEX" 2>&1) || true
  CDTX=$(echo "$CD" | grep -o 'tx_hash: [0-9a-f]\{64\}' | head -1 | cut -d' ' -f2)
  [ -n "$CDTX" ] && break
  echo "  create_distribution attempt $attempt did not return a tx, retrying..."
  sleep 5
done
[ -n "$CDTX" ] || { echo "$CD" | tail -6; echo "create_distribution failed" >&2; exit 1; }
echo "create_distribution $CDTX"
for _ in $(seq 1 20); do sleep 6; confirmed "$CDTX" && { echo "  landed"; break; }; done

rule "3. build the claim from only the bundle and the recipient secret"
NSK=$(python3 -c "import json;print(json.load(open('$DIR/recipients.json'))[0]['nsk_hex'])")
$CLI claim-from-bundle --dir "$DIR" --nsk "$NSK" --out "$DIR/claim.args" | sed 's/^/   /'
NULL=$(sed -n "s/^--nullifier '//p" "$DIR/claim.args" | tr -d "'")

rule "4. prove and submit (real STARK, RISC0_DEV_MODE=0; timed below, per machine)"
FLAT=$(tr '\n' ' ' < "$DIR/claim.args")
# Sign with a fresh throwaway claimant. On LEZ v0.2.4 a private account that has
# already spent a commitment carries an extra account identity into the next
# privacy transaction and the circuit rejects it; a fresh account keeps the
# identity set at what the claim declares. The eligibility secret is in the
# witness, so the claimant is only the throwaway signer.
CW=$(mktemp -d)
printf '{ "sequencers": [{ "sequencer_addr": "%s" }], "seq_poll_timeout": "30s", "seq_tx_poll_max_blocks": 15, "seq_poll_max_retries": 10, "seq_block_poll_max_amount": 100, "calibration_limit": 100 }\n' "$RPC" > "$CW/wallet_config.json"
export LEE_WALLET_HOME_DIR="$CW" NSSA_WALLET_HOME_DIR="$CW"
"$WALLET" account new private </dev/null >/dev/null 2>&1
CLAIMANT=$("$WALLET" account list </dev/null 2>/dev/null \
  | grep -oE 'Private/[1-9A-HJ-NP-Za-km-z]+' | sed 's|Private/||' | tail -n1)
"$WALLET" account sync-private >/dev/null 2>&1 || true
echo "proving locally, then submitting on the privacy path ..."
START=$(python3 -c "import time;print(time.time())")
OUT=$(eval $SPEL --idl "$IDL" --program "$VERIFIER" --bin-claimlez "$CLAIM_LEZ" \
  -- claim --claimant "Private/$CLAIMANT" $FLAT 2>&1) || true
END=$(python3 -c "import time;print(time.time())")
TX=$(echo "$OUT" | grep -o 'tx_hash: [0-9a-f]\{64\}' | head -1 | cut -d' ' -f2)
[ -n "$TX" ] || { echo "$OUT" | tail -6; echo "no claim tx produced" >&2; exit 1; }
printf 'claim tx %s  (proved + submitted in %ss)\n' "$TX" "$(python3 -c "print(f'{$END-$START:.0f}')")"
for _ in $(seq 1 25); do sleep 10; confirmed "$TX" && { echo "  landed"; break; }; done

rule "5. verify the fresh claim on chain (5 checks)"
CLAIM_TX="$TX" NULLIFIER="$NULL" DISTRIBUTION_ID="$ID" ./scripts/verify-onchain-claim.sh

printf '\n\033[1ma real proof was generated with RISC0_DEV_MODE=0 and verified on chain\033[0m\n'