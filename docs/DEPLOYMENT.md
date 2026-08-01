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
| Claim program (LEZ-native, `claim_lez.bin`) | `1f6a0ec0…eb249508` | `5a200887d9b1a27a206cbf09aac419da122aeb921df56ecfc6e7676210c5dcc9` |
| Claim verifier (SPEL, `claim_verifier.bin`) | `a7b7cf26…6fe7b77d` | `9e0a1929ad5c115e6a131a5d113a2db9ff158e5d5920c4ed6729e82a52d5580b` |

## Distributions and claims

Three distributions are committed on chain under the **current** verifier
(ImageID `a7b7cf26…`) with `create_distribution`; each commits only its
eligibility root, in a PDA whose address is `[distribution_id, root]` derived
from the verifier's own program id.

| Distribution | id | recipients | create_distribution tx |
|---|---|---|---|
| 1 | `b1…0001` | 12 | `2fad9747f7f39d8935b8a760146abd851e3e3235856a924ef625138a64cc6309` |
| 2 | `b2…0002` | 10 | `72a32e082ae387428226853be81d6aa1f56796da96eb1c2a50e30a743ca27e78` |
| 3 | `d4…0004` | 6 | `ecf619d7c0e8aebd74e6f8318a9badb1dfe6b18a7e48dfd6390e4c4b8a2ad511` |

> An earlier pair of distributions (`a1…`, `a2…`) was committed under a previous
> verifier ImageID (`66ea2b79…`), before the destination binding was added and
> the verifier redeployed. Because a distribution PDA is derived from the
> verifier's own program id, those are not claimable under the current binary and
> are not used here. The distributions above are the ones under the deployed
> verifier.

The full list of live claims across these three distributions is committed at
[`artifacts/e2e/claims.tsv`](../artifacts/e2e/claims.tsv) as
`distribution_id, claim_tx, nullifier`: 23 privacy-preserving claims (10 + 7 + 6),
each independently verifiable with `verify-onchain-claim.sh`.

A claim is a **privacy-preserving** transaction: its instruction data is not
published, and its only public trace is the claim marker PDA, owned by the
verifier, seeded by the distribution and the recipient's nullifier. A verified
example claim against the deployed verifier, with the destination bound into the
proof:

```
distribution:        b1…0001, create_distribution 2fad9747f7f39d8935b8a760146abd851e3e3235856a924ef625138a64cc6309
claim tx (privacy):  5cb5148572e92cced712a1e5af756b9f3e9b886e284c12f21324c6c8e5c0f9a0
nullifier:           dac661f3cf147e7cc3e88bf340e183702542bdb9c1756bccf21b86cab66ee96d
claim marker PDA:    8VCwNfgAUMQbEztyTJvQPB4gr8uqQPik2QX99yWBwvcS  (owned by the verifier)
```

Verify it from public data over RPC:

```bash
CLAIM_TX=5cb5148572e92cced712a1e5af756b9f3e9b886e284c12f21324c6c8e5c0f9a0 \
NULLIFIER=dac661f3cf147e7cc3e88bf340e183702542bdb9c1756bccf21b86cab66ee96d \
DISTRIBUTION_ID=b100000000000000000000000000000000000000000000000000000000000001 \
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

> A privacy-preserving claim is not shown by the block explorer (it publishes no
> instruction data), so its explorer page reads "not found" even though
> `getTransaction` returns it over RPC. Verify claims over RPC, for example with
> `scripts/verify-onchain-claim.sh`. Reported upstream at
> `logos-blockchain/lez-explorer-ui#15`.

## Signer

Deployed from the wallet signer used for the LP-0005 work; the deploy
transactions are content-addressed and independent of the signer.
