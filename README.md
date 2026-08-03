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

## Deployed on the public testnet

Both programs are live on the public LEZ testnet. A deploy tx hash is
`SHA256(borsh(bytecode))` — content addressed — so the committed binaries under
`artifacts/programs/` hash to exactly these transactions (reproduce with
`scripts/verify-onchain-claim.sh` step 1):

| Program | ImageID | Deploy tx |
|---|---|---|
| Claim program (LEZ-native, `claim_lez.bin`) | `1f6a0ec0…eb249508` | `5a200887…10c5dcc9` |
| Claim verifier (SPEL, `claim_verifier.bin`) | `a7b7cf26…6fe7b77d` | `9e0a1929…a52d5580b` |

Three distributions are committed under the verifier and **23 privacy-preserving
claims** are landed against them; the full `(distribution, claim tx, nullifier)`
manifest is [`artifacts/e2e/claims.tsv`](artifacts/e2e/claims.tsv). To deploy the
two programs yourself from the committed bytecode:

```
wallet deploy-program artifacts/programs/claim_lez.bin
wallet deploy-program artifacts/programs/claim_verifier.bin
```

Then `scripts/deploy-and-claim.sh` runs the whole create-then-claim flow against
the testnet (real proving, funded account needed). The exact env vars, the sync
step, and how to re-verify are in [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md).

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

- **CLI, step by step.** Build the tool, then build a padded distribution and
  claim a row from only the published bundle and a recipient secret:

  ```
  cargo build --release -p airdrop-cli          # produces target/release/airdrop
  ID=de00000000000000000000000000000000000000000000000000000000000001
  airdrop demo-distribution --count 5 --id "$ID" --pad 8 --out ./dist   # bundle padded to 8 rows
  NSK=$(python3 -c "import json;print(json.load(open('dist/recipients.json'))[2]['nsk_hex'])")
  airdrop claim-from-bundle --dir ./dist --nsk "$NSK" --out ./claim.args
  ```

  `claim.args` holds the nullifier, marker seed, and proof witness for that
  recipient — the same values the on-chain claim consumes. A secret that is not in
  the set opens no row and the command refuses.
- **Basecamp app, step by step.** `app/lp-0003-airdrop.lgx` carries a
  `darwin-arm64` and a `linux-amd64` variant. Install it into Basecamp's plugins
  directory and launch; the `lp-0003-airdrop` tile drives the same `airdrop` CLI,
  so it computes the same commitments as the chain. Install commands and the load
  contract are in [`app/README.md`](app/README.md).
- **Full lifecycle against a real sequencer, from nothing.**
  `scripts/e2e-local-sequencer.sh` starts its own standalone `sequencer_service`,
  deploys both programs, commits a distribution, and submits a real
  `RISC0_DEV_MODE=0` privacy claim it then verifies over RPC — no funded account,
  no public testnet. It needs a `logos-execution-zone` checkout to build the
  sequencer from (`git clone https://github.com/logos-blockchain/logos-execution-zone _external/lez`,
  or point `LEZ_SRC` at an existing one); the script header lists the env vars.
  This is what the `.github/workflows/e2e-local-sequencer.yml` CI job runs, which
  clones LEZ for you. The zero-dependency clean-clone demo is `scripts/demo.sh`
  above.
- **Full create-then-claim on the public testnet** (real proving, funded account
  needed): `scripts/deploy-and-claim.sh`; see [`docs/DEPLOYMENT.md`](docs/DEPLOYMENT.md).

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
