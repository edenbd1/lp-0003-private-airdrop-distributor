#!/usr/bin/env python3
"""Decide, by rendering, whether the LEZ explorer has indexed a transaction.

WHY THIS EXISTS

Explorer links in a submission should be measured rather than asserted.

When this script was written the explorer was a WASM application: every
`/transaction/<hash>` URL returned the same 2416-byte shell and fetched its
content client-side, so an indexed transaction and a hash that *cannot exist*
were byte-identical over `curl` and a size comparison could not tell them apart.
That is why this drives a browser at all.

That is no longer true. Re-measured 2026-08-15, the explorer server-side renders
(Leptos): a claim comes back around 366 kB with its `Type:` and `Proof Size:` in
the body, and a hash that cannot exist comes back as a 2416-byte page reading
`Failed to load transaction: error running server function: Transaction not
found`. So `curl` does separate them now, and docs/DEPLOYMENT.md gives that
one-liner.

Rendering is kept as the second opinion, not as a necessity: it reads the DOM a
reviewer actually sees rather than a byte count, and it keeps working if the
explorer goes back to rendering client-side. The impossible hash stays as the
control — whatever the explorer shows for "this does not exist" is the baseline
every real hash is compared against, and if that control ever renders as a
*found* transaction the baseline is meaningless and the whole run is abandoned.

The hashes come from the artifacts, not from the prose:

  - the two deploy transactions are recomputed from the committed binaries
    (`SHA256(borsh(bytecode))`, content addressed), so they cannot drift from
    what is actually deployed;
  - the claims are read from artifacts/e2e/claims.tsv.

    ./scripts/check-explorer.py                    # deploys + a sample of claims
    ./scripts/check-explorer.py --claims 23        # deploys + every claim
    ./scripts/check-explorer.py <hash> [<hash>...] # arbitrary hashes

Needs playwright with firefox:  pip install playwright && playwright install firefox
"""
import hashlib
import pathlib
import struct
import sys

BASE = "https://explorer.testnet.lez.logos.co"
IMPOSSIBLE = "de" * 32
NOT_FOUND = "Transaction not found"
ROOT = pathlib.Path(__file__).resolve().parent.parent


def deploy_hash(path):
    """A LEZ deploy tx hash is SHA256(borsh(bytecode)) = SHA256(u32_le(len) || bytes)."""
    b = path.read_bytes()
    return hashlib.sha256(struct.pack("<I", len(b)) + b).hexdigest()


def committed_cases(n_claims):
    cases = []
    for name in ("claim_lez", "claim_verifier"):
        p = ROOT / "artifacts" / "programs" / f"{name}.bin"
        if p.exists():
            cases.append((f"deploy:{name}", deploy_hash(p)))

    tsv = ROOT / "artifacts" / "e2e" / "claims.tsv"
    if tsv.exists():
        rows = [l.split("\t") for l in tsv.read_text().splitlines() if l.strip()]
        rows = [r for r in rows if len(r) >= 2 and len(r[1]) == 64]
        for i, r in enumerate(rows[:n_claims]):
            cases.append((f"claim:{i}", r[1]))
    return cases


def main():
    try:
        from playwright.sync_api import sync_playwright
    except ImportError:
        sys.exit("playwright is not installed: pip install playwright && playwright install firefox")

    args = sys.argv[1:]
    n_claims = 3
    if args and args[0] == "--claims":
        n_claims = int(args[1])
        args = args[2:]

    cases = [("control:impossible", IMPOSSIBLE)]
    cases += [(f"arg{i}", h) for i, h in enumerate(args)] if args else committed_cases(n_claims)
    if len(cases) == 1:
        sys.exit("no hashes found under artifacts/ and none given")

    results = []
    with sync_playwright() as p:
        browser = p.firefox.launch(headless=True)
        page = browser.new_page(viewport={"width": 1280, "height": 1400})
        for label, h in cases:
            page.goto(f"{BASE}/transaction/{h}", wait_until="load", timeout=60000)
            try:
                page.wait_for_load_state("networkidle", timeout=30000)
            except Exception:
                pass
            page.wait_for_timeout(4000)
            results.append((label, h, " ".join(page.inner_text("body").split())))
        browser.close()

    control = results[0][2]
    # The control is the whole basis of the comparison. If a hash that cannot
    # exist renders as a found transaction, the baseline is wrong and every
    # verdict derived from it is meaningless — so refuse to print any.
    if NOT_FOUND not in control:
        sys.exit(
            f"control ({IMPOSSIBLE[:8]}…) is a hash that cannot exist, but the explorer did "
            f"not render it as not-found. The baseline is invalid, so no verdict below it "
            f"would mean anything. Control rendered:\n  {control[:300]}"
        )

    print(f"{'hash':<22} {'':<10} verdict")
    for label, h, text in results[1:]:
        verdict = "NOT INDEXED (same as control)" if text == control else "INDEXED"
        print(f"{label:<22} {h[:8]}…  {verdict}")
    print(f"\ncontrol ({IMPOSSIBLE[:8]}…, a hash that cannot exist) renders:\n  {control[:300]}")


if __name__ == "__main__":
    main()
