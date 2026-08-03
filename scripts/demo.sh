#!/usr/bin/env bash
# LP-0003 end-to-end demo, runnable from a clean clone with no funded account.
#
# WHAT THIS PROVES, AND WHAT IT DOES NOT
#
# Steps 1-3 run the adversarial suites against the *sequencer's own executor* —
# the same executor, the same input order, the same 32M session limit the chain
# uses (`lee/state_machine/src/program.rs:55-110`). A rejection you see there is
# the rejection the chain performs, byte for byte, because it is the same
# committed binary fed the same inputs.
#
# What the local steps do not do is *prove*. Proving a claim takes real
# wall-clock and establishes nothing extra about which inputs are accepted, so
# the suites execute rather than prove. The real proofs, generated with
# RISC0_DEV_MODE=0 and verified on chain by LEZ's privacy circuit, are what
# `scripts/deploy-and-claim.sh` produces against the public testnet — the
# resulting transactions are listed in `artifacts/e2e/claims.tsv`, and the last
# step here reads one of them straight off the chain.
#
#   ./scripts/demo.sh
#
# Needs: a Rust toolchain and curl/python3. Step 3 (the deployed-binary tests
# through the sequencer's executor) additionally needs the risc0 VM `r0vm`
# (`cargo risczero install`); if it is absent the step is skipped with a note
# rather than failing. `spel` on PATH is optional (step 0's program id). No
# funded account, no local sequencer.

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export RISC0_DEV_MODE=0

WORK="${WORK:-$ROOT/.demo}"
rm -rf "$WORK"; mkdir -p "$WORK"

DEMO_ID=de00000000000000000000000000000000000000000000000000000000000001
CLI=target/release/airdrop

rule() { printf '\n\033[1m== %s\033[0m\n' "$1"; }

rule "0. environment"
echo "RISC0_DEV_MODE=$RISC0_DEV_MODE  (0 = real proofs, no mock receipts)"
rustc --version
test -f artifacts/programs/claim_verifier.bin \
  || { echo "artifacts/programs/claim_verifier.bin missing. Build with: cargo risczero build --manifest-path crates/claim-verifier-spel/methods/guest/Cargo.toml" >&2; exit 1; }
spel program-id artifacts/programs/claim_verifier.bin 2>/dev/null | grep -E '📦|ProgramId \(hex\)' || true

rule "1. the claim bindings, in the circuit"
echo "10 adversarial tests over the claim logic: non-members, borrowed Merkle"
echo "paths, invented roots, forged nullifiers, inflated allocations, redirected"
echo "destinations. Plus a tree builder that round-trips sizes 1..=33 and 100."
cargo test -p airdrop-core --quiet 2>&1 | grep -E "result: ok\. [1-9]" | sed 's/^/   /'

rule "2. the encrypted bundle, adversarially"
echo "10 tests: a row opens only for its recipient, a tampered or swapped-header"
echo "row does not open, a low-order header opens for no one, padding rows open"
echo "for no one, and a recovered row reconstructs the exact committed leaf."
cargo test -p airdrop-crypto --quiet 2>&1 | grep -E "result: ok\. [1-9]" | sed 's/^/   /'

rule "3. the on-chain checks, through the sequencer's executor"
echo "7 tests against the built verifier binary: an honest claim accepted, a"
echo "forged nullifier / unanchored root / inflated allocation / forged marker /"
echo "double-claim each rejected with its documented error code."
if command -v r0vm >/dev/null 2>&1; then
  cargo test -p claim-verifier-tests --quiet 2>&1 | grep -E "result: ok\. [1-9]" | sed 's/^/   /'
else
  echo "   (skipped: needs the risc0 VM r0vm, install with 'cargo risczero install';"
  echo "    CI runs these on every push against the committed binary)"
fi

rule "4. a distribution, padded so its size is hidden"
cargo build --release --quiet -p airdrop-cli
$CLI demo-distribution --count 5 --id "$DEMO_ID" --pad 8 --out "$WORK" >/dev/null
ROWS=$(python3 -c "import json;print(len(json.load(open('$WORK/bundle.json'))))")
echo "5 eligible recipients, bundle padded to $ROWS rows: the published bundle"
echo "reveals neither who is eligible nor how many recipients there really are."

rule "5. a recipient claims from only the bundle and their secret"
NSK=$(python3 -c "import json;print(json.load(open('$WORK/recipients.json'))[2]['nsk_hex'])")
$CLI claim-from-bundle --dir "$WORK" --nsk "$NSK" --out "$WORK/claim.args" | sed 's/^/   /'
echo "   (the recipient scanned all $ROWS rows, skipped the padding, and kept the"
echo "    one row whose payload reconstructs a leaf anchored to the committed root)"

rule "6. a stranger's secret opens nothing"
STRANGER=ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff
if $CLI claim-from-bundle --dir "$WORK" --nsk "$STRANGER" --out "$WORK/none.args" >/dev/null 2>&1; then
  echo "   UNEXPECTED: a non-recipient opened a row" >&2; exit 1
fi
echo "   refused: no row in the bundle opens for a secret that is not eligible."

rule "7. compute cost"
if command -v r0vm >/dev/null 2>&1; then
  cargo test -p claim-verifier-tests --quiet -- --ignored --nocapture 2>&1 \
    | grep -E 'user cycles|proving cycles|budget consumed' | sed 's/^/   /'
else
  echo "   (measured through r0vm, which is absent here; see docs/benchmarks/cu-budget.md)"
  echo "   claim: 333,565 user cycles / 524,288 proving cycles = 1.56% of the budget"
fi

rule "8. what an observer sees"
python3 - "$WORK/claim.args" <<'PY'
import re, sys
txt = open(sys.argv[1]).read()
null = re.search(r"--nullifier '([0-9a-f]{64})'", txt).group(1)
marker = re.search(r"--claim-marker-seed '([0-9a-f]{64})'", txt).group(1)
print("On chain, this claim is a single marker PDA seeded by:\n")
print("   ", marker)
print(f"""
That seed is SHA256(prefix || distribution_id || nullifier), and the nullifier
   {null}
is SHA256(prefix || distribution_id || nsk) for a recipient secret nobody else
holds. An observer who knows every candidate address still cannot compute the
nullifier, so they cannot map this marker back to an address. That is the
unlinkability criterion.""")
PY

rule "9. the live deployment, read straight off the chain"
echo "Everything above is local. This checks a claim that is actually deployed on"
echo "the public testnet, taken from the committed manifest artifacts/e2e/claims.tsv:"
echo
read -r ID TX NULL < <(sed -n '2p' artifacts/e2e/claims.tsv)
echo "   distribution ${ID:0:8}...  claim ${TX:0:16}..."
echo
if CLAIM_TX="$TX" NULLIFIER="$NULL" DISTRIBUTION_ID="$ID" ./scripts/verify-onchain-claim.sh; then
  :
else
  echo
  echo "If this failed, the public testnet may be unreachable from here, or reset."
  echo "All claim hashes are in artifacts/e2e/claims.tsv and can be checked with any"
  echo "JSON-RPC client via getTransaction."
fi

printf '\n\033[1mdemo complete\033[0m — working directory %s\n' "$WORK"
echo "The full create-then-claim flow against the testnet (real RISC0_DEV_MODE=0"
echo "proving, funded account needed) is scripts/deploy-and-claim.sh; see docs/DEPLOYMENT.md."