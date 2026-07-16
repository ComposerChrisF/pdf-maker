# bug-0009: Numeric spec values accept out-of-range magnitudes — negative alpha renders content invisible at exit 0

**Severity:** Medium (worst case is invisible-yet-successful output; the rest is degenerate-output hygiene)
**Type:** Code bug (missing validation in `src/spec_types/`).  The spec is not the problem, though the ranges should be stated in `--help`/README once enforced (bug-0012).  One sub-item (`max_dpi=0`) needs a ruling from Chris because an existing unit test asserts the current acceptance.

## Description

The spec parsers accept any value that lexes as a number; nothing checks ranges.  Verified and traced consequences:

1. **`alpha` outside [0, 1] is silently clamped by medpdf** (`PdfColor::clamped()` runs inside `add_rect`/`add_line`/`add_text_params`).  `alpha=5` clamps to 1.0 — the alpha silently disappears (no ExtGState is even emitted).  Worse, a negative alpha clamps to 0.0: the caller gets a **fully invisible** watermark/rect/line and exit 0.  A user who typos `alpha=-0.5`, or writes `alpha=50` meaning 50%, gets silent wrong output.
2. **`--blank-page` accepts `w=0,h=0`** (verified: writes a page with MediaBox `[0 0 0 0]`, exit 0) and, by the same code path, negative dimensions.  Degenerate MediaBoxes are invalid per the PDF spec and interact badly with imposition (a 0×0 source page is silently skipped by the `src_w <= 0` guard in `apply_nup`).
3. By code trace (same pattern, not individually run): watermark `size<=0`, draw-line `width<=0`, draw-rect `w`/`h` `<=0`, and negative `margin`/`gutter`/`binding_margin` on `--nup`/`--booklet` all parse successfully and produce degenerate or off-page output.  (The imposition _derived-geometry_ check is bug-0006; this report is the parse-level half.)
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
- `--nup` `margin`, `gutter` ≥ 0; `--booklet` `binding_margin` ≥ 0; `paper_w`, `paper_h` > 0.
- `max_dpi`: per Chris’s ruling (item 4).

These are all `String`-error returns from `FromStr`, so clap surfaces them as usage errors (exit 2) — consistent with how `count=0` and `repeat=0` are already rejected in the same files, which is the pattern to copy.

## Why this fix addresses the bug

Every listed value has no meaningful interpretation outside its range; today each is either silently clamped (alpha), silently degenerate (dimensions), or silently off-page (negative margins).  Parse-time rejection converts all of them into loud, pre-work usage errors at the one layer that sees the caller’s literal input — and matches the precedent the codebase already set for `count` and `repeat`.
