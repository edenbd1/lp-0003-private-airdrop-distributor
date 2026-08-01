//! The full recipient path: build a real distribution tree, seal a bundle,
//! then, with only a recipient's secret and the published bundle, recover the
//! row and produce a claim the on-chain program would accept. This ties the
//! encryption layer to `airdrop_core::claim`, so a change to either that broke
//! the join would fail here.

use airdrop_core::{
    build_eligibility_tree, claim, compute_claim_nullifier, compute_eligibility_leaf,
    derive_account_id, derive_npk, ClaimStatement, ClaimWitness,
};
use airdrop_crypto::{decrypt_row, derive_enc_keypair, enc_public_key, encrypt_row, RowPayload};

const DIST_ID: [u8; 32] = [0xC3; 32];

#[test]
fn a_recipient_claims_from_only_the_bundle_and_their_secret() {
    // Distributor side: assign recipients, build the tree, seal the bundle.
    let n = 10usize;
    let mut nsks = Vec::new();
    let mut allocs = Vec::new();
    let mut salts = Vec::new();
    let mut leaves = Vec::new();
    for i in 0..n {
        let nsk = [i as u8 + 1; 32];
        let alloc = 100u128 + i as u128 * 7;
        let salt = [0x30 ^ i as u8; 32];
        let aid = derive_account_id(&derive_npk(&nsk), 0);
        leaves.push(compute_eligibility_leaf(&aid, alloc, &salt));
        nsks.push(nsk);
        allocs.push(alloc);
        salts.push(salt);
    }
    let (root, paths) = build_eligibility_tree(&leaves);

    let mut bundle: Vec<_> = (0..n)
        .map(|i| {
            let payload = RowPayload {
                allocation: allocs[i],
                salt: salts[i],
                leaf_index: paths[i].0,
                merkle_path: paths[i].1.clone(),
            };
            encrypt_row(
                &enc_public_key(&nsks[i]),
                &serde_json::to_vec(&payload).unwrap(),
                None,
            )
        })
        .collect();
    bundle.sort_by_key(|r| r.ephemeral_public); // publishable, order carries nothing

    // Recipient side: knows only DIST_ID, root, their own nsk, and the bundle.
    let me = 4usize;
    let my_nsk = nsks[me];
    let keys = derive_enc_keypair(&my_nsk);
    let recovered: RowPayload = bundle
        .iter()
        .find_map(|r| decrypt_row(&keys, r))
        .map(|pt| serde_json::from_slice(&pt).unwrap())
        .expect("exactly one row opens for me");

    let nullifier = compute_claim_nullifier(&DIST_ID, &my_nsk);
    let witness = ClaimWitness {
        nsk: my_nsk,
        identifier: 0,
        allocation: recovered.allocation,
        salt: recovered.salt,
        merkle_path: recovered.merkle_path,
        leaf_index: recovered.leaf_index,
    };
    let statement = ClaimStatement {
        distribution_root: root,
        distribution_id: DIST_ID,
        allocation: recovered.allocation,
        nullifier,
    };

    // The recovered row builds a claim the program accepts against the committed
    // root, and it is the right allocation.
    assert!(claim(&witness, &statement).is_ok());
    assert_eq!(recovered.allocation, allocs[me]);
}
