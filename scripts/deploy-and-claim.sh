#!/usr/bin/env bash
# LP-0003 end-to-end on the public LEZ testnet: deploy the two programs, commit
# two eligibility distributions, and submit claims against them on the
# privacy-preserving path. Each claim is a real Risc0 proof composed on chain via
# env::verify (RISC0_DEV_MODE=0).
#
# This is the script behind the "2 distributions, >= 20 unique claims" criterion.
# It is idempotent on the deploys (content-addressed) and on create_distribution
# (init fails harmlessly if a distribution already exists).
#
# Env:
#   SIGNER          Public account id that pays and authors (must be funded)
#   CLAIMANT        Private account id that signs the claims (authorized)
#   SPEL_BIN        spel >= 0.6.0 (default: spel on PATH)
#   WALLET_BIN      wallet from LEZ v0.2.0 (default: wallet on PATH)
#   COUNT1 COUNT2   recipients per distribution (default 12 and 10)
#
# The wallet may print "Transaction NOT confirmed" for a privacy claim whose
# proving outruns its polling window; the tx lands anyway. This script checks
# getTransaction, not the CLI verdict.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

SPEL_BIN="${SPEL_BIN:-spel}"
WALLET_BIN="${WALLET_BIN:-wallet}"
RPC="${SEQUENCER_URL:-https://testnet.lez.logos.co}"
COUNT1="${COUNT1:-12}"
COUNT2="${COUNT2:-10}"
: "${SIGNER:?set SIGNER to a funded Public account id}"
: "${CLAIMANT:?set CLAIMANT to an authorized Private account id}"

CLI=target/release/airdrop
cargo build --release -p airdrop-cli --bin airdrop >/dev/null 2>&1

IDL=idl/claim_verifier.idl.json
VERIFIER=artifacts/programs/claim_verifier.bin
CLAIM_LEZ=artifacts/programs/claim_lez.bin

confirmed() {
  # LEZ v0.2.2 getTransaction returns Option<(LeeTransaction, BlockId)>: a JSON
  # array `"result":[<tx>,<block>]` for a found tx, `"result":null` otherwise
  # (v0.2.0 returned the tx as a bare string `"result":"..."`).
  curl -s -m 20 -X POST "$RPC" -H 'Content-Type: application/json' \
    -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"getTransaction\",\"params\":[\"$1\"]}" \
    | grep -qE '"result":\['
}
wait_tx() { # hash label
  for _ in $(seq 1 40); do confirmed "$1" && { echo "  confirmed $2 $1"; return 0; }; sleep 6; done
  echo "  TIMEOUT $2 $1" >&2; return 1
}

echo "[1/4] deploy programs (content-addressed)"
"$WALLET_BIN" deploy-program "$CLAIM_LEZ" >/dev/null 2>&1 || true
"$WALLET_BIN" deploy-program "$VERIFIER"  >/dev/null 2>&1 || true

deploy_hash() { python3 -c "
import hashlib,struct,sys;b=open(sys.argv[1],'rb').read()
print(hashlib.sha256(struct.pack('<I',len(b))+b).hexdigest())" "$1"; }
wait_tx "$(deploy_hash "$CLAIM_LEZ")" "claim program"
wait_tx "$(deploy_hash "$VERIFIER")"  "claim verifier"

# A fresh run produces NEW privacy-claim transactions (each is signed with a
# nonce, so the hashes differ from any prior run). It therefore writes to its own
# output file and does NOT overwrite artifacts/e2e/claims.tsv, which is the
# committed, historical list of the live claims cited in docs/DEPLOYMENT.md.
MANIFEST="${MANIFEST:-artifacts/e2e/deploy-run.tsv}"
mkdir -p artifacts/e2e
: > "$MANIFEST"

# distribution index -> (id, count, dir)
run_distribution() { # id count dir base step
  local id="$1" count="$2" dir="$3" base="$4" step="$5"
  echo "[dist $id] build tree of $count recipients"
  "$CLI" demo-distribution --count "$count" --id "$id" --base "$base" --step "$step" --out "$dir" >/dev/null
  local root
  root=$(python3 -c "import json;print(json.load(open('$dir/distribution.json'))['root_hex'])")
  echo "  root $root ; committing on chain"
  "$SPEL_BIN" --idl "$IDL" --program "$VERIFIER" \
    -- create_distribution --authority "Public/$SIGNER" \
    --distribution-id "$id" --eligibility-root "$root" >/dev/null 2>&1 || true

  for i in $(seq 0 $((count-1))); do
    local args="$dir/claim_$i.args"
    # Recipient-side flow: claim from only the published encrypted bundle and the
    # recipient's own secret, the way a real recipient would. The demo knows each
    # secret from recipients.json; a real recipient supplies their own.
    local nsk
    nsk=$(python3 -c "import json;print(json.load(open('$dir/recipients.json'))[$i]['nsk_hex'])")
    "$CLI" claim-from-bundle --dir "$dir" --nsk "$nsk" --out "$args" >/dev/null 2>&1 \
      || { echo "  SKIP $id $i (row did not open)"; continue; }
    local flat; flat=$(tr '\n' ' ' < "$args")
    # A privacy transaction spends the signer's commitment, so the claimant's
    # private account must be re-synced before each claim or its membership proof
    # is stale and the sequencer drops the transaction.
    "$WALLET_BIN" account sync-private >/dev/null 2>&1 || true
    echo "  claim $i of dist ${id:0:8} (from encrypted bundle) ..."
    local out; out=$(eval "$SPEL_BIN" --idl "$IDL" --program "$VERIFIER" \
      --bin-claimlez "$CLAIM_LEZ" \
      -- claim --claimant "Private/$CLAIMANT" $flat 2>&1)
    local tx; tx=$(echo "$out" | grep -o 'tx_hash: [0-9a-f]\{64\}' | head -1 | cut -d' ' -f2)
    local null; null=$(grep -o "'[0-9a-f]\{64\}'" "$args" | sed -n '3p' | tr -d "'")
    [ -n "$tx" ] && printf '%s\t%s\t%s\n' "$id" "$tx" "$null" >> "$MANIFEST"
  done
}

echo "[2/4] distribution 1"
run_distribution "b100000000000000000000000000000000000000000000000000000000000001" "$COUNT1" artifacts/e2e/dist1 100 10
# COUNT2=0 runs a single distribution — used by the local-sequencer e2e, where one
# real proof already exercises every integration point and the public-testnet run
# is what accumulates the full claim count.
if [ "$COUNT2" -gt 0 ]; then
  echo "[3/4] distribution 2"
  run_distribution "b200000000000000000000000000000000000000000000000000000000000002" "$COUNT2" artifacts/e2e/dist2 500 25
fi

echo "[4/4] confirm claims landed"
n=0
while IFS=$'\t' read -r id tx null; do
  confirmed "$tx" && n=$((n+1))
done < "$MANIFEST"
total=$(wc -l < "$MANIFEST" | tr -d ' ')
ndist=$([ "$COUNT2" -gt 0 ] && echo 2 || echo 1)
echo "  $n / $total claims confirmed on chain, across $ndist distribution(s)"
echo "  manifest: $MANIFEST (distribution_id, claim_tx, nullifier)"
echo
echo "Verify any one with:"
echo "  CLAIM_TX=<tx> NULLIFIER=<nullifier> ./scripts/verify-onchain-claim.sh"
