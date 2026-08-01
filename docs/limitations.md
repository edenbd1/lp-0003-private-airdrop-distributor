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

Be precise about the scope: the destination is committed inside the proof, but it
is **not** published (a privacy transaction carries no instruction data) and it is
**not** folded into the marker PDA seed (the seed is
`H(prefix || distribution_id || nullifier)` only). So no on-chain observer or
later transaction can read the bound destination from the marker alone. The
binding is useful to a token-delivery step that runs in the **same** transaction
as the claim, where it can consume the proven destination; it is not a standalone
on-chain record an unrelated integrator can look up. Adding the destination to the
marker seed, or emitting a delivery in the claim itself, is the natural extension
for an integration that needs the destination to be publicly enforceable.

## The encrypted bundle reveals the number of recipients

`airdrop demo-distribution` emits one encrypted row per recipient. An observer of
the published bundle therefore learns the exact **cardinality** of the eligibility
set, even though they learn nothing about who is eligible or any allocation. If
the set size is itself sensitive, pad the bundle with dummy rows: `airdrop
demo-distribution --pad <n>` rounds the row count up to a multiple of `n` with
rows sealed to random keys that never open for anyone, so the published count no
longer reveals the real one.

Be precise about what padding hides: it defeats a **counter**, not a **measurer**.
A dummy row is a fixed length, while a real row's ciphertext grows with the tree
depth and the serialized allocation, so an observer who groups rows by length can
still separate real rows from padding and recover the true count. Full
length-indistinguishability would require sealing every row (real and dummy) at a
single constant plaintext length; the current padding hides the count only against
an observer who does not exploit row length.

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

## Deferred hardenings (non-exploitable, would re-key the deployment)

An adversarial review confirmed no bypass of the claim's security properties. The
items below are defence-in-depth, none of them exploitable as built, and each one
is deferred because it changes a guest ImageID: because a distribution PDA and a
marker PDA are both derived from the verifier's program id, redeploying the
verifier re-keys every committed distribution and invalidates the live claims.
They are recorded here so the trade-off is explicit rather than hidden.

- **Reject the public path in the verifier program.** The claim carries `nsk` in
  its witness and is safe only on the privacy path (see
  [`privacy-model.md`](privacy-model.md)). The program does not itself refuse a
  public-path invocation; the tooling never issues one. Enforcing it in-program
  (checking the caller against the privacy circuit id) would make the guarantee
  structural instead of procedural.
- **Anchor check compares against `DEFAULT` rather than `self`.** The claim
  requires the distribution PDA to be non-default-owned; it does not additionally
  assert the owner *is this verifier*. That is safe today because a PDA address
  embeds the program id, so a `[distribution_id, root]` PDA in this program's
  namespace can only be owned by `DEFAULT` or this program. The stricter check is
  a belt-and-suspenders nicety.
- **Domain-separate internal Merkle nodes.** Leaves are prefixed
  (`ELIGIBILITY_LEAF_PREFIX`); internal nodes are a bare `SHA256(L || R)`. This is
  not exploitable because the guest always *recomputes* the leaf from the account,
  allocation, and salt and never accepts a prover-supplied leaf value, so an
  internal node cannot be presented as a leaf. Prefixing internal nodes would keep
  that safe even if a future code path accepted a raw leaf.
