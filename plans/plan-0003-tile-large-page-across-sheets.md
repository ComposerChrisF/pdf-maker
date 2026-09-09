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
  --tile "paper=tabloid,overlap=0.75,units=in,marks=labels"
```

Spec keys, deliberately mirroring `--nup`’s vocabulary — and its **defaults** — rather than
inventing a second one:

| Key | Default | Meaning |
|---|---|---|
| `paper` | `letter` | Target sheet size — same vocabulary as `--nup`’s `paper` |
| `paper_w`, `paper_h` | — | Custom sheet, exactly as on `--nup` |
| `orientation` | `auto` | `portrait`, `landscape`, `auto`.  Not cosmetic — a 25:1 banner is four sheets tabloid-landscape and seven tabloid-portrait |
| `overlap` | **`0.75`** | Overlap between adjacent tiles.  Decision 1: the default must not be 0, and 0.5 is not enough |
| `margin` | `0` | Unprintable margin held clear inside each sheet |
| `pages` | `all` | Which source pages to tile |
| `order` | `row` | `row` (left-to-right, top-to-bottom) or `col` |
| `marks` | `none` | `none`, `labels` (row/column captions), `crop`, or both |
| `scale` | `1.0` | Output scale, applied to the source **before** the grid is computed.  **Not** fit-to-page: the point of tiling is 1:1 output |
| `align` | `center` | Where the grid’s excess coverage goes — `center` or `start`.  Decision 4 |
| `max_sheets` | `400` | Refuse a run that would emit more sheets than this.  Decision 5 |
| `units` | `in` | `pt`, `in`, `mm`, `cm` — **`in`**, matching `--nup` and `--booklet` (`Unit::In`, `src/spec_types/layout.rs:163`), not the `pt` the shape flags use |

### Five design decisions that need stating, not discovering

**1.  `overlap` must default to a non-zero value — and to 0.75in, not 0.5in.**  Consumer
printers hold roughly a quarter inch unprintable at each edge, so the overlap the person
taping the sheets together actually has is `overlap - 2 x unprintable`.  At `overlap=0.5in`
that is **zero**: the printed strips meet exactly, with nothing spare for a hand-aligned
seam, and any drift opens a white line through the artwork.  At `overlap=0.75in` a quarter
inch of real overlap survives — enough to tape against.  Both Publisher and Affinity default
non-zero for the same reason.  `--help` must say _why_ it is not zero, or someone will
helpfully “fix” it.

**The `units` default is load-bearing here.**  An earlier draft of this plan defaulted
`units` to `pt`, which defeats this decision outright: `overlap=0.75` would then mean 0.75
**points**, about a hundredth of an inch, and the carefully-chosen safe default would
silently be no overlap at all.  The table above says `in`, which is also what `--nup` and
`--booklet` already do.

**2.  `--tile` multiplies pages, which no other `pdf-maker` operation does.**  Merge selects
pages, watermarks and shapes overlay them, `--pad-to` appends: all are page-count-preserving
or page-count-additive in an obvious way.  Tiling turns one page into N, and that changes
what composition means:

- A watermark applied **before** tiling is cut across the seams — usually wrong.
- A watermark applied **after** tiling repeats on every sheet — usually wanted.

**This is already settled, and not in the direction an earlier draft of this plan proposed.**
`--nup` and `--booklet` run inside `run()` immediately after `merge_pages` and **before**
`apply_overlays` and `apply_drawing_commands` (`src/main.rs:601`), so overlays and watermarks
already land on the finished sheets today.  `--tile` takes the same slot in the same
`else if` chain — after merge, before overlays, before padding.  Consequences to document
rather than let callers discover: a `pages=` spec on a watermark addresses **sheets**, not
source pages; `--pad-to 4` pads the **sheet** count, which is what a duplex print of the tiles
wants; and a caller who wants artwork tiled _with_ the source runs `pdf-maker` twice.

**3.  Assembly marks are not decoration.**  Taping twelve sheets together in the right order
is the actual failure point, and it happens after printing when the source is no longer on
screen.  `marks=labels` turns a jigsaw into a procedure, and it is cheap.  Specifics, so the
implementation does not have to guess:

- The caption names the source page as well as the cell — `p1 R2C3 of 4x1` — because
  `pages=all` over a multi-page source interleaves several grids into one output file, and
  `R2C3` alone is then ambiguous.
- Place it inside the **overlap band**, in the corner nearest that sheet’s own outer edge, so
  the neighbouring sheet covers it once the seam is lapped.
- `marks=crop` draws the tile boundary — the inner edge of the overlap band — so the
  assembler knows where to cut when butting rather than lapping.

**4.  The grid almost never divides evenly, and where the slack goes is a visible choice.**
The last tile in a row is partly empty by construction.  `align=center` distributes the excess
over both ends of the run, so every sheet carries roughly the same amount of artwork;
`align=start` pins the first tile to the source’s left/top edge and leaves all the slack on
the final sheet.  For the WACDA banner — 25:1, four sheets — `start` yields three full sheets
and one nearly blank one, which reads as a bug to whoever prints it.  Hence `center` as the
default.  The alternative, widening the overlap until it divides evenly, is tidier on paper
but makes the effective overlap depend on the artwork; record it as rejected rather than
leaving it to be re-proposed.

**5.  A units mistake here costs a ream of paper, so the sheet count needs a ceiling.**
`--tile` is the first `pdf-maker` operation whose output size is _derived_ rather than stated:
`scale=10`, or a source measured in points against an overlap measured in inches, silently
turns one page into hundreds.  `max_sheets=400` refuses the run, exit 1, naming the computed
grid and the ceiling, and telling the caller to raise `max_sheets` if they meant it.  Same
class of guard as `bug-0006`’s degenerate-cell check — an absurd result from a
plausible-looking argument — and much cheaper to add now than after the first 600-sheet spool.

### Conflicts and exit codes

- **`--tile` conflicts with `--nup` and `--booklet`**, exactly as those two conflict with each
  other, and for the same reason: they are one `else if` chain over the page list.  Declare it
  with clap’s `conflicts_with`, which makes it exit **2** for free.
- **Degenerate geometry is exit 1, naming the arithmetic**, per `bug-0006`’s standing
  requirement for the imposition path.  The tiling form of it is a **non-positive step**: with
  a per-sheet content window `W = sheet - 2*margin` and a step `T = W - overlap`, an
  `overlap >= W` gives `T <= 0` and the grid never terminates.  Report the paper size, the
  margin, the overlap, and the resulting step — not a hang, and not one wrong sheet.
- **Coverage, not sign, is the invariant on `overlap` and `margin`** — see below.  This
  replaces the “reject negative values at parse time” line an earlier draft carried.
- A `pages` spec naming a page beyond the end is exit **1** naming the real count, which is
  already the tool’s contract (`page_spec::expand`).
- **`max_sheets` exceeded is exit 1**, naming the grid and the ceiling (decision 5).

### Negative values: check coverage, not the sign

Ruled 2026-09-09.  `bug-0009` proposed rejecting negative `margin` / `gutter` /
`binding_margin` at parse time across the imposition flags.  That is the wrong instrument, and
it would foreclose a real layout: a **negative `--nup` margin is a bleed**, and it already
works.  Verified on v0.13.2 — `--nup "n=4,margin=-0.25,units=in"` places four cells at
`0.5227 0 0 0.5227 -15.95 396 cm`, a _positive_ scale with origins off the sheet edge, each
cell clipped to its own rect.  That is full-bleed 4-up, not garbage; the sign is not the
fault.  What `bug-0006` describes is a **non-positive derived cell**, and only a large
_positive_ margin produces one.

The principle, portfolio-shaped: **an extent may not be negative; an offset may.**  `w`, `h`,
`paper_w`, `paper_h`, `size`, `width` and `count` are extents and stay `> 0` at parse;
`alpha` keeps its `[0, 1]` domain; `margin`, `gutter`, `binding_margin`, `x` and `y` are
offsets, take any sign, and are guarded by the derived check instead.

For `--tile` the derived check is **coverage**, because a gap between tiles is source content
silently dropped — the failure mode this tool treats as cardinal:

- The drawn strip is `W = sheet - 2*margin`, of which only `min(W, sheet)` lands on paper, so
  a negative `margin` buys nothing and costs coverage.
- Gap-free assembly therefore requires `T <= min(W, sheet)`, i.e. `overlap >= max(0,
  -2*margin)`.
- Failing that is exit 1 naming the gap in source units — never a silently perforated banner.

So a negative `margin` is not rejected on sight; it is rejected when the arithmetic says the
tiles would not meet.  `overlap` is bounded below by that inequality rather than by a sign
check, and above by the non-positive-step check.

## Implementation Notes

- Reuse `apply_nup`’s cell geometry rather than writing a second copy; the difference is which
  side of the relation is fixed.  Tiling computes
  `cols = ceil((src_w*scale - overlap) / (sheet_w - 2*margin - overlap))`, clamped to at least
  1 — a source narrower than one sheet must still emit one tile, and the unclamped expression
  goes non-positive there — and the same for rows.
- **`scale` applies to the source before the grid is computed**, or the sheet count and the
  placement disagree.
- **Measure the source with `medpdf::placed_page_size(doc, page, 1.0, rotation)`, not with
  `get_page_media_box` extents and not with `get_page_effective_size`.**  Settled with the
  medpdf session 2026-09-09.  The footprint is exactly linear in scale — `placed_page_size(d,
  p, s, r) == s * placed_page_size(d, p, 1.0, r)` for every rotation — so a fit is one
  division and never an iteration.  Measure _with the placement rotation you intend_: 0° and
  180° both preserve the footprint, so a `--tile` implementation tested only on those looks
  correct and falls over on the first 90°, which is exactly the case that matters here
  (a landscape banner onto portrait sheets).  `get_page_effective_size` agrees only while the
  placement rotation is 0.
- **Hoist the page-number → `ObjectId` lookup out of the per-tile loop.**  `doc.get_pages()` is
  a full page-tree walk, and `--tile` calls per sheet where `--nup` called per source page —
  the one place in this design where the sheet-multiplying property has a cost.  (From the
  pdf-orchestrator session, 2026-09-09.)
- **Keep `overlap` in destination points.**  In `cols = ceil((src_w*scale - overlap) /
  (sheet_w - 2*margin - overlap))` the source term is now
  `placed_page_size(.., scale, rotation).0`, which is already destination points, so the units
  agree.  If `overlap` is ever re-expressed as a fraction of the _source_ page, the `/Rotate`
  transposition reaches the grid through the back door — a comment at the site is the right
  guard, not a test.
- Each output sheet is the source page placed with a translation and clipped to the sheet box
  — the same XObject-placement machinery `--nup` already uses, so `bug-0007`’s orphan-stream
  fault applies here and is _multiplied by the sheet count_: a twelve-sheet banner leaks
  twelve objects.  `--tile` is the mode that makes that bug matter.
- **Land after medpdf `bug-0023` and `bug-0024`.**  They are the deepest prerequisites and they
  are not in this repo: `place_page` currently ignores the source `/Rotate` and leaves the
  meaning of `(x, y)` undefined for a non-zero-origin MediaBox.  Both were ruled 2026-07-24 —
  honor `/Rotate`, and compensate the origin so `(x, y, scale)` alone places the visible box —
  and both rulings carry the action item “audit pdf-maker imposition”.  `--tile` is affected
  more than `--nup`: the tile grid is derived from the source’s _effective_ width and height, so
  an unhonored `/Rotate 90` swaps rows and columns and yields the wrong **sheet count**, while
  an uncompensated origin shifts every tile by `scale x origin`.  Writing this feature against
  the pre-fix primitive means writing it twice.  (The 2026 WACDA artifact itself is clean on
  both counts — `/Rotate 0`, MediaBox `[0 0 792 1224]`, DeviceRGB — so the exposure is to the
  _future_ large-format sources from PowerPoint or Affinity, not to that file.)  Raise the
  `medpdf = "0.12"` floor in `Cargo.toml` when the fixing release lands, or an older build
  silently restores the old placement.
- **Land after `bug-0005`, `bug-0006` and `bug-0009`** — all three are open against the
  imposition path this extends, and one of them (`bug-0005`) is that `--help` documents no
  `--nup` keys at all.  Adding a third imposition mode on top of an undocumented, unvalidated
  one would compound both faults.  **`bug-0007` is not a prerequisite but should be started
  first**: its fix may belong in medpdf, and a sibling-crate release is the longest lead time
  in this queue.  **`bug-0008` is _not_ a prerequisite**, contrary to this plan’s first draft
  — it concerns `auto_grid` mapping an `n` onto a grid, and `--tile` derives its grid from
  geometry and never calls `auto_grid`.
- `--dry-run` should report the computed grid (`4 x 1 sheets`) so the geometry can be checked
  without producing a file — and so should `--json`, on every run, since the grid is derived
  information the caller never stated.
- Sequence the doc work accordingly: land `--tile` **before** `bug-0012`, so the spec-drift
  sweep documents all three imposition modes once instead of twice.

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
