# Known limitations

Stated plainly, so a reviewer does not have to find them.

## The claim marker records eligibility; the destination is bound in the proof

A successful claim leaves a marker PDA, owned by the verifier, seeded by the
distribution and the recipient's nullifier. That marker is proof-of-eligibility:
it says "this recipient claimed this distribution, once." The claim also carries a
`destination`, and it is bound into the proof: the chained claim program requires
`witness.destination == statement.destination`, so whoever holds only the proven
claim cannot redirect the allocation without re-proving, which needs the secret.
Actually moving `N` tokens to that destination is the integration's job; what the
primitive guarantees is that the destination a claim commits to cannot be changed
by the submission.

## Unlinkability is against passive observers, not voluntary disclosure

The nullifier is secret-bound, so an observer who knows the candidate set cannot
link a claim to an address. It does **not** protect against a recipient who
chooses to reveal their own `nsk`, or who claims and then publicly associates
themselves with the marker. Voluntary self-identification is out of scope.

## The distributor learns the candidate set by construction

An allowlist has a curator, and the curator chooses the addresses. So the
distributor knows who *could* claim. What it does not learn is who *does* claim,
or when. If the requirement is that even the distributor cannot enumerate
candidates, that is a different primitive (a permissionless, self-registering
set) and out of scope here.

## The privacy-preserving claim is not shown on the block explorer

A claim is a privacy-preserving transaction. The public testnet's indexer does
not display privacy-preserving transactions, so a claim's explorer page reads
"not found" and the marker account page can show a stale default owner even
though `getTransaction` and `getAccount` report the real state over RPC. This is
a Logos indexer limitation, not a dead transaction. It is also intrinsic to what
the claim proves: the privacy path is the only one that verifies a proof on
chain, and it is the one the explorer does not index. Verification therefore
reads the chain over RPC, which is what `scripts/verify-onchain-claim.sh` does.

## Proving cost

A privacy-preserving claim proves locally before submission; on Apple Silicon CPU
this is on the order of seconds for the guest plus the privacy circuit's
composition. Deployments sensitive to first-claim latency should prove in the
background and submit when ready.

## Set size and tree depth

The eligibility tree is a binary SHA-256 Merkle tree; proof size and in-guest
folding cost grow with `log2(set size)`. The tree builder pads to a power of two
with a domain-separated sentinel that commits to no account, so padding entries
can never be claimed.
