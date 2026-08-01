// LP-0003 airdrop claim as a native LEZ program.
//
// WHY THIS EXISTS
//
// The one path on LEZ v0.2.0 where a proof is genuinely verified on chain is the
// privacy-preserving transaction: the client proves locally, and LEZ's privacy
// circuit composes each chained call with a real `env::verify` over the callee's
// `ProgramOutput` (`lee/privacy_preserving_circuit/src/execution_state.rs:149`),
// which the sequencer then checks against the pinned
// `PRIVACY_PRESERVING_CIRCUIT_ID`. To take part in that composition, the claim
// proof has to *be* a LEZ program: read via `read_lee_inputs`, emit a
// `ProgramOutput`. That is this file.
//
// PRIVACY
//
// The witness travels in the instruction. That is safe only on the privacy path:
// a privacy `Message` publishes commitments and nullifiers but carries neither
// `program_id` nor `instruction_data`
// (`lee/state_machine/src/privacy_preserving_transaction/message.rs:14-24`). A
// public `Message` publishes `instruction_data` verbatim, so this program must
// never be invoked on the public path. It is not: the verifier reaches it through
// a ChainedCall inside a privacy transaction.
//
// WHAT IT PROVES
//
// Via the shared `airdrop_core::claim`:
//   1. npk        = SHA256(NPK_DERIVE_PREFIX || nsk)
//   2. account_id = SHA256(PRIVATE_ACCOUNT_ID_PREFIX || npk || identifier_LE)
//   3. leaf       = SHA256(ELIGIBILITY_LEAF_PREFIX || account_id || allocation_LE
//                          || salt)
//   4. leaf folds via merkle_path to statement.distribution_root
//   5. nullifier  = SHA256(CLAIM_NULLIFIER_PREFIX || distribution_id || nsk)
//
// A false claim makes `claim` return an error, which this program turns into a
// panic. A panic aborts the guest, so no proof exists for a claim that is not
// eligible, over-allocates, or forges a nullifier. The verifier program then
// checks, on chain, that statement.distribution_root equals the distributor's
// committed root, which is what makes the anchored-root guarantee real.

#![no_main]

risc0_zkvm::guest::entry!(main);

use airdrop_core::{claim, ClaimInstruction};
use lee_core::program::{read_lee_inputs, AccountPostState, ProgramInput, ProgramOutput};

fn main() {
    let (
        ProgramInput {
            self_program_id,
            caller_program_id,
            pre_states,
            instruction,
        },
        instruction_words,
    ) = read_lee_inputs::<ClaimInstruction>();

    // The whole zero-knowledge statement. Returns an error, which we escalate to
    // a panic, if the claim is not eligible or is internally inconsistent, so no
    // proof can be produced for it.
    let leaf = claim(&instruction.witness, &instruction.statement)
        .unwrap_or_else(|e| panic!("claim is not valid: {e:?}"));

    // A claimed leaf is never the zero hash, and the nullifier the caller pins is
    // proved equal to the one this witness yields (checked inside `claim`), so a
    // caller cannot claim a marker for one nullifier while having proved another.
    assert!(leaf != [0u8; 32], "leaf must be non-zero");
    assert!(
        instruction.statement.nullifier != [0u8; 32],
        "nullifier must be non-zero for the caller's PDA seed",
    );

    // The claim itself moves no balances: any token transfer is the verifier
    // program's job on the public post-state. Every post-state is its pre-state
    // unchanged, which satisfies every rule in `validate_execution`.
    let post_states: Vec<AccountPostState> = pre_states
        .iter()
        .map(|pre| AccountPostState::new(pre.account.clone()))
        .collect();

    ProgramOutput::new(
        self_program_id,
        caller_program_id,
        instruction_words,
        pre_states,
        post_states,
    )
    .write();
}
