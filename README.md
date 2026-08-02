# LP-0003 — Private Allowlist / Airdrop Distributor

A reusable primitive for the Logos Execution Zone: a distributor commits to an
eligibility set on chain, and an eligible recipient claims their allocation by
proving membership in zero knowledge, without revealing which address they hold.
The claim proof is verified **on chain** through the LEZ privacy-preserving
transaction path, and a double-claim is prevented by a secret-bound nullifier.

## What it guarantees

- **A distributor commits an eligibility set without publishing addresses.** The
  set is a Merkle root; only the root goes on chain.
- **An eligible recipient claims without revealing which entry is theirs.** The
  claim is a membership proof against the committed root.
- **A recipient cannot claim twice.** The claim marker is a PDA seeded by a
  nullifier bound to the recipient's secret, so a second claim targets the same
  address and fails.
- **An observer cannot link a claim to an address.** The nullifier is a function
  of a secret the recipient holds, so someone who knows every candidate address
  still cannot compute it.
- **A recipient cannot claim more than they were granted.** The allocation is
  sealed into the committed leaf.
- **The eligibility set can be published, encrypted.** The distributor posts one
  bundle of rows, each sealed to a key derived from the recipient's secret, so no
  side channel is needed and an observer learns neither who is eligible nor any
  allocation.

## Why the root is anchored, and why that matters

A membership proof is only as good as the root it is proved against. If the
claimant chose the root, they could invent a one-leaf tree holding themselves.
Here the distributor commits the root first: `create_distribution` initialises a
PDA whose address is derived from `[distribution_id, eligibility_root]`, so the
address itself is the commitment. `claim` references the distribution as the PDA
for `[distribution_id, distribution_root]` and requires it to be owned by this
program. An invented root lands on an uninitialised address and is rejected, so
the proved membership is always against the distributor's real on-chain
commitment.

## The two verification paths over one proof

- **On chain.** The claim verifier is a SPEL `#[lez_program]`
  (`crates/claim-verifier-spel`). Its `claim` instruction declares a
  `ChainedCall` to the LEZ-native claim program (`crates/claim-circuit`), so on
  the privacy-preserving path LEZ's circuit composes the membership proof with a
  real `env::verify` (`lee/privacy_preserving_circuit/src/execution_state.rs:149`)
  and the sequencer verifies the receipt against the pinned
  `PRIVACY_PRESERVING_CIRCUIT_ID`. No program on the public path could do this: a
  public transaction re-executes rather than proves
  (`lee/state_machine/src/program.rs:73-77`).
- **The primitive.** `crates/airdrop-core` holds the claim statement and its
  derivations, shared by the guest and the verifier so there is one source of
  truth for what a claim proves.

## Quickstart

From a clean clone, with a Rust toolchain (and, for step 3's deployed-binary
tests, the risc0 VM `r0vm` from `cargo risczero install`; the step is skipped with
a note if it is absent):

```
./scripts/demo.sh
```

runs the whole thing with `RISC0_DEV_MODE=0` and no funded account: the
adversarial suites (circuit, bundle, and the deployed binary through the
sequencer's executor), a padded distribution, a recipient opening their row from
only the bundle and their secret, the compute cost, and finally a live on-chain
verification of a deployed claim.

- **CLI.** The `airdrop` tool builds a distribution (`demo-distribution`, with
  `--pad` to hide the recipient count) and claims from only the published bundle
  and a recipient secret (`claim-from-bundle`).
- **Basecamp app.** `app/lp-0003-airdrop.lgx` (see [`app/README.md`](app/README.md));
  the GUI drives the same CLI, so it computes the same commitments as the chain.
- **Full create-then-claim on testnet** (real proving, funded account needed):
  `scripts/deploy-and-claim.sh`; see [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md).

## Verify it yourself

Every claim's on-chain evidence is checkable from public data, reading the chain
over RPC rather than trusting the block explorer:

```
./scripts/verify-onchain-claim.sh
```

It confirms both programs are the bytecode in this repository (deploy hashes are
`SHA256(borsh(bytecode))`, content addressed), that the claim transaction is
`PrivacyPreserving` and not `Public`, that its receipt is `Succinct` and not a
dev-mode fake, and that the claim marker PDA derived from the verifier ImageID
and the nullifier is owned by the verifier program.

## The adversarial audit

The security design is not asserted, it is tested against the deployed bytecode.
`crates/claim-verifier-tests` runs `claim_verifier.bin` through the sequencer's
own executor and requires each way of stealing a claim to be rejected, with an
honest claim accepted as the control:

| Attack | Rejected with |
|---|---|
| Prove membership against an invented root | `4003` not anchored |
| Substitute a nullifier of your choosing | `4002` nullifier mismatch |
| Claim more than your sealed allocation | `4006` allocation mismatch |
| Land the claim at a misrepresenting address | `4004` marker seed mismatch |

`crates/airdrop-core` adds the circuit-level audit: a non-member, an inflated
allocation, a cross-distribution replay, and a stolen leaf paired with a foreign
secret are each constructed and required to fail.

## Layout

| Component | Path | Role |
|---|---|---|
| Shared primitive | `crates/airdrop-core` | Eligibility leaf, claim statement, nullifier, marker seed, tree builder |
| Encrypted bundle | `crates/airdrop-crypto` | Publishable per-recipient encrypted eligibility rows (X25519 + ChaCha20-Poly1305) |
| Claim circuit | `crates/claim-circuit` | LEZ-native guest emitting a `ProgramOutput`, composed by `env::verify` |
| Claim verifier | `crates/claim-verifier-spel` | SPEL program: anchors the root, verifies the proof on chain, claims the marker |
| Verifier audit | `crates/claim-verifier-tests` | Runs the deployed binary through the sequencer's executor |
| Distributor / claim tooling | `crates/airdrop-cli` | Build a distribution, emit a recipient's claim arguments |

Deployment record and reproduction steps are in
[`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md); the privacy model in
[`docs/privacy-model.md`](docs/privacy-model.md); the deployed error codes in
[`docs/error-codes.md`](docs/error-codes.md); the honest boundaries in
[`docs/limitations.md`](docs/limitations.md).

## A note on the LEZ source citations

Paths like `lee/state_machine/src/program.rs:73-77` resolve against
`logos-execution-zone` at tag `v0.2.0`, the release the public testnet runs and
the one this project builds against.

## License

MIT OR Apache-2.0.
