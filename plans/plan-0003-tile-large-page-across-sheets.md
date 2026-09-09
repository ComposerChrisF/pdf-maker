# Plan: `--tile` — split one large page across many sheets, with overlap

## Problem

Microsoft Publisher is removed from Microsoft 365 on **2026-10-01**, and one of the
capabilities going with it has no replacement in the portfolio: **printing a large-format
page tiled across smaller sheets, with an adjustable overlap so the sheets can be taped
together.**

Verified on a real artifact, 2026-09-09 —
`FraleyMusic/Proj-Conferences/~~~Common Materials~~~/Banner-20260226-A3-2026WACDA.pub.pdf`:

```
Pages:     4
MediaBox:  [0 0 792 1224]        11in x 17in per sheet
Producer:  Microsoft: Print To PDF
Image1:    9966 x 396 px         the banner artwork, ~25:1
```

That is Publisher’s tiling output: one large source page, printed across four tabloid
sheets.  Chris has used this for conference banners for years (ACDA 2023, ACDA 2024,
Chorus America 2025, WACDA 2026).

**Nothing else Chris has does it.**

- **PowerPoint has no tiled printing at all.**  It scales one slide onto one sheet; every
  route to tiling is a workaround (export to PDF and tile in Acrobat, paste into Excel, or
  rely on a particular printer driver).  This matters because PowerPoint is otherwise the
  right Publisher successor for layout work.
- **Affinity Publisher does have it** — a Tiled print mode with an overlap setting — but
  `.afpub` is a single-vendor undocumented format, which fails the criterion that motivates
  all of this: a **20-to-30-year horizon**, without re-converting everything every five to
  ten years.  Affinity is the right tool for the banner _today_ and the wrong thing to
  depend on for the capability.

**Tiling is a print-time operation on a PDF, not a property of the source format.**  So it
belongs to whatever tool owns page geometry — and here that is `pdf-maker`, which already
owns imposition.

## Why `pdf-maker`, specifically

`--tile` is the **inverse of `--nup`**, and shares its arithmetic:

| | Source pages | Output sheets |
|---|---|---|
| `--nup` | many | one |
| `--tile` | one | many |

Both compute a grid over a target paper size, place a scaled source rectangle into each
cell, and must reject geometry that leaves no usable area.  `apply_nup` already does the
cell math (`cell_w`/`cell_h`), already takes `paper`, `margin`, and `gutter`, and already
has the validation obligations spelled out in `bug-0006` and `bug-0009`.  Building tiling
anywhere else would re-derive arithmetic that exists here — and re-derive its bugs.

## Proposed Change

```
pdf-maker -o banner-tiled.pdf banner.pdf "all" \
  --tile "paper=tabloid,overlap=0.5,units=in,marks=labels"
```

Spec keys, deliberately mirroring `--nup`’s vocabulary rather than inventing a second one:

| Key | Default | Meaning |
|---|---|---|
| `paper` | `letter` | Target sheet size — same vocabulary as `--nup`’s `paper` |
| `overlap` | **`0.5`** | Overlap between adjacent tiles.  See below — the default must not be 0 |
| `margin` | `0` | Additional unprinted margin inside each sheet |
| `pages` | `all` | Which source pages to tile |
| `order` | `row` | `row` (left-to-right, top-to-bottom) or `col` |
| `marks` | `none` | `none`, `labels` (row/column captions), `crop`, or both |
| `scale` | `1.0` | Output scale.  **Not** fit-to-page: the point of tiling is 1:1 output |
| `units` | `pt` | `pt`, `in`, `mm`, `cm`, matching the rest of the tool |

### Three design decisions that need stating, not discovering

**1.  `overlap` must default to a non-zero value.**  Consumer printers have a non-printable
margin of roughly 1/8 inch, so `overlap=0` silently loses a strip of artwork at every seam —
the output looks right in a viewer and is wrong on paper.  Both Publisher and Affinity
default non-zero for this reason.  `0.5in` is a safe default and `--help` should say _why_
it is not zero, or someone will helpfully “fix” it.

**2.  `--tile` multiplies pages, which no other `pdf-maker` operation does.**  Merge selects
pages, watermarks and shapes overlay them, `--pad-to` appends: all are page-count-preserving
or page-count-additive in an obvious way.  Tiling turns one page into N, and that changes
what composition means:

- A watermark applied **before** tiling is cut across the seams — usually wrong.
- A watermark applied **after** tiling repeats on every sheet — usually wanted, e.g. a
  “sheet 3 of 12” caption.

So the pipeline order must be documented, not left to fall out of the implementation.
Proposed: **`--tile` runs last**, after merge and after all overlays, so overlays land on the
finished sheets.  A caller who wants the other behaviour runs `pdf-maker` twice.

**3.  Assembly marks are not decoration.**  Taping twelve sheets together in the right order
is the actual failure point, and it happens after printing when the source is no longer on
screen.  `marks=labels` printing “R2 C3” in the overlap zone — which is trimmed or hidden by
the tape — turns a jigsaw into a procedure.  This is cheap and it is the difference between
the feature being used and being abandoned.

### Conflicts and exit codes

- **`--tile` conflicts with `--nup` and `--booklet`**, exactly as those two conflict with
  each other.  Exit **2** — decidable from the argument text alone.
- **Degenerate geometry is exit 1, naming the arithmetic**, per `bug-0006`’s standing
  requirement for the imposition path: an `overlap` or `margin` that leaves no usable cell
  must report the paper size, the overlap, the margin, and the resulting cell dimension —
  not silently emit zero tiles or one wrong one.
- **Negative or zero `overlap`/`margin`/`scale` are rejected at parse time** (exit 2), per
  `bug-0009`, so the absurd-positive and the negative cases are both loud.
- A `pages` spec naming a page beyond the end is exit **1** naming the real count, which is
  already the tool’s contract.

## Implementation Notes

- Reuse `apply_nup`’s cell geometry rather than writing a second copy; the difference is
  which side of the relation is fixed.  Tiling computes `cols = ceil((src_w - overlap) /
  (sheet_w - overlap))` and the same for rows, then emits a cell per (row, col) with the
  source page drawn at an offset.
- Each output sheet is the source page placed with a translation and clipped to the sheet
  box — the same XObject-placement machinery `--nup` already uses, so the orphan-stream
  fault in `bug-0007` applies here too and must not be repeated.
- **Land after `bug-0005`, `bug-0006`, `bug-0008`, and `bug-0009`**, or alongside them.  All
  four are open against the imposition path this extends, and one of them (`bug-0005`) is
  that `--help` does not document the `--nup` keys at all.  Adding a third imposition mode on
  top of an undocumented, unvalidated one would compound both faults.
- `--dry-run` should report the computed grid (`4 x 1 sheets`) so the geometry can be checked
  without producing a file.

## Why Not a Workaround

- **Acrobat Pro** does tiled printing, and is a subscription whose vendor can retire a
  feature — which is the precise failure being worked around here.  It also does not help a
  batch or scripted flow.
- **Printer drivers** sometimes tile, inconsistently, and the capability disappears with the
  printer.
- **Keeping Publisher** is not available after 2026-10-01.
- **Affinity** solves the banner and not the capability: it cannot be scripted into the
  existing PDF pipeline, and it re-introduces the single-vendor-format dependency this whole
  migration exists to escape.

A tool Chris owns, source-controlled, operating on PDF — a published ISO standard — is the
only option that scores well on the horizon that motivated the question.

## Urgency — moderate, and explicitly not blocking the Publisher deadline

The 2026 WACDA banner is **already tiled** in its existing PDF and remains printable.  What
is lost on 2026-10-01 is the ability to re-tile a **changed** banner — so the deadline for
this plan is the _next_ conference banner, not 1 October.

Filed 2026-09-09, from the Publisher retirement review.
