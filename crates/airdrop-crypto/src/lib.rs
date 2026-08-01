//! Encrypted eligibility bundles for LP-0003.
//!
//! A distributor turns each recipient's private claim data (allocation, salt,
//! leaf index, Merkle path) into an encrypted row addressed to that recipient,
//! and publishes the whole set as a bundle. Because every row is sealed to a key
//! only the recipient can derive, the bundle can be posted in the open without
//! revealing who is eligible or what they are owed. A recipient scans the
//! bundle, decrypts the one row that opens for them, and checks that the
//! recovered data reconstructs the exact leaf committed on chain before spending
//! a proof on it.
//!
//! WHY THIS IS BUILT THE WAY IT IS
//!
//!   * **One root secret, separated keys.** The recipient holds a single secret,
//!     `nsk`, which already derives their nullifier and account. The encryption
//!     key is a *separate* X25519 key derived from `nsk` through a domain
//!     separator, so a decryption key can never be confused with a signing or
//!     nullifier secret. Reusing `nsk` directly for encryption would be key
//!     reuse across primitives; deriving through a domain tag avoids it while
//!     keeping the recipient's key material to one secret.
//!
//!   * **Authenticated encryption.** Each row is sealed with ChaCha20-Poly1305,
//!     so a tampered row fails to open rather than yielding garbage a claimant
//!     might act on.
//!
//!   * **Sender-anonymous, receiver-testable.** Rows carry an ephemeral public
//!     key, so the distributor is not linkable across rows, and a recipient
//!     simply trial-opens each row; the one whose tag verifies is theirs.
//!
//!   * **The plaintext is not trusted.** Decryption is only a delivery channel.
//!     The recovered `(allocation, salt, leaf_index, path)` is checked against
//!     the on-chain committed root by `airdrop_core::claim` before any claim is
//!     built, so a malicious distributor cannot make a recipient prove a leaf
//!     that is not actually in the set.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};

/// Domain separator deriving the recipient's encryption secret from `nsk`.
/// Keeps the encryption key distinct from the nullifier/account key material.
const ENC_KEY_PREFIX: &[u8] = b"/lp-0003/v0.1/enc-key-from-nsk/";

/// The recipient's encryption secret and public key, both derived from `nsk`.
pub struct EncKeypair {
    secret: StaticSecret,
    pub public: [u8; 32],
}

/// Derive the recipient's X25519 encryption keypair from their root secret.
#[must_use]
pub fn derive_enc_keypair(nsk: &[u8; 32]) -> EncKeypair {
    let mut h = Sha256::new();
    h.update(ENC_KEY_PREFIX);
    h.update(nsk);
    let seed: [u8; 32] = h.finalize().into();
    let secret = StaticSecret::from(seed);
    let public = PublicKey::from(&secret).to_bytes();
    EncKeypair { secret, public }
}

/// Just the recipient's public encryption key, for a distributor who only knows
/// public data about the recipient.
#[must_use]
pub fn enc_public_key(nsk: &[u8; 32]) -> [u8; 32] {
    derive_enc_keypair(nsk).public
}

/// One sealed row: an ephemeral public key, a nonce, and the AEAD ciphertext.
/// Publishable; only the addressed recipient can open it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EncryptedRow {
    pub ephemeral_public: [u8; 32],
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

fn kdf(shared: &[u8; 32], ephemeral_public: &[u8; 32]) -> [u8; 32] {
    // Bind the symmetric key to the ephemeral public key so a row cannot be
    // replayed under a different header.
    let mut h = Sha256::new();
    h.update(b"/lp-0003/v0.1/row-kdf/");
    h.update(shared);
    h.update(ephemeral_public);
    h.finalize().into()
}

fn os_random(buf: &mut [u8]) {
    getrandom::getrandom(buf).expect("OS randomness");
}

/// Seal `plaintext` to `recipient_public`. Anyone can call this; only the
/// recipient can open the result. `ephemeral_seed` makes it deterministic for
/// tests; pass `None` for a fresh ephemeral key.
#[must_use]
pub fn encrypt_row(
    recipient_public: &[u8; 32],
    plaintext: &[u8],
    ephemeral_seed: Option<[u8; 32]>,
) -> EncryptedRow {
    let eph_seed = ephemeral_seed.unwrap_or_else(|| {
        let mut s = [0u8; 32];
        os_random(&mut s);
        s
    });
    let eph_secret = StaticSecret::from(eph_seed);
    let eph_public = PublicKey::from(&eph_secret).to_bytes();

    let shared = eph_secret
        .diffie_hellman(&PublicKey::from(*recipient_public))
        .to_bytes();
    let key = kdf(&shared, &eph_public);
    let cipher = ChaCha20Poly1305::new((&key).into());

    let mut nonce_bytes = [0u8; 12];
    os_random(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .expect("ChaCha20-Poly1305 encryption does not fail on valid input");

    EncryptedRow {
        ephemeral_public: eph_public,
        nonce: nonce_bytes,
        ciphertext,
    }
}

/// Try to open `row` with the recipient's keypair. Returns the plaintext if this
/// row is addressed to them (the AEAD tag verifies), or `None` otherwise. A
/// recipient calls this on each row in a bundle; the one that returns `Some` is
/// theirs.
#[must_use]
pub fn decrypt_row(keys: &EncKeypair, row: &EncryptedRow) -> Option<Vec<u8>> {
    let shared = keys
        .secret
        .diffie_hellman(&PublicKey::from(row.ephemeral_public))
        .to_bytes();
    let key = kdf(&shared, &row.ephemeral_public);
    let cipher = ChaCha20Poly1305::new((&key).into());
    cipher
        .decrypt(Nonce::from_slice(&row.nonce), row.ciphertext.as_ref())
        .ok()
}

/// The claim data a distributor seals into each row. Serialised compactly so the
/// recipient recovers exactly what they need to build a claim.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RowPayload {
    pub allocation: u128,
    pub salt: [u8; 32],
    pub leaf_index: u64,
    pub merkle_path: Vec<[u8; 32]>,
}
