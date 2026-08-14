# Solution: LP-0003 — Private Allowlist / Airdrop Distributor

**Submitted by:** edenbd1

## Summary

A private airdrop distributor for LEZ. A distributor commits an eligibility
set's Merkle root on chain; an eligible recipient claims their allocation on the
privacy-preserving path, and the only public trace is a marker PDA seeded by a
secret-bound nullifier. An observer who knows every candidate address still
cannot tell which of them claimed, and no address is ever revealed.

The membership proof is genuinely verified on chain. `claim` declares a
`ChainedCall` to a LEZ-native claim program, so LEZ's privacy circuit composes it
with a real `env::verify` and the sequencer checks the receipt against the
node-pinned `PRIVACY_PRESERVING_CIRCUIT_ID`. The eligibility root is anchored by
PDA address — `create_distribution` initialises the account seeded by
`[distribution_id, root]`, so an invented root resolves to an account nobody ever
created and the claim is rejected.

Everything is live on the public LEZ testnet: two programs deployed, two
distributions committed, and **23 privacy-preserving claims** landed and
independently verifiable. From a clean clone, `./scripts/verify-onchain-claim.sh`
re-checks any one of them over JSON-RPC.

## Repository

- **Repo:** <https://github.com/edenbd1/lp-0003-private-airdrop-distributor> — dual-licensed MIT OR Apache-2.0
- **Narrated demo video:** <https://youtu.be/rtCGv0zqC4I>
- **Solution write-up:** this document
- **Deployment + how to re-verify:** [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md)
- **Basecamp app:** `app/lp-0003-airdrop.lgx` (SHA-256 `cd999028b4461539880c19f755c1ab2fe667c2c2c2e1c03dbd6419c154b08955`), with both `darwin-arm64` and `linux-amd64` variants

## Public-testnet deployment

Both programs are deployed on the public LEZ testnet. A LEZ program-deployment tx
hash is `SHA256(borsh(bytecode))` — content addressed — so the committed binaries
under `artifacts/programs/` hash to exactly these deploy transactions, which is
what `scripts/verify-onchain-claim.sh` step 1 checks.

| Program | ImageID | Deploy tx |
|---|---|---|
| Claim program (LEZ-native) | `e9843420…67b57449` | `59c2160b40c5d0f4cce01fd89e7755dbafd9c7d088a071d6f6fd3a10cbbea7c5` |
| Claim verifier (SPEL) | `31edc17c…a110385d` | `7b16e471b35ce8c718e066d80a8198f8831ebc7b6704583ddd28bb287092e34c` |

Two distributions are committed under the current verifier, and 23 claims are
landed against them (12 + 11). A distribution's PDA is derived from
`[distribution_id, eligibility_root]`, so the root is its on-chain identity:

| Distribution | id | recipients | eligibility root |
|---|---|---|---|
| 1 | `b1…0001` | 12 | `e48c5f4f…ea6a45bcc` |
| 2 | `b2…0002` | 11 | `7b5078e8…9ed4eb44a` |

The full list of (distribution, claim tx, nullifier) is committed at
[`artifacts/e2e/claims.tsv`](artifacts/e2e/claims.tsv). A claim is a
**privacy-preserving** transaction, so it publishes no `program_id` or
`instruction_data`. It is live over RPC (`getTransaction`) immediately; the block
explorer is a separate index and reaches it later, so on a recent hash the RPC is
the source of truth (measured, see [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md)).
Once indexed, the explorer shows the claim's type, its proof size and the marker
PDA — and nothing naming a distribution or an address. Verify any claim from
public data over JSON-RPC:

```bash
CLAIM_TX=441ccd15e7b5eac388a0849481e95db409f1b6f23a202b6ee1a3ce37ae112c86 \
NULLIFIER=3db769e851c291d82cb79d717f1256710bb67b06a50cc52bea3f4ae1fea32b99 \
DISTRIBUTION_ID=b100000000000000000000000000000000000000000000000000000000000001 \
./scripts/verify-onchain-claim.sh
```

which confirms the transaction is `PrivacyPreserving`, its receipt is a `Succinct`
STARK the sequencer verified, and the claim marker PDA is owned by the verifier:
the membership proof was verified on chain as a precondition of acceptance.

> This deployment targets **LEZ v0.2.4** (commit `47eba25`). Both programs are
> built against it, which is why these ImageIDs differ from any earlier v0.2.0 or
> v0.2.2 write-up, and why the deploy transactions differ with them: a deploy hash
> is `SHA256(borsh(bytecode))`, so a new ImageID is a new deploy. The ImageID moves
> even where the dependency sources do not, because a guest's panic messages embed
> the cargo checkout path, and that path contains the pinned revision. The SPEL
> framework has no release targeting a current LEZ, so `vendor/spel` carries the
> upstream v0.6.0 sources repinned and ported to v0.2.4 (see `vendor/spel`).

## Approach

### The decision that shaped everything: which transaction path

The first thing I checked was whether a LEZ **public** transaction verifies a
proof. It does not — the sequencer re-executes the program host-side
(`lee/state_machine/src/program/mod.rs:73-77`, commented *"Execute the program
(without proving)"*). An airdrop built there would be a membership check wearing a
zero-knowledge costume, which is the ground on which earlier submissions in this
program were rejected.

The path that works is the **privacy-preserving transaction**: the client proves
locally, LEZ's privacy circuit composes each chained call with a real
`env::verify` (`lee/privacy_preserving_circuit/src/execution_state.rs:149-155`), and
the sequencer verifies the receipt against the pinned circuit id. For that
composition to happen the callee must *be* a LEZ program emitting a
`ProgramOutput`, which is why `claim_lez` exists in the shape it does rather than
as a plain guest.

It also has a privacy consequence I depend on: a privacy `Message` publishes
neither `program_id` nor `instruction_data`, so the witness — which contains the
recipient's secret `nsk` — can travel in the instruction. On the public path the
same bytes would be published verbatim, so the claim must never be submitted
there. The tooling never does.

### Anchoring the eligibility root, and the attack it closes

A membership proof establishes membership against whatever root the statement
names, which on its own is worthless: anyone can build a one-leaf tree holding
themselves. So the eligibility root is **anchored by address**. `create_distribution`
initialises a PDA seeded by `[distribution_id, root]`, and `claim` requires that
PDA to be owned by the verifier. An invented root gives a different address that
was never created, whose owner is the default — rejected (`4003`). There is no
code path that trusts a caller-supplied root without this anchor.

### Secret-bound nullifiers, and one marker per claim

Each claim occupies a PDA seeded by
`compute_claim_marker(distribution_id, nullifier)`, where the nullifier is
`SHA256(CLAIM_NULLIFIER_PREFIX ‖ distribution_id ‖ nsk)`. The marker is `init`, so
a second claim by the same recipient targets an occupied PDA and the on-chain
constraint refuses it with `AccountAlreadyInitialized` — demonstrated both against
the deployed binary (`claim-verifier-tests`) and live on chain. Because the
nullifier commits to `nsk`, an observer who knows every candidate address cannot
compute it, so a marker cannot be mapped back to an address (`4002` guards a
forged nullifier). Because both the nullifier and the marker seed commit to
`distribution_id`, a claim from one distribution cannot be replayed against
another.

### Binding the destination

`claim` carries a `destination` and enforces `witness.destination ==
statement.destination` inside the proof, so whoever holds only a proven claim
cannot redirect the allocation without re-proving, which needs the secret. Stated
precisely: the destination is committed **inside the proof** but is **not**
published and **not** folded into the marker seed, so it is useful to a delivery
step running in the same transaction, not as a standalone on-chain record. This
scope is stated plainly in [`docs/limitations.md`](docs/limitations.md).

### The encrypted bundle: delivery without a side channel

The distributor does not need a private channel to each recipient. It publishes
one `bundle.json`: each recipient's claim data (allocation, salt, leaf index,
Merkle path) sealed to an X25519 key derived from that recipient's `nsk` through a
domain separator distinct from the nullifier and account key, authenticated with
ChaCha20-Poly1305. An observer sees only ciphertext and ephemeral keys. A
recipient trial-opens each row and keeps the one whose payload reconstructs a leaf
that anchors to the committed root — so a malicious distributor cannot make them
prove a leaf that is not in the set, and a junk row injected into the open bundle
is skipped rather than adopted. Non-contributory X25519 shared secrets are
rejected, so a crafted low-order ephemeral header cannot collapse every
recipient's key to the same value. `--pad` rounds the row count up with dummy rows
that open for no one, hiding the recipient count.

### What did not work

- **Storing the eligibility root in account data.** Folding it into the
  distribution PDA address is strictly stronger: it makes an invented root
  *unrepresentable* (a never-created address) rather than merely checked.
- **Anchoring the distribution by mutating an existing account.** SPEL's
  `IntoPostState` is implemented for the original account binding, not a rebound
  `let`; the clean construction anchors by PDA seed and writes no data.
- **Enabling risc0's `prove` feature** so tests prove in-process. It drags in GPU
  backends (Metal on macOS) that most machines lack; CI installs `r0vm` and the
  adversarial suites execute through the sequencer's own executor instead.

### Why the Logos stack

The design rests on two properties nothing centralised provides. First,
**trustless execution with real proof composition**: eligibility is enforced by a
circuit the sequencer verifies, not by a server that could be asked to lie about
who claimed. Second, **shielded accounts as a first-class primitive**: a claim is
bound to a secret that never appears on chain, so unlinkability holds against
anyone who knows the candidate set, not merely against a passer-by. A centralised
airdrop service can hide the recipient list from the public; it cannot prove to a
third party that it distributed honestly, and it cannot hide who claimed from
itself.

## Design topics

Named here so each is findable, with the deeper treatment linked.

### Commitment scheme

Each eligible recipient is a Merkle leaf
`compute_eligibility_leaf(account_id, allocation, salt) = SHA256(ELIGIBILITY_LEAF_PREFIX ‖ account_id ‖ allocation_le ‖ salt)`,
where `account_id = derive_account_id(derive_npk(nsk), identifier)`. The
distributor commits only the Merkle root on chain via `create_distribution`, in a
PDA seeded by `[distribution_id, root]`. Allocations and the leaf set stay private;
they can be published encrypted (`crates/airdrop-crypto`) without revealing who is
eligible.

### Claim-uniqueness mechanism

A secret-bound nullifier
`compute_claim_nullifier(distribution_id, nsk) = SHA256(CLAIM_NULLIFIER_PREFIX ‖ distribution_id ‖ nsk)`
seeds a marker PDA `compute_claim_marker(distribution_id, nullifier)`, created with
`init`. A second claim by the same recipient targets an occupied PDA and is refused
(`AccountAlreadyInitialized`). The nullifier is deterministic per
`(distribution, recipient)`, so each recipient claims exactly once, and because it
commits to `nsk` an observer cannot compute it.

### Trusted setup

**None.** The proof system is Risc0, a STARK, which is transparent: no ceremony,
no structured reference string, no toxic waste. On-chain verification is the LEZ
privacy circuit composing the chained `env::verify` over a **Succinct STARK**
receipt (`verify-onchain-claim.sh` step 3 asserts the receipt is Succinct, not a
Groth16 wrap and not a dev-mode fake), so no trusted setup enters the trust base.

### LEZ account model compatibility

The scheme is built on LEZ shielded accounts. A recipient's `account_id` derives
from their secret `nsk` through the nullifier public key (`npk`), and the claim is
a privacy-preserving transaction that spends the signer's commitment and publishes
no `program_id` or `instruction_data`. The witness (including `nsk`) travels only
on the privacy path. See [`docs/privacy-model.md`](docs/privacy-model.md).

### Security assumptions

(a) SHA-256 collision/preimage resistance for the leaf, root, nullifier, and
marker derivations; (b) Risc0 STARK soundness and the LEZ privacy circuit's
`env::verify` composition, which the sequencer checks against the node-pinned
`PRIVACY_PRESERVING_CIRCUIT_ID`; (c) the recipient keeps `nsk` secret; (d) X25519 +
ChaCha20-Poly1305 for the optional encrypted bundle. Trust boundary: the
distributor chooses the eligible set (it is trusted for *who is eligible*) but is
**not** trusted for claim data — the recipient re-checks that the decrypted leaf
anchors to the committed root before proving, so a malicious distributor cannot
make a recipient prove a leaf that is not in the set.

### Known limitations and integration

Stated in [`docs/limitations.md`](docs/limitations.md): the encrypted bundle
reveals cardinality unless padded (and padding hides count from a counter, not a
length-measurer); the destination is bound in the proof but not in the marker
seed; `create_distribution` is permissionless, so an integrator must key
proof-of-claim on `(distribution_id, root)`, not `distribution_id` alone; two
non-exploitable hardenings remain open, each for a reason of its own.
To integrate: commit a root with `create_distribution`, publish the bundle, and
have each recipient run `claim-from-bundle` then submit on the privacy path; gate
downstream on the marker PDA owned by the verifier, checked as in
`scripts/verify-onchain-claim.sh`.

## Success Criteria Checklist

### Functionality

- [x] **A distributor commits an eligibility set on chain without revealing the
      addresses.** `create_distribution` commits only the Merkle root, in a PDA
      seeded by `[distribution_id, root]`. The set can also be published
      **encrypted** (`crates/airdrop-crypto`), so no side channel is needed.
- [x] **An eligible recipient claims without revealing which address.** `claim` is
      a membership proof on the privacy path; the witness never reaches the
      journal, and a privacy tx publishes no instruction data.
- [x] **No double-claim.** Marker PDA seeded by
      `compute_claim_marker(distribution_id, nullifier)`; `init` refuses the
      second claim. Tested against the deployed binary and live on chain.
- [x] **An on-chain observer cannot link a claim to an address.** Secret-bound
      nullifier; see [`docs/privacy-model.md`](docs/privacy-model.md).
- [x] **Full privacy model documented**, with threat model and named residual
      leakage. [`docs/privacy-model.md`](docs/privacy-model.md),
      [`docs/limitations.md`](docs/limitations.md).
- [x] **Reference integration on testnet.** `scripts/deploy-and-claim.sh` runs the
      full create-then-claim flow; its result is the live claims in
      [`artifacts/e2e/claims.tsv`](artifacts/e2e/claims.tsv).
- [x] **≥2 distributions, ≥20 claims.** 2 distributions, 23 live claims (12 +
      11), each verifiable with `scripts/verify-onchain-claim.sh`.
- [x] **Full documentation and a clean public repository.**

### Usability

- [x] **Module/SDK for building Logos modules.** `crates/airdrop-core` (the claim
      primitive) and `crates/airdrop-cli` (the `airdrop` tool).
- [x] **Basecamp app GUI with local build instructions and a loadable asset.**
      `app/`, packaged as `app/lp-0003-airdrop.lgx` with both a `darwin-arm64` and
      a `linux-amd64` variant. Verified loading in LogosBasecamp 0.2.2
      (`Successfully loaded UI module: "lp-0003-airdrop"`) and driven to a built
      claim; see [`app/README.md`](app/README.md).
- [x] **SPEL IDL.** `idl/claim_verifier.idl.json`, both instructions.

### Reliability

- [x] **Proof-generation failures surface a clear error.** `claim-from-bundle`
      verifies the witness locally before emitting anything, so a bad witness fails
      in microseconds rather than after proving.
- [x] **A failed claim does not mark the recipient as claimed.** The marker is
      written only on a successful claim.
- [x] **Deterministic, documented error codes.** `4001`–`4006` in
      [`docs/error-codes.md`](docs/error-codes.md), each mapped to the attack it
      stops and the test that proves it.

### Performance

- [x] **CU cost of each on-chain operation documented.**
      [`docs/benchmarks/cu-budget.md`](docs/benchmarks/cu-budget.md): `claim` is
      318,242 user cycles / 524,288 proving cycles = 1.56% of the public budget,
      measured by replaying through the sequencer's own executor and reproducible
      with one command. `create_distribution` is lighter (a single PDA init).

### Supportability

- [x] **Deployed and tested on LEZ testnet.** Two programs deployed; three
      distributions and 23 claims live and independently re-verifiable over
      JSON-RPC.
- [x] **E2E tests against a LEZ sequencer, in CI.** `claim-verifier-tests` runs the
      built `claim_verifier.bin` through the sequencer's own executor — same
      executor, same input order, same 32M session limit — on every push, and a
      separate job fails if a program ImageID drifts from its committed binary.
- [x] **CI green on the default branch.**
- [x] **README documents end-to-end usage**, and `./scripts/demo.sh` runs from a
      clean clone with `RISC0_DEV_MODE=0`, no funded account needed.
- [x] **Recorded narrated video demo.** <https://youtu.be/rtCGv0zqC4I> — the
      terminal is on screen throughout, `RISC0_DEV_MODE=0` is visible before any
      proving starts, and the claim is proved and submitted to the public testnet
      during the recording. It is a single take, and nothing is cut out: the
      stretch where the proof runs and nothing on screen changes is played at
      eight times speed, and the on-screen clock reports the real elapsed time.

## FURPS Self-Assessment

### Functionality

Two instructions: `create_distribution` and `claim`. Four documented bindings —
anchored root, secret-bound nullifier, one-marker-per-claim, destination — each
with adversarial coverage. **Limitations, stated rather than buried:** the
destination is bound in the proof but not published or in the marker seed; the
encrypted bundle reveals the recipient count unless padded, and padding hides the
count from a counter but not from an observer measuring row length; `create_distribution`
is permissionless, so an integrator should key proof-of-claim on
`(distribution_id, root)`, not `distribution_id` alone.

### Usability

Two surfaces over one library: the `airdrop` CLI and the Basecamp app, both
computing the same commitments from the same code. Onboarding is
`git clone && ./scripts/demo.sh`. The CLI's refusals are written to be read by a
human and explain why. The Basecamp `.lgx` ships both a `darwin-arm64` and a
`linux-amd64` variant, so it loads on macOS and Linux; the CLI is the
cross-platform path beyond those.

### Reliability

Local pre-verification before proving; a marker written only on success;
deterministic error codes `4001`–`4006`. The scripts confirm a claim by polling
`getTransaction` over RPC rather than by trusting the block explorer, which does
not hold every transaction the RPC does (see below), so a landed claim is never
mistaken for a failed one.

### Performance

`claim` at 1.56% of the public compute budget, in a single segment. Wall-clock is
dominated by proving, and runs to minutes rather than seconds with
`RISC0_DEV_MODE=0`. It is deliberately not quoted as one fixed number: it depends
on the machine and on what else is competing for its cores, and the same claim
measurably slows when an unrelated build is running. `scripts/prove-one-claim.sh`
times it and prints what it measured, and the recorded demo shows that clock
running rather than asserting a figure. That latency is the real constraint on a
lifecycle run and is stated as such rather than hidden behind the cycle count.

### Supportability

30 tests across the workspace plus an executor-level suite against the deployed
binary, all green in CI, which runs the adversarial e2e unconditionally and fails
on any ImageID drift. The two guest crates are excluded from the host workspace
because they target `riscv32im-risc0-zkvm-elf`, but the deployed verifier is still
under test because `claim-verifier-tests` exercises the built binary — including
the anchoring check, with a case that a weaker form of it would let through.

## Supporting Materials

- [`docs/privacy-model.md`](docs/privacy-model.md) — threat model, what is hidden
  from whom, what is deliberately public, residual leakage
- [`docs/limitations.md`](docs/limitations.md) — scope boundaries and deferred
  hardenings
- [`docs/error-codes.md`](docs/error-codes.md)
- [`docs/benchmarks/cu-budget.md`](docs/benchmarks/cu-budget.md)
- [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md) — every tx hash and how to re-verify
- [`docs/onchain-audit.md`](docs/onchain-audit.md) — live adversarial demos
- Narrated demo video: <https://youtu.be/rtCGv0zqC4I>

## What the testnet run taught, and what it cost

Two things went wrong on the way, both worth stating because they are invisible
until you hit them and neither is a program bug:

1. **A claimant account must be re-synced before each claim.** A privacy
   transaction spends the signer's commitment, so a second claim built against a
   stale commitment is dropped by the sequencer. `account sync-private` before
   each claim fixes it, and `scripts/deploy-and-claim.sh` does so.
2. **The block explorer and the privacy design are two separate things, and
   conflating them weakens the second.** (a) The explorer lags the sequencer:
   measured, a deploy confirmed on chain at 02:15 was absent from the explorer at
   03:51 and present at 04:07, while a cannot-exist hash returns "not found" at
   every point — so "not shown" on a recent hash means "not indexed yet", not
   "dead", and `getTransaction` returns it immediately. That is an indexing delay,
   unrelated to the design. (b) Independently, a privacy transaction publishes no
   `program_id` or `instruction_data`, so it is unattributable however good the
   indexer is — which is the point of the scheme, and which the explorer itself
   demonstrates: an indexed claim renders its type, its proof size and the marker
   PDA, and nothing that names a distribution or an address.
   `scripts/verify-onchain-claim.sh` establishes it from the marker PDA, and
   [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md) shows the full measurement.

## Terms & Conditions

By submitting this solution, I confirm that I have read and agree to the
[Terms & Conditions](../TERMS.md).
