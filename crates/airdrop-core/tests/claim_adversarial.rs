//! Adversarial audit of the LP-0003 claim primitive, encoded as tests.
//!
//! Each of the five ways a claim could be cheated gets a test that constructs
//! the cheat and requires it to fail, plus an honest control that must pass. If
//! any of these ever regresses, the airdrop gate stops gating.
//!
//! The on-chain root check (proved root equals the distributor's committed root)
//! is not exercised here because it lives in the verifier program, not the
//! circuit. It is covered by the program's own tests. What is covered here is
//! everything the circuit itself must guarantee before the chain is even asked.

use airdrop_core::{
    build_eligibility_tree, claim, compute_claim_marker, compute_claim_nullifier,
    compute_eligibility_leaf, derive_account_id, derive_npk, ClaimError, ClaimStatement,
    ClaimWitness,
};

const DIST_ID: [u8; 32] = [0xD1; 32];
const DEST: [u8; 32] = [0xDE; 32];

/// Build a realistic eligibility set of `n` recipients and return the tree root,
/// the per-recipient secrets/allocations/salts, and their membership paths.
struct Setup {
    root: [u8; 32],
    nsks: Vec<[u8; 32]>,
    allocations: Vec<u128>,
    salts: Vec<[u8; 32]>,
    paths: Vec<(u64, Vec<[u8; 32]>)>,
}

fn setup(n: usize) -> Setup {
    let mut nsks = Vec::new();
    let mut allocations = Vec::new();
    let mut salts = Vec::new();
    let mut leaves = Vec::new();
    for i in 0..n {
        let nsk = [i as u8 + 1; 32];
        let alloc = 100u128 + i as u128 * 10;
        let salt = [0x50 ^ i as u8; 32];
        let account_id = derive_account_id(&derive_npk(&nsk), 0);
        leaves.push(compute_eligibility_leaf(&account_id, alloc, &salt));
        nsks.push(nsk);
        allocations.push(alloc);
        salts.push(salt);
    }
    let (root, paths) = build_eligibility_tree(&leaves);
    Setup {
        root,
        nsks,
        allocations,
        salts,
        paths,
    }
}

fn witness_for(s: &Setup, i: usize) -> ClaimWitness {
    ClaimWitness {
        nsk: s.nsks[i],
        identifier: 0,
        allocation: s.allocations[i],
        salt: s.salts[i],
        merkle_path: s.paths[i].1.clone(),
        leaf_index: s.paths[i].0,
        destination: DEST,
    }
}

fn statement_for(s: &Setup, i: usize) -> ClaimStatement {
    let nullifier = compute_claim_nullifier(&DIST_ID, &s.nsks[i]);
    ClaimStatement {
        distribution_root: s.root,
        distribution_id: DIST_ID,
        allocation: s.allocations[i],
        nullifier,
        destination: DEST,
    }
}

/// The control. If an honest recipient cannot claim, every rejection below is
/// meaningless.
#[test]
fn an_eligible_recipient_claims() {
    let s = setup(8);
    for i in 0..8 {
        claim(&witness_for(&s, i), &statement_for(&s, i))
            .unwrap_or_else(|e| panic!("recipient {i} should be eligible, got {e:?}"));
    }
}

/// Binding 1: an account not in the committed set cannot claim, even with a
/// well-formed witness of its own.
#[test]
fn a_non_member_is_rejected() {
    let s = setup(8);
    // An outsider with a valid-looking entry that was never placed in the tree.
    let outsider_nsk = [0xFF; 32];
    let account_id = derive_account_id(&derive_npk(&outsider_nsk), 0);
    let salt = [0xAB; 32];
    let leaf = compute_eligibility_leaf(&account_id, 100, &salt);
    // They borrow member 0's path, which does not fold their leaf to the root.
    let w = ClaimWitness {
        nsk: outsider_nsk,
        identifier: 0,
        allocation: 100,
        salt,
        merkle_path: s.paths[0].1.clone(),
        leaf_index: s.paths[0].0,
        destination: DEST,
    };
    let st = ClaimStatement {
        distribution_root: s.root,
        distribution_id: DIST_ID,
        allocation: 100,
        nullifier: compute_claim_nullifier(&DIST_ID, &outsider_nsk),
        destination: DEST,
    };
    let _ = leaf;
    assert_eq!(claim(&w, &st), Err(ClaimError::NotEligible));
}

/// Binding 2: a recipient cannot claim more than the allocation sealed in their
/// leaf. Inflating the amount changes the leaf and breaks membership.
#[test]
fn inflating_the_allocation_is_rejected() {
    let s = setup(8);
    let mut w = witness_for(&s, 3);
    let mut st = statement_for(&s, 3);
    // Claim ten times the granted amount, consistently in witness and statement
    // so it is not merely the internal-consistency check that trips.
    w.allocation = s.allocations[3] * 10;
    st.allocation = s.allocations[3] * 10;
    assert_eq!(claim(&w, &st), Err(ClaimError::NotEligible));
}

/// Binding 2, second face: the statement and witness allocation must agree, so a
/// claimant cannot prove one amount while asking the chain to credit another.
#[test]
fn a_statement_allocation_that_disagrees_with_the_witness_is_rejected() {
    let s = setup(8);
    let w = witness_for(&s, 2);
    let mut st = statement_for(&s, 2);
    st.allocation = s.allocations[2] + 1;
    assert_eq!(claim(&w, &st), Err(ClaimError::AllocationMismatch));
}

/// Binding 3: the same recipient claiming twice in one distribution yields the
/// same nullifier and therefore the same marker address. The circuit accepts
/// both proofs, but the chain can only ever claim that one marker once.
#[test]
fn a_second_claim_reuses_the_same_marker() {
    let s = setup(8);
    let n1 = compute_claim_nullifier(&DIST_ID, &s.nsks[5]);
    let n2 = compute_claim_nullifier(&DIST_ID, &s.nsks[5]);
    assert_eq!(n1, n2, "nullifier must be deterministic per recipient");
    assert_eq!(
        compute_claim_marker(&DIST_ID, &n1),
        compute_claim_marker(&DIST_ID, &n2),
        "the two claims must land on the same marker, so the second cannot init"
    );
}

/// Binding 4: a marker earned in one distribution is not valid in another. The
/// same recipient produces different nullifiers and different markers per
/// distribution.
#[test]
fn claims_do_not_cross_distributions() {
    let s = setup(8);
    let other: [u8; 32] = [0xD2; 32];
    let n_here = compute_claim_nullifier(&DIST_ID, &s.nsks[1]);
    let n_there = compute_claim_nullifier(&other, &s.nsks[1]);
    assert_ne!(n_here, n_there, "nullifier must be scoped per distribution");
    assert_ne!(
        compute_claim_marker(&DIST_ID, &n_here),
        compute_claim_marker(&other, &n_there),
        "a marker must not be presentable in another distribution"
    );
}

/// Binding 5, and the unlinkability property: the nullifier depends on the
/// secret. An observer who knows every candidate account id still cannot compute
/// the nullifier, so a completed claim is not linkable to an address.
#[test]
fn the_nullifier_is_not_computable_from_public_data() {
    let s = setup(8);
    // Everything an observer could know: the account id, the allocation, the
    // salt, the distribution id. None of these is the secret.
    let account_id = derive_account_id(&derive_npk(&s.nsks[0]), 0);
    let real = compute_claim_nullifier(&DIST_ID, &s.nsks[0]);
    // A different secret, even for the same public account view, gives a
    // different nullifier, so the mapping cannot be inverted without nsk.
    let wrong_secret_guess = account_id; // an observer's best public-data guess
    let guessed = compute_claim_nullifier(&DIST_ID, &wrong_secret_guess);
    assert_ne!(real, guessed);
}

/// Binding 5, forgery face: a claimant cannot pair a member's committed leaf with
/// their own secret. Substituting the secret changes the derived account, so the
/// leaf no longer folds to the root.
#[test]
fn a_stolen_leaf_with_a_foreign_secret_is_rejected() {
    let s = setup(8);
    // Attacker takes victim 4's path and salt and allocation, but signs with
    // their own secret. account_id changes, so the leaf changes.
    let attacker_nsk = [0x77; 32];
    let w = ClaimWitness {
        nsk: attacker_nsk,
        identifier: 0,
        allocation: s.allocations[4],
        salt: s.salts[4],
        merkle_path: s.paths[4].1.clone(),
        leaf_index: s.paths[4].0,
        destination: DEST,
    };
    let st = ClaimStatement {
        distribution_root: s.root,
        distribution_id: DIST_ID,
        allocation: s.allocations[4],
        nullifier: compute_claim_nullifier(&DIST_ID, &attacker_nsk),
        destination: DEST,
    };
    assert_eq!(claim(&w, &st), Err(ClaimError::NotEligible));
}

/// The nullifier check itself: a claimant cannot substitute a nullifier of their
/// choosing while proving a real membership.
#[test]
fn a_forged_nullifier_is_rejected() {
    let s = setup(8);
    let w = witness_for(&s, 6);
    let mut st = statement_for(&s, 6);
    st.nullifier = [0x00; 32];
    assert_eq!(claim(&w, &st), Err(ClaimError::NullifierMismatch));
}

/// The destination is bound: a public destination that disagrees with the one
/// the witness commits is rejected, so a claim cannot be redirected after it is
/// built.
#[test]
fn a_redirected_destination_is_rejected() {
    let s = setup(8);
    let w = witness_for(&s, 1);
    let mut st = statement_for(&s, 1);
    st.destination = [0xBA; 32]; // a relayer trying to redirect the allocation
    assert_eq!(claim(&w, &st), Err(ClaimError::DestinationMismatch));
}
