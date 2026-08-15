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

## The block explorer does not show every transaction, and privacy is separate

Two independent facts, kept apart because conflating them weakens the second:

1. **The explorer lags the sequencer.** The public testnet's explorer is a
   separate index and reaches a transaction later than the RPC does. Measured on
   this deployment: a deploy confirmed on chain at 02:15 was still absent from the
   explorer at 03:51 and present at 04:07, and every claim sampled from the
   previous run is indexed. So "not shown" on a fresh hash means "not indexed
   yet", not "dead": `getTransaction` over RPC returns each live transaction
   immediately, and `null` for a hash that cannot exist, which makes the RPC the
   source of truth for anything recent. `scripts/check-explorer.py` reproduces the
   measurement by rendering the pages headless against an impossible hash as the
   control. It renders because the explorer used to be a WASM application serving
   an identical shell for every hash, which `curl` could not probe; re-measured
   2026-08-15 it server-side renders, so `curl` now separates a real transaction
   (~366 kB, carrying `Type:` and `Proof Size:`) from one that cannot exist (2416
   bytes, `Transaction not found`). Rendering is kept as the second opinion: it
   reads the DOM a reviewer sees rather than a byte count.
2. **A privacy claim is unattributable by construction.** Independently of any
   indexer, a privacy-preserving transaction publishes no `program_id` and no
   `instruction_data`. Even a perfect explorer could show only that a privacy
   transaction occurred, never which distribution or address it concerns. That is
   the point of the scheme, and it is what `scripts/verify-onchain-claim.sh`
   establishes from the marker PDA, which no explorer can show or forge.

## Proving cost

A privacy-preserving claim proves locally before submission, and that takes
minutes rather than seconds with `RISC0_DEV_MODE=0` — proving the chained guest
plus the privacy circuit's composition of its receipt being nearly all of it. The
exact figure is a property of the machine, not of the design, and moves
substantially with contention, so `scripts/prove-one-claim.sh` prints the one it
measured rather than this document quoting a number that would not reproduce.
Deployments sensitive to first-claim latency should prove in the background and
submit when ready.

## Set size and tree depth

The eligibility tree is a binary SHA-256 Merkle tree; proof size and in-guest
folding cost grow with `log2(set size)`. The tree builder pads to a power of two
with a domain-separated sentinel that commits to no account, so padding entries
can never be claimed.

## Deferred hardenings (non-exploitable)

An adversarial review confirmed no bypass of the claim's security properties. The
items below are defence-in-depth and none is exploitable as built. Each changes a
guest ImageID, and that is not free: a distribution PDA and a marker PDA are both
derived from the verifier's program id, so redeploying the verifier re-keys every
committed distribution and invalidates the live claims. They are recorded here so
the trade-off is explicit rather than hidden.

The migration to LEZ v0.2.4 forced that re-key anyway, which made it the moment to
fold in the one hardening whose cost was otherwise the only thing deferring it:

- ~~**Anchor check compares against `DEFAULT` rather than `self`.**~~ **Done.**
  `claim` now requires the distribution PDA's owner to equal `self_program_id`,
  rather than merely to be non-default. The weaker form was already safe — a PDA
  address embeds the program id, so an address in this program's namespace can
  only be owned by `DEFAULT` or by this program — but the guarantee now rests on
  the check itself instead of on that derivation argument.
  `a_distribution_owned_by_another_program_is_rejected` covers it. Run against the
  pre-hardening binary that test fails — the attack is accepted there — which is
  what makes it a test of this change rather than of the PDA derivation.

Two remain, each for a reason of its own rather than for the re-key cost:

- **Reject the public path in the verifier program.** The claim carries `nsk` in
  its witness and is safe only on the privacy path (see
  [`privacy-model.md`](privacy-model.md)). The program does not itself refuse a
  public-path invocation; the tooling never issues one. The obvious in-guest test
  does not work: `caller_program_id` is the default for a top-level instruction,
  which a claim is on either path, so it does not separate them. Making this
  structural needs a discriminator LEZ does not currently expose to the guest.
- **Domain-separate internal Merkle nodes.** Leaves are prefixed
  (`ELIGIBILITY_LEAF_PREFIX`); internal nodes are a bare `SHA256(L || R)`. This is
  not exploitable because the guest always *recomputes* the leaf from the account,
  allocation, and salt and never accepts a prover-supplied leaf value, so an
  internal node cannot be presented as a leaf. Prefixing internal nodes would keep
  that safe even if a future code path accepted a raw leaf.
