# On-chain adversarial demonstrations

The unit tests prove the deployed verifier rejects forged verifier-level inputs.
This file records three demonstrations run against the **live** deployed programs
on the privacy-preserving path, showing the deeper guarantees are real and not
decorative: a claim that does not genuinely prove membership, redirects its
destination, or repeats cannot even produce a transaction. Each was reproduced on
the public testnet; the failures happen at proof-generation time, so no invalid
claim ever reaches a block.

## 1. Membership is genuinely verified on chain, the chained call is not inert

Take a valid claim and tamper one word of the witness's Merkle path, leaving the
nsk and allocation intact so every check the *verifier program* performs still
passes (nullifier re-derives, allocation matches, marker seed matches, root is
anchored). Submit it. The chained `claim_lez` guest runs `airdrop_core::claim`,
which folds the tampered path and finds it does not reach the committed root:

```
Guest panicked: claim is not valid: NotEligible
Failed to submit privacy-preserving transaction:
  ProgramProveFailed("Guest panicked: claim is not valid: NotEligible")
```

No proof can be generated, so no transaction exists. If the ChainedCall were
decorative, this claim would have succeeded, since the verifier's own checks all
pass. It does not: the membership proof is composed and verified on chain.

## 2. The destination cannot be redirected

Take a valid claim and change only the `--destination` argument, as a relayer
redirecting the allocation would, leaving the destination the witness commits
unchanged. The guest enforces `witness.destination == statement.destination`:

```
Guest panicked: claim is not valid: DestinationMismatch
```

The submission cannot redirect the allocation without re-proving, which needs the
secret.

## 3. A second claim by the same recipient is rejected

Submit a valid claim; it lands and claims the marker PDA. Submit the same claim
again. The marker PDA is `init`, so it must be uninitialised:

```
account validation failed: AccountAlreadyInitialized { account_index: 0 }
```

The first claim landed on chain (tx `07365dc7866540d5e3f7dfad419f1ecc1790f9b4650f12a16a0697d585c66129`);
the second cannot be built.

## Reproduce

These use the deployed programs and a funded private claimant. The valid claim is
built with `airdrop claim-from-bundle`; the two attack variants tamper the emitted
arguments (a Merkle-path word; the destination argument) before submission. The
double-claim is simply the same claim submitted twice. All three fail at
`send_privacy_preserving_tx` with the messages above.
