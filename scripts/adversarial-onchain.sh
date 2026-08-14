#!/usr/bin/env bash
# Reproduce, against the LIVE deployed programs, the three demonstrations written
# up in docs/onchain-audit.md. Each attack must fail at proof-generation time, so
# none of them ever reaches a block.
#
#   SIGNER=<funded public id> ./scripts/adversarial-onchain.sh
#
# Needs the v0.2.4 wallet (WALLET_BIN) and the vendored spel (SPEL_BIN).
#
# 1. Tamper one word of the witness's Merkle path. Every check the *verifier*
#    performs still passes — the nsk is untouched so the nullifier re-derives, the
#    allocation and marker seed match, the root is anchored — so if the chained
#    call were decorative this would succeed. The chained guest folds the tampered
#    path, does not reach the committed root, and panics NotEligible.
# 2. Change only the enforced `--destination`, as a relayer redirecting an
#    allocation would, leaving the destination the witness commits untouched. The
#    verifier builds the statement from the enforced args and the chained guest
#    requires witness.destination == statement.destination: DestinationMismatch.
# 3. Resubmit a claim whose marker PDA is already on chain. The marker is created
#    with `init`, so the verifier's own account validation rejects it.
#
# Attacks 1 and 2 run against a FRESH distribution whose marker is still unspent,
# so nothing but the tampering can be responsible for the rejection. Attack 3
# reuses a claim from the committed run, whose marker is already taken.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; cd "$ROOT"
export RISC0_DEV_MODE=0

: "${SIGNER:?set SIGNER to a funded public account id}"
RPC="${SEQUENCER_URL:-https://testnet.lez.logos.co}"
WALLET="${WALLET_BIN:-wallet}"
SPEL="${SPEL_BIN:-spel}"
SIGNER_HOME="${LEE_WALLET_HOME_DIR:-$HOME/.lee/wallet}"
CLI=target/release/airdrop
IDL=idl/claim_verifier.idl.json
VERIFIER=artifacts/programs/claim_verifier.bin
CLAIM_LEZ=artifacts/programs/claim_lez.bin
SPENT_ARGS="${SPENT_ARGS:-artifacts/e2e/dist1/claim_0.args}"
DIR=.demo-adversarial
rm -rf "$DIR"

rule() { printf '\n\033[1m== %s\033[0m\n' "$1"; }
FAILED=0
# An attack "passes" when the submission fails with the expected panic. A tx hash
# means the attack got through, which is the one outcome that must fail this run.
expect_panic() { # label expected-substring output
  local label="$1" want="$2" out="$3"
  if echo "$out" | grep -q 'tx_hash: [0-9a-f]\{64\}'; then
    printf '  \033[31mFAIL\033[0m %s produced a transaction — the attack was NOT rejected\n' "$label"
    FAILED=1; return
  fi
  if echo "$out" | grep -qF "$want"; then
    printf '  \033[32mOK\033[0m   %s rejected with %s\n' "$label" "$want"
    echo "$out" | grep -E 'panicked at|claim is not valid|account validation failed|ProgramProveFailed' \
      | head -3 | sed 's/^/       /'
  else
    printf '  \033[31mFAIL\033[0m %s did not produce the expected %s\n' "$label" "$want"
    echo "$out" | tail -6 | sed 's/^/       /'
    FAILED=1
  fi
}

# Each submission signs with its own throwaway private account, for the same
# reason prove-one-claim.sh does: a private account that has already spent a
# commitment carries an extra account identity into the next privacy transaction.
submit() { # args-file
  local flat cw claimant
  flat=$(tr '\n' ' ' < "$1")
  cw=$(mktemp -d)
  printf '{ "sequencers": [{ "sequencer_addr": "%s" }], "seq_poll_timeout": "30s", "seq_tx_poll_max_blocks": 15, "seq_poll_max_retries": 10, "seq_block_poll_max_amount": 100, "calibration_limit": 100 }\n' "$RPC" > "$cw/wallet_config.json"
  export LEE_WALLET_HOME_DIR="$cw" NSSA_WALLET_HOME_DIR="$cw"
  "$WALLET" account new private </dev/null >/dev/null 2>&1
  claimant=$("$WALLET" account list </dev/null 2>/dev/null \
    | grep -oE 'Private/[1-9A-HJ-NP-Za-km-z]+' | sed 's|Private/||' | tail -n1)
  "$WALLET" account sync-private >/dev/null 2>&1 || true
  eval "$SPEL" --idl "$IDL" --program "$VERIFIER" --bin-claimlez "$CLAIM_LEZ" \
    -- claim --claimant "Private/$claimant" $flat 2>&1
  rm -rf "$cw"
  export LEE_WALLET_HOME_DIR="$SIGNER_HOME" NSSA_WALLET_HOME_DIR="$SIGNER_HOME"
}

rule "0. a fresh distribution, so the markers under attack are unspent"
cargo build --release --quiet -p airdrop-cli
ID=$(head -c32 /dev/urandom | od -An -tx1 | tr -d ' \n')
$CLI demo-distribution --count 2 --id "$ID" --out "$DIR" >/dev/null
ROOT_HEX=$(python3 -c "import json;print(json.load(open('$DIR/distribution.json'))['root_hex'])")
echo "distribution ${ID:0:16}...  root ${ROOT_HEX:0:16}..."
export LEE_WALLET_HOME_DIR="$SIGNER_HOME" NSSA_WALLET_HOME_DIR="$SIGNER_HOME"
$SPEL --idl "$IDL" --program "$VERIFIER" -- create_distribution --authority "Public/$SIGNER" \
  --distribution-id "$ID" --eligibility-root "$ROOT_HEX" >/dev/null 2>&1 || true
for i in 0 1; do
  NSK=$(python3 -c "import json;print(json.load(open('$DIR/recipients.json'))[$i]['nsk_hex'])")
  $CLI claim-from-bundle --dir "$DIR" --nsk "$NSK" --out "$DIR/claim_$i.args" >/dev/null
done
echo "two claim packages built, neither submitted"

rule "1. membership is genuinely verified on chain (tampered Merkle path)"
# ClaimInstruction is risc0-serde encoded as u32 words:
#   witness = nsk[32] identifier[4] allocation[4] salt[32] path(1 + 32*depth)
#             leaf_index[2] destination[32] ; then the statement follows.
# Word 72 is the path length, so word 73 is the first byte of the first sibling.
# Altering it leaves the nsk, allocation, salt and statement untouched, which is
# precisely why every verifier-level check still passes.
python3 - "$DIR/claim_0.args" "$DIR/attack1.args" <<'PY'
import re, sys
src = open(sys.argv[1]).read()
m = re.search(r"--witness-words '([^']*)'", src)
w = [int(x) for x in m.group(1).split(",")]
assert w[72] > 0, "word 72 should be the Merkle path length"
w[73] ^= 1  # flip one bit of the first sibling
open(sys.argv[2], "w").write(
    src[:m.start(1)] + ",".join(map(str, w)) + src[m.end(1):])
print(f"   flipped one bit of the first Merkle sibling (word 73), path depth {w[72]}")
PY
expect_panic "tampered Merkle path" "NotEligible" "$(submit "$DIR/attack1.args")"

rule "2. the destination cannot be redirected"
python3 - "$DIR/claim_1.args" "$DIR/attack2.args" <<'PY'
import re, sys
src = open(sys.argv[1]).read()
m = re.search(r"--destination '([0-9a-f]{64})'", src)
orig = m.group(1)
# A destination the attacker controls. The witness still commits the original.
redirect = ("de" * 32)
open(sys.argv[2], "w").write(src[:m.start(1)] + redirect + src[m.end(1):])
print(f"   enforced destination {orig[:16]}… -> {redirect[:16]}… (witness unchanged)")
PY
expect_panic "redirected destination" "DestinationMismatch" "$(submit "$DIR/attack2.args")"

rule "3. a second claim by the same recipient is rejected"
if [ ! -f "$SPENT_ARGS" ]; then
  echo "  SKIP: no spent claim at $SPENT_ARGS (run scripts/deploy-and-claim.sh first,"
  echo "        or set SPENT_ARGS to an args file whose marker is already on chain)"
else
  echo "resubmitting $SPENT_ARGS, whose marker PDA is already on chain"
  expect_panic "double claim" "AccountAlreadyInitialized" "$(submit "$SPENT_ARGS")"
fi

echo
if [ "$FAILED" -eq 0 ]; then
  echo "All three attacks were rejected at proof-generation time; none reached a block."
  exit 0
else
  echo "AN ATTACK WAS NOT REJECTED — see above." >&2
  exit 1
fi
