# On-chain adversarial demonstrations

The unit tests prove the deployed verifier rejects forged verifier-level inputs.
This file records three demonstrations run against the **live** deployed programs
on the privacy-preserving path, showing the deeper guarantees are real and not
decorative: a claim that does not genuinely prove membership, redirects its
destination, or repeats cannot even produce a transaction. Each was reproduced on
the public testnet against the current **v0.2.4** verifier (ImageID
`31edc17c…a110385d`); the failures happen at proof-generation time, so no invalid
claim ever reaches a block.

They are not a one-off transcript: `scripts/adversarial-onchain.sh` re-runs all
three against whatever is deployed, and fails if any of them produces a
transaction instead of the rejection below. Attacks 1 and 2 are self-contained;
attack 3 needs a claim whose marker PDA is already on chain, which a clean clone
does not have (see [Reproduce](#reproduce)). The script counts what it actually
ran and exits 2 rather than claiming three when it performed two.

## 1. Membership is genuinely verified on chain, the chained call is not inert

Take a valid claim and tamper one word of the witness's Merkle path, leaving the
nsk and allocation intact so every check the *verifier program* performs still
passes (nullifier re-derives, allocation matches, marker seed matches, root is
anchored). Submit it. The chained `claim_lez` guest runs `airdrop_core::claim`,
which folds the tampered path and finds it does not reach the committed root:

```
thread '<unnamed>' panicked at src/bin/claim_lez.rs:62:29:
claim is not valid: NotEligible
Failed to submit privacy-preserving transaction:
  TransactionBuildError(ProgramProveFailed("Guest panicked: claim is not valid: NotEligible"))
```

No proof can be generated, so no transaction exists. If the ChainedCall were
decorative, this claim would have succeeded, since the verifier's own checks all
pass. It does not: the membership proof is composed and verified on chain. The
panic is in `claim_lez` (the chained guest), not the verifier.

## 2. The destination cannot be redirected

Take a valid claim and change only the `--destination` argument, as a relayer
redirecting the allocation would, leaving the destination the witness commits
unchanged. The guest enforces `witness.destination == statement.destination`:

```
thread '<unnamed>' panicked at src/bin/claim_lez.rs:62:29:
claim is not valid: DestinationMismatch
Failed to submit privacy-preserving transaction:
  TransactionBuildError(ProgramProveFailed("Guest panicked: claim is not valid: DestinationMismatch"))
```

The submission cannot redirect the allocation without re-proving, which needs the
secret.

## 3. A second claim by the same recipient is rejected

Submit a valid claim; it lands and claims the marker PDA. Submit the same claim
again. The marker PDA is created with `init`, so it must be uninitialised, and the
verifier's account validation rejects the second attempt:

```
thread '<unnamed>' panicked at src/bin/claim_verifier.rs:63:1:
account validation failed: AccountAlreadyInitialized { account_index: 0 }
Failed to submit privacy-preserving transaction:
  TransactionBuildError(ProgramProveFailed("Guest panicked: account validation failed: AccountAlreadyInitialized { account_index: 0 }"))
```

Reproduced against the live deployment: the first claim of distribution `b1…0001`
landed as `441ccd15e7b5eac388a0849481e95db409f1b6f23a202b6ee1a3ce37ae112c86` and
claimed marker PDA `B4VTZUENS1Ckmaiv8h44QcU5r276pEgTE2nEwGSGsf16` (owned by the
verifier, confirmed by `scripts/verify-onchain-claim.sh`). Resubmitting that same
claim, as a different signer, fails to build with the message above; the second
transaction cannot be produced. Here the panic is in the verifier's account
validation, not `claim_lez`.

## Reproduce

```bash
# attacks 1 and 2 only; exits 2 (INCOMPLETE), naming what it could not run
SIGNER=<funded public id> ./scripts/adversarial-onchain.sh

# all three: point SPENT_ARGS at a claim whose marker PDA is already on chain
SIGNER=<funded public id> SPENT_ARGS=artifacts/e2e/dist1/claim_0.args \
  ./scripts/adversarial-onchain.sh
```

These use the deployed programs and a fresh, throwaway private claimant. The valid
claim is built with `airdrop claim-from-bundle`; the two attack variants tamper the
emitted arguments (a Merkle-path word; the destination argument) before submission,
against a fresh distribution whose marker is still unspent — so nothing but the
tampering can account for the rejection. The double-claim resubmits a claim whose
marker is already on chain. All three fail at proof generation with the messages
above, so none reaches a block.

Attack 3 is the one that needs an input the repository cannot carry: an args file
whose marker is already spent. Those files hold a recipient's `nsk`, so they are
gitignored and never committed — `scripts/deploy-and-claim.sh` writes them into
`artifacts/e2e/dist*/`, and `SPENT_ARGS` points the script at one. A run without
it performs two attacks, says so per attack in its summary, and exits 2; only a
3-of-3 run exits 0 and prints the three-attack verdict. The double-claim
rejection is separately exercised with no funded account and no chain by
`cargo test -p claim-verifier-tests` (`claim_rejects.rs`), which CI runs on every
push against the committed verifier binary, so §3 above is covered even when this
script cannot run it.

Exit status: `0` all three ran and were rejected, `1` an attack was not rejected
or was rejected for the wrong reason, `2` an attack could not run — a missing
input or a missing dependency, neither of which is a fact about the deployment.

The script treats a produced transaction as a failure, not just an unexpected
error message: an attack that got through would fail the run loudly rather than
pass for the wrong reason.
