# Privacy model

What each party learns, and what they do not. This is the disclosure the LP-0003
functionality criterion asks for.

## Threat model

The privacy claims below are stated relative to this threat model. The adversaries
are the parties in the next section, ordered by strength:

- A **passive on-chain observer** who sees every transaction and account, and who
  may also know the entire candidate set of eligible addresses.
- The **other recipients**, who additionally know their own secrets and their own
  place in the set.
- The **distributor**, who additionally knows the whole eligible set by
  construction (it chose it).

All are assumed to follow the protocol except where noted (a malicious distributor
is considered under "what the integrator can rely on" and in
[`limitations.md`](limitations.md)). The cryptographic assumptions are SHA-256
collision/preimage resistance, Risc0 STARK soundness with the LEZ privacy circuit's
`env::verify` composition, and secrecy of each recipient's `nsk`. **"Unlinkable"
means precisely:** none of these adversaries can map a claim's on-chain trace (its
marker PDA) back to which address in the candidate set made it, because the marker
is seeded by a nullifier `H(prefix ‖ distribution_id ‖ nsk)` that requires the
secret `nsk`. What is *out of scope*: a recipient who voluntarily reveals their own
`nsk` or self-identifies, and timing correlation from a recipient who claims the
instant a distribution opens — both named under Boundaries.

## The parties

- **Distributor.** Builds the eligibility set and commits its Merkle root on
  chain via `create_distribution`.
- **Recipient.** Holds a secret `nsk`; their eligibility leaf commits to the
  account derived from it, an allocation, and a per-entry salt.
- **On-chain observer.** Sees every transaction and account on the chain.
- **Integrator.** Consumes a claim marker PDA as proof that a given recipient
  claimed a given distribution.

## What the distributor learns

The distributor chooses who is eligible, so it knows the candidate set: the
account ids and allocations it placed in the tree. It does **not** learn which
recipients later claim, nor when, because a claim is a privacy-preserving
transaction whose instruction data is not published and whose only public trace,
the marker PDA, is seeded by a nullifier the distributor cannot compute (it does
not hold any recipient's `nsk`).

## What an on-chain observer learns

- That a distribution exists, and its committed root, from the distribution PDA.
- That *a* claim happened against *a* distribution, from a claim marker PDA owned
  by the verifier program.
- The allocation carried by that claim, if the integration records it.

An observer does **not** learn:

- Which address in the eligibility set claimed. The marker is seeded by
  `SHA256(CLAIM_NULLIFIER_PREFIX || distribution_id || nsk)`. Because `nsk` is
  secret, an observer who knows every candidate account id still cannot compute
  the nullifier, so they cannot map a marker back to an address. This is the
  unlinkability property.
- The recipient's `nsk`, salt, or Merkle path: a privacy `Message` publishes
  neither `program_id` nor `instruction_data`
  (`privacy_preserving_transaction/message.rs:14-27`), and the witness travels
  only in the instruction, only on that path.

> **A claim must be submitted on the privacy path.** The witness carries the
> recipient's root secret `nsk`, so it stays private only because a privacy
> transaction does not publish instruction data. A *public* transaction publishes
> `instruction_data` verbatim, which would disclose `nsk` and de-anonymise all of
> that recipient's claims. Submitting publicly cannot forge or steal a claim (a
> public transaction is re-executed and a bad claim still panics), but it would
> leak the secret. The claim tooling (`airdrop` CLI, `deploy-and-claim.sh`) always
> submits on the privacy path. The verifier program does not *itself* reject the
> public path today; doing so in-program is a listed hardening in
> [`limitations.md`](limitations.md), still open because LEZ exposes the guest no
> value that separates the two paths — `caller_program_id` is the default for a
> top-level instruction, which a claim is on either one.

## What the integrator can rely on

An integration gating on "claimed distribution D" computes the marker address for
a recipient's nullifier and checks it exists and is owned by the verifier
program. Because the marker seed commits to `distribution_id`, a marker from one
distribution cannot be presented for another, and because claiming requires the
marker to be uninitialised, a recipient can be counted at most once per
distribution.

> **Key proof-of-claim on `(distribution_id, root)`, not `distribution_id` alone.**
> `create_distribution` is permissionless, so anyone can anchor another root under
> the same `distribution_id` (a different PDA). The nullifier and marker commit to
> `distribution_id` but not to the root, so a marker does not record *which* root
> was claimed. This cannot enable a double-claim (the same `nsk` yields the same
> marker across roots) or touch another distribution's funds, but an integrator
> that keys eligibility on `distribution_id` only, ignoring the specific committed
> root, could be misled by an attacker-anchored decoy root. Bind the root you
> intend, as `verify-onchain-claim.sh` does.

## The encrypted bundle: how a recipient learns their row

The distributor does not need a private channel to each recipient. It publishes
one `bundle.json`: every recipient's claim data (allocation, salt, leaf index,
Merkle path) sealed to a key derived from that recipient's secret, in shuffled
order. Because each row is authenticated-encrypted to an X25519 key derived from
the recipient's `nsk` through a domain separator (distinct from the nullifier and
account key), the bundle can be posted in the open:

- An observer sees only ciphertext and ephemeral public keys, learning neither
  who is eligible nor any allocation, and cannot tell which row belongs to whom.
- A recipient needs only the bundle and their own `nsk`. They trial-open each row
  and keep the one whose authentication tag verifies.
- Before spending a proof, the recipient checks the decrypted row reconstructs a
  leaf that anchors to the committed root, so a malicious distributor cannot make
  them prove a leaf that is not in the set.

This is delivery, not trust: the on-chain guarantee still comes from Merkle
membership against the committed root. The encryption only means the eligibility
data can be published without a side channel. See `crates/airdrop-crypto`.

## Boundaries

These are stated plainly in [`limitations.md`](limitations.md). In short: the
scheme protects against passive linking, not against a recipient who chooses to
reveal their own `nsk`; and the marker records that a claim happened, with token
delivery to a destination being the integration layer's responsibility.
