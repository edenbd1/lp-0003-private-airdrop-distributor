// LP-0003 on-chain airdrop claim verifier.
//
// WHAT MAKES THE CLAIM REAL ON CHAIN
//
// A LEZ public transaction re-executes rather than proves
// (`lee/state_machine/src/program.rs:73-77`), so no program on the public path
// can verify a claim proof. This program targets the privacy-preserving path,
// where LEZ's circuit composes each chained call with a real `env::verify` over
// the callee's `ProgramOutput`
// (`lee/privacy_preserving_circuit/src/execution_state.rs:149`) and the sequencer
// checks the receipt against the pinned `PRIVACY_PRESERVING_CIRCUIT_ID`. The
// `claim` instruction declares a ChainedCall to the LEZ-native claim program, so
// the membership proof is genuinely verified on chain as a precondition of the
// transaction being accepted.
//
// WHY THE ROOT IS ANCHORED, NOT PROVER-CHOSEN
//
// The claim proof establishes membership against whatever root the statement
// names. On its own that is not enough: a claimant could invent a one-leaf tree
// holding themselves. The distribution account is a PDA whose address is derived
// from `[distribution_id, eligibility_root]`. `create_distribution` initialises
// exactly that PDA, so it becomes owned by this program. `claim` references the
// distribution as the PDA for `[distribution_id, distribution_root]` and requires
// it to be owned by this program. An invented root gives a different PDA address
// that was never initialised, so its owner is the default and the claim is
// rejected. The root is therefore anchored by the distributor's on-chain
// commitment, the property LP-0005's balance attestation could not have.
//
// PRIVACY AND UNLINKABILITY
//
// A privacy `Message` publishes neither `program_id` nor `instruction_data`
// (`privacy_preserving_transaction/message.rs:14-24`). The only public trace a
// claim leaves is the marker PDA, seeded by
// `SHA256(CLAIM_MARKER_PREFIX || distribution_id || nullifier)`. The nullifier is
// `SHA256(CLAIM_NULLIFIER_PREFIX || distribution_id || nsk)`, a function of the
// recipient's secret, so an observer who knows every candidate address still
// cannot link the marker to one of them. Claiming the marker requires it to be
// uninitialised, which is the double-claim guard: a second claim by the same
// recipient in the same distribution targets the same address and fails.

#![no_main]

use spel_framework::prelude::*;

risc0_zkvm::guest::entry!(main);

const E_BAD_WITNESS: u32 = 4001;
const E_NULLIFIER_MISMATCH: u32 = 4002;
const E_ROOT_NOT_ANCHORED: u32 = 4003;
const E_MARKER_SEED_MISMATCH: u32 = 4004;
const E_ALLOCATION_MISMATCH: u32 = 4006;

/// ProgramId of the LEZ-native claim program (`claim_lez.bin`,
/// ImageID `1f6a0ec0036ec2c0c23dcc8380aeb9be26128608db3bb6f5b10d3f30eb249508`).
/// The deployment is content-addressed, so this pins exactly the audited binary.
///
/// Verify with:
///   spel program-id artifacts/programs/claim_lez.bin
pub const CLAIM_LEZ_PROGRAM_ID: nssa_core::program::ProgramId = [
    3222170143, 3233967619, 2211200450, 3199839872, 143004198, 4122360795, 809438641, 143992043,
];

#[lez_program]
mod claim_verifier {
    #[allow(unused_imports)]
    use super::*;

    /// Publish an eligibility set. Anyone can create a distribution; it is theirs,
    /// funded by them, and independent of every other. What matters is that a
    /// given `(distribution_id, root)` maps to exactly one on-chain PDA, which
    /// `init` guarantees by refusing to overwrite an existing account.
    ///
    /// Accounts:
    /// - `distribution` (init, PDA seeded by `[distribution_id, eligibility_root]`):
    ///   the on-chain commitment of the eligibility root. Its address encodes the
    ///   root, so nothing needs to be written into its data.
    /// - `authority` (signer): the distributor.
    #[instruction]
    pub fn create_distribution(
        #[account(init, pda = [arg("distribution_id"), arg("eligibility_root")])]
        distribution: AccountWithMetadata,
        #[account(signer)]
        authority: AccountWithMetadata,
        distribution_id: [u8; 32],
        eligibility_root: [u8; 32],
    ) -> SpelResult {
        // No chained call and no data to write: the PDA address is the commitment.
        Ok(SpelOutput::execute(vec![distribution, authority], vec![]))
    }

    /// Claim an allocation by proving membership in a committed eligibility set,
    /// without revealing which entry.
    ///
    /// Accounts:
    /// - `claim_marker` (init, PDA seeded by `claim_marker_seed`): the public,
    ///   replay-guarded record that this recipient claimed this distribution.
    /// - `distribution` (PDA seeded by `[distribution_id, distribution_root]`):
    ///   the anchored root. Required to be owned by this program, so an invented
    ///   root, whose PDA was never initialised, is rejected.
    /// - `claimant` (signer): the account submitting the claim.
    ///
    /// Args:
    /// - `witness_words`: the claim witness, risc0-serde encoded. Carried to the
    ///   chained call. Safe only because a privacy transaction publishes no
    ///   instruction data; this must never be invoked on the public path.
    /// - `distribution_root`, `distribution_id`, `allocation`, `nullifier`: the
    ///   public statement the proof establishes.
    /// - `claim_marker_seed`: the marker PDA seed, re-derived here.
    /// - `destination`: where the allocation is destined. The chained proof binds
    ///   it to the witness, so the submission cannot redirect it.
    #[instruction]
    pub fn claim(
        #[account(init, pda = arg("claim_marker_seed"))]
        claim_marker: AccountWithMetadata,
        #[account(pda = [arg("distribution_id"), arg("distribution_root")])]
        distribution: AccountWithMetadata,
        #[account(signer)]
        claimant: AccountWithMetadata,
        witness_words: Vec<u32>,
        distribution_root: [u8; 32],
        distribution_id: [u8; 32],
        allocation: u128,
        nullifier: [u8; 32],
        claim_marker_seed: [u8; 32],
        destination: [u8; 32],
    ) -> SpelResult {
        // 1. Decode the witness and rebuild the statement the proof establishes.
        let witness: airdrop_core::ClaimWitness = risc0_zkvm::serde::from_slice(&witness_words)
            .map_err(|_| SpelError::custom(E_BAD_WITNESS, "witness_words did not decode"))?;

        let statement = airdrop_core::ClaimStatement {
            distribution_root,
            distribution_id,
            allocation,
            nullifier,
            // The claim's allocation is bound to this destination. The chained
            // proof establishes that the witness commits the same destination,
            // so the submission cannot redirect the allocation.
            destination,
        };

        // 2. Re-derive the nullifier from the witness secret and the distribution,
        //    and require it to equal the pinned one. Without this a caller could
        //    prove one claim while occupying another's marker.
        let derived_nullifier =
            airdrop_core::compute_claim_nullifier(&distribution_id, &witness.nsk);
        if derived_nullifier != nullifier {
            return Err(SpelError::custom(
                E_NULLIFIER_MISMATCH,
                "nullifier does not match the supplied witness",
            ));
        }

        // 3. The allocation claimed must be the one sealed in the witness leaf.
        if witness.allocation != allocation {
            return Err(SpelError::custom(
                E_ALLOCATION_MISMATCH,
                "allocation does not match the witness",
            ));
        }

        // 4. Anchor the root. The macro's `pda = [distribution_id, distribution_root]`
        //    constraint already guarantees `distribution` is the PDA for exactly
        //    the claimed root. Requiring it to be owned by this program rejects an
        //    invented root: only `create_distribution` initialises these PDAs, and
        //    it does so for the real committed root, so a fabricated root lands on
        //    an uninitialised address whose owner is the default.
        if distribution.account.program_owner == nssa_core::program::DEFAULT_PROGRAM_ID {
            return Err(SpelError::custom(
                E_ROOT_NOT_ANCHORED,
                "no distribution is committed for this (id, root): the root is not anchored",
            ));
        }

        // 5. Bind the marker address to the distribution and the nullifier, so the
        //    public trace records which distribution was claimed and can be checked
        //    by an integrator against exactly this recipient's marker.
        let expected_seed = airdrop_core::compute_claim_marker(&distribution_id, &nullifier);
        if expected_seed != claim_marker_seed {
            return Err(SpelError::custom(
                E_MARKER_SEED_MISMATCH,
                "claim_marker_seed does not commit to the distribution and nullifier",
            ));
        }

        // 6. Declare the chained call. The privacy circuit executes and proves the
        //    claim program, then discharges the assumption with env::verify over
        //    its ProgramOutput. Merkle membership against `distribution_root` and
        //    the nullifier derivation are proved there, in zero knowledge.
        let instruction = airdrop_core::ClaimInstruction { witness, statement };
        let chained = vec![nssa_core::program::ChainedCall::new(
            CLAIM_LEZ_PROGRAM_ID,
            Vec::new(),
            &instruction,
        )];

        // 7. Claim the marker PDA; pass distribution and claimant through
        //    unchanged. The marker's existence, owned by this program, is the
        //    proof-of-claim an integration consumes.
        Ok(SpelOutput::execute(
            vec![claim_marker, distribution, claimant],
            chained,
        ))
    }
}
