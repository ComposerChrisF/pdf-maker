// Imposition specs (N-up, booklet) and shared layout helpers.

use std::str::FromStr;

use medpdf::Unit;

use super::parse::{KvParser, parse_paper_size};

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum Orientation {
    #[default]
    Auto,
    Landscape,
    Portrait,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum GridOrder {
    #[default]
    LeftToRightTopToBottom,
    RightToLeftTopToBottom,
    TopToBottomLeftToRight,
    TopToBottomRightToLeft,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum DuplexFlip {
    #[default]
    None,
    ShortEdge,
    LongEdge,
}

fn parse_orientation(v: &str) -> Result<Orientation, String> {
    match v.to_lowercase().as_str() {
        "auto" => Ok(Orientation::Auto),
        "landscape" => Ok(Orientation::Landscape),
        "portrait" => Ok(Orientation::Portrait),
        _ => Err(format!(
            "Invalid orientation: '{v}'. Use auto, landscape, or portrait."
        )),
    }
}

fn parse_grid_order(v: &str) -> Result<GridOrder, String> {
    match v.to_lowercase().as_str() {
        "lrtb" => Ok(GridOrder::LeftToRightTopToBottom),
        "rltb" => Ok(GridOrder::RightToLeftTopToBottom),
        "tblr" => Ok(GridOrder::TopToBottomLeftToRight),
        "tbrl" => Ok(GridOrder::TopToBottomRightToLeft),
        _ => Err(format!(
            "Invalid order: '{v}'. Use lrtb, rltb, tblr, or tbrl."
        )),
    }
}

fn parse_duplex_flip(v: &str) -> Result<DuplexFlip, String> {
    match v.to_lowercase().as_str() {
        "none" => Ok(DuplexFlip::None),
        "short_edge" => Ok(DuplexFlip::ShortEdge),
        "long_edge" => Ok(DuplexFlip::LongEdge),
        _ => Err(format!(
            "Invalid flip value: '{v}'. Use none, short_edge, or long_edge."
        )),
    }
}

/// The values of `n` that have a canonical page-per-sheet layout.
///
/// `n` is restricted to these (bug-0008): the previous fallback rounded an
/// arbitrary `n` up to a grid and then filled every cell, so `n=3` silently
/// produced 4-up, `n=5` produced 6-up and `n=7` produced 9-up — the flag's
/// stated meaning ("input pages per sheet") and its behavior disagreed with no
/// warning. Anything outside this set is a usage error pointing the caller at
/// `cols=`/`rows=`, which already expresses any grid exactly.
pub(super) const CANONICAL_N: &[u32] = &[1, 2, 4, 6, 8, 9, 16];

/// Maps a canonical `n` to its `(cols, rows)` grid.
///
/// `2 => (2, 1)` — two pages **side by side** on a landscape sheet, which is what
/// every print dialog means by 2-up and what this tool's own `--booklet` already
/// does. It was `(1, 2)` (stacked on a portrait sheet) until 2026-09-09, the one
/// entry in this table that disagreed with the convention every other entry
/// follows, and it cost 29% of linear scale on letter sources (0.5 vs 0.647).
/// The stacked layout remains available explicitly as `cols=1,rows=2`.
///
/// Callers must validate against [`CANONICAL_N`] first; a non-canonical `n`
/// reaching here is a bug, not a layout question.
pub(super) fn auto_grid(n: u32) -> (u32, u32) {
    match n {
        1 => (1, 1),
        2 => (2, 1),
        4 => (2, 2),
        6 => (2, 3),
        8 => (2, 4),
        9 => (3, 3),
        16 => (4, 4),
        // Unreachable via NupSpec::from_str, which rejects non-canonical n.
        // Kept total rather than panicking: a wrong grid beats a crash, and the
        // parse-level check is the real guard.
        _ => {
            let cols = (n as f64).sqrt().ceil() as u32;
            let rows = n.div_ceil(cols);
            (cols, rows)
        }
    }
}

pub(super) fn resolve_paper_dims(
    paper: &Option<String>,
    paper_w: Option<f32>,
    paper_h: Option<f32>,
    unit: Unit,
    default: (f32, f32),
) -> Result<(f32, f32), String> {
    match (paper, paper_w, paper_h) {
        (Some(name), None, None) => parse_paper_size(name),
        (None, Some(w), Some(h)) => Ok((unit.to_points(w), unit.to_points(h))),
        (None, None, None) => Ok(default),
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            Err("Specify either 'paper' or both 'paper_w' and 'paper_h', not both".to_string())
        }
        (None, Some(_), None) | (None, None, Some(_)) => {
            Err("Both 'paper_w' and 'paper_h' are required for custom paper size".to_string())
        }
    }
}

pub(super) fn apply_orientation(
    w: f32,
    h: f32,
    orientation: Orientation,
    cols: u32,
    rows: u32,
) -> (f32, f32) {
    let want_landscape = match orientation {
        Orientation::Landscape => true,
        Orientation::Portrait => false,
        Orientation::Auto => cols > rows,
    };
    let needs_swap = (want_landscape && h > w) || (!want_landscape && w > h);
    if needs_swap { (h, w) } else { (w, h) }
}

#[derive(Debug, Clone)]
pub struct NupSpec {
    pub cols: u32,
    pub rows: u32,
    pub paper_width: f32,
    pub paper_height: f32,
    pub margin: f32,
    pub gutter: f32,
    pub order: GridOrder,
    pub border: bool,
    pub repeat: u32,
}

/// Where the grid's surplus coverage goes when the tiles do not divide evenly.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum TileAlign {
    /// Split the surplus between both ends, so every sheet carries roughly the same
    /// amount of artwork. Default because the alternative leaves one nearly blank
    /// sheet, which reads as a bug to whoever prints it.
    #[default]
    Center,
    /// Pin the first tile to the source's left/top edge and leave all the surplus on
    /// the final sheet.
    Start,
}

/// Which assembly marks to draw.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct TileMarks {
    /// Row/column captions naming the source page and cell.
    pub labels: bool,
    /// Trim lines at the tile boundary, for butting rather than lapping.
    pub crop: bool,
}

/// Sheet order across the grid.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum TileOrder {
    /// Left to right, then top to bottom.
    #[default]
    Row,
    /// Top to bottom, then left to right.
    Col,
}

fn parse_tile_align(v: &str) -> Result<TileAlign, String> {
    match v.to_lowercase().as_str() {
        "center" | "centre" => Ok(TileAlign::Center),
        "start" => Ok(TileAlign::Start),
        _ => Err(format!("Invalid align: '{v}'. Use center or start.")),
    }
}

fn parse_tile_order(v: &str) -> Result<TileOrder, String> {
    match v.to_lowercase().as_str() {
        "row" => Ok(TileOrder::Row),
        "col" | "column" => Ok(TileOrder::Col),
        _ => Err(format!("Invalid order: '{v}'. Use row or col.")),
    }
}

fn parse_tile_marks(v: &str) -> Result<TileMarks, String> {
    let mut marks = TileMarks::default();
    for part in v.split('+') {
        match part.trim().to_lowercase().as_str() {
            "none" => {}
            "labels" => marks.labels = true,
            "crop" => marks.crop = true,
            "both" => {
                marks.labels = true;
                marks.crop = true;
            }
            other => {
                return Err(format!(
                    "Invalid marks: '{other}'. Use none, labels, crop, both, or labels+crop."
                ));
            }
        }
    }
    Ok(marks)
}

/// Split one large page across many smaller sheets, with overlap for taping.
///
/// The inverse of [`NupSpec`]: N-up puts many source pages on one sheet, tiling puts
/// one source page on many sheets. Both compute a grid over a target paper size and
/// place a scaled source rectangle into each cell.
///
/// Unlike `NupSpec`, the paper dimensions here are stored **unrotated** and the
/// orientation is resolved later, in `apply_tile`. That is not an inconsistency: the
/// N-up grid is known at parse time, so `apply_orientation` can run there, whereas
/// the tile grid depends on the source page's size, which the parser has never seen.
#[derive(Debug, Clone)]
pub struct TileSpec {
    pub paper_width: f32,
    pub paper_height: f32,
    pub orientation: Orientation,
    /// Overlap between adjacent tiles, in points. Never zero by default — see the
    /// `--help` text for why.
    pub overlap: f32,
    /// Unprintable margin held clear inside each sheet, in points.
    pub margin: f32,
    pub pages: String,
    pub order: TileOrder,
    pub marks: TileMarks,
    /// Output scale, applied to the source *before* the grid is computed.
    pub scale: f32,
    pub align: TileAlign,
    /// Refuse a run that would emit more sheets than this.
    pub max_sheets: u32,
}

pub const TILE_KEYS: &[&str] = &[
    "paper",
    "paper_w",
    "paper_h",
    "orientation",
    "overlap",
    "margin",
    "pages",
    "order",
    "marks",
    "scale",
    "align",
    "max_sheets",
    "units",
];

impl FromStr for TileSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kv = KvParser::parse(s, "tile", TILE_KEYS)?;

        let paper = kv.get("paper").map(str::to_string);
        let paper_w = kv.optional_positive("paper_w")?;
        let paper_h = kv.optional_positive("paper_h")?;
        let orientation = kv.optional_with("orientation", parse_orientation)?;
        let unit: Unit = kv.optional_units()?.map(Unit::from).unwrap_or(Unit::In);

        // `overlap` and `margin` are offsets, not extents, so they are NOT sign-checked
        // here (the 2026-09-09 ruling). The invariant that actually matters for tiling
        // is COVERAGE — whether adjacent tiles still meet — and that is a derived
        // quantity checked in `apply_tile`, where the sheet and source sizes are known.
        let overlap = kv.optional_parse::<f32>("overlap")?;
        let margin = kv.optional_parse::<f32>("margin")?;

        let scale = kv.optional_positive("scale")?.unwrap_or(1.0);
        let pages = kv.get("pages").unwrap_or("all").to_string();
        let order = kv
            .optional_with("order", parse_tile_order)?
            .unwrap_or_default();
        let marks = kv
            .optional_with("marks", parse_tile_marks)?
            .unwrap_or_default();
        let align = kv
            .optional_with("align", parse_tile_align)?
            .unwrap_or_default();
        let max_sheets = kv.optional_with("max_sheets", |v| {
            let n = v
                .parse::<u32>()
                .map_err(|_| format!("Invalid max_sheets value: '{v}'. Use a positive integer."))?;
            if n == 0 {
                Err("max_sheets must be a positive integer".to_string())
            } else {
                Ok(n)
            }
        })?;

        let (pw, ph) = resolve_paper_dims(&paper, paper_w, paper_h, unit, (612.0, 792.0))?;

        Ok(TileSpec {
            paper_width: pw,
            paper_height: ph,
            orientation: orientation.unwrap_or_default(),
            // 0.75in, not 0.5in: a consumer printer holds roughly a quarter inch
            // unprintable at each edge, so the overlap the person taping the sheets
            // actually has is `overlap - 2 x unprintable`. At 0.5in that is zero.
            overlap: unit.to_points(overlap.unwrap_or(0.75)),
            margin: unit.to_points(margin.unwrap_or(0.0)),
            pages,
            order,
            marks,
            scale,
            align,
            max_sheets: max_sheets.unwrap_or(400),
        })
    }
}

pub const NUP_KEYS: &[&str] = &[
    "n",
    "cols",
    "rows",
    "paper",
    "paper_w",
    "paper_h",
    "orientation",
    "margin",
    "gutter",
    "units",
    "order",
    "border",
    "repeat",
];

impl FromStr for NupSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kv = KvParser::parse(s, "nup", NUP_KEYS)?;

        let n = kv.optional_parse::<u32>("n")?;
        let cols_in = kv.optional_parse::<u32>("cols")?;
        let rows_in = kv.optional_parse::<u32>("rows")?;
        let paper = kv.get("paper").map(str::to_string);
        let paper_w = kv.optional_positive("paper_w")?;
        let paper_h = kv.optional_positive("paper_h")?;
        let orientation = kv.optional_with("orientation", parse_orientation)?;
        let margin = kv.optional_parse::<f32>("margin")?;
        let gutter = kv.optional_parse::<f32>("gutter")?;
        let unit: Unit = kv.optional_units()?.map(Unit::from).unwrap_or(Unit::In);
        let order = kv.optional_with("order", parse_grid_order)?;
        let border = kv.optional_with("border", |v| {
            v.parse::<bool>()
                .map_err(|_| format!("Invalid border value: '{v}'. Use true or false."))
        })?;
        // `repeat=auto` parses to sentinel 0 and is resolved below once cols*rows is known.
        let repeat = kv.optional_with("repeat", |v| match v.to_lowercase().as_str() {
            "auto" => Ok(0u32),
            _ => {
                let n = v.parse::<u32>().map_err(|_| {
                    format!("Invalid repeat value: '{v}'. Use a positive integer or 'auto'.")
                })?;
                if n == 0 {
                    Err("repeat must be a positive integer or 'auto'".to_string())
                } else {
                    Ok(n)
                }
            }
        })?;

        let (cols, rows) = match (n, cols_in, rows_in) {
            (Some(n_val), None, None) => {
                if n_val == 0 {
                    return Err("n must be greater than 0".to_string());
                }
                if !CANONICAL_N.contains(&n_val) {
                    let list = CANONICAL_N
                        .iter()
                        .map(u32::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(format!(
                        "n={n_val} has no canonical {n_val}-up layout. Use one of: {list}; \
                         or specify the grid exactly with cols= and rows= \
                         (e.g. cols=3,rows=1 for three across)."
                    ));
                }
                auto_grid(n_val)
            }
            (None, Some(c), Some(r)) => {
                if c == 0 || r == 0 {
                    return Err("cols and rows must be greater than 0".to_string());
                }
                (c, r)
            }
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
                return Err("Specify either 'n' or both 'cols' and 'rows', not both".to_string());
            }
            (None, Some(_), None) | (None, None, Some(_)) => {
                return Err("Both 'cols' and 'rows' are required when not using 'n'".to_string());
            }
            (None, None, None) => {
                return Err("Either 'n' or both 'cols' and 'rows' are required".to_string());
            }
        };

        let (pw, ph) = resolve_paper_dims(&paper, paper_w, paper_h, unit, (612.0, 792.0))?;
        let (pw, ph) = apply_orientation(pw, ph, orientation.unwrap_or_default(), cols, rows);

        let repeat = match repeat {
            Some(0) => cols * rows,
            Some(v) => v,
            None => 1,
        };

        Ok(NupSpec {
            cols,
            rows,
            paper_width: pw,
            paper_height: ph,
            margin: unit.to_points(margin.unwrap_or(0.0)),
            gutter: unit.to_points(gutter.unwrap_or(0.0)),
            order: order.unwrap_or_default(),
            border: border.unwrap_or(false),
            repeat,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BookletSpec {
    pub paper_width: f32,
    pub paper_height: f32,
    pub binding_margin: f32,
    pub flip: DuplexFlip,
    pub back: u32,
}

pub const BOOKLET_KEYS: &[&str] = &[
    "paper",
    "paper_w",
    "paper_h",
    "binding_margin",
    "units",
    "flip",
    "back",
];

impl FromStr for BookletSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // `--booklet` with no value reaches here as "" (see Args.booklet `default_missing_value`);
        // short-circuit to defaults so the bare flag still works.
        if s.trim().is_empty() {
            return Ok(BookletSpec {
                paper_width: 792.0,
                paper_height: 612.0,
                binding_margin: 0.0,
                flip: DuplexFlip::None,
                back: 0,
            });
        }

        let kv = KvParser::parse(s, "booklet", BOOKLET_KEYS)?;

        let paper = kv.get("paper").map(str::to_string);
        let paper_w = kv.optional_positive("paper_w")?;
        let paper_h = kv.optional_positive("paper_h")?;
        let binding_margin = kv.optional_parse::<f32>("binding_margin")?;
        let unit: Unit = kv.optional_units()?.map(Unit::from).unwrap_or(Unit::In);
        let flip = kv.optional_with("flip", parse_duplex_flip)?;
        let back = kv.optional_parse::<u32>("back")?.unwrap_or(0);

        let (pw, ph) = resolve_paper_dims(&paper, paper_w, paper_h, unit, (792.0, 612.0))?;
        // For named paper sizes, ensure landscape orientation
        let (pw, ph) = if paper.is_some() && ph > pw {
            (ph, pw)
        } else {
            (pw, ph)
        };

        Ok(BookletSpec {
            paper_width: pw,
            paper_height: ph,
            binding_margin: unit.to_points(binding_margin.unwrap_or(0.0)),
            flip: flip.unwrap_or_default(),
            back,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- auto_grid ---

    #[test]
    fn test_auto_grid_known_values() {
        assert_eq!(auto_grid(2), (2, 1));
        assert_eq!(auto_grid(4), (2, 2));
        assert_eq!(auto_grid(6), (2, 3));
        assert_eq!(auto_grid(8), (2, 4));
        assert_eq!(auto_grid(9), (3, 3));
        assert_eq!(auto_grid(16), (4, 4));
    }

    #[test]
    fn test_auto_grid_fallback() {
        assert_eq!(auto_grid(3), (2, 2));
        assert_eq!(auto_grid(5), (3, 2));
        assert_eq!(auto_grid(7), (3, 3));
        assert_eq!(auto_grid(1), (1, 1));
    }

    // --- NupSpec ---

    #[test]
    fn test_nup_spec_with_n() {
        let spec = NupSpec::from_str("n=4").unwrap();
        assert_eq!(spec.cols, 2);
        assert_eq!(spec.rows, 2);
        assert!((spec.paper_width - 612.0).abs() < f32::EPSILON);
        assert!((spec.paper_height - 792.0).abs() < f32::EPSILON);
        assert!(!spec.border);
    }

    #[test]
    fn test_nup_spec_explicit_grid() {
        let spec = NupSpec::from_str("cols=3,rows=2,paper=a4").unwrap();
        assert_eq!(spec.cols, 3);
        assert_eq!(spec.rows, 2);
        assert!((spec.paper_width - 841.89).abs() < 0.01);
        assert!((spec.paper_height - 595.28).abs() < 0.01);
    }

    #[test]
    fn test_nup_spec_with_options() {
        let spec = NupSpec::from_str("n=4,margin=0.5,gutter=0.25,units=in,border=true,order=rltb")
            .unwrap();
        assert!((spec.margin - 36.0).abs() < f32::EPSILON);
        assert!((spec.gutter - 18.0).abs() < f32::EPSILON);
        assert!(spec.border);
        assert_eq!(spec.order, GridOrder::RightToLeftTopToBottom);
    }

    #[test]
    fn test_nup_spec_portrait_orientation() {
        let spec = NupSpec::from_str("n=4,orientation=portrait").unwrap();
        assert!((spec.paper_width - 612.0).abs() < f32::EPSILON);
        assert!((spec.paper_height - 792.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_nup_spec_landscape_orientation() {
        let spec = NupSpec::from_str("n=4,orientation=landscape").unwrap();
        assert!((spec.paper_width - 792.0).abs() < f32::EPSILON);
        assert!((spec.paper_height - 612.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_nup_spec_custom_paper() {
        // Pin the orientation so this tests unit conversion ONLY. Without it,
        // n=2's landscape auto-orientation swaps the axes and the test silently
        // becomes a test of two things at once.
        let spec =
            NupSpec::from_str("n=2,paper_w=11,paper_h=17,units=in,orientation=portrait").unwrap();
        assert!((spec.paper_width - 792.0).abs() < f32::EPSILON);
        assert!((spec.paper_height - 1224.0).abs() < f32::EPSILON);
    }

    /// `n=2` means two pages SIDE BY SIDE on a landscape sheet — the print-dialog
    /// convention, and what `--booklet` already does. Ruled 2026-09-09 (bug-0008).
    #[test]
    fn test_nup_n2_is_side_by_side_landscape() {
        let spec = NupSpec::from_str("n=2").unwrap();
        assert_eq!((spec.cols, spec.rows), (2, 1));
        assert!(
            (spec.paper_width - 792.0).abs() < f32::EPSILON,
            "letter landscape"
        );
        assert!((spec.paper_height - 612.0).abs() < f32::EPSILON);
    }

    /// The documented escape hatch: the pre-2026-09-09 stacked-portrait layout is
    /// still reachable, just spelled explicitly. If this breaks, `--help`'s advice
    /// for anyone who wanted the old `n=2` is wrong.
    #[test]
    fn test_nup_stacked_portrait_still_available_explicitly() {
        let spec = NupSpec::from_str("cols=1,rows=2").unwrap();
        assert_eq!((spec.cols, spec.rows), (1, 2));
        assert!(
            (spec.paper_width - 612.0).abs() < f32::EPSILON,
            "letter portrait"
        );
        assert!((spec.paper_height - 792.0).abs() < f32::EPSILON);
    }

    /// A non-canonical `n` is a usage error naming the accepted set and the way to
    /// get any other grid — it must never silently round up to a filled grid
    /// (bug-0008: n=3 used to yield 4-up, n=5 6-up, n=7 9-up).
    #[test]
    fn test_nup_non_canonical_n_is_rejected() {
        for n in [3u32, 5, 7, 10, 12] {
            let err =
                NupSpec::from_str(&format!("n={n}")).expect_err("non-canonical n must be rejected");
            assert!(
                err.contains(&n.to_string()),
                "error should name the bad n: {err}"
            );
            assert!(
                err.contains("cols="),
                "error should point at cols=/rows=: {err}"
            );
        }
        // ...and every canonical value still parses.
        for n in CANONICAL_N {
            assert!(
                NupSpec::from_str(&format!("n={n}")).is_ok(),
                "canonical n={n} must parse"
            );
        }
    }

    #[test]
    fn test_nup_spec_missing_n_and_grid() {
        assert!(NupSpec::from_str("paper=letter").is_err());
    }

    #[test]
    fn test_nup_spec_n_and_cols_conflict() {
        assert!(NupSpec::from_str("n=4,cols=2").is_err());
    }

    #[test]
    fn test_nup_spec_cols_without_rows() {
        assert!(NupSpec::from_str("cols=2").is_err());
    }

    #[test]
    fn test_nup_spec_zero_n() {
        assert!(NupSpec::from_str("n=0").is_err());
    }

    #[test]
    fn test_nup_spec_unknown_key() {
        assert!(NupSpec::from_str("n=4,bogus=val").is_err());
    }

    #[test]
    fn test_nup_spec_paper_and_paper_w_conflict() {
        assert!(NupSpec::from_str("n=4,paper=letter,paper_w=100").is_err());
    }

    #[test]
    fn test_nup_spec_paper_w_without_paper_h() {
        assert!(NupSpec::from_str("n=4,paper_w=100").is_err());
    }

    #[test]
    fn test_nup_spec_repeat_default() {
        assert_eq!(NupSpec::from_str("n=4").unwrap().repeat, 1);
    }

    #[test]
    fn test_nup_spec_repeat_auto() {
        assert_eq!(NupSpec::from_str("n=4,repeat=auto").unwrap().repeat, 4);
    }

    #[test]
    fn test_nup_spec_repeat_auto_3x2() {
        assert_eq!(
            NupSpec::from_str("cols=3,rows=2,repeat=auto")
                .unwrap()
                .repeat,
            6
        );
    }

    #[test]
    fn test_nup_spec_repeat_explicit() {
        assert_eq!(NupSpec::from_str("n=4,repeat=3").unwrap().repeat, 3);
    }

    #[test]
    fn test_nup_spec_repeat_zero_error() {
        assert!(NupSpec::from_str("n=4,repeat=0").is_err());
    }

    #[test]
    fn test_nup_spec_invalid_orientation() {
        let result = NupSpec::from_str("n=4,orientation=bogus");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("orientation"));
    }

    // --- BookletSpec ---

    #[test]
    fn test_booklet_spec_defaults() {
        let spec = BookletSpec::from_str("").unwrap();
        assert!((spec.paper_width - 792.0).abs() < f32::EPSILON);
        assert!((spec.paper_height - 612.0).abs() < f32::EPSILON);
        assert!((spec.binding_margin - 0.0).abs() < f32::EPSILON);
        assert_eq!(spec.flip, DuplexFlip::None);
    }

    #[test]
    fn test_booklet_spec_with_paper() {
        let spec = BookletSpec::from_str("paper=a4").unwrap();
        assert!((spec.paper_width - 841.89).abs() < 0.01);
        assert!((spec.paper_height - 595.28).abs() < 0.01);
    }

    #[test]
    fn test_booklet_spec_with_options() {
        let spec = BookletSpec::from_str("binding_margin=0.25,units=in,flip=short_edge").unwrap();
        assert!((spec.binding_margin - 18.0).abs() < f32::EPSILON);
        assert_eq!(spec.flip, DuplexFlip::ShortEdge);
    }

    #[test]
    fn test_booklet_spec_custom_paper() {
        let spec = BookletSpec::from_str("paper_w=17,paper_h=11,units=in").unwrap();
        assert!((spec.paper_width - 1224.0).abs() < f32::EPSILON);
        assert!((spec.paper_height - 792.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_booklet_spec_flip_long_edge() {
        let spec = BookletSpec::from_str("flip=long_edge").unwrap();
        assert_eq!(spec.flip, DuplexFlip::LongEdge);
    }

    #[test]
    fn test_booklet_spec_invalid_flip() {
        assert!(BookletSpec::from_str("flip=invalid").is_err());
    }

    #[test]
    fn test_booklet_spec_unknown_key() {
        assert!(BookletSpec::from_str("bogus=val").is_err());
    }

    #[test]
    fn test_booklet_spec_paper_and_paper_w_conflict() {
        assert!(BookletSpec::from_str("paper=letter,paper_w=100").is_err());
    }

    #[test]
    fn test_booklet_spec_back_default() {
        assert_eq!(BookletSpec::from_str("").unwrap().back, 0);
    }

    #[test]
    fn test_booklet_spec_with_back() {
        assert_eq!(BookletSpec::from_str("back=3").unwrap().back, 3);
    }

    #[test]
    fn test_booklet_spec_back_invalid() {
        assert!(BookletSpec::from_str("back=abc").is_err());
    }

    #[test]
    fn test_booklet_spec_back_with_other_options() {
        let spec = BookletSpec::from_str("back=2,flip=short_edge").unwrap();
        assert_eq!(spec.back, 2);
        assert_eq!(spec.flip, DuplexFlip::ShortEdge);
    }

    #[test]
    fn test_booklet_spec_paper_w_without_paper_h() {
        assert!(BookletSpec::from_str("paper_w=100").is_err());
    }
}

#[cfg(test)]
mod range_validation_tests {
    use super::*;
    use crate::imposition::CellGeometry;
    use crate::spec_types::{BlankPageSpec, DrawLineSpec, DrawRectSpec, WatermarkSpec};

    /// Extents are rejected at or below zero (bug-0009).
    #[test]
    fn extents_must_be_positive() {
        assert!(BlankPageSpec::from_str("w=0,h=792").is_err());
        assert!(BlankPageSpec::from_str("w=612,h=-1").is_err());
        assert!(NupSpec::from_str("n=4,paper_w=0,paper_h=792").is_err());
        assert!(BookletSpec::from_str("paper_w=-612,paper_h=792").is_err());
        assert!(DrawRectSpec::from_str("x=0,y=0,w=0,h=10").is_err());
        assert!(DrawLineSpec::from_str("x1=0,y1=0,x2=1,y2=1,width=0").is_err());
        assert!(WatermarkSpec::from_str("text=X,font=@Helvetica,x=0,y=0,size=0").is_err());
    }

    /// Alpha outside [0,1] is rejected rather than silently clamped.
    ///
    /// The clamp was the fault: `alpha=5` clamped to 1.0 and emitted no ExtGState,
    /// and `alpha=-0.5` clamped to 0.0 — a fully INVISIBLE mark at exit 0.
    #[test]
    fn alpha_must_be_a_fraction() {
        for bad in ["5", "-0.5", "50", "1.0001"] {
            assert!(
                DrawRectSpec::from_str(&format!("x=0,y=0,w=1,h=1,alpha={bad}")).is_err(),
                "alpha={bad} must be rejected"
            );
        }
        // The endpoints are legal: an explicit alpha=0 is stated intent, not a typo.
        for ok in ["0", "0.0", "1", "1.0", "0.5"] {
            assert!(
                DrawRectSpec::from_str(&format!("x=0,y=0,w=1,h=1,alpha={ok}")).is_ok(),
                "alpha={ok} must be accepted"
            );
        }
    }

    /// The other half of the ruling, and the one most at risk of being "tidied up"
    /// by someone applying the extent rule uniformly: **offsets may be negative.**
    ///
    /// A negative margin is a full bleed — a real layout that already worked — and
    /// it can never produce bug-0006's fault, because it makes the cell LARGER.
    /// If this test starts failing, the fix is to restore the behavior, not to
    /// update the test.
    #[test]
    fn offsets_may_be_negative() {
        let spec = NupSpec::from_str("n=4,margin=-0.25,units=in").expect("bleed is legal");
        assert!(spec.margin < 0.0, "negative margin must survive parsing");

        assert!(
            NupSpec::from_str("n=4,gutter=-10,units=pt").is_ok(),
            "a negative gutter overlaps cells, which is a layout, not an error"
        );
        assert!(
            BookletSpec::from_str("binding_margin=-0.5,units=in").is_ok(),
            "a negative binding margin overlaps at the spine"
        );
        assert!(
            DrawRectSpec::from_str("x=-10,y=-10,w=100,h=100").is_ok(),
            "negative draw coordinates place content off the page edge deliberately"
        );
    }

    /// Non-positive derived cells are caught on the DERIVED quantity, which is what
    /// lets the sign checks above stay off the offsets (bug-0006).
    #[test]
    fn degenerate_cells_are_rejected_with_the_arithmetic() {
        let err = CellGeometry::compute(612.0, 792.0, 2, 2, 360.0, 0.0)
            .expect_err("360pt margins leave no room on letter");
        let msg = err.to_string();
        for expected in ["360", "612", "792", "2x2"] {
            assert!(
                msg.contains(expected),
                "error should name {expected}: {msg}"
            );
        }

        // A large gutter fails the same way, without enumerating it at parse time.
        assert!(CellGeometry::compute(612.0, 792.0, 4, 1, 0.0, 300.0).is_err());

        // And the bleed case stays legal, with a LARGER cell than the plain split.
        let bleed = CellGeometry::compute(612.0, 792.0, 2, 2, -18.0, 0.0).expect("bleed is legal");
        let plain = CellGeometry::compute(612.0, 792.0, 2, 2, 0.0, 0.0).unwrap();
        assert!(bleed.cell_w > plain.cell_w);
        assert!((CellGeometry::overhang(-18.0) - 18.0).abs() < 1e-9);
        assert!(
            (CellGeometry::overhang(18.0)).abs() < 1e-9,
            "no overhang when inset"
        );
    }
}
