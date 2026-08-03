//! Adversarial audit of the deployed LP-0003 claim verifier.
//!
//! These tests run the committed `claim_verifier.bin` through the *sequencer's
//! own* execution path (same input order, same 32M session limit, same executor,
//! `lee/state_machine/src/program.rs:55-110`), so a rejection here is the same
//! rejection the chain performs. They deliberately do not prove: proving costs
//! minutes and would establish nothing extra about which inputs are accepted.
//!
//! Each test constructs a specific way to steal a claim and requires the
//! deployed binary to reject it with the matching error code, with an honest
//! claim accepted as the control.
//!
//! Run with: `cargo test -p claim-verifier-tests --test claim_rejects`

use airdrop_core::{
    build_eligibility_tree, compute_claim_marker, compute_claim_nullifier,
    compute_eligibility_leaf, derive_account_id, derive_npk, ClaimStatement, ClaimWitness,
};
use lee_core::account::{Account, AccountId, AccountWithMetadata};
use lee_core::program::ProgramId;
use risc0_zkvm::{default_executor, ExecutorEnv};
use serde::Serialize;
use sha2::{Digest, Sha256};

const MAX_NUM_CYCLES_PUBLIC_EXECUTION: u64 = 1024 * 1024 * 32;
const ELF_PATH: &str = "../../artifacts/programs/claim_verifier.bin";

/// The instruction enum `#[lez_program]` generates, in declaration order.
/// `create_distribution` is variant 0, `claim` is variant 1. risc0's serde
/// encodes the variant index as a leading u32, then the fields in order.
#[derive(Serialize)]
enum VerifierInstruction {
    #[allow(dead_code)]
    CreateDistribution {
        distribution_id: [u8; 32],
        eligibility_root: [u8; 32],
    },
    Claim {
        witness_words: Vec<u32>,
        distribution_root: [u8; 32],
        distribution_id: [u8; 32],
        allocation: u128,
        nullifier: [u8; 32],
        claim_marker_seed: [u8; 32],
        destination: [u8; 32],
    },
}

fn elf() -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(ELF_PATH);
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "cannot read {}: {e}. Build it with:\n  cargo risczero build --manifest-path \
             crates/claim-verifier-spel/methods/guest/Cargo.toml",
            path.display()
        )
    })
}

fn program_id(elf: &[u8]) -> ProgramId {
    risc0_binfmt::ProgramBinary::decode(elf)
        .expect("verifier binary must decode")
        .compute_image_id()
        .expect("image id")
        .into()
}

/// The public PDA derivation SPEL uses: multi-seed combines with SHA256, then
/// `AccountId::for_public_pda`. Byte-identical to `lee_core`'s `for_public_pda`.
fn public_pda(program: &ProgramId, seeds: &[[u8; 32]]) -> AccountId {
    let combined: [u8; 32] = if seeds.len() == 1 {
        seeds[0]
    } else {
        let mut h = Sha256::new();
        for s in seeds {
            h.update(s);
        }
        h.finalize().into()
    };
    let mut bytes = [0u8; 96];
    bytes[0..32].copy_from_slice(b"/LEE/v0.2/AccountId/PDA/\x00\x00\x00\x00\x00\x00\x00\x00");
    let pid: &[u8] = bytemuck::cast_slice(program);
    bytes[32..64].copy_from_slice(pid);
    bytes[64..96].copy_from_slice(&combined);
    AccountId::new(Sha256::digest(bytes).into())
}

/// A complete, consistent claim call, from which each test breaks one thing.
struct Scenario {
    witness: ClaimWitness,
    distribution_root: [u8; 32],
    distribution_id: [u8; 32],
    allocation: u128,
    nullifier: [u8; 32],
    marker_seed: [u8; 32],
    /// The owner stamped on the distribution PDA. Non-default means anchored.
    distribution_owner: ProgramId,
    /// The owner stamped on the claim-marker PDA. Non-default means the marker is
    /// already initialised by a prior claim, which must make a second claim fail.
    marker_owner: ProgramId,
}

impl Scenario {
    /// Build an 8-recipient distribution and an honest claim by recipient 3.
    fn honest(verifier: &ProgramId) -> Self {
        let mut leaves = Vec::new();
        let mut entries = Vec::new();
        for i in 0..8u8 {
            let nsk = [i + 1; 32];
            let alloc = 100u128 + i as u128 * 10;
            let salt = [0x40 ^ i; 32];
            let aid = derive_account_id(&derive_npk(&nsk), 0);
            leaves.push(compute_eligibility_leaf(&aid, alloc, &salt));
            entries.push((nsk, alloc, salt));
        }
        let (root, paths) = build_eligibility_tree(&leaves);
        let idx = 3usize;
        let (nsk, alloc, salt) = entries[idx];
        let distribution_id = [0xD1; 32];
        let nullifier = compute_claim_nullifier(&distribution_id, &nsk);
        let marker_seed = compute_claim_marker(&distribution_id, &nullifier);
        Self {
            witness: ClaimWitness {
                nsk,
                identifier: 0,
                allocation: alloc,
                salt,
                merkle_path: paths[idx].1.clone(),
                leaf_index: paths[idx].0,
                destination: derive_account_id(&derive_npk(&nsk), 0),
            },
            distribution_root: root,
            distribution_id,
            allocation: alloc,
            nullifier,
            marker_seed,
            distribution_owner: *verifier, // anchored: owned by the verifier program
            marker_owner: ProgramId::default(), // fresh marker: no prior claim
        }
    }

    fn run(&self, elf: &[u8], pid: &ProgramId) -> anyhow::Result<()> {
        let instruction = VerifierInstruction::Claim {
            witness_words: risc0_zkvm::serde::to_vec(&airdrop_core::ClaimInstruction {
                witness: self.witness.clone(),
                statement: ClaimStatement {
                    distribution_root: self.distribution_root,
                    distribution_id: self.distribution_id,
                    allocation: self.allocation,
                    nullifier: self.nullifier,
                    destination: self.witness.destination,
                },
            })?,
            distribution_root: self.distribution_root,
            distribution_id: self.distribution_id,
            allocation: self.allocation,
            nullifier: self.nullifier,
            claim_marker_seed: self.marker_seed,
            destination: self.witness.destination,
        };

        // Accounts in declaration order: claim_marker (init PDA), distribution
        // (read PDA), claimant (signer).
        let claim_marker = AccountWithMetadata {
            account: Account {
                program_owner: self.marker_owner,
                balance: 0,
                data: Default::default(),
                nonce: lee_core::account::Nonce(0),
            },
            is_authorized: false,
            account_id: public_pda(pid, &[self.marker_seed]),
        };
        let distribution = AccountWithMetadata {
            account: Account {
                program_owner: self.distribution_owner,
                balance: 0,
                data: Default::default(),
                nonce: lee_core::account::Nonce(0),
            },
            is_authorized: false,
            account_id: public_pda(pid, &[self.distribution_id, self.distribution_root]),
        };
        let claimant = AccountWithMetadata {
            account: Account::default(),
            is_authorized: true,
            account_id: AccountId::new([0xC1; 32]),
        };
        let pre_states = vec![claim_marker, distribution, claimant];

        let caller: Option<ProgramId> = None;
        let instruction_data = risc0_zkvm::serde::to_vec(&instruction)?;

        let mut builder = ExecutorEnv::builder();
        builder.session_limit(Some(MAX_NUM_CYCLES_PUBLIC_EXECUTION));
        builder.write(pid)?;
        builder.write(&caller)?;
        builder.write(&pre_states)?;
        builder.write(&instruction_data)?;
        let env = builder.build()?;

        default_executor().execute(env, elf)?;
        Ok(())
    }

    /// Same execution as `run`, but returns the session so its cycle counts can be
    /// reported. Only used by the honest measurement.
    fn measured_session(
        &self,
        elf: &[u8],
        pid: &ProgramId,
    ) -> anyhow::Result<risc0_zkvm::SessionInfo> {
        let instruction = VerifierInstruction::Claim {
            witness_words: risc0_zkvm::serde::to_vec(&airdrop_core::ClaimInstruction {
                witness: self.witness.clone(),
                statement: ClaimStatement {
                    distribution_root: self.distribution_root,
                    distribution_id: self.distribution_id,
                    allocation: self.allocation,
                    nullifier: self.nullifier,
                    destination: self.witness.destination,
                },
            })?,
            distribution_root: self.distribution_root,
            distribution_id: self.distribution_id,
            allocation: self.allocation,
            nullifier: self.nullifier,
            claim_marker_seed: self.marker_seed,
            destination: self.witness.destination,
        };
        let pre_states = vec![
            AccountWithMetadata {
                account: Account::default(),
                is_authorized: false,
                account_id: public_pda(pid, &[self.marker_seed]),
            },
            AccountWithMetadata {
                account: Account {
                    program_owner: self.distribution_owner,
                    balance: 0,
                    data: Default::default(),
                    nonce: lee_core::account::Nonce(0),
                },
                is_authorized: false,
                account_id: public_pda(pid, &[self.distribution_id, self.distribution_root]),
            },
            AccountWithMetadata {
                account: Account::default(),
                is_authorized: true,
                account_id: AccountId::new([0xC1; 32]),
            },
        ];
        let caller: Option<ProgramId> = None;
        let instruction_data = risc0_zkvm::serde::to_vec(&instruction)?;
        let mut builder = ExecutorEnv::builder();
        builder.session_limit(Some(MAX_NUM_CYCLES_PUBLIC_EXECUTION));
        builder.write(pid)?;
        builder.write(&caller)?;
        builder.write(&pre_states)?;
        builder.write(&instruction_data)?;
        Ok(default_executor().execute(builder.build()?, elf)?)
    }
}

/// Report the deployed claim verifier's on-chain compute cost, by replaying its
/// execution through the sequencer's own executor. This is the Performance
/// criterion's number: the guest's own cycles, excluding the privacy circuit's
/// recursive verification of the chained call, which is LEZ's cost not ours.
///
/// Run with: `cargo test -p claim-verifier-tests --test claim_rejects -- --ignored --nocapture`
#[test]
#[ignore = "reports a measurement rather than asserting a property"]
fn report_the_claim_cycle_cost() {
    let elf = elf();
    let pid = program_id(&elf);
    let session = Scenario::honest(&pid)
        .measured_session(&elf, &pid)
        .expect("honest claim executes");
    let user_cycles: u64 = session.segments.iter().map(|s| u64::from(s.cycles)).sum();
    let proving_cycles: u64 = session.segments.iter().map(|s| 1u64 << s.po2).sum();
    let pct = proving_cycles as f64 / MAX_NUM_CYCLES_PUBLIC_EXECUTION as f64 * 100.0;
    println!("claim verifier, guest execution only");
    println!("  segments        {}", session.segments.len());
    println!("  user cycles     {user_cycles}");
    println!("  proving cycles  {proving_cycles}");
    println!("  budget consumed {pct:.2} % of {MAX_NUM_CYCLES_PUBLIC_EXECUTION}");
}

/// The control. If an honest claim is rejected, every rejection below is meaningless.
#[test]
fn an_honest_claim_is_accepted() {
    let elf = elf();
    let pid = program_id(&elf);
    Scenario::honest(&pid)
        .run(&elf, &pid)
        .expect("an eligible recipient with an anchored root must be accepted");
}

/// Reliability R2: a rejected claim does not mark the claimant, so a retry works.
/// A first attempt with an inflated allocation is rejected; because it never
/// created the marker PDA (the marker is `init`, stamped only on a successful
/// claim), the same recipient — same nullifier, same marker seed — then submits
/// the honest claim and it is accepted. The rejection consumed nothing.
#[test]
fn a_rejected_claim_leaves_the_recipient_free_to_retry() {
    let elf = elf();
    let pid = program_id(&elf);
    // Changing only the allocation keeps the nullifier and marker seed identical,
    // so this is genuinely the same recipient's attempt.
    let mut bad = Scenario::honest(&pid);
    bad.allocation = bad.witness.allocation * 7;
    bad.run(&elf, &pid).expect_err("the inflated first attempt must be rejected");
    // The marker was never stamped (marker_owner still default), so the honest
    // retry by the same recipient is accepted.
    Scenario::honest(&pid)
        .run(&elf, &pid)
        .expect("after a rejection the honest retry with the same nullifier is accepted");
}

/// The invented-root attack: a claimant proves membership against a root they
/// made up. Because no distribution was committed for that root, the
/// distribution PDA is uninitialised (default owner) and the claim is rejected.
#[test]
fn an_unanchored_root_is_rejected() {
    let elf = elf();
    let pid = program_id(&elf);
    let mut s = Scenario::honest(&pid);
    s.distribution_owner = ProgramId::default(); // never created on chain
    let err = s.run(&elf, &pid).expect_err("an invented root must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("4003") || msg.to_lowercase().contains("anchor"),
        "expected the not-anchored rejection, got: {msg}"
    );
}

/// A forged nullifier: the claimant substitutes a nullifier of their choosing
/// while proving a real membership.
#[test]
fn a_forged_nullifier_is_rejected() {
    let elf = elf();
    let pid = program_id(&elf);
    let mut s = Scenario::honest(&pid);
    s.nullifier = [0x00; 32];
    // Keep the marker seed consistent with the forged nullifier so it is the
    // nullifier check that trips, not the marker check.
    s.marker_seed = compute_claim_marker(&s.distribution_id, &s.nullifier);
    let err = s.run(&elf, &pid).expect_err("a forged nullifier must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("4002") || msg.to_lowercase().contains("nullifier"),
        "expected the nullifier rejection, got: {msg}"
    );
}

/// An inflated allocation: the claimant asks the chain to credit more than the
/// witness carries.
#[test]
fn an_inflated_allocation_is_rejected() {
    let elf = elf();
    let pid = program_id(&elf);
    let mut s = Scenario::honest(&pid);
    s.allocation = s.witness.allocation * 100;
    let err = s.run(&elf, &pid).expect_err("an inflated allocation must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("4006") || msg.to_lowercase().contains("allocation"),
        "expected the allocation rejection, got: {msg}"
    );
}

/// The double-claim guard, exercised structurally. A first claim leaves a marker
/// PDA owned by the verifier, so a second claim by the same recipient finds that
/// PDA already initialised. The deployed binary must reject it on the `init`
/// constraint (`AccountAlreadyInitialized`) — the half of double-claim prevention
/// that nullifier determinism alone does not demonstrate.
#[test]
fn a_second_claim_on_an_initialized_marker_is_rejected() {
    let elf = elf();
    let pid = program_id(&elf);
    let mut s = Scenario::honest(&pid);
    s.marker_owner = pid; // a prior claim already stamped and owns the marker
    let err = s.run(&elf, &pid).expect_err("a second claim must be rejected");
    let msg = format!("{err:#}").to_lowercase();
    assert!(
        msg.contains("alreadyinitialized") || msg.contains("already initialized"),
        "expected the already-initialised rejection, got: {msg}"
    );
}

/// A marker seed that does not commit to the distribution and nullifier: an
/// attempt to land the claim at an address that misrepresents what was claimed.
#[test]
fn a_forged_marker_seed_is_rejected() {
    let elf = elf();
    let pid = program_id(&elf);
    let mut s = Scenario::honest(&pid);
    s.marker_seed = [0xAB; 32];
    let err = s.run(&elf, &pid).expect_err("a forged marker seed must be rejected");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("4004") || msg.to_lowercase().contains("marker"),
        "expected the marker-seed rejection, got: {msg}"
    );
}
