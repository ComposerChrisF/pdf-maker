# bug-0018: Imposition sizes cells from the **pre-rotation** MediaBox, so a `/Rotate 90` source is mis-scaled and mis-centered

**Severity:** Medium-High (silent wrong output: content overflows its cell or runs off the sheet, at exit 0) — but only for sources carrying `/Rotate` 90 or 270
**Type:** Code bug (`src/imposition.rs`, both `apply_nup` and `apply_booklet`)
**Status:** **Code trace, not yet reproduced.**  No `/Rotate 90` fixture exists in this repo and pdf-maker cannot produce one, so this is derived from the code plus medpdf 0.13.0’s documented contract, not from a run.  **Build the fixture before fixing.**
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

## Reproduction to build first

There is no `/Rotate` fixture in this repo and `--blank-page` cannot make one.  Construct it in the test harness the way `create_test_pdf` builds its pages, adding `"Rotate" => 90` to the page dictionary; then:

1. `--nup "cols=2,rows=2"` over four such pages.
2. Read the `re` rects from the output — `place_page` emits its clip rectangle as exactly `(x, y, placed_w, placed_h)`, so one operator pins the whole contract with no rendering (technique from the pdf-orchestrator session, 2026-09-09).
3. Assert every rect lies inside its cell and is centred in it.  Pre-fix, expect a rect wider than its cell.

Pin the same invariant for `apply_booklet`.

## Related

- **medpdf bug-0023 / bug-0024**, fixed in medpdf 0.13.0 — the contract change that exposed this.
- **`plans/plan-0003`** (`--tile`) — the reason this matters now.  `--tile` derives its _grid_ from the source’s effective dimensions, so an unhonored transposition there yields the wrong **number of sheets**, not merely wrong content on them.  Fix this before `--tile` is written, or `--tile` inherits it.
- **pdf-orchestrator bug-0050** is the destination-side twin (drawing onto a `/Rotate 90` page using the raw MediaBox).  Worth asking the same question here about `--watermark`, `--draw-rect`, `--draw-line` and `--draw-image` applied to imported rotated pages — not covered by this report.
