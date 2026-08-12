# Deployment

Every transaction below is live on the public Logos Execution Zone testnet.
"Live" here means `getTransaction` over JSON-RPC returns a non-null result; see
[What the explorer shows](#what-the-explorer-shows-and-what-it-does-not) for how
that relates to the block explorer, which is a separate, and irregular, index.

```
Network:            Public LEZ testnet
Sequencer JSON-RPC: https://testnet.lez.logos.co
Block explorer:     https://explorer.testnet.lez.logos.co
LEZ version:        v0.2.2 (commit d6e4ae6)
spel:               v0.6.0
cargo-risczero:     3.0.5
```

## Deployed programs

A LEZ program-deployment tx hash is `SHA256(borsh(bytecode))`, content addressed,
so redeploying the byte-identical binary reproduces the identical hash. The two
binaries under `artifacts/programs/` therefore hash to exactly these deploy
transactions, which is what `scripts/verify-onchain-claim.sh` step 1 checks.

| Program | ImageID | Deploy tx | On the explorer |
|---|---|---|---|
| Claim program (LEZ-native, `claim_lez.bin`) | `8faaa67c…b48c79c0` | `4a8dab27…8c5fdf59` | [displays](https://explorer.testnet.lez.logos.co/transaction/4a8dab271c2ac4f3b19c38b45e3f05fa4f413a0ac84a7b28030abebc8c5fdf59) |
| Claim verifier (SPEL, `claim_verifier.bin`) | `51a07a8b…77e8e4ab` | `90f615d4…a22defe7` | [displays](https://explorer.testnet.lez.logos.co/transaction/90f615d4045db10c2e42c44d15bf80f36a7a72e31df51e3bda6c46e4a22defe7) |

## Distributions and claims

Two distributions are committed on chain under the **current** verifier
(ImageID `51a07a8b…`) with `create_distribution`; each commits only its
eligibility root, in a PDA whose address is `[distribution_id, root]` derived
from the verifier's own program id.

| Distribution | id | recipients | eligibility root |
|---|---|---|---|
| 1 | `b1…0001` | 12 | `87f8333b…8f99eaf4` |
| 2 | `b2…0002` | 11 | `676250fb…63afb678` |

> This deployment is on **LEZ v0.2.2** (commit `d6e4ae6`), the version the public
> testnet runs after its 2026-08-05 reset and upgrade from v0.2.0. Any earlier
> v0.2.0 deployment and its claims no longer exist on the reset chain (the privacy
> circuit id changed, so v0.2.0 proofs no longer verify), which is why these
> addresses differ from earlier write-ups.

The full list of live claims across these two distributions is committed at
[`artifacts/e2e/claims.tsv`](../artifacts/e2e/claims.tsv) as
`distribution_id, claim_tx, nullifier`: 23 privacy-preserving claims (12 + 11),
each independently verifiable with `verify-onchain-claim.sh`.

A claim is a **privacy-preserving** transaction: its instruction data is not
published, and its only public trace is the claim marker PDA, owned by the
verifier, seeded by the distribution and the recipient's nullifier. A verified
example claim against the deployed verifier, with the destination bound into the
proof:

```
distribution:        b1…0001, create_distribution 2fad9747f7f39d8935b8a760146abd851e3e3235856a924ef625138a64cc6309
claim tx (privacy):  d9236824835c9f6a986c3bc687c04e2c722ad0984009fb0a936767d3c584e13b   (live via RPC; not indexed by the explorer)
nullifier:           4920f6fc4e4c50597b45cef083126decfe432a1100815f16bcfb128b0dfcbef8   (a commitment, not a transaction)
claim marker PDA:    8VCwNfgAUMQbEztyTJvQPB4gr8uqQPik2QX99yWBwvcS  (owned by the verifier)
```

The `nullifier` is a commitment used to seed the marker PDA; it is **not** a
transaction hash, so `getTransaction` returns null on it, which is expected.

Verify the claim from public data over RPC:

```bash
CLAIM_TX=d9236824835c9f6a986c3bc687c04e2c722ad0984009fb0a936767d3c584e13b \
NULLIFIER=4920f6fc4e4c50597b45cef083126decfe432a1100815f16bcfb128b0dfcbef8 \
DISTRIBUTION_ID=b100000000000000000000000000000000000000000000000000000000000001 \
./scripts/verify-onchain-claim.sh
```

which confirms the transaction is `PrivacyPreserving`, its receipt is a `Succinct`
STARK the sequencer verified, and the marker PDA is owned by the verifier: the
membership proof was verified on chain as a precondition of acceptance. The marker
ownership is the thing no block explorer can show or forge, and it is what
`verify-onchain-claim.sh` checks.

> **The claimant's private account must be synced before each claim.** A
> privacy transaction spends the signer's commitment, so the wallet must
> `account sync-private` before building the next claim, or the membership proof
> is against a stale commitment and the sequencer drops the transaction.
> `scripts/deploy-and-claim.sh` syncs before each claim for this reason.

## What the explorer shows, and what it does not

A reviewer's first instinct is to paste a hash into the explorer. Two of the
transactions above do not display there, so this is stated up front, with the
measurement so it can be reproduced.

Measured on 2026-08-03, all six transactions are **live over RPC**
(`getTransaction` returns a non-null base64 result for each). On the explorer,
four display and two do not:

| Transaction | Type | Explorer |
|---|---|---|
| `4a8dab27…` deploy claim_lez | public | displays |
| `90f615d4…` deploy verifier | public | displays |
| `2fad9747…` create_distribution 1 | public | displays |
| `72a32e08…` create_distribution 2 | public | displays |
| `ecf619d7…` create_distribution 3 | **public** | **not indexed** |
| `d9236824…` example claim | privacy | not indexed |

The control that gives the measurement meaning: a hash that **cannot exist**
(`dededede…` repeated to 64 chars) returns from the explorer the byte-for-byte
same "not found" response as the two that do not display. So "not shown" means
"the indexer has no record of this hash", not "the transaction is empty" — while
the RPC, which does hold it, returns it. Reproduce with:

```bash
# a base64 "result" = present on chain; null = absent
curl -s -X POST https://testnet.lez.logos.co -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getTransaction","params":["<hash>"]}'
```

Note that `create_distribution 3` is a **public** transaction that does not
display, while the two public distributions before it do. The indexer's coverage
is therefore **irregular, not type-based**: it is not the case that "the explorer
only shows deploys" or "only public transactions", and that would be false in one
`curl`. The correct statement is narrow: this indexer does not have every hash the
sequencer does, and the source of truth is the RPC (and
`scripts/verify-onchain-claim.sh` on top of it).

### The example claim not displaying has two separate causes

These are independent and should not be conflated:

1. **The indexer gap above.** The claim transaction is simply not in this
   indexer, exactly as `create_distribution 3` (a public transaction) is not.
   This has nothing to do with the privacy design; a public transaction hits it
   too.
2. **Privacy by construction.** Independently of any indexer, a
   privacy-preserving transaction publishes no `program_id` and no
   `instruction_data`. So even a perfect indexer could show only that *a* privacy
   transaction occurred, never which distribution or which address it concerns.
   That unattributability is the point of this submission, and it is what
   `verify-onchain-claim.sh` establishes from the marker PDA rather than from any
   explorer.

The explorer's failure to show privacy transactions is reported upstream at
`logos-blockchain/lez-explorer-ui#15`; that issue is about cause (1).

## If the testnet is reset

The public testnet has been reset before (it wiped LP-0005's history). If it is
reset again:

- The **deploy** transactions are content-addressed (`SHA256(borsh(bytecode))`),
  so redeploying the committed binaries reproduces the **identical** deploy
  hashes, and their explorer pages return unchanged.
- The **create_distribution** and **claim** transactions are signed with a nonce,
  so they would be new transactions with new hashes after a reset;
  `scripts/deploy-and-claim.sh` re-creates them and writes a fresh manifest.

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

## Signer

Deployed from the wallet signer used for the LP-0005 work; the deploy
transactions are content-addressed and independent of the signer.
