use lopdf::{Document, ObjectId};
use medpdf::{DrawLineParams, MedpdfError, PdfColor, PlacePageParams};

use crate::spec_types::{BookletSpec, DuplexFlip, GridOrder, NupSpec};

struct PagePlacement {
    source_page: u32, // 1-based; 0 = blank slot (skip)
    x: f64,
    y: f64,
    scale: f64,
    rotation: f64,
}

struct SheetLayout {
    placements: Vec<PagePlacement>,
}

struct BorderRect {
    sheet_index: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

/// One imposition cell's usable dimensions, already proven positive.
///
/// Construction is the validation: [`CellGeometry::compute`] is the only way to make
/// one, and it refuses any parameter combination that leaves no room. Downstream code
/// can therefore divide by these without re-checking.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CellGeometry {
    pub cell_w: f64,
    pub cell_h: f64,
}

impl CellGeometry {
    /// Computes the per-cell box for a grid on a sheet, or fails naming the arithmetic.
    ///
    /// The check is on the DERIVED quantity, not on the sign of any input, and that is
    /// deliberate (bug-0006 plus the 2026-09-09 negative-offset ruling in bug-0009).
    /// A negative `margin` is a legitimate full-bleed layout — it makes the cell
    /// LARGER — so rejecting negative margins at parse time would foreclose a real
    /// use while still missing the actual fault. The fault is a non-positive cell,
    /// and only a large POSITIVE margin or gutter produces one.
    ///
    /// A non-positive cell is never a layout anyone wanted: every downstream number
    /// becomes meaningless, and the negative scale it produces is what turns a bad
    /// parameter into a plausible-looking corrupt PDF at exit 0.
    pub(crate) fn compute(
        paper_w: f64,
        paper_h: f64,
        cols: u32,
        rows: u32,
        margin: f64,
        gutter: f64,
    ) -> Result<Self, MedpdfError> {
        let avail_w = paper_w - 2.0 * margin - (cols as f64 - 1.0) * gutter;
        let avail_h = paper_h - 2.0 * margin - (rows as f64 - 1.0) * gutter;
        let cell_w = avail_w / cols as f64;
        let cell_h = avail_h / rows as f64;

        for (label, cell, paper) in [("width", cell_w, paper_w), ("height", cell_h, paper_h)] {
            if cell <= 0.0 {
                return Err(MedpdfError::new(format!(
                    "--nup: margin={margin}pt and gutter={gutter}pt leave no room on \
                     {paper_w}x{paper_h}pt paper for a {cols}x{rows} grid \
                     (cell {label} {cell:.1}pt, from {paper}pt). \
                     Reduce the margin or gutter, use larger paper, or use a smaller grid."
                )));
            }
        }

        Ok(Self { cell_w, cell_h })
    }

    /// How far content spills beyond each sheet edge, in points.
    ///
    /// Zero for every ordinary layout. Non-zero means a negative `margin` was used —
    /// a deliberate full bleed — and saying so is the obligation the 2026-09-09
    /// negative-offset ruling attaches to permitting negative offsets at all: the
    /// layout is legal, so the consequence must be visible rather than discovered
    /// on paper.
    ///
    /// It is exactly `-margin`, which is worth deriving rather than eyeballing.
    /// The first cell starts at `x = margin`, so it extends `|margin|` left of zero.
    /// The last cell's right edge is
    /// `margin + cols*cell_w + (cols-1)*gutter = margin + avail_w + (cols-1)*gutter`,
    /// and since `avail_w = paper_w - 2*margin - (cols-1)*gutter` that collapses to
    /// `paper_w - margin` — i.e. `|margin|` past the right edge. The gutter cancels,
    /// so it cannot contribute, and the same holds vertically.
    pub(crate) fn overhang(margin: f64) -> f64 {
        (-margin).max(0.0)
    }
}

/// Returns the computed [`CellGeometry`] so the caller can report it.
///
/// Reporting the derived geometry is not decoration: it is the obligation attached
/// to permitting negative offsets (bug-0006 / bug-0009). A bleed is a legal layout,
/// so a typo'd `margin=-0.5` is also legal — and the only thing separating the two
/// is whether the caller can see what the tool computed.
pub fn apply_nup(
    doc: &mut Document,
    page_ids: &mut Vec<ObjectId>,
    spec: &NupSpec,
) -> Result<CellGeometry, MedpdfError> {
    let num_pages = page_ids.len() as u32;
    let cells_per_sheet = spec.cols * spec.rows;

    let paper_w = spec.paper_width as f64;
    let paper_h = spec.paper_height as f64;
    let margin = spec.margin as f64;
    let gutter = spec.gutter as f64;

    let geom = CellGeometry::compute(paper_w, paper_h, spec.cols, spec.rows, margin, gutter)?;
    let (cell_w, cell_h) = (geom.cell_w, geom.cell_h);

    eprintln!(
        "Grid {}x{} on {:.0}x{:.0}pt paper; cell {:.1}x{:.1}pt",
        spec.cols, spec.rows, paper_w, paper_h, cell_w, cell_h
    );
    let overhang = CellGeometry::overhang(margin);
    if overhang > 0.0 {
        eprintln!(
            "Note: margin={margin:.1}pt is negative, so content bleeds up to {overhang:.1}pt \
             beyond each sheet edge and is trimmed by the page box."
        );
    }

    // Measure each source page as it will actually be PLACED, not by its raw
    // MediaBox extents.
    //
    // `get_page_media_box` is the PRE-rotation box (medpdf 0.13.0 documents it so),
    // while `place_page` honors the source `/Rotate`. Sizing a cell from the raw box
    // therefore transposes the footprint for a `/Rotate 90` source: the page is
    // placed upright but scaled against the wrong pair of numbers, so it overflows
    // its cell and can run off the sheet entirely (bug-0018 — measured 396pt of
    // content in a 306pt cell, 90pt of it lost past the paper edge).
    //
    // `placed_page_size` is computed from the same transform `place_page` emits, so
    // the geometry planned against and the geometry that lands cannot drift. It is
    // measured here at scale 1 because the footprint is exactly linear in scale
    // (`placed_page_size(d, p, s, r) == s * placed_page_size(d, p, 1.0, r)`), which
    // makes a fit-to-cell one division rather than an iteration.
    //
    // Rotation is 0 here deliberately: N-up never rotates a placement, and the
    // booklet back side rotates by 180, which preserves the footprint. A 90 or 270
    // placement rotation TRANSPOSES it, so any future mode that turns a page to fit
    // (`--tile` onto portrait sheets, say) must re-measure with its own rotation
    // rather than reuse this.
    let placed_sizes: Vec<(f64, f64)> = page_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            medpdf::placed_page_size(doc, id, 1.0, 0.0)
                .map(|(w, h)| (w as f64, h as f64))
                .ok_or_else(|| {
                    MedpdfError::new(format!(
                        "Could not measure page {}; aborting N-up imposition",
                        i + 1
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Build expanded page list when repeat > 1
    let expanded_pages: Vec<u32> = if spec.repeat > 1 {
        (0..num_pages)
            .flat_map(|p| std::iter::repeat_n(p, spec.repeat as usize))
            .collect()
    } else {
        (0..num_pages).collect()
    };
    let total_cells = expanded_pages.len() as u32;

    // Build sheet layouts and border rects
    let mut sheets = Vec::new();
    let mut borders = Vec::new();

    for group_start in (0..total_cells).step_by(cells_per_sheet as usize) {
        let sheet_index = sheets.len();
        let mut placements = Vec::new();

        for i in 0..cells_per_sheet {
            let cell_idx = group_start + i;
            if cell_idx >= total_cells {
                break;
            }

            let page_idx = expanded_pages[cell_idx as usize];
            let (row, col) = grid_position(i, spec.cols, spec.rows, spec.order);
            let (src_w, src_h) = placed_sizes[page_idx as usize];

            if src_w <= 0.0 || src_h <= 0.0 {
                continue;
            }

            let scale = (cell_w / src_w).min(cell_h / src_h);

            let x = margin + col as f64 * (cell_w + gutter) + (cell_w - src_w * scale) / 2.0;
            let y = margin
                + (spec.rows - 1 - row) as f64 * (cell_h + gutter)
                + (cell_h - src_h * scale) / 2.0;

            placements.push(PagePlacement {
                source_page: page_idx + 1,
                x,
                y,
                scale,
                rotation: 0.0,
            });

            if spec.border {
                borders.push(BorderRect {
                    sheet_index,
                    x,
                    y,
                    w: src_w * scale,
                    h: src_h * scale,
                });
            }
        }

        sheets.push(SheetLayout { placements });
    }

    impose_pages(doc, page_ids, &sheets, spec.paper_width, spec.paper_height)?;

    // Draw borders after imposition
    for border in &borders {
        let page_id = page_ids[border.sheet_index];
        let color = PdfColor::rgb(0.5, 0.5, 0.5);
        let line_w = 0.5;
        let (bx, by, bw, bh) = (
            border.x as f32,
            border.y as f32,
            border.w as f32,
            border.h as f32,
        );

        // Bottom
        medpdf::add_line(
            doc,
            page_id,
            &DrawLineParams::new(bx, by, bx + bw, by)
                .line_width(line_w)
                .color(color),
        )?;
        // Right
        medpdf::add_line(
            doc,
            page_id,
            &DrawLineParams::new(bx + bw, by, bx + bw, by + bh)
                .line_width(line_w)
                .color(color),
        )?;
        // Top
        medpdf::add_line(
            doc,
            page_id,
            &DrawLineParams::new(bx + bw, by + bh, bx, by + bh)
                .line_width(line_w)
                .color(color),
        )?;
        // Left
        medpdf::add_line(
            doc,
            page_id,
            &DrawLineParams::new(bx, by + bh, bx, by)
                .line_width(line_w)
                .color(color),
        )?;
    }

    Ok(geom)
}

/// Returns the per-page slot geometry, for the same reason as [`apply_nup`].
pub fn apply_booklet(
    doc: &mut Document,
    page_ids: &mut Vec<ObjectId>,
    spec: &BookletSpec,
) -> Result<CellGeometry, MedpdfError> {
    let num_pages = page_ids.len() as u32;

    let paper_w = spec.paper_width as f64;
    let paper_h = spec.paper_height as f64;
    let binding_margin = spec.binding_margin as f64;
    let half_w = (paper_w - binding_margin) / 2.0;
    if half_w <= 0.0 {
        return Err(MedpdfError::new(format!(
            "--booklet: binding_margin={binding_margin}pt leaves no room on {paper_w}pt-wide \
             paper (each half would be {half_w:.1}pt). Reduce binding_margin or use wider paper."
        )));
    }
    if paper_h <= 0.0 {
        return Err(MedpdfError::new(format!(
            "--booklet: paper height {paper_h}pt is not positive"
        )));
    }
    eprintln!(
        "Booklet halves {:.1}x{:.1}pt on {:.0}x{:.0}pt sheets",
        half_w, paper_h, paper_w, paper_h
    );

    // Measure each source page as it will actually be PLACED, not by its raw
    // MediaBox extents.
    //
    // `get_page_media_box` is the PRE-rotation box (medpdf 0.13.0 documents it so),
    // while `place_page` honors the source `/Rotate`. Sizing a cell from the raw box
    // therefore transposes the footprint for a `/Rotate 90` source: the page is
    // placed upright but scaled against the wrong pair of numbers, so it overflows
    // its cell and can run off the sheet entirely (bug-0018 — measured 396pt of
    // content in a 306pt cell, 90pt of it lost past the paper edge).
    //
    // `placed_page_size` is computed from the same transform `place_page` emits, so
    // the geometry planned against and the geometry that lands cannot drift. It is
    // measured here at scale 1 because the footprint is exactly linear in scale
    // (`placed_page_size(d, p, s, r) == s * placed_page_size(d, p, 1.0, r)`), which
    // makes a fit-to-cell one division rather than an iteration.
    //
    // Rotation is 0 here deliberately: booklet never rotates a placement, and the
    // booklet back side rotates by 180, which preserves the footprint. A 90 or 270
    // placement rotation TRANSPOSES it, so any future mode that turns a page to fit
    // (`--tile` onto portrait sheets, say) must re-measure with its own rotation
    // rather than reuse this.
    let placed_sizes: Vec<(f64, f64)> = page_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            medpdf::placed_page_size(doc, id, 1.0, 0.0)
                .map(|(w, h)| (w as f64, h as f64))
                .ok_or_else(|| {
                    MedpdfError::new(format!(
                        "Could not measure page {}; aborting booklet imposition",
                        i + 1
                    ))
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if spec.back >= num_pages {
        return Err(MedpdfError::new(format!(
            "back={} must be less than total page count ({})",
            spec.back, num_pages
        )));
    }

    let pairs = if spec.back > 0 {
        let mapping = build_virtual_mapping(num_pages, spec.back);
        booklet_page_order_mapped(&mapping)
    } else {
        booklet_page_order(num_pages)
    };

    let mut sheets = Vec::new();
    for (pair_idx, pair) in pairs.iter().enumerate() {
        let is_back = pair_idx % 2 == 1;
        let mut placements = Vec::new();

        for (side, &page_num) in pair.iter().enumerate() {
            if page_num == 0 {
                continue;
            }

            let (src_w, src_h) = placed_sizes[(page_num - 1) as usize];

            if src_w <= 0.0 || src_h <= 0.0 {
                continue;
            }

            let scale = (half_w / src_w).min(paper_h / src_h);

            // Center within the half
            let cx = if side == 0 {
                // Left half
                (half_w - src_w * scale) / 2.0
            } else {
                // Right half
                half_w + binding_margin + (half_w - src_w * scale) / 2.0
            };
            let cy = (paper_h - src_h * scale) / 2.0;

            // Apply duplex flip for back pages.
            //
            // The compensation counteracts the physical flip the duplexer
            // performs, so it depends on the (sheet orientation, flip) PAIR and
            // not on the flip alone. The axis that inverts content is the one
            // parallel to the content's horizontal: the LONG edge of a landscape
            // sheet, the SHORT edge of a portrait one.
            //
            // Confirmed by physical duplex print 2026-09-09 (bug-0001). The
            // previous rule keyed on the flip alone and rotated for ShortEdge
            // unconditionally, which is the portrait rule applied to the default
            // landscape sheet — so BOTH settings printed their backs upside down,
            // for opposite reasons. That is exactly what the test produced.
            //
            // The rotated back page takes the SAME (x, y) as an unrotated one:
            // since medpdf 0.13.0 (bug-0023/bug-0024) `place_page` anchors the
            // placed page's bounding box at (x, y) for any rotation, so the two
            // branches differ only in the rotation. The former `+ src_w * scale,
            // + src_h * scale` existed solely to undo the old contract's rotation
            // excursion; against 0.13.0 it double-compensates and throws the back
            // pages clean off the sheet (bug-0018).
            let landscape = paper_w > paper_h;
            let needs_180 = is_back
                && match spec.flip {
                    DuplexFlip::None => false,
                    DuplexFlip::LongEdge => landscape,
                    DuplexFlip::ShortEdge => !landscape,
                };
            let (x, y, rotation) = if needs_180 {
                (cx, cy, 180.0)
            } else {
                (cx, cy, 0.0)
            };

            placements.push(PagePlacement {
                source_page: page_num,
                x,
                y,
                scale,
                rotation,
            });
        }

        sheets.push(SheetLayout { placements });
    }

    impose_pages(doc, page_ids, &sheets, spec.paper_width, spec.paper_height)?;
    Ok(CellGeometry {
        cell_w: half_w,
        cell_h: paper_h,
    })
}

/// Serializes the current document to memory and reloads it as the source,
/// then reinitializes `doc` and places pages from the source onto new sheets.
/// The roundtrip is necessary because lopdf's `Document` doesn't implement Clone.
fn impose_pages(
    doc: &mut Document,
    page_ids: &mut Vec<ObjectId>,
    sheets: &[SheetLayout],
    sheet_w: f32,
    sheet_h: f32,
) -> Result<(), MedpdfError> {
    let mut buf = Vec::new();
    doc.save_to(&mut buf)?;
    let source_doc = Document::load_mem(&buf)?;

    // Reinitialize document
    *doc = crate::init_document();
    page_ids.clear();

    for sheet in sheets {
        let dest_page_id = medpdf::create_blank_page(doc, sheet_w, sheet_h)?;
        page_ids.push(dest_page_id);

        for placement in &sheet.placements {
            if placement.source_page == 0 {
                continue;
            }
            let params = PlacePageParams::new(placement.x, placement.y, placement.scale)
                .rotation(placement.rotation);
            medpdf::place_page(
                doc,
                dest_page_id,
                &source_doc,
                placement.source_page,
                &params,
            )?;
        }
    }

    Ok(())
}

fn grid_position(index: u32, cols: u32, rows: u32, order: GridOrder) -> (u32, u32) {
    match order {
        GridOrder::LeftToRightTopToBottom => (index / cols, index % cols),
        GridOrder::RightToLeftTopToBottom => (index / cols, (cols - 1) - (index % cols)),
        GridOrder::TopToBottomLeftToRight => (index % rows, index / rows),
        GridOrder::TopToBottomRightToLeft => (index % rows, (cols - 1) - (index / rows)),
    }
}

fn build_virtual_mapping(page_count: u32, back: u32) -> Vec<u32> {
    let front = page_count - back;
    let total = page_count.div_ceil(4) * 4;
    let blanks = total - page_count;
    (1..=front)
        .chain(std::iter::repeat_n(0u32, blanks as usize))
        .chain((front + 1)..=page_count)
        .collect()
}

fn booklet_page_order_mapped(mapping: &[u32]) -> Vec<[u32; 2]> {
    let total = mapping.len() as u32;
    debug_assert!(total.is_multiple_of(4));
    let num_sheets = total / 4;
    let mut pairs = Vec::with_capacity((num_sheets * 2) as usize);

    for s in 0..num_sheets {
        let front_left = total - 2 * s;
        let front_right = 2 * s + 1;
        pairs.push([
            mapping[(front_left - 1) as usize],
            mapping[(front_right - 1) as usize],
        ]);

        let back_left = 2 * s + 2;
        let back_right = total - 2 * s - 1;
        pairs.push([
            mapping[(back_left - 1) as usize],
            mapping[(back_right - 1) as usize],
        ]);
    }

    pairs
}

fn booklet_page_order(page_count: u32) -> Vec<[u32; 2]> {
    let total = page_count.div_ceil(4) * 4;
    let num_sheets = total / 4;
    let mut pairs = Vec::with_capacity((num_sheets * 2) as usize);

    for s in 0..num_sheets {
        // Front: [total - 2*s, 2*s + 1]
        let front_left = total - 2 * s;
        let front_right = 2 * s + 1;
        pairs.push([
            if front_left <= page_count {
                front_left
            } else {
                0
            },
            if front_right <= page_count {
                front_right
            } else {
                0
            },
        ]);

        // Back: [2*s + 2, total - 2*s - 1]
        let back_left = 2 * s + 2;
        let back_right = total - 2 * s - 1;
        pairs.push([
            if back_left <= page_count {
                back_left
            } else {
                0
            },
            if back_right <= page_count {
                back_right
            } else {
                0
            },
        ]);
    }

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_position_lrtb() {
        // 3 cols, 2 rows: indices 0..5
        assert_eq!(
            grid_position(0, 3, 2, GridOrder::LeftToRightTopToBottom),
            (0, 0)
        );
        assert_eq!(
            grid_position(1, 3, 2, GridOrder::LeftToRightTopToBottom),
            (0, 1)
        );
        assert_eq!(
            grid_position(2, 3, 2, GridOrder::LeftToRightTopToBottom),
            (0, 2)
        );
        assert_eq!(
            grid_position(3, 3, 2, GridOrder::LeftToRightTopToBottom),
            (1, 0)
        );
        assert_eq!(
            grid_position(4, 3, 2, GridOrder::LeftToRightTopToBottom),
            (1, 1)
        );
        assert_eq!(
            grid_position(5, 3, 2, GridOrder::LeftToRightTopToBottom),
            (1, 2)
        );
    }

    #[test]
    fn test_grid_position_rltb() {
        assert_eq!(
            grid_position(0, 3, 2, GridOrder::RightToLeftTopToBottom),
            (0, 2)
        );
        assert_eq!(
            grid_position(1, 3, 2, GridOrder::RightToLeftTopToBottom),
            (0, 1)
        );
        assert_eq!(
            grid_position(2, 3, 2, GridOrder::RightToLeftTopToBottom),
            (0, 0)
        );
        assert_eq!(
            grid_position(3, 3, 2, GridOrder::RightToLeftTopToBottom),
            (1, 2)
        );
    }

    #[test]
    fn test_grid_position_tblr() {
        assert_eq!(
            grid_position(0, 3, 2, GridOrder::TopToBottomLeftToRight),
            (0, 0)
        );
        assert_eq!(
            grid_position(1, 3, 2, GridOrder::TopToBottomLeftToRight),
            (1, 0)
        );
        assert_eq!(
            grid_position(2, 3, 2, GridOrder::TopToBottomLeftToRight),
            (0, 1)
        );
        assert_eq!(
            grid_position(3, 3, 2, GridOrder::TopToBottomLeftToRight),
            (1, 1)
        );
    }

    #[test]
    fn test_grid_position_tbrl() {
        assert_eq!(
            grid_position(0, 3, 2, GridOrder::TopToBottomRightToLeft),
            (0, 2)
        );
        assert_eq!(
            grid_position(1, 3, 2, GridOrder::TopToBottomRightToLeft),
            (1, 2)
        );
        assert_eq!(
            grid_position(2, 3, 2, GridOrder::TopToBottomRightToLeft),
            (0, 1)
        );
        assert_eq!(
            grid_position(3, 3, 2, GridOrder::TopToBottomRightToLeft),
            (1, 1)
        );
    }

    #[test]
    fn test_booklet_page_order_4_pages() {
        let pairs = booklet_page_order(4);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], [4, 1]); // front
        assert_eq!(pairs[1], [2, 3]); // back
    }

    #[test]
    fn test_booklet_page_order_8_pages() {
        let pairs = booklet_page_order(8);
        assert_eq!(pairs.len(), 4);
        assert_eq!(pairs[0], [8, 1]); // front sheet 0
        assert_eq!(pairs[1], [2, 7]); // back sheet 0
        assert_eq!(pairs[2], [6, 3]); // front sheet 1
        assert_eq!(pairs[3], [4, 5]); // back sheet 1
    }

    #[test]
    fn test_booklet_page_order_5_pages() {
        let pairs = booklet_page_order(5);
        // Pads to 8
        assert_eq!(pairs.len(), 4);
        assert_eq!(pairs[0], [0, 1]); // page 8 doesn't exist
        assert_eq!(pairs[1], [2, 0]); // page 7 doesn't exist
        assert_eq!(pairs[2], [0, 3]); // page 6 doesn't exist
        assert_eq!(pairs[3], [4, 5]);
    }

    #[test]
    fn test_booklet_page_order_1_page() {
        let pairs = booklet_page_order(1);
        // Pads to 4
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], [0, 1]); // page 4 doesn't exist
        assert_eq!(pairs[1], [0, 0]); // pages 2, 3 don't exist
    }

    #[test]
    fn test_booklet_page_order_12_pages() {
        let pairs = booklet_page_order(12);
        assert_eq!(pairs.len(), 6);
        assert_eq!(pairs[0], [12, 1]);
        assert_eq!(pairs[1], [2, 11]);
        assert_eq!(pairs[2], [10, 3]);
        assert_eq!(pairs[3], [4, 9]);
        assert_eq!(pairs[4], [8, 5]);
        assert_eq!(pairs[5], [6, 7]);
    }

    #[test]
    fn test_build_virtual_mapping_basic() {
        // 6 pages, back=2 → [1,2,3,4,0,0,5,6]
        let mapping = build_virtual_mapping(6, 2);
        assert_eq!(mapping, vec![1, 2, 3, 4, 0, 0, 5, 6]);
    }

    #[test]
    fn test_build_virtual_mapping_no_padding() {
        // 8 pages, back=4 → already multiple of 4, no blanks
        let mapping = build_virtual_mapping(8, 4);
        assert_eq!(mapping, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn test_build_virtual_mapping_back_1() {
        // 5 pages, back=1 → pads to 8: [1,2,3,4,0,0,0,5]
        let mapping = build_virtual_mapping(5, 1);
        assert_eq!(mapping, vec![1, 2, 3, 4, 0, 0, 0, 5]);
    }

    #[test]
    fn test_booklet_page_order_mapped_6_back2() {
        // mapping [1,2,3,4,0,0,5,6] → standard booklet on 8 virtual pages
        let mapping = build_virtual_mapping(6, 2);
        let pairs = booklet_page_order_mapped(&mapping);
        assert_eq!(pairs.len(), 4);
        assert_eq!(pairs[0], [6, 1]); // virtual 8→6, virtual 1→1
        assert_eq!(pairs[1], [2, 5]); // virtual 2→2, virtual 7→5
        assert_eq!(pairs[2], [0, 3]); // virtual 6→0, virtual 3→3
        assert_eq!(pairs[3], [4, 0]); // virtual 4→4, virtual 5→0
    }

    #[test]
    fn test_booklet_page_order_mapped_identity() {
        // mapping [1,2,3,4] should match booklet_page_order(4)
        let mapping = vec![1, 2, 3, 4];
        let mapped = booklet_page_order_mapped(&mapping);
        let direct = booklet_page_order(4);
        assert_eq!(mapped, direct);
    }
}
