# bug-0018: Imposition sizes cells from the **pre-rotation** MediaBox, so a `/Rotate 90` source is mis-scaled and mis-centered

**Severity:** Medium-High (silent wrong output: content overflows its cell or runs off the sheet, at exit 0) — but only for sources carrying `/Rotate` 90 or 270
**Type:** Code bug (`src/imposition.rs`, both `apply_nup` and `apply_booklet`)
**Status:** **REPRODUCED 2026-09-09** with measured numbers — see “Reproduction” below.  Filed as a code trace; upgraded the same day once the medpdf session pointed out that `medpdf::set_page_rotation` is public and builds the fixture in five lines.
**Filed:** 2026-09-09, during the medpdf 0.13.0 adoption

## Description

Both imposition functions measure the source page with `medpdf::get_page_media_box` and derive everything from those extents:

```rust
let src_w = mb[2] - mb[0];
let src_h = mb[3] - mb[1];
let scale = (cell_w / src_w).min(cell_h / src_h);          // apply_nup
let scale = (half_w / src_w).min(paper_h / src_h);         // apply_booklet
let x = margin + col * (cell_w + gutter) + (cell_w - src_w * scale) / 2.0;
```

As of **medpdf 0.13.0**, `get_page_media_box` is explicitly documented as the **pre-rotation** box, and `place_page` honors the source page’s `/Rotate` (medpdf bug-0023).  So for a source with `/Rotate 90` or `/Rotate 270` the placed footprint is **transposed** — `(src_h * scale, src_w * scale)` — while pdf-maker computed `scale` to fit the _untransposed_ dimensions and centered the cell using them.

Two consequences, both silent:

1. **Overflow.**  `scale` was chosen so `src_w x src_h` fits the cell; the actual footprint is `src_h x src_w`.  For a portrait source in a portrait cell the rotated footprint is wider than the cell, so the page bleeds into its neighbours or off the sheet.
2. **Mis-centering.**  The `(cell_w - src_w * scale) / 2.0` centering term uses the wrong extent, so even where it fits, it sits off-centre.

`apply_booklet` has the same shape with `half_w` / `paper_h`.

## What changed, and why this is the moment to fix it

This is not new arithmetic, but its symptom changed when this repo adopted medpdf 0.13.0:

- **Before 0.13.0:** `place_page` ignored `/Rotate` entirely, so the page was placed unrotated.  The arithmetic was self-consistent — the result was visibly _sideways_ but correctly scaled and centred within its cell.
- **After 0.13.0:** the page is placed as displayed — upright — but pdf-maker still sizes the cell from the unrotated box.  Upright and wrong size is arguably worse than sideways and right size, because the overflow can silently cover adjacent content.

Neither state is correct; the fix is the same either way.

## Suggested fix

medpdf 0.13.0 added the helpers this code needs, precisely so callers stop modelling placement geometry themselves:

```
medpdf::get_page_effective_size(doc, page_id)                  -> Option<(f32, f32)>
medpdf::placed_page_size(doc, page_id, scale, rotation)        -> Option<(f32, f32)>
```

- Replace the `src_w` / `src_h` derivation with **`get_page_effective_size`** — MediaBox extents swapped under `/Rotate` 90/270 — and compute `scale` and the centering terms from those.  That alone fixes both consequences.
- Where a placement rotation is also in play (the `--booklet` back side, and `--tile` when it lands), confirm the footprint with **`placed_page_size(doc, page, scale, rotation)`**, which is computed from the same transform `place_page` emits, so the geometry planned against and the geometry that lands cannot drift.  Note a 90° or 270° _placement_ rotation transposes the footprint too — independent of `/Rotate`, and wrong at any point in this repo’s history.
- Keep `get_page_media_box` only where the raw box is genuinely wanted (nothing in `imposition.rs` currently qualifies).

## Reproduction (verified 2026-09-09, pdf-maker v0.14.0, medpdf 0.13.2)

The fixture needs no external file: `medpdf::set_page_rotation` is public, so a `/Rotate 90`
source is four lines in the test harness.

```rust
let mut doc = Document::load("bugs/bug-0001/input-4page.pdf").unwrap();
for pid in doc.get_pages().values().copied().collect::<Vec<_>>() {
    medpdf::set_page_rotation(&mut doc, pid, 90).unwrap();
}
doc.save(&src).unwrap();
```

Then `--nup "cols=2,rows=2,paper=letter"` over those four pages, and read the `re` rectangles
`place_page` emits — each is exactly `(x, y, placed_w, placed_h)`.

**Sheet 612×792, so each cell is 306 wide × 396 tall.  Measured placements:**

```
re: [  0, 396, 396, 306]
re: [306, 396, 396, 306]
re: [  0,   0, 396, 306]
re: [306,   0, 396, 306]
```

Every placement is **396 wide in a 306-wide cell** — 90 pt of overflow, 29 % — and 306 tall in
a 396-tall cell, under-filling the height by the same 90 pt.  Two consequences, both silent at
exit 0:

- **Content overlaps the neighbouring cell.**  The left column occupies x ∈ [0, 396] where its
  cell ends at 306.
- **Content runs off the paper.**  The right column occupies x ∈ [306, 702] on a 612-wide
  sheet — 90 pt lost off the right edge, unrecoverable.

The arithmetic confirms the mechanism exactly.  Source MediaBox is 612 × 792 pre-rotation, so
`scale = min(306/612, 396/792) = 0.5`, and the _rotated_ footprint is `792 × 0.5` by
`612 × 0.5` = **396 × 306** — precisely what was measured.  Sizing from the effective
(post-rotation) 792 × 612 instead gives `scale = min(306/792, 396/612) = 0.386` and a
306 × 236.5 footprint, which fits.

That the failure is a wrong _number_ rather than a wrong _shape_ is why this needed a run: a
trace establishes that two numbers disagree, not which one is wrong.

## Second site: `--pad-to` sizes its pad pages the same way

Found 2026-09-09 while answering the pdf-orchestrator session’s question about drawing onto
pages pdf-maker did not create.  Same root cause, different function, so it is folded in here
rather than given an ID of its own.

`src/main.rs:496` measures the **last page** with `get_page_media_box` to decide how big the
blank pages appended by `--pad-to` should be:

```rust
let media_box = medpdf::get_page_media_box(doc, last_page_id)...;
let width  = media_box[2] - media_box[0];
let height = media_box[3] - media_box[1];
```

On a last page carrying `/Rotate 90`, that is the pre-rotation box, so the appended blank comes
out portrait behind a page that displays landscape — a pad page the wrong way round, at exit 0.
The fix is the same one-word substitution: `get_page_effective_size`.

Also code-trace, not reproduced, and it wants the same `/Rotate 90` fixture.

**Not affected, checked in the same pass:** the `--watermark` / `--draw-rect` / `--draw-line` /
`--draw-image` family takes absolute caller coordinates and never measures the page — with the
exception of `h_align` and `v_align`, which resolve inside medpdf against whatever box medpdf
consults.  That one is medpdf’s question, not this report’s, and it is the same shape as
pdf-orchestrator’s bug-0050.

## Related

- **medpdf bug-0023 / bug-0024**, fixed in medpdf 0.13.0 — the contract change that exposed this.
- **`plans/plan-0003`** (`--tile`) — the reason this matters now.  `--tile` derives its _grid_ from the source’s effective dimensions, so an unhonored transposition there yields the wrong **number of sheets**, not merely wrong content on them.  Fix this before `--tile` is written, or `--tile` inherits it.
- **pdf-orchestrator bug-0050** is the destination-side twin (drawing onto a `/Rotate 90` page using the raw MediaBox).  Worth asking the same question here about `--watermark`, `--draw-rect`, `--draw-line` and `--draw-image` applied to imported rotated pages — not covered by this report.
