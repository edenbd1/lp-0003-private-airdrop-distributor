# Deployment

Every tx hash here is independently verifiable on the public Logos Execution Zone
testnet via `getTransaction`, or by `./scripts/verify-onchain-claim.sh`.

```
Network:            Public LEZ testnet
Sequencer JSON-RPC: https://testnet.lez.logos.co
LEZ version:        v0.2.0
spel:               v0.6.0
cargo-risczero:     3.0.5
```

## Deployed programs

A LEZ program-deployment tx hash is `SHA256(borsh(bytecode))`, content addressed,
so redeploying the byte-identical binary reproduces the identical hash. The two
binaries under `artifacts/programs/` therefore hash to exactly these deploy
transactions, which is what `scripts/verify-onchain-claim.sh` step 1 checks.

| Program | ImageID | Deploy tx |
|---|---|---|
| Claim program (LEZ-native, `claim_lez.bin`) | `744041c6…6e576c62` | `94215cf71cabc3d8144ed2a138e957b18224e46b13eac04bab2c87398de81225` |
| Claim verifier (SPEL, `claim_verifier.bin`) | `66ea2b79…e1dba7db` | `ffd449d8d4ff95e46e27b0f38a8a468c3179bd6ede57ce1cf41258504870e430` |

## Distributions and claims

Two distributions are committed on chain with `create_distribution`; each commits
only its eligibility root, in a PDA whose address is `[distribution_id, root]`.

| Distribution | id | recipients | create_distribution tx | distribution PDA (owned by verifier) |
|---|---|---|---|---|
| 1 | `a1…0001` | 12 | `49ea47687885df9a7259d4b149cebab60e99b070c481fdac5e751478c0645274` | `6DSxTiaCfZkCp2yxuDbLz2j8da8VYkvimZpx1wQk5uWy` |
| 2 | `a2…0002` | 10 | `ecc5782fa230e4304b81b5471ab4b02cf59a03e98031dfbefc19f060a40521d1` | (derived from its root) |

A claim is a **privacy-preserving** transaction: its instruction data is not
published, and its only public trace is the claim marker PDA, owned by the
verifier, seeded by the distribution and the recipient's nullifier. A verified
example claim:

```
claim tx (privacy):  a5e27bcc1d524316e54cd7b535246918b5ceb171abbc26cb51ecc9b82bf02d46
distribution:        a1…0001, recipient 5
nullifier:           a2380fe7a1f17a6433bdd8a2dd417ce8408ad56e9ae64f6c57d89ac910a37731
claim marker PDA:    FsKFPQqg1qveHAuTmWMs2KfBw79MqPk2F8zP5pEYzLEq  (owned by the verifier)
```

Verify it from public data over RPC:

```bash
CLAIM_TX=a5e27bcc1d524316e54cd7b535246918b5ceb171abbc26cb51ecc9b82bf02d46 \
NULLIFIER=a2380fe7a1f17a6433bdd8a2dd417ce8408ad56e9ae64f6c57d89ac910a37731 \
DISTRIBUTION_ID=a100000000000000000000000000000000000000000000000000000000000001 \
./scripts/verify-onchain-claim.sh
```

which confirms the transaction is `PrivacyPreserving`, its receipt is a `Succinct`
STARK the sequencer verified, and the marker PDA is owned by the verifier: the
membership proof was verified on chain as a precondition of acceptance.

> **The claimant's private account must be synced before each claim.** A
> privacy transaction spends the signer's commitment, so the wallet must
> `account sync-private` before building the next claim, or the membership proof
> is against a stale commitment and the sequencer drops the transaction.
> `scripts/deploy-and-claim.sh` syncs before each claim for this reason.

## How to reproduce

Pre-requisites: macOS arm64 or Linux with `cargo`, `docker`, and the Logos
toolchain (`wallet` and `spel` from LEZ/spel at the versions above).

```bash
# 1. Build the two guests (content-addressed; the committed binaries are these).
cargo risczero build --manifest-path crates/claim-circuit/methods/guest-lez/Cargo.toml
cargo risczero build --manifest-path crates/claim-verifier-spel/methods/guest/Cargo.toml

# 2. Deploy them (reproduces the hashes above).
wallet deploy-program artifacts/programs/claim_lez.bin
wallet deploy-program artifacts/programs/claim_verifier.bin

# 3. Build a distribution and its recipient claim packages.
cargo run -p airdrop-cli --bin airdrop -- demo-distribution \
  --count 12 --id <hex32> --out dist/

# 4. Commit the eligibility root on chain, then submit a claim per recipient.
./scripts/deploy-and-claim.sh   # see the script header for env vars
```

> The wallet may print `Transaction NOT confirmed` for a privacy-preserving
> claim whose proving outruns its polling window; the transaction lands anyway.
> Check with `getTransaction`, not the CLI verdict.

## Signer

Deployed from the wallet signer used for the LP-0005 work; the deploy
transactions are content-addressed and independent of the signer.
