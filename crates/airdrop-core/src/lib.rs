//! Shared primitives for LP-0003, a private allowlist / airdrop distributor on
//! the Logos Execution Zone.
//!
//! WHAT THIS PROVES, AND WHY EACH BINDING IS HERE
//!
//! A distributor commits to an eligibility set on chain as a single Merkle root.
//! An eligible recipient later claims their allocation by proving, in zero
//! knowledge, that they hold an entry in that set, without revealing which one.
//! The design is deliberately built so that none of the five ways this could be
//! cheated is left open. Each is enforced in-circuit by [`claim`] and, on chain,
//! by checking the proved root against the distributor's committed root.
//!
//!   1. **Membership is against an anchored root.** The distributor publishes the
//!      root on chain first; the claim proof is verified against that exact root,
//!      not one the claimant invented. This is the property LP-0005's balance
//!      attestation could not have on its own, and it is free here because an
//!      airdrop has a distributor who commits the set up front.
//!
//!   2. **The allocation is sealed into the leaf.** A leaf commits to
//!      `(account_id, allocation, salt)`, so a claimant who edits the allocation
//!      produces a different leaf that no longer folds to the root. You cannot
//!      claim more than you were granted.
//!
//!   3. **Double-claim is caught by a secret-bound nullifier.** The nullifier is
//!      `H(prefix || distribution_id || nsk)`, a function of the recipient's
//!      secret key. It is deterministic per `(distribution, recipient)`, so the
//!      second claim reuses the marker PDA and fails, and it is unlinkable to any
//!      address because an observer who knows the candidate set still cannot
//!      compute it without the secret.
//!
//!   4. **Claims do not cross distributions.** The nullifier and the marker seed
//!      both carry `distribution_id`, so a marker earned in one distribution is
//!      not valid in another.
//!
//!   5. **The public key ties the secret to the committed leaf.** The circuit
//!      proves `npk = H(prefix || nsk)` and `account_id = derive_account_id(npk,
//!      identifier)`, so the nullifier's secret is the same secret that owns the
//!      committed entry. Without this a claimant could pair someone else's leaf
//!      with their own nullifier.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Domain separators
// ---------------------------------------------------------------------------

/// 32-byte domain separator for LEZ private account ids.
/// Byte-identical to the LEZ derivation (`nssa/core/src/nullifier.rs`), reused
/// so a recipient's airdrop account is the same account the rest of LEZ sees.
/// ASCII `"/LEE/v0.3/AccountId/Private/"` (28 bytes) + 4 zero bytes.
pub const PRIVATE_ACCOUNT_ID_PREFIX: [u8; 32] = [
    b'/', b'L', b'E', b'E', b'/', b'v', b'0', b'.', b'3', b'/', b'A', b'c', b'c', b'o', b'u', b'n',
    b't', b'I', b'd', b'/', b'P', b'r', b'i', b'v', b'a', b't', b'e', b'/', 0, 0, 0, 0,
];

/// Domain separator deriving the public nullifier key from the secret one.
/// `npk = SHA256(NPK_DERIVE_PREFIX || nsk)`. Local to LP-0003.
/// ASCII `"/lp-0003/v0.1/npk-from-nsk/"` (27 bytes) + 5 zero bytes.
pub const NPK_DERIVE_PREFIX: [u8; 32] = [
    b'/', b'l', b'p', b'-', b'0', b'0', b'0', b'3', b'/', b'v', b'0', b'.', b'1', b'/', b'n', b'p',
    b'k', b'-', b'f', b'r', b'o', b'm', b'-', b'n', b's', b'k', b'/', 0, 0, 0, 0, 0,
];

/// Domain separator for an eligibility-set leaf.
/// ASCII `"/lp-0003/v0.1/EligLeaf/"` (23 bytes) + 9 zero bytes.
pub const ELIGIBILITY_LEAF_PREFIX: [u8; 32] = [
    b'/', b'l', b'p', b'-', b'0', b'0', b'0', b'3', b'/', b'v', b'0', b'.', b'1', b'/', b'E', b'l',
    b'i', b'g', b'L', b'e', b'a', b'f', b'/', 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Domain separator for a claim nullifier.
/// ASCII `"/lp-0003/v0.1/ClaimNul/"` (23 bytes) + 9 zero bytes.
pub const CLAIM_NULLIFIER_PREFIX: [u8; 32] = [
    b'/', b'l', b'p', b'-', b'0', b'0', b'0', b'3', b'/', b'v', b'0', b'.', b'1', b'/', b'C', b'l',
    b'a', b'i', b'm', b'N', b'u', b'l', b'/', 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Domain separator for the on-chain claim-marker PDA seed.
/// ASCII `"/lp-0003/v0.1/ClaimMark/"` (24 bytes) + 8 zero bytes.
pub const CLAIM_MARKER_PREFIX: [u8; 32] = [
    b'/', b'l', b'p', b'-', b'0', b'0', b'0', b'3', b'/', b'v', b'0', b'.', b'1', b'/', b'C', b'l',
    b'a', b'i', b'm', b'M', b'a', b'r', b'k', b'/', 0, 0, 0, 0, 0, 0, 0, 0,
];

// ---------------------------------------------------------------------------
// Reused LEZ primitives (byte-identical to LP-0005 / upstream LEZ)
// ---------------------------------------------------------------------------

/// 32-byte domain separator for LEZ private account commitments.
/// ASCII `"/LEE/v0.3/Commitment/"` (21 bytes) + 11 zero bytes.
pub const COMMITMENT_PREFIX: [u8; 32] = [
    b'/', b'L', b'E', b'E', b'/', b'v', b'0', b'.', b'3', b'/', b'C', b'o', b'm', b'm', b'i', b't',
    b'm', b'e', b'n', b't', b'/', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// Re-derive a LEZ regular private-account id from `(npk, identifier)`.
#[must_use]
pub fn derive_account_id(npk: &[u8; 32], identifier: u128) -> [u8; 32] {
    let mut bytes = [0u8; 80];
    bytes[0..32].copy_from_slice(&PRIVATE_ACCOUNT_ID_PREFIX);
    bytes[32..64].copy_from_slice(npk);
    bytes[64..80].copy_from_slice(&identifier.to_le_bytes());
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

/// Derive the public nullifier key from the secret one. The claimant keeps
/// `nsk` private; the eligibility leaf commits to the account derived from it.
#[must_use]
pub fn derive_npk(nsk: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(NPK_DERIVE_PREFIX);
    h.update(nsk);
    h.finalize().into()
}

/// Fold a Merkle path from a leaf to its root.
/// `hash_two(L, R) = SHA256(L || R)`, LEZ ordering by the index bit.
#[must_use]
pub fn fold_merkle_path(leaf: &[u8; 32], leaf_index: u64, siblings: &[[u8; 32]]) -> [u8; 32] {
    let mut node = *leaf;
    let mut idx = leaf_index;
    for sib in siblings {
        let mut h = Sha256::new();
        if idx & 1 == 0 {
            h.update(node);
            h.update(sib);
        } else {
            h.update(sib);
            h.update(node);
        }
        node = h.finalize().into();
        idx >>= 1;
    }
    node
}

// ---------------------------------------------------------------------------
// LP-0003 primitives
// ---------------------------------------------------------------------------

/// One eligible recipient's committed entry: the leaf the distributor places in
/// the tree. `salt` is chosen per entry by the distributor; it hides the
/// allocation and prevents two recipients with the same allocation from sharing
/// a leaf.
///
/// `leaf = SHA256(ELIGIBILITY_LEAF_PREFIX || account_id || allocation_le || salt)`
#[must_use]
pub fn compute_eligibility_leaf(account_id: &[u8; 32], allocation: u128, salt: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(ELIGIBILITY_LEAF_PREFIX);
    h.update(account_id);
    h.update(allocation.to_le_bytes());
    h.update(salt);
    h.finalize().into()
}

/// The claim nullifier, bound to the recipient's secret and the distribution.
///
/// `nullifier = SHA256(CLAIM_NULLIFIER_PREFIX || distribution_id || nsk)`
///
/// Deterministic per `(distribution, recipient)` so a second claim collides, and
/// unlinkable because it depends on the secret `nsk`, which an observer who knows
/// the candidate account set still does not have.
#[must_use]
pub fn compute_claim_nullifier(distribution_id: &[u8; 32], nsk: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(CLAIM_NULLIFIER_PREFIX);
    h.update(distribution_id);
    h.update(nsk);
    h.finalize().into()
}

/// The on-chain claim-marker PDA seed. Carries `distribution_id` so a marker is
/// scoped to one distribution, and the nullifier so it is the single address a
/// given recipient's claim can occupy.
///
/// `seed = SHA256(CLAIM_MARKER_PREFIX || distribution_id || nullifier)`
#[must_use]
pub fn compute_claim_marker(distribution_id: &[u8; 32], nullifier: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(CLAIM_MARKER_PREFIX);
    h.update(distribution_id);
    h.update(nullifier);
    h.finalize().into()
}

/// Private inputs to a claim proof. None of these reach the journal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimWitness {
    /// Recipient's secret nullifier key.
    pub nsk: [u8; 32],
    /// LEZ per-account identifier.
    pub identifier: u128,
    /// Allocation granted to this recipient, sealed in the leaf.
    pub allocation: u128,
    /// Per-entry salt the distributor used when building the leaf.
    pub salt: [u8; 32],
    /// Merkle siblings from the leaf up to (but excluding) the root.
    pub merkle_path: Vec<[u8; 32]>,
    /// 0-indexed leaf position in the eligibility tree.
    pub leaf_index: u64,
}

/// The public statement a claim proof establishes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimStatement {
    /// The distributor's committed eligibility-set root. Checked on chain to
    /// equal the root stored in the distribution account.
    pub distribution_root: [u8; 32],
    /// Identifies the distribution; binds the nullifier and marker to it.
    pub distribution_id: [u8; 32],
    /// The allocation being claimed. Must equal the sealed leaf allocation.
    pub allocation: u128,
    /// The nullifier that prevents a second claim.
    pub nullifier: [u8; 32],
}

/// Errors a claim proof can fail with, mapped to on-chain codes by the verifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimError {
    /// The recipient's account is not a member of the committed root.
    NotEligible,
    /// The claimed allocation does not match the sealed leaf allocation.
    AllocationMismatch,
    /// The supplied nullifier is not the one this witness yields.
    NullifierMismatch,
}

/// The in-circuit claim logic. Returns the eligibility leaf on success, or a
/// [`ClaimError`] naming exactly which binding failed. The guest calls this and
/// commits [`ClaimStatement`] to the journal only if it returns `Ok`.
///
/// This deliberately takes the same shape as LP-0005's `attest`: witness plus a
/// public statement it must satisfy, so the two share an audit vocabulary.
pub fn claim(witness: &ClaimWitness, statement: &ClaimStatement) -> Result<[u8; 32], ClaimError> {
    // Tie the secret to the committed public account (binding 5).
    let npk = derive_npk(&witness.nsk);
    let account_id = derive_account_id(&npk, witness.identifier);

    // The allocation the caller claims must be the one sealed in the leaf
    // (binding 2): recompute the leaf from the claimed allocation and require it
    // to be the leaf that actually anchors.
    if statement.allocation != witness.allocation {
        return Err(ClaimError::AllocationMismatch);
    }
    let leaf = compute_eligibility_leaf(&account_id, witness.allocation, &witness.salt);

    // Membership against the claimed root (binding 1). The verifier program then
    // checks this root equals the distributor's on-chain commitment.
    let recovered = fold_merkle_path(&leaf, witness.leaf_index, &witness.merkle_path);
    if recovered != statement.distribution_root {
        return Err(ClaimError::NotEligible);
    }

    // Double-claim nullifier, bound to the secret and the distribution
    // (bindings 3 and 4).
    let nullifier = compute_claim_nullifier(&statement.distribution_id, &witness.nsk);
    if nullifier != statement.nullifier {
        return Err(ClaimError::NullifierMismatch);
    }

    Ok(leaf)
}

/// Distributor-side helper: build a Merkle tree over the eligibility leaves and
/// return the root plus, for each leaf, its `(leaf_index, siblings)` path. Pads
/// to a power of two with a fixed sentinel so paths are well-formed. Kept host
/// only because a distributor builds the set off chain and publishes just the
/// root.
#[cfg(feature = "std")]
#[must_use]
pub fn build_eligibility_tree(leaves: &[[u8; 32]]) -> ([u8; 32], Vec<(u64, Vec<[u8; 32]>)>) {
    assert!(!leaves.is_empty(), "an eligibility set needs at least one entry");

    // Pad to a power of two with a domain-separated sentinel that no real leaf
    // can equal (it commits to no account).
    let sentinel: [u8; 32] = {
        let mut h = Sha256::new();
        h.update(ELIGIBILITY_LEAF_PREFIX);
        h.update(b"PAD");
        h.finalize().into()
    };
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    while (level.len() & (level.len() - 1)) != 0 {
        level.push(sentinel);
    }
    let width = level.len();

    // Record each original leaf's path as we fold up.
    let mut paths: Vec<(u64, Vec<[u8; 32]>)> = (0..leaves.len())
        .map(|i| (i as u64, Vec::new()))
        .collect();

    let mut nodes = level;
    let mut idx_width = width;
    while idx_width > 1 {
        let mut next = Vec::with_capacity(idx_width / 2);
        for pair in 0..idx_width / 2 {
            let l = nodes[2 * pair];
            let r = nodes[2 * pair + 1];
            // Append the sibling to any original leaf sitting under this pair.
            for (leaf_i, path) in paths.iter_mut() {
                let pos_at_level = (*leaf_i as usize) >> path.len();
                if pos_at_level == 2 * pair {
                    path.push(r);
                } else if pos_at_level == 2 * pair + 1 {
                    path.push(l);
                }
            }
            let mut h = Sha256::new();
            h.update(l);
            h.update(r);
            next.push(h.finalize().into());
        }
        nodes = next;
        idx_width /= 2;
    }

    (nodes[0], paths)
}
