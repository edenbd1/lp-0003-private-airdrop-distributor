# Deployment

Every transaction below is live on the public Logos Execution Zone testnet.
"Live" here means `getTransaction` over JSON-RPC returns a non-null result; see
[What the explorer shows](#what-the-explorer-shows-and-what-it-does-not) for how
that relates to the block explorer, a separate index that reaches a transaction
about two hours after the RPC does.

```
Network:            Public LEZ testnet
Sequencer JSON-RPC: https://testnet.lez.logos.co
Block explorer:     https://explorer.testnet.lez.logos.co
LEZ version:        v0.2.4 (commit 47eba25)
spel:               v0.6.0 sources, repinned and ported to v0.2.4 (vendor/spel)
cargo-risczero:     3.0.5
```

## Deployed programs

A LEZ program-deployment tx hash is `SHA256(borsh(bytecode))`, content addressed,
so redeploying the byte-identical binary reproduces the identical hash. The two
binaries under `artifacts/programs/` therefore hash to exactly these deploy
transactions, which is what `scripts/verify-onchain-claim.sh` step 1 checks.

| Program | ImageID | Deploy tx | On the explorer |
|---|---|---|---|
| Claim program (LEZ-native, `claim_lez.bin`) | `e9843420…67b57449` | `59c2160b…cbbea7c5` | [link](https://explorer.testnet.lez.logos.co/transaction/59c2160b40c5d0f4cce01fd89e7755dbafd9c7d088a071d6f6fd3a10cbbea7c5) |
| Claim verifier (SPEL, `claim_verifier.bin`) | `31edc17c…a110385d` | `7b16e471…7092e34c` | [link](https://explorer.testnet.lez.logos.co/transaction/7b16e471b35ce8c718e066d80a8198f8831ebc7b6704583ddd28bb287092e34c) |

## Distributions and claims

Two distributions are committed on chain under the **current** verifier
(ImageID `31edc17c…`) with `create_distribution`; each commits only its
eligibility root, in a PDA whose address is `[distribution_id, root]` derived
from the verifier's own program id.

| Distribution | id | recipients | eligibility root |
|---|---|---|---|
| 1 | `b1…0001` | 12 | `e48c5f4f…ea6a45bcc` |
| 2 | `b2…0002` | 11 | `7b5078e8…9ed4eb44a` |

> This deployment is on **LEZ v0.2.4** (commit `47eba25`). Both guests are built
> against it, and a guest's ImageID depends on the pinned revision, so the
> ImageIDs — and with them the content-addressed deploy transactions, and every
> PDA derived from the verifier's program id — differ from any earlier v0.2.0 or
> v0.2.2 write-up. Earlier deployments and their claims are superseded.

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
distribution:        b1…0001
claim tx (privacy):  441ccd15e7b5eac388a0849481e95db409f1b6f23a202b6ee1a3ce37ae112c86   (live via RPC)
nullifier:           3db769e851c291d82cb79d717f1256710bb67b06a50cc52bea3f4ae1fea32b99   (a commitment, not a transaction)
claim marker PDA:    B4VTZUENS1Ckmaiv8h44QcU5r276pEgTE2nEwGSGsf16  (owned by the verifier)
```

The `nullifier` is a commitment used to seed the marker PDA; it is **not** a
transaction hash, so `getTransaction` returns null on it, which is expected.

Verify the claim from public data over RPC:

```bash
CLAIM_TX=441ccd15e7b5eac388a0849481e95db409f1b6f23a202b6ee1a3ce37ae112c86 \
NULLIFIER=3db769e851c291d82cb79d717f1256710bb67b06a50cc52bea3f4ae1fea32b99 \
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

A reviewer's first instinct is to paste a hash into the block explorer. The
explorer gets there eventually, but the sequencer's JSON-RPC is immediate and is
the source of truth, so this is stated up front with a control anyone can
reproduce.

`getTransaction` over RPC returns a non-null result for every live transaction in
this submission (the two deploys, both `create_distribution` transactions, and all
23 claims) and `null` for a hash the chain does not hold. A hash that **cannot
exist** (`dededede…` repeated to 64 characters) returns the same `null`. So a
non-null result means the transaction is really on chain, and `null` means the
chain has no such hash, not that a transaction is "empty":

```bash
# non-null "result" = present on chain; null = absent.
# The deploy returns a base64 result; the impossible dede…de hash returns null.
curl -s -X POST https://testnet.lez.logos.co -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"getTransaction","params":["59c2160b40c5d0f4cce01fd89e7755dbafd9c7d088a071d6f6fd3a10cbbea7c5"]}'
```

The block explorer at `explorer.testnet.lez.logos.co` is a **separate index**, and
it lags the sequencer.

**A correction, because this document said otherwise.** It used to state that the
explorer was a WASM application returning the same shell for every
`/transaction/<hash>` URL and fetching its content client-side, so that an
indexed transaction and a hash that cannot exist were byte-identical over `curl`.
That was true when `scripts/check-explorer.py` was written — it is why the script
drives a browser at all — and it is not true now. Re-measured **2026-08-15**, the
explorer server-side renders, so `curl` does separate the two cases:

```bash
# a claim: ~366 kB, and the body carries Type: and Proof Size:
curl -s https://explorer.testnet.lez.logos.co/transaction/441ccd15e7b5eac388a0849481e95db409f1b6f23a202b6ee1a3ce37ae112c86 | wc -c

# a hash that cannot exist: 2416 bytes, and the body says why
curl -s "https://explorer.testnet.lez.logos.co/transaction/$(python3 -c 'print("de"*32)')" | wc -c
```

The second returns `Failed to load transaction: error running server function:
Transaction not found`. Compare the bodies rather than only the sizes — a size is
a weaker signal that happens to work today.

`scripts/check-explorer.py` remains the stronger check and is worth running as a
second opinion: it renders each page headless and compares it against that same
impossible hash as a control, so it reads the DOM a reviewer actually sees, and it
keeps working if the explorer returns to client-side rendering. If the control
ever renders as a *found* transaction the script aborts rather than report
anything, because the baseline every verdict rests on would be invalid.

**Which of these actually runs.** `scripts/check-chain-refs.py` resolves every
explorer link in these documents through the node on every markdown push, and
once a day it also fetches each transaction page and compares it against a
freshly measured not-found shell — no browser, so it runs in CI rather than by
hand. `scripts/check-explorer.py` drives a real browser and remains the stronger
second opinion, but nothing schedules it: run it yourself when a rendering
question matters. A gate that runs and a gate that could are not the same thing,
and this says which is which.

The most recent green pass is
[run 33084394804](https://github.com/edenbd1/lp-0003-private-airdrop-distributor/actions/runs/33084394804),
on the reviewed commit itself — every explorer link in this repository resolved
against the node on the tree you are reading.


### It indexes these transactions, but not immediately

Measured on this deployment: the claim program's deploy, confirmed on chain at
02:15, was still absent from the explorer at 03:51 and present at 04:07. Every
claim sampled from the previous run's manifest is indexed. So "the explorer says
Transaction not found" on a freshly submitted hash means *not indexed yet*, not
missing — and `getTransaction` over RPC is the immediate, authoritative answer.

### What it shows for a claim is the privacy property itself

Once indexed, a claim renders like this:

```
Type: Privacy-Preserving Transaction
Public Accounts: 2    Private Actions: 7    Proof Size: 260947 bytes
Public Accounts: B4VTZUENS1Ckmaiv8h44QcU5r276pEgTE2nEwGSGsf16
                6cPhtwbqHVVTeGXaRNKU1UJw9HtY855pPipNavLawEHz
```

Both accounts, because there are two and quoting one would flatter this. The
first is the claim marker; the second is the distribution PDA, which a claim must
touch — its address *is* `[distribution_id, root]`, so the distribution is
visible by construction and no design could hide it while still anchoring the
root by address.

What is absent is what matters: no `program_id`, no `instruction_data`, no
recipient address, and nothing that maps the marker back to one. An observer sees
that a privacy transaction occurred against a known distribution, carrying a real
proof — and cannot tell *which member of that eligibility set* claimed, which is
the property `docs/privacy-model.md` states and the only one worth claiming. A
public deployment transaction, by contrast, renders its bytecode size, because
nothing about it is private.

Reproduce either result:

```bash
./scripts/check-explorer.py                    # the deploys and a sample of claims
./scripts/check-explorer.py <hash> [<hash>...] # any hashes
```

## If the testnet is reset

The public testnet has been reset before, and a reset wipes deployment history.
If it is reset again:

- The **deploy** transactions are content-addressed (`SHA256(borsh(bytecode))`),
  so redeploying the committed binaries reproduces the **identical** deploy
  hashes, and their explorer pages return unchanged.
- The **create_distribution** and **claim** transactions are signed with a nonce,
  so they would be new transactions with new hashes after a reset;
  `scripts/deploy-and-claim.sh` re-creates them and writes a fresh manifest.

## How to reproduce

Pre-requisites: macOS arm64 or Linux with `cargo`, `docker`, `curl`, `python3`,
`jq`, and the Logos toolchain (`wallet` and `spel` from LEZ/spel at the versions
above). `verify-onchain-claim.sh` needs `curl`, `python3` and `jq`, and refuses
to start (exit 2) naming the missing one rather than reporting the chain
evidence as absent.

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

The deploy transactions are content-addressed: a deployment carries only the
bytecode, with no signer and no nonce, so the hash is reproducible from the
committed artifact by anyone.
