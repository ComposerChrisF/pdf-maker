# bug-0006: Oversized margins/gutters produce negative imposition cells — pages come out mirrored and shrunken at exit 0

**Severity:** Medium-High (silent wrong output; the file looks plausible until opened)
**Type:** Code bug.  The spec is not the problem (no imposition spec exists; bug-0012).

## Description

`apply_nup` in `src/imposition.rs` computes

```text
avail_w = paper_w − 2·margin − (cols−1)·gutter
cell_w  = avail_w / cols        (same for height)
scale   = min(cell_w/src_w, cell_h/src_h)
```

with no check that the available area is positive.  A margin or gutter large enough to consume the paper yields negative cell dimensions, hence a **negative scale**, which `place_page` accepts (it only requires the scale to be finite).  The result is content drawn mirrored through the origin at a tiny size — silent garbage, exit 0.  The same shape exists in `apply_booklet`: `half_w = (paper_w − binding_margin) / 2` goes non-positive when `binding_margin ≥ paper_w`.

Negative `margin=`, `gutter=`, and `binding_margin=` values are also accepted at parse (any `f32` parses), which produces content placed off-paper; bug-0009 carries the parse-level range validation, this report carries the derived-geometry check.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o four.pdf --blank-page "w=612,h=792,count=4"
pdf-maker -o out.pdf four.pdf all --nup "n=2,margin=5,units=in" --json   # exit 0
pdf-dump out.pdf --operators --page 1
```

Observed: exit 0, 2 sheets, and `cm` matrices of the form `-0.1764706 0 0 -0.1764706 ...` — a negative uniform scale (letter paper is 612 pt wide; two 360 pt margins leave −108 pt).

Rust test sketch: run the command above via the `tests/cli_tests.rs` harness and assert non-zero exit and an error naming the margin/paper mismatch; before the fix it exits 0.  A unit test on the (to-be-extracted) cell-geometry computation asserting an `Err` for `paper_w=612, margin=360, cols=2` pins it more tightly.

## Suggested fix

In `apply_nup`, after computing `cell_w`/`cell_h`, error (exit 1 via the normal `MedpdfError` path) when either is `<= 0`, naming the paper size, margin, gutter, and grid so the caller can see the arithmetic — e.g. `--nup: margin=360pt and gutter=0pt leave no room on 612x792pt paper for a 1x2 grid (cell width -54pt)`.  In `apply_booklet`, error when `half_w <= 0`, naming `binding_margin` and the paper width.  Pair with bug-0009’s parse-level rejection of negative margins/gutters so both the absurd-positive and the negative cases are loud.

## Why this fix addresses the bug

A non-positive cell is never a layout the caller wanted — every downstream number (scale, placement) is meaningless once it appears, and the negative scale is what turns a bad parameter into a plausible-looking corrupt PDF.  Checking the derived quantity at the single place it is computed catches every parameter combination that produces it (large margin, large gutter, tiny custom paper, large binding margin) without enumerating them at parse time.

## History

Present since imposition landed in `56f7b01` (v0.10.0).  That commit’s message says it “guards against degenerate MediaBoxes” — source-page degeneracy is indeed guarded (`if src_w <= 0.0 ... continue`), but sheet-side degeneracy from margins was missed.
