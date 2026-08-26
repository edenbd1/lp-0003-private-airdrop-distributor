# Error codes

Deterministic, documented codes for every invalid-claim scenario, as required by
the LP-0003 reliability criterion. These are the codes the **deployed** claim
verifier returns, so they are what an integrator sees from the chain.

## Deployed claim verifier (`4xxx`)

Defined in
`crates/claim-verifier-spel/methods/guest/src/bin/claim_verifier.rs`.

| Code | Constant | When it fires | What the claimant should do | The test that watches it fire |
|---:|---|---|---|---|
| `4001` | `E_BAD_WITNESS` | `witness_words` did not decode as `ClaimWitness` | Wire-format drift; regenerate the claim arguments with the current CLI. | `undecodable_witness_words_are_refused_before_anything_else_is_read` |
| `4002` | `E_NULLIFIER_MISMATCH` | The pinned `nullifier` is not the one the supplied witness yields | An attempt to prove one entry while occupying another's marker. Reject. | `a_forged_nullifier_is_rejected` |
| `4003` | `E_ROOT_NOT_ANCHORED` | No distribution is committed for this `(distribution_id, root)`: the distribution PDA is uninitialised | The root was not committed by a distributor, or the claimant invented it. Reject. | `an_unanchored_root_is_rejected` |
| `4004` | `E_MARKER_SEED_MISMATCH` | `claim_marker_seed` is not `compute_claim_marker(distribution_id, nullifier)` | A forged marker seed, i.e. landing the claim at an address that misrepresents what was claimed. Reject. | `a_forged_marker_seed_is_rejected` |
| `4006` | `E_ALLOCATION_MISMATCH` | The claimed `allocation` is not the one sealed in the witness | An attempt to claim more than was granted. Reject. | `an_inflated_allocation_is_rejected` |

Every code above has a test that watches it fire against the **deployed**
`claim_verifier.bin`, named in the last column and living in
`crates/claim-verifier-tests/tests/claim_rejects.rs`. `4001` was the last one
without: it fires on the witness decode, before the statement, the anchoring or
the marker are looked at, so no mutation of a well-formed claim reaches it and
the test has to supply the undecodable words itself.

`4005` and `4007` are reserved. Numbering is stable; new codes are appended.

A program error surfaces from the sequencer as a failed transaction rather than a
decodable numeric field, so the code is a diagnostic for whoever built the call.
What an integrator branches on is the claim marker PDA: present and owned by the
verifier means the claim passed at exactly the distribution and nullifier folded
into its address.

## Circuit-level errors (`airdrop_core::ClaimError`)

The guest escalates each of these to a panic, so no proof exists for a claim that
fails them. Defined in `crates/airdrop-core/src/lib.rs`.

| Variant | When it fires |
|---|---|
| `NotEligible` | The recipient's leaf does not fold to the claimed root. |
| `AllocationMismatch` | The statement allocation disagrees with the witness. |
| `NullifierMismatch` | The statement nullifier is not the one the witness yields. |

## The double-claim rejection, which carries no `4xxx` code

A second claim by the same recipient is refused **before the guest runs**, by the
account constraint on the marker: it is declared `init`, so a marker that already
exists makes the instruction fail with LEZ's own
`AccountAlreadyInitialized { account_index: 0 }`. There is no `4xxx` for it and
there should not be — inventing one would mean the program had been reached,
which would mean the guard had already been passed.

| What fires | Where it comes from | The test that watches it fire |
|---|---|---|
| `AccountAlreadyInitialized` | the LEZ account model, on the `init` marker | `a_second_claim_on_an_initialized_marker_is_rejected` |

The criterion asks for deterministic, documented error codes for the invalid-proof
and double-claim scenarios. Both are deterministic; this one is documented here
rather than in the table above because it is not the program's to number, and
saying so is more useful than a code that would misdescribe where the refusal
happens. It is also demonstrated live on chain, under a *different* signer, so
what refuses is the marker and not the submitter.

## What is never in an error

No error message contains a field of `ClaimWitness`: not `nsk`, not `salt`, not
the `merkle_path`, not the `leaf_index`. Diagnostics name the public statement
field that disagrees or the code above, never a private input. Those fields never
leave the prover process.
