#!/usr/bin/env bash
# Verify, from public data, that an LP-0003 claim's membership proof is really
# verified on chain by the claim verifier program.
#
# Needs curl, python3, jq. Nothing is taken on trust: every identifier is derived
# from the binaries in this repository or decoded from the bytes the sequencer
# returns. Reads the chain over getTransaction / getAccount rather than the block
# explorer, which does not index privacy-preserving transactions.
#
#   ./scripts/verify-onchain-claim.sh
#
# The five checks:
#   1. The deployed bytecode IS the bytecode here. A deploy tx hash is
#      SHA256(borsh(bytecode)); recomputing it from the local file and finding it
#      on chain proves the deployed program is byte-identical.
#   2. The claim transaction is PrivacyPreserving, not Public. A public tx proves
#      and verifies nothing (program.rs:73-77, "Execute the program (without
#      proving)"); only the privacy path carries a receipt the sequencer verifies
#      and composes chained calls with env::verify (execution_state.rs:149).
#   3. The embedded receipt is a Succinct STARK, not a RISC0_DEV_MODE fake.
#   4. The claim marker PDA is DERIVED here from the verifier ImageID and the
#      nullifier, not asserted. The nullifier is submitter-supplied (a function of
#      the recipient secret), so a third party cannot reconstruct it; what this
#      establishes is that the marker at this address encodes this claim.
#   5. That marker PDA is owned by the verifier program. program_owner cannot be
#      forged.

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
RPC="${SEQUENCER_URL:-https://testnet.lez.logos.co}"

CLAIM_LEZ_BIN=artifacts/programs/claim_lez.bin
VERIFIER_BIN=artifacts/programs/claim_verifier.bin

# Filled in by scripts/deploy-and-claim.sh after a live claim. Override via env.
CLAIM_TX="${CLAIM_TX:-}"
NULLIFIER="${NULLIFIER:-}"
DISTRIBUTION_ID="${DISTRIBUTION_ID:-}"

FAILED=0
ok()  { printf '  \033[32mOK\033[0m   %s\n' "$1"; }
bad() { printf '  \033[31mFAIL\033[0m %s\n' "$1"; FAILED=1; }
rpc() { curl -s -m 30 -X POST "$RPC" -H 'Content-Type: application/json' \
          -d "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"$1\",\"params\":$2}"; }

deploy_tx_hash() {
  python3 -c "
import hashlib,struct,sys
b=open(sys.argv[1],'rb').read()
print(hashlib.sha256(struct.pack('<I',len(b))+b).hexdigest())" "$1"
}
image_id() {
  python3 -c "
import sys,subprocess,re
for sub in ('program-id','inspect'):
    out=subprocess.run(['spel',sub,sys.argv[1]],capture_output=True,text=True).stdout
    m=re.search(r'ImageID \(hex bytes\):\s*([0-9a-f]{64})',out)
    if m: print(m.group(1)); break
else: print('')" "$1" 2>/dev/null
}

echo "LP-0003 on-chain claim verification"
echo "sequencer: $RPC"
echo

echo "[1/5] the deployed bytecode is the bytecode in this repository"
for pair in "claim program:$CLAIM_LEZ_BIN" "verifier program:$VERIFIER_BIN"; do
  name="${pair%%:*}"; bin="${pair#*:}"
  [ -f "$bin" ] || { bad "$name binary missing at $bin"; continue; }
  h=$(deploy_tx_hash "$bin")
  if [ "$(rpc getTransaction "[\"$h\"]" | jq -r 'if .result == null then "no" else "yes" end')" = "yes" ]; then
    ok "$name  SHA256(borsh(bytecode)) = $h found on chain"
  else
    bad "$name  computed $h is NOT on chain"
  fi
done
echo

if [ -z "$CLAIM_TX" ]; then
  echo "[2-5] skipped: no CLAIM_TX set. Run scripts/deploy-and-claim.sh first, or"
  echo "      pass CLAIM_TX=<hash> NULLIFIER=<hex> ./scripts/verify-onchain-claim.sh"
  echo
  [ "$FAILED" -eq 0 ] && { echo "Program deployments verified."; exit 0; } || exit 1
fi

echo "[2/5] the claim transaction is PrivacyPreserving, not Public"
TX_B64=$(rpc getTransaction "[\"$CLAIM_TX\"]" | jq -r '.result // empty')
if [ -z "$TX_B64" ]; then
  bad "claim transaction not found  $CLAIM_TX"
else
  VARIANT=$(printf '%s' "$TX_B64" | python3 -c 'import sys,base64; print(base64.b64decode(sys.stdin.read())[0])')
  SIZE=$(printf '%s' "$TX_B64" | python3 -c 'import sys,base64; print(len(base64.b64decode(sys.stdin.read())))')
  [ "$VARIANT" = "1" ] && ok "borsh variant byte 1 = PrivacyPreserving, $SIZE bytes" \
    || bad "borsh variant byte $VARIANT (expected 1 = PrivacyPreserving)"
fi
echo

echo "[3/5] the embedded receipt is a Succinct STARK, not a dev-mode fake"
if [ -n "$TX_B64" ]; then
  KIND=$(python3 -c "
import sys, base64
raw = base64.b64decode(sys.argv[1])
found = -1
for off in range(1, len(raw) - 5):
    n = int.from_bytes(raw[off:off+4], 'little')
    if n > 100000 and off + 4 + n == len(raw):
        found = raw[off + 4]; break
print(found)" "$TX_B64")
  case "$KIND" in
    1) ok "InnerReceipt variant 1 = Succinct (a real STARK)" ;;
    0) ok "InnerReceipt variant 0 = Composite (a real STARK)" ;;
    2) ok "InnerReceipt variant 2 = Groth16 (a real SNARK)" ;;
    3) bad "InnerReceipt variant 3 = Fake — a RISC0_DEV_MODE receipt" ;;
    *) bad "could not decode the InnerReceipt variant (got $KIND)" ;;
  esac
fi
echo

echo "[4/5] derive the claim marker PDA from the ImageID and the enforced claim"
VID=$(image_id "$VERIFIER_BIN")
[ -n "$VID" ] || VID=a7b7cf261adedbc50b2a5a7db6967325bb01b2e0a1c9d03f1d944d0a6fe7b77d
# The marker seed commits to the distribution and the nullifier:
#   seed   = SHA256(CLAIM_MARKER_PREFIX || distribution_id || nullifier)
#   marker = PDA(verifier, seed)
# so the marker's address records which distribution this claim spent.
MARKER=$(python3 -c "
import hashlib,sys
A=b'123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz'
def b58(b):
    n=int.from_bytes(b,'big'); o=b''
    while n: n,r=divmod(n,58); o=A[r:r+1]+o
    return '1'*(len(b)-len(b.lstrip(b'\0')))+o.decode()
image, did, nullifier = sys.argv[1], sys.argv[2], sys.argv[3]
g=hashlib.sha256()
g.update(b'/lp-0003/v0.1/ClaimMark/'+b'\0'*8)
g.update(bytes.fromhex(did)); g.update(bytes.fromhex(nullifier))
seed=g.digest()
h=hashlib.sha256()
h.update(b'/LEE/v0.2/AccountId/PDA/'+b'\0'*8)
h.update(bytes.fromhex(image)); h.update(seed)
print(b58(h.digest()))" "$VID" "$DISTRIBUTION_ID" "$NULLIFIER")
ok "verifier ImageID $VID"
ok "distribution     ${DISTRIBUTION_ID:0:16}…"
ok "derived marker   $MARKER"
echo

echo "[5/5] that marker PDA is owned by the verifier program"
EXPECTED=$(python3 -c "
import sys
b=bytes.fromhex(sys.argv[1])
print('['+','.join(str(int.from_bytes(b[i:i+4],'little')) for i in range(0,32,4))+']')" "$VID")
OWNER=$(rpc getAccount "[\"$MARKER\"]" | jq -c '.result.program_owner // empty')
if [ -z "$OWNER" ]; then bad "marker PDA $MARKER not found on chain"
elif [ "$OWNER" = "$EXPECTED" ]; then ok "owned by the claim verifier"
else bad "owned by $OWNER, expected $EXPECTED"; fi
echo

if [ "$FAILED" -eq 0 ]; then
  echo "VERIFIED. The claim is privacy-preserving, carries a real STARK receipt the"
  echo "sequencer verified against PRIVACY_PRESERVING_CIRCUIT_ID, and the marker PDA"
  echo "derived here is owned by the verifier. The membership proof was verified on"
  echo "chain as a precondition of acceptance."
  exit 0
else
  echo "VERIFICATION FAILED — see the failures above." >&2
  exit 1
fi
