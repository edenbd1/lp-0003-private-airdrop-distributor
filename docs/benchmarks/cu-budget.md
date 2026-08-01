# Compute-unit budget

The LP-0003 performance criterion asks for the CU cost of each on-chain
operation. The LEZ testnet RPC exposes no per-transaction CU field, so the figure
is obtained by replaying the sequencer's own execution of the deployed binary:
same inputs, same 32M session limit, same executor
(`lee/state_machine/src/program.rs`). This is the identical computation the chain
performs, measured rather than reported by the node.

Measured against the deployed `claim_verifier.bin`
(ImageID `a7b7cf26…6fe7b77d`):

| Metric | `claim` |
|---|---|
| Segments | 1 |
| **User cycles** | **333,565** |
| **Proving cycles (sum of 2^po2)** | **524,288** |
| Public execution budget | 33,554,432 |
| **Budget consumed (proving cycles)** | **1.56 %** |

`create_distribution` is lighter still: it initialises one PDA and writes no data.

The claim verifier is cheap because it does no heavy in-guest cryptography
itself: it re-derives the nullifier and marker seed (a few SHA-256 hashes),
reads the anchored distribution's owner, and declares the chained call. Merkle
membership and the nullifier derivation are proved in the chained `claim_lez`
guest, and the privacy circuit's recursive `env::verify` of that receipt is
LEZ's cost, charged to the privacy circuit rather than to this instruction and
not counted here.

## Reproduce

```bash
cargo test -p claim-verifier-tests --test claim_rejects -- --ignored --nocapture
```

which prints the `claim verifier, guest execution only` line above, computed from
the same `SessionInfo` the sequencer's executor produces.
