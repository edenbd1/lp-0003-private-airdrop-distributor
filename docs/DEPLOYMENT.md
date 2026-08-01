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

Filled in by `scripts/deploy-and-claim.sh`, which commits two distributions with
`create_distribution` and submits claims against them on the privacy-preserving
path. A claim is a privacy transaction, so its instruction data is not published;
the public trace is the claim marker PDA, owned by the verifier, seeded by the
distribution and the recipient's nullifier.

_This section is populated after the live claim run; see the script output and
`scripts/verify-onchain-claim.sh` for the current transactions and markers._

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
