#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Every truncated hash in a document is some full hash this repository holds.

    ./scripts/check-truncated-hashes.py solutions/LP-0003.md

WHY THIS EXISTS

A submission document writes hashes short — `441ccd15…ae112c86` — because the
full ones are unreadable in a table. A short hash cannot be pasted into the block
explorer, cannot be fed to the RPC, and looks exactly like a real one. So it is
the cheapest thing in the whole submission to get wrong, and the most expensive
to be caught getting wrong: a reviewer who finds one hash that resolves to
nothing has no reason to trust the next.

This resolves each `<8 hex>…<8-9 hex>` against every full 64-hex string in the
tracked tree and requires exactly one match. Zero means the document refers to
something the repository has no record of. More than one means the truncation is
too short to identify anything, which is the same problem wearing a different
hat.

It caught a real one: a distribution table shipped two `create_distribution`
hashes that appeared in no file, no commit and no log, and one of them carried a
nine-character tail where every other truncation in the document used eight — the
tell that it had not been produced by truncating anything.

THIS RUNS BY HAND, AND THAT IS THE POINT

The document it checks lives on the pull request, not in this tree, so no
workflow can reach it. Saying that here is deliberate: a gate nobody schedules
and nobody mentions is indistinguishable, from the outside, from one that passes.
Run it against the file you are about to submit, before you submit it.
"""
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
TRUNC = re.compile(r"\b([0-9a-f]{8})…([0-9a-f]{8,9})\b")
FULL = re.compile(r"\b[0-9a-f]{64}\b")


def pool():
    files = subprocess.run(["git", "-C", str(ROOT), "ls-files"],
                           capture_output=True, text=True).stdout.split()
    out = set()
    for f in files:
        try:
            out |= {m.lower() for m in FULL.findall((ROOT / f).read_text(errors="replace"))}
        except (OSError, UnicodeDecodeError):
            continue
    return out


def main(argv):
    if len(argv) < 2:
        sys.exit("  usage: check-truncated-hashes.py <document.md> [more...]")
    known = pool()
    if not known:
        sys.exit("  no full hash appears anywhere in this tree, so nothing here could "
                 "be resolved. That is the checker failing, not the document.")
    bad = 0
    for doc in argv[1:]:
        text = pathlib.Path(doc).read_text(errors="replace")
        found = TRUNC.findall(text)
        for pre, suf in found:
            hits = [h for h in known if h.startswith(pre) and h.endswith(suf)]
            if len(hits) == 1:
                continue
            bad += 1
            if not hits:
                print("    %s…%s matches no hash this repository holds" % (pre, suf))
            else:
                print("    %s…%s matches %d of them, so it names none"
                      % (pre, suf, len(hits)))
        print("  %s: %d truncated hash(es), %d unresolved"
              % (doc, len(found), bad))
    if bad:
        print("  a truncated hash nobody can resolve is an assertion, not evidence.")
        return 1
    print("  every truncated hash is one this repository can produce in full.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
