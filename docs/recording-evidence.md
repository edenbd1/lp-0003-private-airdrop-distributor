# What the recording proves, measured rather than described

Two checks run against the published films. Both read the files; neither reads
this document.

    ./scripts/check-transcript.py recordings/lp-0003-claim-and-double-claim.srt <film.mp4>

The short film attached to the pull request shows one claim proved and submitted
on the privacy path, then the same claim refused. Its narration is committed
beside it as
[`recordings/lp-0003-claim-and-double-claim.srt`](../recordings/lp-0003-claim-and-double-claim.srt),
so the film can be read and grepped instead of watched.

## Measured 2026-08-24

```
lp-0003-claim-and-double-claim.srt: 12 cue(s), film: 44.2 s
  ok    structure, and the last cue lands 1.2 s before the end
  ok    dev mode is 0              spoken at  5s, and on screen there
  ok    Five checks                spoken at 14s, and on screen there
  ok    marker already exists      spoken at 29s, and on screen there
  ok    public LEZ testnet         spoken at 38s, and on screen there
  transcript matches the film: structure, fit, and 4 anchor(s) tied to the picture.
```

The anchors are the part that matters. The narration is spoken, so its words are
never on screen — but what it *talks about* is, and each anchor requires the
picture to show it while the line is being said. Where the narration says "the
marker already exists", the frame at that second must show
`AccountAlreadyInitialized`. A transcript written from memory passes every
structural check and fails these.

The checker refuses to pass on structure alone: a transcript it has no anchors
for is rejected outright, and a run that manages fewer than two tested anchors
fails and says so. That rule exists because the first version of this check did
exactly what it was written to prevent — run against a film whose narration
never used any of its phrases, it reported "anchor not testable" four times and
then printed "transcript matches the film".

## Which commit the film shows

The film was shot at
[`8267c22`](https://github.com/edenbd1/lp-0003-private-airdrop-distributor/commit/8267c22696f7e0fd5b56da53cdb80bd0065a9e9f)
and shows that hash on screen in its opening seconds, over a clean tree. The
reviewed commit is later, because the transcript of a film can only be committed
after the film exists. What separates them is nineteen files and 1,665 lines, and the honest
summary is not "documentation only". The two program binaries under
`artifacts/programs/` are unchanged, so the chain sees the same thing — but
`scripts/verify-onchain-claim.sh` gained a step the film does not show, and the
film demonstrates the five-check version. That step closes a real hole: the
script used to print VERIFIED when given one claim's transaction paired with
another claim's nullifier, because the two halves of the check never met. So the
film shows a correct run of a script that has since been made stricter, not a run
of the script a reviewer will get. `git diff` between the two
says so in 5 files.

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
