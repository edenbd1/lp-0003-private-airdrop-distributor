# Privacy model

What each party learns, and what they do not. This is the disclosure the LP-0003
functionality criterion asks for.

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
  (`privacy_preserving_transaction/message.rs:14-24`), and the witness travels
  only in the instruction, only on that path.

## What the integrator can rely on

An integration gating on "claimed distribution D" computes the marker address for
a recipient's nullifier and checks it exists and is owned by the verifier
program. Because the marker seed commits to `distribution_id`, a marker from one
distribution cannot be presented for another, and because claiming requires the
marker to be uninitialised, a recipient can be counted at most once per
distribution.

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
