// Smaller spec types: overlay, padding, blank pages.

use std::path::PathBuf;
use std::str::FromStr;

use medpdf::Unit;

use super::parse::{KvParser, parse_paper_size};

#[derive(Debug, Clone)]
pub struct OverlaySpec {
    pub file: PathBuf,
    pub src_page: u32,
    pub target_pages: String,
}

impl FromStr for OverlaySpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kv = KvParser::parse(s, "overlay", &["file", "src_page", "target_pages"])?;
        Ok(OverlaySpec {
            file: PathBuf::from(kv.required_str("file")?),
            src_page: kv.required_parse::<u32>("src_page")?,
            target_pages: kv
                .get("target_pages")
                .map(str::to_string)
                .unwrap_or_else(|| "all".to_string()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct PadToSpec {
    pub pages: u32,
}

impl FromStr for PadToSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pages = s.parse::<u32>().map_err(|e| e.to_string())?;
        if pages == 0 {
            return Err("pad-to value must be greater than 0".to_string());
        }
        Ok(PadToSpec { pages })
    }
}

#[derive(Debug, Clone)]
pub struct PadFileSpec {
    pub file: PathBuf,
    pub page: u32,
}

impl FromStr for PadFileSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kv = KvParser::parse(s, "pad-file", &["file", "page"])?;
        Ok(PadFileSpec {
            file: PathBuf::from(kv.required_str("file")?),
            page: kv.optional_parse::<u32>("page")?.unwrap_or(1),
        })
    }
}

#[derive(Debug, Clone)]
pub struct BlankPageSpec {
    pub width: f32,
    pub height: f32,
    pub count: u32,
}

impl FromStr for BlankPageSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        if !trimmed.contains('=') {
            let (w, h) = parse_paper_size(trimmed).map_err(|_| {
                format!(
                    "Unknown page size: '{}'. Use letter, a4, legal, or w=...,h=...",
                    trimmed
                )
            })?;
            return Ok(BlankPageSpec {
                width: w,
                height: h,
                count: 1,
            });
        }

        let kv = KvParser::parse(trimmed, "blank-page", &["w", "h", "units", "count"])?;
        let unit: Unit = kv.optional_units()?.map(Unit::from).unwrap_or(Unit::Pt);
        let w_raw = kv.required_parse::<f32>("w")?;
        let h_raw = kv.required_parse::<f32>("h")?;
        let count = kv.optional_parse::<u32>("count")?.unwrap_or(1);
        if count == 0 {
            return Err("blank-page 'count' must be greater than 0".to_string());
        }
        Ok(BlankPageSpec {
            width: unit.to_points(w_raw),
            height: unit.to_points(h_raw),
            count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- OverlaySpec ---

    #[test]
    fn test_overlay_spec_full() {
        let spec = OverlaySpec::from_str("file=overlay.pdf,src_page=2,target_pages=1-5").unwrap();
        assert_eq!(spec.file, PathBuf::from("overlay.pdf"));
        assert_eq!(spec.src_page, 2);
        assert_eq!(spec.target_pages, "1-5");
    }

    #[test]
    fn test_overlay_spec_defaults() {
        let spec = OverlaySpec::from_str("file=overlay.pdf,src_page=1").unwrap();
        assert_eq!(spec.target_pages, "all");
    }

    #[test]
    fn test_overlay_spec_missing_file() {
        let result = OverlaySpec::from_str("src_page=1,target_pages=all");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("file"));
    }

    #[test]
    fn test_overlay_spec_missing_src_page() {
        let result = OverlaySpec::from_str("file=overlay.pdf,target_pages=all");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("src_page"));
    }

    #[test]
    fn test_overlay_spec_unknown_key() {
        let result = OverlaySpec::from_str("file=overlay.pdf,src_page=1,bogus=val");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bogus"));
    }

    #[test]
    fn test_overlay_spec_invalid_src_page_value() {
        assert!(OverlaySpec::from_str("file=f.pdf,src_page=abc").is_err());
    }

    #[test]
    fn test_overlay_spec_empty_string() {
        // Empty string yields a single empty part with no '='.
        assert!(OverlaySpec::from_str("").is_err());
    }

    // --- PadToSpec ---

    #[test]
    fn test_pad_to_spec_valid() {
        let spec = PadToSpec::from_str("4").unwrap();
        assert_eq!(spec.pages, 4);
    }

    #[test]
    fn test_pad_to_spec_invalid() {
        assert!(PadToSpec::from_str("abc").is_err());
        assert!(PadToSpec::from_str("-1").is_err());
    }

    #[test]
    fn test_pad_to_spec_zero() {
        assert!(PadToSpec::from_str("0").is_err());
    }

    #[test]
    fn test_pad_to_spec_large_value() {
        assert_eq!(PadToSpec::from_str("1000").unwrap().pages, 1000);
    }

    #[test]
    fn test_pad_to_spec_one() {
        assert_eq!(PadToSpec::from_str("1").unwrap().pages, 1);
    }

    #[test]
    fn test_pad_to_spec_float_fails() {
        assert!(PadToSpec::from_str("1.5").is_err());
    }

    #[test]
    fn test_pad_to_spec_empty_fails() {
        assert!(PadToSpec::from_str("").is_err());
    }

    // --- PadFileSpec ---

    #[test]
    fn test_pad_file_spec_full() {
        let spec = PadFileSpec::from_str("file=blank.pdf,page=3").unwrap();
        assert_eq!(spec.file, PathBuf::from("blank.pdf"));
        assert_eq!(spec.page, 3);
    }

    #[test]
    fn test_pad_file_spec_default_page() {
        let spec = PadFileSpec::from_str("file=blank.pdf").unwrap();
        assert_eq!(spec.page, 1);
    }

    #[test]
    fn test_pad_file_spec_missing_file() {
        assert!(PadFileSpec::from_str("page=1").is_err());
    }

    #[test]
    fn test_pad_file_spec_invalid_kv() {
        assert!(PadFileSpec::from_str("noequalssign").is_err());
    }

    #[test]
    fn test_pad_file_spec_unknown_key() {
        let result = PadFileSpec::from_str("file=f.pdf,bogus=val");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bogus"));
    }

    #[test]
    fn test_pad_file_spec_invalid_page() {
        assert!(PadFileSpec::from_str("file=f.pdf,page=abc").is_err());
    }

    // --- BlankPageSpec ---

    #[test]
    fn test_blank_page_spec_letter() {
        let spec = BlankPageSpec::from_str("letter").unwrap();
        assert!((spec.width - 612.0).abs() < f32::EPSILON);
        assert!((spec.height - 792.0).abs() < f32::EPSILON);
        assert_eq!(spec.count, 1);
    }

    #[test]
    fn test_blank_page_spec_a4() {
        let spec = BlankPageSpec::from_str("a4").unwrap();
        assert!((spec.width - 595.28).abs() < 0.01);
        assert!((spec.height - 841.89).abs() < 0.01);
    }

    #[test]
    fn test_blank_page_spec_legal() {
        let spec = BlankPageSpec::from_str("legal").unwrap();
        assert!((spec.width - 612.0).abs() < f32::EPSILON);
        assert!((spec.height - 1008.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_blank_page_spec_case_insensitive() {
        let spec = BlankPageSpec::from_str("LETTER").unwrap();
        assert!((spec.width - 612.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_blank_page_spec_explicit_dims() {
        let spec = BlankPageSpec::from_str("w=612,h=792").unwrap();
        assert!((spec.width - 612.0).abs() < f32::EPSILON);
        assert!((spec.height - 792.0).abs() < f32::EPSILON);
        assert_eq!(spec.count, 1);
    }

    #[test]
    fn test_blank_page_spec_with_units() {
        let spec = BlankPageSpec::from_str("w=8.5,h=11,units=in").unwrap();
        assert!((spec.width - 612.0).abs() < f32::EPSILON);
        assert!((spec.height - 792.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_blank_page_spec_with_count() {
        let spec = BlankPageSpec::from_str("w=100,h=100,count=3").unwrap();
        assert_eq!(spec.count, 3);
    }

    #[test]
    fn test_blank_page_spec_missing_w() {
        let result = BlankPageSpec::from_str("h=792");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("w"));
    }

    #[test]
    fn test_blank_page_spec_missing_h() {
        let result = BlankPageSpec::from_str("w=612");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("h"));
    }

    #[test]
    fn test_blank_page_spec_unknown_name() {
        let result = BlankPageSpec::from_str("tabloid");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("tabloid"));
    }

    #[test]
    fn test_blank_page_spec_unknown_key() {
        let result = BlankPageSpec::from_str("w=100,h=100,bogus=val");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bogus"));
    }

    #[test]
    fn test_blank_page_spec_count_zero() {
        let result = BlankPageSpec::from_str("w=100,h=100,count=0");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("count"));
    }

    #[test]
    fn test_blank_page_spec_invalid_w() {
        assert!(BlankPageSpec::from_str("w=abc,h=100").is_err());
    }
}
