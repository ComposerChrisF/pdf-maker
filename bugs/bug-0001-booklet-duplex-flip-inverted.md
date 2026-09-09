# bug-0001: Booklet duplex-flip compensation appears inverted (`short_edge` rotates backs, `long_edge` does not)

**Severity:** High (for `--booklet` users; back sides print upside down when following standard printer guidance)
**Type:** Code bug (suspected) — **requires Chris’s physical confirmation before the fix lands**.  The spec is not the problem in the usual sense: no spec for `--booklet` exists at all (see bug-0012), so there is nothing written to contradict.  The design intent itself is what needs confirming.

## Description

`src/imposition.rs` (`apply_booklet`, the `is_back && spec.flip == DuplexFlip::ShortEdge` branch) rotates back-side pages 180° when `flip=short_edge` and applies **no** rotation for `flip=long_edge` (and `none`).  The in-code comment claims:

> LongEdge: no rotation needed — long-edge duplex is the natural orientation for landscape booklets, so it behaves like None.

That claim contradicts universal duplex-printing guidance, which is the opposite: **for landscape-oriented sheets, short-edge binding is the natural setting** (both sides come out upright with no compensation), while long-edge binding flips the back upside down and therefore _needs_ the 180° compensation.  The default booklet sheet here is landscape (792×612 pt), so on the default paper the two flip values appear to be exactly swapped.

Geometric argument: flipping a landscape sheet about its **short** (vertical) edge maps top edge to top edge — back content prints upright with no rotation.  Flipping about its **long** (horizontal) edge maps top to bottom — back content must be pre-rotated 180°.

Note the further subtlety: the correct compensation depends on **sheet orientation**, not on the flip value alone.  For a _portrait_ custom sheet (`paper_w=612,paper_h=792` — a top-fold “flip-book” booklet), the current behavior (rotate on `short_edge`) would actually be correct.  The bug is that the code applies the portrait rule unconditionally, while the default and typical booklet sheet is landscape.

## Reproduction (verified 2026-07-16, v0.13.1)

```bash
pdf-maker -o four.pdf --blank-page "w=612,h=792,count=4"
pdf-maker -o short.pdf four.pdf all --booklet "flip=short_edge"
pdf-maker -o long.pdf  four.pdf all --booklet "flip=long_edge"
pdf-dump short.pdf --operators --page 2   # back sheet
pdf-dump long.pdf  --operators --page 2
```

Observed: `short.pdf` page 2 contains `cm` matrices `-0.64705884 0 0 -0.64705884 ...` (180° rotation); `long.pdf` page 2 contains `0.64705884 0 0 0.64705884 ...` (no rotation).

For a Rust test: build a 4-page input (the `create_test_pdf` helper in `tests/cli_tests.rs`), run `--booklet "flip=long_edge"`, extract page 2’s content streams, and assert the scale coefficients are **negative** (180° applied) once the fix lands — and positive for `flip=short_edge` on the default landscape paper.  Blank pages created by `--blank-page` have empty content streams that `place_page` skips, so the test input must have non-empty page content (the `create_test_pdf` helper qualifies) for the `cm` operators to appear.

## Suggested fix

**Confirm first** with a physical duplex print of a small booklet under each printer flip setting (this is the one finding in this hunt that a terminal cannot verify).  If confirmed, in `apply_booklet` replace the flip test with an orientation-aware rule:

```rust
let landscape = paper_w > paper_h;
let needs_180 = is_back
    && match spec.flip {
        DuplexFlip::None => false,
        DuplexFlip::LongEdge => landscape,
        DuplexFlip::ShortEdge => !landscape,
    };
```

and rotate when `needs_180`.  Update the misleading comment, and document the semantics (which flip value to choose for which paper orientation) in `--help` and the README booklet section created by bug-0012.

## Why this fix addresses the bug

The compensation exists solely to counteract the physical flip the duplexer performs.  The physical flip that inverts content is “about the axis parallel to the content’s horizontal” — the long edge for a landscape sheet, the short edge for a portrait sheet.  Keying the rotation on the (orientation, flip) pair instead of flip alone makes the output upright for every combination, including the custom-portrait case the current code accidentally gets right.

## History

Introduced in `56f7b01` (v0.10.0, “Add n-up and booklet imposition”), unchanged since.  No spec or feature-plan document accompanied the commit, so the intended flip semantics were never written down — this is also why bug-0012 (missing imposition documentation) matters here.

## Verification kit — generated 2026-09-09, awaiting a physical print

Chris asked for something printable.  Built with the v0.13.2 release binary against medpdf
0.13.0, reproducing the **current, suspected-wrong** behavior:

```
/Users/chris/Chris/App/Rust/Pdf/pdf-maker/bugs/bug-0001/HOW-TO-VERIFY.md
/Users/chris/Chris/App/Rust/Pdf/pdf-maker/bugs/bug-0001/input-4page.pdf
/Users/chris/Chris/App/Rust/Pdf/pdf-maker/bugs/bug-0001/booklet-SHORT-edge.pdf
/Users/chris/Chris/App/Rust/Pdf/pdf-maker/bugs/bug-0001/booklet-LONG-edge.pdf
```

Read `HOW-TO-VERIFY.md` first.  Print each booklet file with the printer’s binding setting
matched to its name, then answer one question per sheet: **is the back right-side up?**

The source pages carry a large numeral, `TOP — page N` near the top edge and a grey
`bottom — page N` near the bottom, so an inverted back is unmistakable.  Sheet 1 holds source
pages 4 and 1, sheet 2 holds 2 and 3 — standard saddle-stitch order, and not what is under
test.

**Confirmed on this build** (so the printed artifact matches the analysis): `flip=short_edge`
emits `-0.64705884 0 0 -0.64705884` on the back sheet (180° applied) and `flip=long_edge`
emits `0.64705884 0 0 0.64705884` (none), on landscape 792×612 sheets.

**Prediction: both print with their backs upside down**, for opposite reasons — the test is
symmetric, so it needs no judgement about which file is which.  Backs upright on both would
mean the code is right and this report should be closed as not-a-bug with the reasoning
corrected.  One upright and one inverted would mean the analysis is wrong in a third way, and
_which one_ changes the fix — so record which.

Please note the outcome, the printer model, and the driver’s exact wording for the setting.
Duplex vocabulary is inconsistent enough between drivers that the wording is part of the
evidence.
