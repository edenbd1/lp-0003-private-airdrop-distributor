//! The distributor's tree builder must produce paths that actually fold to the
//! root, for any set size, not just powers of two.
use airdrop_core::{
    build_eligibility_tree, claim, compute_claim_nullifier, compute_eligibility_leaf,
    derive_account_id, derive_npk, ClaimStatement, ClaimWitness,
};

const DIST_ID: [u8; 32] = [0x33; 32];

fn roundtrip_for_size(n: usize) {
    let mut nsks = Vec::new();
    let mut allocs = Vec::new();
    let mut salts = Vec::new();
    let mut leaves = Vec::new();
    for i in 0..n {
        let nsk = [i as u8 + 1; 32];
        let alloc = 1u128 + i as u128;
        let salt = [i as u8; 32];
        let aid = derive_account_id(&derive_npk(&nsk), 0);
        leaves.push(compute_eligibility_leaf(&aid, alloc, &salt));
        nsks.push(nsk); allocs.push(alloc); salts.push(salt);
    }
    let (root, paths) = build_eligibility_tree(&leaves);
    for i in 0..n {
        let w = ClaimWitness {
            nsk: nsks[i], identifier: 0, allocation: allocs[i], salt: salts[i],
            merkle_path: paths[i].1.clone(), leaf_index: paths[i].0,
            destination: [0xDE; 32],
        };
        let st = ClaimStatement {
            distribution_root: root, distribution_id: DIST_ID,
            allocation: allocs[i], nullifier: compute_claim_nullifier(&DIST_ID, &nsks[i]),
            destination: [0xDE; 32],
        };
        claim(&w, &st).unwrap_or_else(|e| panic!("size {n}, leaf {i}: {e:?}"));
    }
}

#[test]
fn every_size_from_1_to_33_roundtrips() {
    for n in 1..=33 {
        roundtrip_for_size(n);
    }
}

#[test]
fn a_realistic_set_of_100_roundtrips() {
    roundtrip_for_size(100);
}
