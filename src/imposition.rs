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

pub fn apply_nup(
    doc: &mut Document,
    page_ids: &mut Vec<ObjectId>,
    spec: &NupSpec,
) -> Result<(), MedpdfError> {
    let num_pages = page_ids.len() as u32;
    let cells_per_sheet = spec.cols * spec.rows;

    let paper_w = spec.paper_width as f64;
    let paper_h = spec.paper_height as f64;
    let margin = spec.margin as f64;
    let gutter = spec.gutter as f64;

    // Compute cell dimensions
    let avail_w = paper_w - 2.0 * margin - (spec.cols as f64 - 1.0) * gutter;
    let avail_h = paper_h - 2.0 * margin - (spec.rows as f64 - 1.0) * gutter;
    let cell_w = avail_w / spec.cols as f64;
    let cell_h = avail_h / spec.rows as f64;

    // Collect source MediaBoxes before we reinitialize the document
    let media_boxes: Vec<[f64; 4]> = page_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            let mb = medpdf::get_page_media_box(doc, id).unwrap_or_else(|| {
                eprintln!("Warning: could not read MediaBox for page {}, assuming letter size", i + 1);
                [0.0, 0.0, 612.0, 792.0]
            });
            [mb[0] as f64, mb[1] as f64, mb[2] as f64, mb[3] as f64]
        })
        .collect();

    // Build sheet layouts and border rects
    let mut sheets = Vec::new();
    let mut borders = Vec::new();

    for group_start in (0..num_pages).step_by(cells_per_sheet as usize) {
        let sheet_index = sheets.len();
        let mut placements = Vec::new();

        for i in 0..cells_per_sheet {
            let page_idx = group_start + i;
            if page_idx >= num_pages {
                break;
            }

            let (row, col) = grid_position(i, spec.cols, spec.rows, spec.order);
            let mb = media_boxes[page_idx as usize];
            let src_w = mb[2] - mb[0];
            let src_h = mb[3] - mb[1];

            if src_w <= 0.0 || src_h <= 0.0 {
                continue;
            }

            let scale = (cell_w / src_w).min(cell_h / src_h);

            let x = margin + col as f64 * (cell_w + gutter)
                + (cell_w - src_w * scale) / 2.0;
            let y = margin + (spec.rows - 1 - row) as f64 * (cell_h + gutter)
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
        let (bx, by, bw, bh) = (border.x as f32, border.y as f32, border.w as f32, border.h as f32);

        // Bottom
        medpdf::add_line(doc, page_id, &DrawLineParams::new(bx, by, bx + bw, by).line_width(line_w).color(color))?;
        // Right
        medpdf::add_line(doc, page_id, &DrawLineParams::new(bx + bw, by, bx + bw, by + bh).line_width(line_w).color(color))?;
        // Top
        medpdf::add_line(doc, page_id, &DrawLineParams::new(bx + bw, by + bh, bx, by + bh).line_width(line_w).color(color))?;
        // Left
        medpdf::add_line(doc, page_id, &DrawLineParams::new(bx, by + bh, bx, by).line_width(line_w).color(color))?;
    }

    Ok(())
}

pub fn apply_booklet(
    doc: &mut Document,
    page_ids: &mut Vec<ObjectId>,
    spec: &BookletSpec,
) -> Result<(), MedpdfError> {
    let num_pages = page_ids.len() as u32;

    let paper_w = spec.paper_width as f64;
    let paper_h = spec.paper_height as f64;
    let binding_margin = spec.binding_margin as f64;
    let half_w = (paper_w - binding_margin) / 2.0;

    // Collect source MediaBoxes
    let media_boxes: Vec<[f64; 4]> = page_ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            let mb = medpdf::get_page_media_box(doc, id).unwrap_or_else(|| {
                eprintln!("Warning: could not read MediaBox for page {}, assuming letter size", i + 1);
                [0.0, 0.0, 612.0, 792.0]
            });
            [mb[0] as f64, mb[1] as f64, mb[2] as f64, mb[3] as f64]
        })
        .collect();

    let pairs = booklet_page_order(num_pages);

    let mut sheets = Vec::new();
    for (pair_idx, pair) in pairs.iter().enumerate() {
        let is_back = pair_idx % 2 == 1;
        let mut placements = Vec::new();

        for (side, &page_num) in pair.iter().enumerate() {
            if page_num == 0 {
                continue;
            }

            let mb = media_boxes[(page_num - 1) as usize];
            let src_w = mb[2] - mb[0];
            let src_h = mb[3] - mb[1];

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
            // LongEdge: no rotation needed — long-edge duplex is the natural
            // orientation for landscape booklets, so it behaves like None.
            let (x, y, rotation) =
                if is_back && spec.flip == DuplexFlip::ShortEdge {
                    (cx + src_w * scale, cy + src_h * scale, 180.0)
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
    Ok(())
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
            medpdf::place_page(doc, dest_page_id, &source_doc, placement.source_page, &params)?;
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

fn booklet_page_order(page_count: u32) -> Vec<[u32; 2]> {
    let total = page_count.div_ceil(4) * 4;
    let num_sheets = total / 4;
    let mut pairs = Vec::with_capacity((num_sheets * 2) as usize);

    for s in 0..num_sheets {
        // Front: [total - 2*s, 2*s + 1]
        let front_left = total - 2 * s;
        let front_right = 2 * s + 1;
        pairs.push([
            if front_left <= page_count { front_left } else { 0 },
            if front_right <= page_count { front_right } else { 0 },
        ]);

        // Back: [2*s + 2, total - 2*s - 1]
        let back_left = 2 * s + 2;
        let back_right = total - 2 * s - 1;
        pairs.push([
            if back_left <= page_count { back_left } else { 0 },
            if back_right <= page_count { back_right } else { 0 },
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
        assert_eq!(grid_position(0, 3, 2, GridOrder::LeftToRightTopToBottom), (0, 0));
        assert_eq!(grid_position(1, 3, 2, GridOrder::LeftToRightTopToBottom), (0, 1));
        assert_eq!(grid_position(2, 3, 2, GridOrder::LeftToRightTopToBottom), (0, 2));
        assert_eq!(grid_position(3, 3, 2, GridOrder::LeftToRightTopToBottom), (1, 0));
        assert_eq!(grid_position(4, 3, 2, GridOrder::LeftToRightTopToBottom), (1, 1));
        assert_eq!(grid_position(5, 3, 2, GridOrder::LeftToRightTopToBottom), (1, 2));
    }

    #[test]
    fn test_grid_position_rltb() {
        assert_eq!(grid_position(0, 3, 2, GridOrder::RightToLeftTopToBottom), (0, 2));
        assert_eq!(grid_position(1, 3, 2, GridOrder::RightToLeftTopToBottom), (0, 1));
        assert_eq!(grid_position(2, 3, 2, GridOrder::RightToLeftTopToBottom), (0, 0));
        assert_eq!(grid_position(3, 3, 2, GridOrder::RightToLeftTopToBottom), (1, 2));
    }

    #[test]
    fn test_grid_position_tblr() {
        assert_eq!(grid_position(0, 3, 2, GridOrder::TopToBottomLeftToRight), (0, 0));
        assert_eq!(grid_position(1, 3, 2, GridOrder::TopToBottomLeftToRight), (1, 0));
        assert_eq!(grid_position(2, 3, 2, GridOrder::TopToBottomLeftToRight), (0, 1));
        assert_eq!(grid_position(3, 3, 2, GridOrder::TopToBottomLeftToRight), (1, 1));
    }

    #[test]
    fn test_grid_position_tbrl() {
        assert_eq!(grid_position(0, 3, 2, GridOrder::TopToBottomRightToLeft), (0, 2));
        assert_eq!(grid_position(1, 3, 2, GridOrder::TopToBottomRightToLeft), (1, 2));
        assert_eq!(grid_position(2, 3, 2, GridOrder::TopToBottomRightToLeft), (0, 1));
        assert_eq!(grid_position(3, 3, 2, GridOrder::TopToBottomRightToLeft), (1, 1));
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
}
