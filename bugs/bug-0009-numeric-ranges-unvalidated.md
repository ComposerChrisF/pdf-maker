# bug-0009: Numeric spec values accept out-of-range magnitudes — negative alpha renders content invisible at exit 0

**Severity:** Medium (worst case is invisible-yet-successful output; the rest is degenerate-output hygiene)
**Type:** Code bug (missing validation in `src/spec_types/`).  The spec is not the problem, though the ranges should be stated in `--help`/README once enforced (bug-0012).  One sub-item (`max_dpi=0`) needs a ruling from Chris because an existing unit test asserts the current acceptance.

## Description

The spec parsers accept any value that lexes as a number; nothing checks ranges.  Verified and traced consequences:

1. **`alpha` outside [0, 1] is silently clamped by medpdf** (`PdfColor::clamped()` runs inside `add_rect`/`add_line`/`add_text_params`).  `alpha=5` clamps to 1.0 — the alpha silently disappears (no ExtGState is even emitted).  Worse, a negative alpha clamps to 0.0: the caller gets a **fully invisible** watermark/rect/line and exit 0.  A user who typos `alpha=-0.5`, or writes `alpha=50` meaning 50%, gets silent wrong output.
2. **`--blank-page` accepts `w=0,h=0`** (verified: writes a page with MediaBox `[0 0 0 0]`, exit 0) and, by the same code path, negative dimensions.  Degenerate MediaBoxes are invalid per the PDF spec and interact badly with imposition (a 0×0 source page is silently skipped by the `src_w <= 0` guard in `apply_nup`).
3. By code trace (same pattern, not individually run): watermark `size<=0`, draw-line `width<=0`, and draw-rect `w`/`h` `<=0` all parse successfully and produce degenerate output.  Negative `margin`/`gutter`/`binding_margin` on `--nup`/`--booklet` also parse — **and, ruled 2026-09-09, that is correct and must stay** (see below).  (The imposition _derived-geometry_ check is bug-0006; this report is the parse-level half.)
4. **`max_dpi=0`** on `--draw-image` is accepted — and `test_draw_image_spec_max_dpi_zero` in `src/spec_types/drawing.rs` deliberately asserts that acceptance, so the current behavior looks intentional.  What zero means downstream in medpdf-image (no limit?  divide-by-zero?) is undefined.  **Chris to rule:** treat 0 as “no downsampling” and document it, or reject it.  Do not silently flip the test.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o two.pdf --blank-page "w=612,h=792,count=2"
pdf-maker -o out.pdf two.pdf all --draw-rect "x=0,y=0,w=100,h=100,alpha=5"
pdf-dump out.pdf --operators --page 1     # q / 0 0 0 rg / re / f / Q — no gs op at all
pdf-maker -o zz.pdf --blank-page "w=0,h=0" --json   # exit 0, page_count 1, MediaBox [0 0 0 0]
```

Contrast: `alpha=0.5` correctly emits `/GS.. gs`.  Rust test sketches: `WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,alpha=1.5")` should return `Err` naming alpha and the range (currently `Ok`); `BlankPageSpec::from_str("w=0,h=0")` should return `Err` (currently `Ok`); likewise negative-value cases per item 3.

## Suggested fix

Validate at parse time, in each `FromStr`, with errors that name the key, the offending value, and the allowed range:

- `alpha` ∈ [0.0, 1.0] on watermark, draw-rect, draw-line, draw-image.  (An explicit `alpha=0` remains legal — stated intent, unlike a clamped `-0.5`.)
- `--blank-page` `w`, `h` > 0; watermark `size` > 0; draw-line `width` > 0; draw-rect `w`, `h` > 0.
- `--nup`/`--booklet` `paper_w`, `paper_h` > 0.  **`margin`, `gutter`, and `binding_margin` are deliberately _not_ sign-checked** — see the ruling below.
- `max_dpi`: per Chris’s ruling (item 4).

These are all `String`-error returns from `FromStr`, so clap surfaces them as usage errors (exit 2) — consistent with how `count=0` and `repeat=0` are already rejected in the same files, which is the pattern to copy.

## Ruling 2026-09-09 — negative offsets stay legal; check the derived quantity instead

Chris’s call, recorded here because this report is where the sign check would have been
written.  **An extent may not be negative; an offset may.**

- **Extents** — `w`, `h`, `paper_w`, `paper_h`, `size`, `width`, `count` — have no meaning
  below zero and are rejected at parse, as this report proposes.  `alpha` keeps its `[0, 1]`
  domain for the same reason.
- **Offsets** — `margin`, `gutter`, `binding_margin`, and the `x`/`y` of every drawing flag —
  take any sign.  A negative one is a **bleed**, which is a layout people actually want, and
  refusing it at parse would foreclose it permanently to save a check that bug-0006 already
  performs better.

Verified on v0.13.2 rather than argued from the code: `--nup "n=4,margin=-0.25,units=in"` over
a four-page letter document emits `0.5227 0 0 0.5227 -15.95 396 cm` per cell — a _positive_
uniform scale, origins a quarter inch outside the sheet, each cell clipped to its own rect.
That is textbook full-bleed 4-up, not the mirrored garbage a negative _cell_ produces.  The
two cases are cleanly separable: a negative **margin** enlarges the cell, while bug-0006’s
fault is a non-positive **cell**, which only a large _positive_ margin can cause.

**The obligation the ruling creates:** because a negative offset is now a supported layout
rather than an accident, the geometry must be _visible_.  bug-0006’s derived check must also
report — in `--json` and in the stderr progress block — the computed cell size and how much of
it falls outside the sheet, so a typo’d `margin=-0.5` is loud rather than silently bled off.
Sign checks are not the instrument; reporting the derived arithmetic is.

Consumers of this ruling: bug-0006 (the derived check), and plan-0003’s `--tile`, where the
same principle appears as a **coverage** invariant — a negative tile `margin` is legal exactly
when `overlap >= -2*margin`, because past that the tiles no longer meet and source content is
silently dropped.

## Why this fix addresses the bug

Every listed value has no meaningful interpretation outside its range; today each is either silently clamped (alpha), silently degenerate (dimensions), or silently off-page (negative margins).  Parse-time rejection converts all of them into loud, pre-work usage errors at the one layer that sees the caller’s literal input — and matches the precedent the codebase already set for `count` and `repeat`.

## RULING 2026-09-09 — item 4, `max_dpi`

Chris’s call, and it is a design rather than a yes/no: **`none` is the public spelling.**

- `max_dpi=none` means “no downsampling”, and is the only way to ask for it.
- Internally that maps to an enum variant (or, if it must stay numeric, 0) — the sentinel
  never appears on the CLI surface, so the public interface is not polluted with a magic
  number whose meaning has to be memorized.
- A **numeric `max_dpi` below 1.0, including 0, is an invalid-value error** naming the key and
  the allowed range.  So the old spelling stops working rather than quietly meaning something.

`test_draw_image_spec_max_dpi_zero` in `src/spec_types/drawing.rs`, which currently asserts
that `0` is accepted, is **rewritten rather than deleted**: it becomes the pair of assertions
that `max_dpi=0` is now rejected and `max_dpi=none` accepted.  Silently flipping it would
erase the record that the old behavior was deliberate.

This closes the last open question in this report; the rest of the parse-level validation
needs no further ruling.
