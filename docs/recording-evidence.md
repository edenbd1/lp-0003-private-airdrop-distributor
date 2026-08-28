# What the recording proves, measured rather than described

Two checks run against the published films. Both read the files; neither reads
this document.

    ./scripts/check-transcript.py recordings/lp-0003-claim-and-double-claim.srt <film.mp4>

The short film attached to the pull request shows one claim proved and submitted
on the privacy path, then the same claim refused. Its narration is committed
beside it as
[`recordings/lp-0003-claim-and-double-claim.srt`](../recordings/lp-0003-claim-and-double-claim.srt),
so the film can be read and grepped instead of watched.

## Measured 2026-08-28, on the reshot film

```
lp-0003-claim-and-double-claim.srt: 16 cue(s), film: 57.2 s
  ok    structure, and the last cue lands 1.2 s before the end
  ok    dev mode is 0              spoken at  6s, and on screen there
  ok    Six checks                 spoken at 15s, and on screen there
  ok    marker already exists      spoken at 42s, and on screen there
  ok    public LEZ testnet         spoken at 51s, and on screen there
  transcript matches the film: structure, fit, and 4 anchor(s) tied to the picture.
```

---
## Which commit the film shows

The film was shot at
[`b4a68a2`](https://github.com/edenbd1/lp-0003-private-airdrop-distributor/commit/b4a68a298c9275b0b7fc835eaeac988682c7ac11)
— **the reviewed commit itself** — and shows that hash on screen in its opening
seconds, over a clean tree. The transcript that describes it is committed one
step later, because a transcript can only exist once its film does.

## The anchor that could not fail, and therefore never checked

The film this replaces said "Five checks" while the script ran six:
`verify-onchain-claim.sh` had gained the step that joins the derived marker to
the transaction being read, closing a hole where the script printed VERIFIED for
one claim's transaction paired with another claim's nullifier.

The anchor meant to hold that line was
`("Five checks", r"\[\s*[1-5]\s*/\s*5\s*\]|VERIFIED")`. When the script went to
six, `[n/5]` stopped matching — and the alternation quietly carried the anchor on
the bare word `VERIFIED`. Green, and no longer testing the step count at all. An
anchor with two ways to succeed tests neither.

The replacement drops the alternation, and does not anchor on the bracketed
counter either. Measured on the film: `[1/6]` is legible for **one second**,
because the terminal scrolls, while the line *"that marker PDA is one of the
accounts THIS transaction touched"* stays on screen for **43**. Anchoring on a
counter that flashes past is anchoring on luck; the new anchor holds the text of
step five, which is both stable and the thing the narration names.

## What the film shows, checked against what the script prints

Every frame was read with OCR and searched for the wordings that are now wrong —
`Five checks` and `[n/5]` appear **0 times**; `[n/6]`, `AccountAlreadyInitialized`
and `DEV_MODE=0` appear on 1, 27 and 49 frames. A transcript check anchors only on
lines the narration names; this reads everything on screen, which is the half
that let the old defect through.

One shot is sped up and it is announced: 710 seconds of proving compressed to
4.5, a factor of **159** written across the picture for the whole of it.
`lp-0003-atelier/cut.log` records every mark and every factor.

## What this does not establish

- **Not every frame.** Up to five frames are read per anchor. Nothing here
  claims the other thousand.
- **Not the wording.** Nothing checks the transcript's sentences against the
  audio. Structure, fit and four anchors are what is proved.
- **An OCR behaviour routed around, not diagnosed.** Tesseract renders `0` as
  `@` in this terminal font, so `RISC0_DEV_MODE=0` reads back as
  `RISC@_DEV_MODE=0`; the anchor patterns allow for it. Separately, on the
  machine these were run on, tesseract returns an empty string for images under
  some temporary directories, silently — the checker probes three frames first
  and refuses to report on the transcript if it can read none of them. Why it
  does that has not been diagnosed, and is not guessed at here.
