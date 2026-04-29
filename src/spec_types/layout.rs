// Imposition specs (N-up, booklet) and shared layout helpers.

use std::str::FromStr;

use medpdf::Unit;

use super::parse::{parse_paper_size, KvParser};

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
        _ => Err(format!("Invalid orientation: '{v}'. Use auto, landscape, or portrait.")),
    }
}

fn parse_grid_order(v: &str) -> Result<GridOrder, String> {
    match v.to_lowercase().as_str() {
        "lrtb" => Ok(GridOrder::LeftToRightTopToBottom),
        "rltb" => Ok(GridOrder::RightToLeftTopToBottom),
        "tblr" => Ok(GridOrder::TopToBottomLeftToRight),
        "tbrl" => Ok(GridOrder::TopToBottomRightToLeft),
        _ => Err(format!("Invalid order: '{v}'. Use lrtb, rltb, tblr, or tbrl.")),
    }
}

fn parse_duplex_flip(v: &str) -> Result<DuplexFlip, String> {
    match v.to_lowercase().as_str() {
        "none" => Ok(DuplexFlip::None),
        "short_edge" => Ok(DuplexFlip::ShortEdge),
        "long_edge" => Ok(DuplexFlip::LongEdge),
        _ => Err(format!("Invalid flip value: '{v}'. Use none, short_edge, or long_edge.")),
    }
}

pub(super) fn auto_grid(n: u32) -> (u32, u32) {
    match n {
        2 => (1, 2),
        4 => (2, 2),
        6 => (2, 3),
        8 => (2, 4),
        9 => (3, 3),
        16 => (4, 4),
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

const NUP_KEYS: &[&str] = &[
    "n", "cols", "rows", "paper", "paper_w", "paper_h", "orientation", "margin", "gutter",
    "units", "order", "border", "repeat",
];

impl FromStr for NupSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kv = KvParser::parse(s, "nup", NUP_KEYS)?;

        let n = kv.optional_parse::<u32>("n")?;
        let cols_in = kv.optional_parse::<u32>("cols")?;
        let rows_in = kv.optional_parse::<u32>("rows")?;
        let paper = kv.get("paper").map(str::to_string);
        let paper_w = kv.optional_parse::<f32>("paper_w")?;
        let paper_h = kv.optional_parse::<f32>("paper_h")?;
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

const BOOKLET_KEYS: &[&str] = &[
    "paper", "paper_w", "paper_h", "binding_margin", "units", "flip", "back",
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
        let paper_w = kv.optional_parse::<f32>("paper_w")?;
        let paper_h = kv.optional_parse::<f32>("paper_h")?;
        let binding_margin = kv.optional_parse::<f32>("binding_margin")?;
        let unit: Unit = kv.optional_units()?.map(Unit::from).unwrap_or(Unit::In);
        let flip = kv.optional_with("flip", parse_duplex_flip)?;
        let back = kv.optional_parse::<u32>("back")?.unwrap_or(0);

        let (pw, ph) = resolve_paper_dims(&paper, paper_w, paper_h, unit, (792.0, 612.0))?;
        // For named paper sizes, ensure landscape orientation
        let (pw, ph) = if paper.is_some() && ph > pw { (ph, pw) } else { (pw, ph) };

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
        assert_eq!(auto_grid(2), (1, 2));
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
        let spec = NupSpec::from_str("n=4,margin=0.5,gutter=0.25,units=in,border=true,order=rltb").unwrap();
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
        let spec = NupSpec::from_str("n=2,paper_w=11,paper_h=17,units=in").unwrap();
        assert!((spec.paper_width - 792.0).abs() < f32::EPSILON);
        assert!((spec.paper_height - 1224.0).abs() < f32::EPSILON);
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
        assert_eq!(NupSpec::from_str("cols=3,rows=2,repeat=auto").unwrap().repeat, 6);
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
