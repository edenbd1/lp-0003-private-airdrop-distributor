//! Adversarial audit of the encrypted eligibility bundle.
//!
//! The bundle is a delivery channel, so its job is narrow but sharp: a row must
//! open for exactly its recipient and no one else, a tampered row must not open,
//! and the recovered data must reconstruct the same on-chain leaf. Each of these
//! gets a test.

use airdrop_core::{compute_eligibility_leaf, derive_account_id, derive_npk};
use airdrop_crypto::{
    decrypt_row, derive_enc_keypair, enc_public_key, encrypt_row, RowPayload,
};

fn payload() -> Vec<u8> {
    let p = RowPayload {
        allocation: 4242,
        salt: [0x11; 32],
        leaf_index: 7,
        merkle_path: vec![[1u8; 32], [2u8; 32], [3u8; 32]],
    };
    serde_json::to_vec(&p).unwrap()
}

/// The recipient can open the row addressed to them.
#[test]
fn the_addressed_recipient_opens_their_row() {
    let nsk = [9u8; 32];
    let epk = enc_public_key(&nsk);
    let row = encrypt_row(&epk, &payload(), None);
    let opened = decrypt_row(&derive_enc_keypair(&nsk), &row).expect("should open");
    assert_eq!(opened, payload());
}

/// A different recipient cannot open someone else's row.
#[test]
fn a_different_recipient_cannot_open_the_row() {
    let alice = [1u8; 32];
    let bob = [2u8; 32];
    let row = encrypt_row(&enc_public_key(&alice), &payload(), None);
    assert!(
        decrypt_row(&derive_enc_keypair(&bob), &row).is_none(),
        "Bob must not open Alice's row"
    );
}

/// A tampered ciphertext fails to open rather than yielding usable garbage.
#[test]
fn a_tampered_row_does_not_open() {
    let nsk = [5u8; 32];
    let mut row = encrypt_row(&enc_public_key(&nsk), &payload(), None);
    row.ciphertext[0] ^= 0xFF;
    assert!(decrypt_row(&derive_enc_keypair(&nsk), &row).is_none());
}

/// A swapped ephemeral header also fails: the key is bound to it.
#[test]
fn a_swapped_header_does_not_open() {
    let nsk = [6u8; 32];
    let mut row = encrypt_row(&enc_public_key(&nsk), &payload(), None);
    row.ephemeral_public[0] ^= 0x01;
    assert!(decrypt_row(&derive_enc_keypair(&nsk), &row).is_none());
}

/// The encryption key is domain-separated from the root secret: it is not `nsk`
/// itself, and two recipients get different keys.
#[test]
fn keys_are_separated_and_distinct() {
    let a = enc_public_key(&[1u8; 32]);
    let b = enc_public_key(&[2u8; 32]);
    assert_ne!(a, b, "distinct secrets give distinct encryption keys");
    assert_ne!(a, [1u8; 32], "the encryption key is not the raw nsk");
}

/// End to end: the recovered payload reconstructs the exact leaf the distributor
/// committed, so a recipient can verify their row against the on-chain root
/// before spending a proof.
#[test]
fn the_recovered_payload_reconstructs_the_committed_leaf() {
    let nsk = [7u8; 32];
    let account_id = derive_account_id(&derive_npk(&nsk), 0);
    let alloc = 999u128;
    let salt = [0x77u8; 32];
    let committed_leaf = compute_eligibility_leaf(&account_id, alloc, &salt);

    // The distributor seals the row.
    let p = RowPayload { allocation: alloc, salt, leaf_index: 0, merkle_path: vec![] };
    let row = encrypt_row(&enc_public_key(&nsk), &serde_json::to_vec(&p).unwrap(), None);

    // The recipient opens it and reconstructs the leaf.
    let opened: RowPayload =
        serde_json::from_slice(&decrypt_row(&derive_enc_keypair(&nsk), &row).unwrap()).unwrap();
    let recovered_leaf =
        compute_eligibility_leaf(&account_id, opened.allocation, &opened.salt);
    assert_eq!(recovered_leaf, committed_leaf);
}
