# ADR-001: LP-0003 architecture

Status: accepted. Context: a private allowlist / airdrop where an eligible
recipient claims without revealing which address they hold, no one claims twice,
and an observer cannot link a claim to an address.

## Decision 1: anchor the eligibility root on chain, do not trust the prover's

A membership proof is only meaningful against a root someone committed to. If the
claimant chooses the root they can invent a one-leaf tree holding themselves. So
the distributor commits the root first: `create_distribution` initialises a PDA
whose address is `[distribution_id, eligibility_root]`, and `claim` requires the
distribution account to be owned by this program. An invented root lands on an
uninitialised address and is rejected (`4003`). The root is therefore the
distributor's on-chain commitment, not a prover-chosen value.

Rejected: passing the root as a plain argument and trusting it — that is exactly
the prover-chosen-root hole.

## Decision 2: verify the proof on chain via the privacy path, not the public one

A LEZ public transaction re-executes rather than proves, so no program on that
path verifies a proof. The claim verifier declares a `ChainedCall` to a LEZ-native
claim program and is invoked through a privacy-preserving transaction, where the
privacy circuit composes the callee with a real `env::verify` and the sequencer
checks the receipt against the pinned circuit id. The membership proof is verified
on chain as a precondition of acceptance.

Rejected: a host-side gate on the public path — cheap and confirmable, but it
verifies no proof.

## Decision 3: bind the nullifier to a secret, and the marker seed to the policy

The nullifier is `H(prefix || distribution_id || nsk)`, a function of the
recipient's secret, so it is deterministic per `(distribution, recipient)` for
double-claim prevention yet unlinkable: an observer who knows every candidate
address still cannot compute it. The marker PDA seed commits to both the
distribution and the nullifier, so the public trace records which distribution was
claimed and cannot be replayed across distributions.

Rejected: an account-derived nullifier — precomputable by anyone who knows the
candidate set, which breaks unlinkability.

## Decision 4: deliver eligibility as a publishable encrypted bundle

Each recipient's claim data is sealed to an X25519 key derived from their secret
through a domain separator, with ChaCha20-Poly1305. The distributor publishes one
bundle; a recipient opens only their row and checks it anchors to the committed
root before proving. No per-recipient private channel, no key reuse between
encryption and the nullifier/signing secret, and a malicious distributor cannot
make a recipient prove a leaf that is not in the set.
