# Compute-unit budget

The LP-0003 performance criterion asks for the CU cost of each on-chain
operation. The LEZ testnet RPC exposes no per-transaction CU field, so the figure
is obtained by replaying the sequencer's own execution of the deployed binary:
same inputs, same 32M session limit, same executor
(`lee/state_machine/src/program/mod.rs`). This is the identical computation the chain
performs, measured rather than reported by the node.

Measured against the deployed `claim_verifier.bin`
(ImageID `31edc17c…a110385d`). The criterion asks for *each* on-chain operation,
and the program has exactly two, so both are counted here rather than one counted
and the other described:

| Metric | `create_distribution` | `claim` |
|---|---|---|
| Segments | 1 | 1 |
| **User cycles** | **108,596** | **318,242** |
| **Proving cycles (sum of 2^po2)** | **262,144** | **524,288** |
| Public execution budget | 33,554,432 | 33,554,432 |
| **Budget consumed (proving cycles)** | **0.78 %** | **1.56 %** |

`create_distribution` costs about a third of `claim` in user cycles: it validates
two accounts, derives one PDA address from `[distribution_id, eligibility_root]`,
and writes no data — the address *is* the commitment. It is not free, and the
figure above is what it actually costs rather than an adjective. Both operations
round up to a power-of-two proving trace, which is why the ratio of proving
cycles (2×) is coarser than the ratio of user cycles (2.9×): `claim` crosses into
the next power of two and `create_distribution` does not.

## Proof-generation wall-clock

The cycle count above is the on-chain compute cost. The separate client-side cost
is the time to *generate* the proof, which the criterion also asks for.

Unlike the cycle count, this is not a property of the design and is deliberately
not quoted as a single fixed number: it is dominated by the machine and by what
else is running on it. On this Apple-Silicon laptop, with `RISC0_DEV_MODE=0` (real
STARKs, no mock receipts), the same claim took roughly half again as long while an
unrelated build was competing for the cores — same binary, same inputs. Any single
figure printed here would be a figure about one machine on one afternoon.

So `scripts/prove-one-claim.sh` times the proof and prints what it measured, and
the recorded demo shows that clock running in real time rather than asserting a
duration. Proving the chained `claim_lez` guest and the privacy circuit's
recursive composition of its receipt is nearly all of the cost; the submission
itself is a few seconds.

This is the real first-claim latency, and it is minutes rather than seconds: a
deployment sensitive to it should prove in the background and submit when ready.

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

which prints both blocks above — `create_distribution, guest execution only` and
`claim verifier, guest execution only` — computed from the same `SessionInfo` the
sequencer's executor produces. Either can be run alone:

```bash
cargo test -p claim-verifier-tests --test claim_rejects \
  report_the_create_distribution_cycle_cost -- --ignored --nocapture
```

Both go through one `report_cycles` helper, so the two numbers cannot drift into
two different definitions of "a cycle".
