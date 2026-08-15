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
#
# Attack 3 therefore needs an args file whose marker PDA is already on chain.
# That file carries a recipient secret, so it is gitignored and a clean clone
# does not have one: run scripts/deploy-and-claim.sh first, or set SPENT_ARGS.
# Without it this script runs 2 of 3 and exits 2 (INCOMPLETE) — the summary is
# computed from what actually ran, so no run can report three having done two.
#
# Exit status: 0 all three attacks ran and were rejected; 1 an attack was not
# rejected (or was rejected for the wrong reason); 2 an attack could not run,
# including because a dependency below is missing.
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

# Preflight, before anything is submitted. If wallet or spel is missing, every
# submission below fails with "No such file or directory", which reads as an
# attack that was not rejected — the loudest possible false negative, and one
# produced entirely by this machine. Name the tool instead.
MISSING=""
for tool in cargo python3 "$WALLET" "$SPEL"; do
  command -v "$tool" >/dev/null 2>&1 || MISSING="$MISSING $tool"
done
if [ -n "$MISSING" ]; then
  printf '\033[31mMISSING DEPENDENCY:\033[0m%s\n' "$MISSING" >&2
  echo "This script needs cargo, python3, the v0.2.4 wallet (WALLET_BIN) and the" >&2
  echo "vendored spel (SPEL_BIN). Nothing was submitted, so this says nothing about" >&2
  echo "what the deployed programs accept." >&2
  exit 2
fi

rule() { printf '\n\033[1m== %s\033[0m\n' "$1"; }
FAILED=0
# The final verdict is COMPUTED from these three slots, never asserted. Each
# starts "notrun" and only a submission that actually happened moves it, so a run
# that skipped an attack cannot print a three-attack verdict. (A submission in
# this programme was closed for reporting success from a job that had taken its
# explicit skip path; a summary must not be able to outrun its evidence.)
LABEL_1="membership, tampered Merkle path"
LABEL_2="destination redirect"
LABEL_3="double claim, marker already on chain"
STATUS_1=notrun; STATUS_2=notrun; STATUS_3=notrun
WHY_1=""; WHY_2=""; WHY_3=""

# An attack "passes" when the submission fails with the expected panic. A tx hash
# means the attack got through, which is the one outcome that must fail this run.
expect_panic() { # slot label expected-substring output
  local slot="$1" label="$2" want="$3" out="$4"
  if echo "$out" | grep -q 'tx_hash: [0-9a-f]\{64\}'; then
    printf '  \033[31mFAIL\033[0m %s produced a transaction — the attack was NOT rejected\n' "$label"
    printf -v "STATUS_$slot" 'failed'; FAILED=1; return
  fi
  if echo "$out" | grep -qF "$want"; then
    printf '  \033[32mOK\033[0m   %s rejected with %s\n' "$label" "$want"
    echo "$out" | grep -E 'panicked at|claim is not valid|account validation failed|ProgramProveFailed' \
      | head -3 | sed 's/^/       /'
    printf -v "STATUS_$slot" 'ok'
  else
    printf '  \033[31mFAIL\033[0m %s did not produce the expected %s\n' "$label" "$want"
    echo "$out" | tail -6 | sed 's/^/       /'
    printf -v "STATUS_$slot" 'failed'; FAILED=1
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
expect_panic 1 "tampered Merkle path" "NotEligible" "$(submit "$DIR/attack1.args")"

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
expect_panic 2 "redirected destination" "DestinationMismatch" "$(submit "$DIR/attack2.args")"

rule "3. a second claim by the same recipient is rejected"
if [ ! -f "$SPENT_ARGS" ]; then
  WHY_3="no spent claim at $SPENT_ARGS"
  echo "  NOT RUN: $WHY_3."
  echo "  This attack needs an args file whose marker PDA is already on chain. Such a"
  echo "  file carries a recipient secret, so it is gitignored and a clean clone has"
  echo "  none: run scripts/deploy-and-claim.sh first, or set SPENT_ARGS to an args"
  echo "  file whose marker is already on chain."
  echo "  (The protection itself is not in doubt here, only this run's coverage of it:"
  echo "   the double-claim rejection is exercised on every push by"
  echo "   'cargo test -p claim-verifier-tests' against the committed verifier binary,"
  echo "   and docs/onchain-audit.md §3 carries the live testnet transcript.)"
else
  echo "resubmitting $SPENT_ARGS, whose marker PDA is already on chain"
  expect_panic 3 "double claim" "AccountAlreadyInitialized" "$(submit "$SPENT_ARGS")"
fi

echo
rule "summary"
RAN=0
for slot in 1 2 3; do
  s="STATUS_$slot"; l="LABEL_$slot"; w="WHY_$slot"
  case "${!s}" in
    ok)     printf '  \033[32mok\033[0m       %s. %s — rejected, no transaction\n' "$slot" "${!l}"
            RAN=$((RAN + 1)) ;;
    failed) printf '  \033[31mFAILED\033[0m   %s. %s — NOT rejected as expected\n' "$slot" "${!l}"
            RAN=$((RAN + 1)) ;;
    *)      printf '  \033[33mNOT RUN\033[0m  %s. %s — %s\n' "$slot" "${!l}" "${!w:-not attempted}" ;;
  esac
done
printf '\n%s of 3 attacks ran.\n' "$RAN"

if [ "$FAILED" -ne 0 ]; then
  echo "AN ATTACK WAS NOT REJECTED — see above." >&2
  exit 1
elif [ "$RAN" -ne 3 ]; then
  echo "INCOMPLETE: this run performed $RAN of the 3 attacks, so it does not establish" >&2
  echo "the three-attack claim in docs/onchain-audit.md. The $RAN that ran were rejected" >&2
  echo "at proof-generation time and reached no block. Supply what the attack above" >&2
  echo "needs and re-run." >&2
  exit 2
else
  echo "All three attacks were rejected at proof-generation time; none reached a block."
  exit 0
fi
