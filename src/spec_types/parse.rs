// Shared parsing helpers and the KvParser used by every spec type's FromStr impl.

use clap::ValueEnum;
use medpdf::{FontStyle, FontWeight, PdfColor, Unit};

/// CLI wrapper for Unit that implements ValueEnum for clap.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CliUnit {
    Pt,
    In,
    Mm,
    Cm,
}

impl From<CliUnit> for Unit {
    fn from(u: CliUnit) -> Unit {
        match u {
            CliUnit::Pt => Unit::Pt,
            CliUnit::In => Unit::In,
            CliUnit::Mm => Unit::Mm,
            CliUnit::Cm => Unit::Cm,
        }
    }
}

/// Splits a string by commas, treating `\,` as an escaped literal comma.
/// Commas inside double-quoted regions are treated as literal (not delimiters).
/// Quote characters are preserved in the output so callers can detect and strip them.
pub(super) fn split_escaped_commas(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = s.chars().peekable();
    let mut in_quotes = false;
    while let Some(c) = chars.next() {
        if c == '"' {
            in_quotes = !in_quotes;
            current.push(c);
        } else if c == '\\' && !in_quotes {
            if let Some(&',') = chars.peek() {
                current.push(',');
                chars.next();
                continue;
            }
            current.push(c);
        } else if c == ',' && !in_quotes {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    parts.push(current);
    parts
}

/// Strips matching surrounding double quotes, if present.
pub(super) fn strip_quotes(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(s)
}

/// Processes escape sequences in text values:
/// - `\uXXXX` → Unicode character (4 hex digits, BMP)
/// - `\U{XXXXX}` → Unicode character (1-6 hex digits, full range)
/// - `\n` → newline, `\t` → tab
/// - `\\` → literal backslash
/// - Any other `\X` → left as-is (backward compatible)
pub(super) fn unescape_text(s: &str) -> Result<String, String> {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        match chars.peek() {
            Some(&'n') => {
                chars.next();
                result.push('\n');
            }
            Some(&'t') => {
                chars.next();
                result.push('\t');
            }
            Some(&'\\') => {
                chars.next();
                result.push('\\');
            }
            Some(&'u') => {
                chars.next();
                let mut hex = String::with_capacity(4);
                for _ in 0..4 {
                    match chars.next() {
                        Some(h) if h.is_ascii_hexdigit() => hex.push(h),
                        Some(h) => {
                            return Err(format!(
                                "Invalid Unicode escape: \\u{hex}{h} (expected 4 hex digits)"
                            ));
                        }
                        None => {
                            return Err(format!(
                                "Incomplete Unicode escape: \\u{hex} (expected 4 hex digits)"
                            ));
                        }
                    }
                }
                let code = u32::from_str_radix(&hex, 16)
                    .map_err(|_| format!("Invalid hex in Unicode escape: \\u{hex}"))?;
                let ch = char::from_u32(code)
                    .ok_or_else(|| format!("Invalid Unicode code point: \\u{hex}"))?;
                result.push(ch);
            }
            Some(&'U') => {
                chars.next();
                match chars.next() {
                    Some('{') => {}
                    _ => {
                        result.push('\\');
                        result.push('U');
                        continue;
                    }
                }
                let mut hex = String::with_capacity(6);
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(h) if h.is_ascii_hexdigit() && hex.len() < 6 => hex.push(h),
                        Some(h) if h.is_ascii_hexdigit() => {
                            return Err(format!(
                                "Unicode escape too long: \\U{{{hex}{h}... (max 6 hex digits)"
                            ));
                        }
                        Some(h) => {
                            return Err(format!(
                                "Invalid character in Unicode escape: \\U{{{hex}{h}"
                            ));
                        }
                        None => return Err(format!("Unclosed Unicode escape: \\U{{{hex}")),
                    }
                }
                if hex.is_empty() {
                    return Err("Empty Unicode escape: \\U{}".to_string());
                }
                let code = u32::from_str_radix(&hex, 16)
                    .map_err(|_| format!("Invalid hex in Unicode escape: \\U{{{hex}}}"))?;
                let ch = char::from_u32(code)
                    .ok_or_else(|| format!("Invalid Unicode code point: \\U{{{hex}}}"))?;
                result.push(ch);
            }
            _ => {
                result.push('\\');
            }
        }
    }
    Ok(result)
}

/// Parses a color string into a `PdfColor`.
///
/// Supports named colors (`black`, `white`, `red`, `blue`, `green`, `yellow`, `cyan`,
/// `magenta`, `orange`, `purple`, `gray`/`grey`) and hex formats (`#RGB`, `#RRGGBB`,
/// `#RRGGBBAA`) with or without the `#` prefix.
pub(super) fn parse_color(s: &str) -> Result<PdfColor, String> {
    match s.to_lowercase().as_str() {
        "black" => return Ok(PdfColor::BLACK),
        "white" => return Ok(PdfColor::WHITE),
        "red" => return Ok(PdfColor::RED),
        "blue" => return Ok(PdfColor::rgb(0.0, 0.0, 1.0)),
        "green" => return Ok(PdfColor::rgb(0.0, 0.5, 0.0)),
        "yellow" => return Ok(PdfColor::rgb(1.0, 1.0, 0.0)),
        "cyan" => return Ok(PdfColor::rgb(0.0, 1.0, 1.0)),
        "magenta" => return Ok(PdfColor::rgb(1.0, 0.0, 1.0)),
        "orange" => return Ok(PdfColor::rgb(1.0, 0.5, 0.0)),
        "purple" => return Ok(PdfColor::rgb(0.5, 0.0, 0.5)),
        "gray" | "grey" => return Ok(PdfColor::rgb(0.5, 0.5, 0.5)),
        _ => {}
    }

    let hex = s.strip_prefix('#').unwrap_or(s);
    let parse_hex =
        |h: &str| u8::from_str_radix(h, 16).map_err(|_| format!("Invalid hex color: '{s}'"));

    match hex.len() {
        3 => {
            let r = parse_hex(&hex[0..1])? * 17;
            let g = parse_hex(&hex[1..2])? * 17;
            let b = parse_hex(&hex[2..3])? * 17;
            Ok(PdfColor::from_rgb8(r, g, b))
        }
        6 => {
            let r = parse_hex(&hex[0..2])?;
            let g = parse_hex(&hex[2..4])?;
            let b = parse_hex(&hex[4..6])?;
            Ok(PdfColor::from_rgb8(r, g, b))
        }
        8 => {
            let r = parse_hex(&hex[0..2])?;
            let g = parse_hex(&hex[2..4])?;
            let b = parse_hex(&hex[4..6])?;
            let a = parse_hex(&hex[6..8])?;
            Ok(PdfColor::rgba(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ))
        }
        _ => Err(format!(
            "Invalid color value: '{s}'. Use a named color (black, white, red, blue, green, \
             yellow, cyan, magenta, orange, purple, gray) or hex (#RGB, #RRGGBB, #RRGGBBAA)."
        )),
    }
}

pub(super) fn parse_font_weight(s: &str) -> Result<FontWeight, String> {
    match s.to_lowercase().as_str() {
        "thin" => Ok(FontWeight::THIN),
        "extra_light" | "extralight" => Ok(FontWeight::EXTRA_LIGHT),
        "light" => Ok(FontWeight::LIGHT),
        "normal" | "regular" => Ok(FontWeight::NORMAL),
        "medium" => Ok(FontWeight::MEDIUM),
        "semi_bold" | "semibold" => Ok(FontWeight::SEMI_BOLD),
        "bold" => Ok(FontWeight::BOLD),
        "extra_bold" | "extrabold" => Ok(FontWeight::EXTRA_BOLD),
        "black" => Ok(FontWeight::BLACK),
        _ => {
            let n: u16 = s.parse().map_err(|_| {
                format!("Invalid weight value: '{s}'. Use a name (thin, light, normal, medium, semibold, bold, extrabold, black) or a number (100-900).")
            })?;
            if !(1..=1000).contains(&n) {
                return Err(format!("Weight {n} out of range. Use 1-1000."));
            }
            Ok(FontWeight(n))
        }
    }
}

pub(super) fn parse_font_style(s: &str) -> Result<FontStyle, String> {
    match s.to_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique(14.0)),
        _ => Err(format!(
            "Invalid style value: '{s}'. Use normal, italic, or oblique."
        )),
    }
}

pub(super) fn parse_paper_size(name: &str) -> Result<(f32, f32), String> {
    match name.to_lowercase().as_str() {
        "letter" => Ok((612.0, 792.0)),
        "a4" => Ok((595.28, 841.89)),
        "legal" => Ok((612.0, 1008.0)),
        _ => Err(format!(
            "Unknown paper size: '{}'. Use letter, a4, or legal.",
            name
        )),
    }
}

/// Parses a comma-separated `key=value` spec string with shared error handling.
///
/// Handles iteration via `split_escaped_commas`, key/value extraction and trimming,
/// rejection of unknown keys, and duplicate-key detection. Callers use the typed
/// accessors to extract individual fields with appropriate parsing.
#[derive(Debug)]
pub(super) struct KvParser {
    pairs: Vec<(String, String)>,
    type_label: &'static str,
}

impl KvParser {
    pub(super) fn parse(
        s: &str,
        type_label: &'static str,
        allowed_keys: &[&str],
    ) -> Result<Self, String> {
        let mut pairs: Vec<(String, String)> = Vec::new();
        for part in split_escaped_commas(s) {
            let (k, v) = part.split_once('=').ok_or_else(|| {
                format!("Invalid key-value pair: '{part}'. Expected 'key=value'.")
            })?;
            let k = k.trim().to_string();
            let v = v.trim().to_string();
            if !allowed_keys.contains(&k.as_str()) {
                return Err(format!("Unknown {type_label} key: '{k}'"));
            }
            if pairs.iter().any(|(existing, _)| existing == &k) {
                return Err(format!("Duplicate {type_label} key: '{k}'"));
            }
            pairs.push((k, v));
        }
        Ok(Self { pairs, type_label })
    }

    pub(super) fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub(super) fn required_str(&self, key: &str) -> Result<&str, String> {
        self.get(key)
            .ok_or_else(|| format!("{} '{}' is required", self.type_label, key))
    }

    pub(super) fn required_parse<T>(&self, key: &str) -> Result<T, String>
    where
        T: std::str::FromStr,
        <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        let v = self.required_str(key)?;
        v.parse::<T>()
            .map_err(|_| format!("Invalid {key} value: '{v}'"))
    }

    pub(super) fn optional_parse<T>(&self, key: &str) -> Result<Option<T>, String>
    where
        T: std::str::FromStr,
        <T as std::str::FromStr>::Err: std::fmt::Display,
    {
        match self.get(key) {
            None => Ok(None),
            Some(v) => v
                .parse::<T>()
                .map(Some)
                .map_err(|_| format!("Invalid {key} value: '{v}'")),
        }
    }

    /// Apply a custom parser to an optional field (e.g. parse_color, parse_font_weight).
    pub(super) fn optional_with<T, F>(&self, key: &str, parser: F) -> Result<Option<T>, String>
    where
        F: FnOnce(&str) -> Result<T, String>,
    {
        self.get(key).map(parser).transpose()
    }

    /// Read units value if present.
    pub(super) fn optional_units(&self) -> Result<Option<CliUnit>, String> {
        match self.get("units") {
            None => Ok(None),
            Some(v) => CliUnit::from_str(v, true)
                .map(Some)
                .map_err(|e| e.to_string()),
        }
    }

    /// Common `layer=over|under` parser; returns Some(true) for over, Some(false) for under.
    pub(super) fn optional_layer(&self) -> Result<Option<bool>, String> {
        match self.get("layer") {
            None => Ok(None),
            Some("over") => Ok(Some(true)),
            Some("under") => Ok(Some(false)),
            Some(v) => Err(format!("Invalid layer value: '{v}'. Use over or under.")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_escaped_commas_no_escapes() {
        let parts = split_escaped_commas("a=1,b=2,c=3");
        assert_eq!(parts, vec!["a=1", "b=2", "c=3"]);
    }

    #[test]
    fn test_split_escaped_commas_single_part() {
        let parts = split_escaped_commas("text=hello");
        assert_eq!(parts, vec!["text=hello"]);
    }

    #[test]
    fn test_split_escaped_commas_empty_string() {
        let parts = split_escaped_commas("");
        assert_eq!(parts, vec![""]);
    }

    #[test]
    fn test_split_escaped_commas_multiple_escapes() {
        let parts = split_escaped_commas(r"a\,b\,c,d");
        assert_eq!(parts, vec!["a,b,c", "d"]);
    }

    #[test]
    fn test_split_escaped_commas_trailing_backslash() {
        let parts = split_escaped_commas(r"text=hello\");
        assert_eq!(parts, vec!["text=hello\\"]);
    }

    #[test]
    fn test_split_escaped_commas_backslash_not_before_comma() {
        let parts = split_escaped_commas(r"path=C:\Users\test,key=val");
        assert_eq!(parts, vec![r"path=C:\Users\test", "key=val"]);
    }

    #[test]
    fn test_split_escaped_commas_consecutive_commas() {
        let parts = split_escaped_commas("a,,b");
        assert_eq!(parts, vec!["a", "", "b"]);
    }

    #[test]
    fn test_split_escaped_commas_quoted() {
        let parts = split_escaped_commas(r#"text="Hello, World",x=1"#);
        assert_eq!(parts, vec![r#"text="Hello, World""#, "x=1"]);
    }

    #[test]
    fn test_split_escaped_commas_mixed_quoted() {
        let parts = split_escaped_commas(r#"text="a,b",font=@H,x=1,y=1"#);
        assert_eq!(parts, vec![r#"text="a,b""#, "font=@H", "x=1", "y=1"]);
    }

    #[test]
    fn test_split_escaped_commas_unclosed_quote() {
        let parts = split_escaped_commas(r#"text="hello,x=1"#);
        assert_eq!(parts, vec![r#"text="hello,x=1"#]);
    }

    #[test]
    fn test_parse_color_gray_alias() {
        let c = parse_color("gray").unwrap();
        assert!((c.r - 0.5).abs() < f32::EPSILON);
        assert!((c.g - 0.5).abs() < f32::EPSILON);
        assert!((c.b - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_color_grey_alias() {
        let c = parse_color("grey").unwrap();
        assert!((c.r - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_color_case_insensitive() {
        let c1 = parse_color("RED").unwrap();
        let c2 = parse_color("Red").unwrap();
        let c3 = parse_color("red").unwrap();
        assert_eq!(c1, c2);
        assert_eq!(c2, c3);
    }

    #[test]
    fn test_parse_color_hex_without_hash() {
        let c = parse_color("FF0000").unwrap();
        assert!((c.r - 1.0).abs() < f32::EPSILON);
        assert!((c.g - 0.0).abs() < f32::EPSILON);
        assert!((c.b - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_color_rrggbbaa() {
        let c = parse_color("#FF000080").unwrap();
        assert!((c.r - 1.0).abs() < f32::EPSILON);
        assert!((c.a - 128.0 / 255.0).abs() < 0.01);
    }

    #[test]
    fn test_parse_color_invalid_hex() {
        let result = parse_color("#GG0000");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_color_invalid_length() {
        let result = parse_color("#12345");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid color value"));
    }

    #[test]
    fn test_parse_color_unknown_name() {
        // After expanding the named set in N4, "tangerine" remains unknown.
        let result = parse_color("tangerine");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_color_black() {
        let c = parse_color("black").unwrap();
        assert_eq!(c, PdfColor::BLACK);
    }

    #[test]
    fn test_parse_color_white() {
        let c = parse_color("white").unwrap();
        assert_eq!(c, PdfColor::WHITE);
    }

    #[test]
    fn test_parse_color_blue() {
        let c = parse_color("blue").unwrap();
        assert!((c.b - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_color_green() {
        let c = parse_color("green").unwrap();
        assert!((c.g - 0.5).abs() < f32::EPSILON);
        assert!((c.r - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_color_yellow() {
        let c = parse_color("yellow").unwrap();
        assert!((c.r - 1.0).abs() < f32::EPSILON);
        assert!((c.g - 1.0).abs() < f32::EPSILON);
        assert!((c.b - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_color_purple() {
        let c = parse_color("purple").unwrap();
        assert!((c.r - 0.5).abs() < f32::EPSILON);
        assert!((c.g - 0.0).abs() < f32::EPSILON);
        assert!((c.b - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_color_orange() {
        let c = parse_color("orange").unwrap();
        assert!((c.r - 1.0).abs() < f32::EPSILON);
        assert!((c.g - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_color_cyan_magenta() {
        let cy = parse_color("cyan").unwrap();
        assert!((cy.g - 1.0).abs() < f32::EPSILON);
        assert!((cy.b - 1.0).abs() < f32::EPSILON);
        let mg = parse_color("magenta").unwrap();
        assert!((mg.r - 1.0).abs() < f32::EPSILON);
        assert!((mg.b - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_unescape_text_ascii() {
        assert_eq!(unescape_text(r"A").unwrap(), "A");
    }

    #[test]
    fn test_unescape_text_em_dash() {
        assert_eq!(unescape_text(r"—").unwrap(), "\u{2014}");
    }

    #[test]
    fn test_unescape_text_accented() {
        assert_eq!(unescape_text(r"é").unwrap(), "é");
    }

    #[test]
    fn test_unescape_text_full_range() {
        assert_eq!(unescape_text(r"\U{1F600}").unwrap(), "\u{1F600}");
    }

    #[test]
    fn test_unescape_text_newline_tab() {
        assert_eq!(unescape_text(r"\n").unwrap(), "\n");
        assert_eq!(unescape_text(r"\t").unwrap(), "\t");
    }

    #[test]
    fn test_unescape_text_literal_backslash() {
        assert_eq!(unescape_text(r"\\").unwrap(), "\\");
    }

    #[test]
    fn test_unescape_text_unknown_escape_passthrough() {
        assert_eq!(unescape_text(r"\q").unwrap(), r"\q");
    }

    #[test]
    fn test_unescape_text_invalid_hex() {
        assert!(unescape_text(r"\u00GG").is_err());
    }

    #[test]
    fn test_unescape_text_incomplete() {
        assert!(unescape_text(r"\u00").is_err());
    }

    #[test]
    fn test_unescape_text_empty_u_braces() {
        assert!(unescape_text(r"\U{}").is_err());
    }

    #[test]
    fn test_unescape_text_mixed() {
        assert_eq!(
            unescape_text(r"Hello — World \U{1F600}").unwrap(),
            "Hello \u{2014} World \u{1F600}"
        );
    }

    #[test]
    fn test_parse_paper_size_letter() {
        let (w, h) = parse_paper_size("letter").unwrap();
        assert!((w - 612.0).abs() < f32::EPSILON);
        assert!((h - 792.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_paper_size_a4() {
        let (w, h) = parse_paper_size("A4").unwrap();
        assert!((w - 595.28).abs() < 0.01);
        assert!((h - 841.89).abs() < 0.01);
    }

    #[test]
    fn test_parse_paper_size_legal() {
        let (w, h) = parse_paper_size("Legal").unwrap();
        assert!((w - 612.0).abs() < f32::EPSILON);
        assert!((h - 1008.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_paper_size_unknown() {
        assert!(parse_paper_size("tabloid").is_err());
    }

    #[test]
    fn test_kv_parser_basic() {
        let kv = KvParser::parse("a=1,b=hello", "test", &["a", "b"]).unwrap();
        assert_eq!(kv.required_str("a").unwrap(), "1");
        assert_eq!(kv.get("b"), Some("hello"));
        assert_eq!(kv.get("missing"), None);
    }

    #[test]
    fn test_kv_parser_unknown_key() {
        let result = KvParser::parse("a=1,bogus=2", "test", &["a", "b"]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bogus"));
    }

    #[test]
    fn test_kv_parser_duplicate_key() {
        let result = KvParser::parse("a=1,a=2", "test", &["a"]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate"));
    }

    #[test]
    fn test_kv_parser_missing_equals() {
        let result = KvParser::parse("noequalssign", "test", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_kv_parser_required_missing() {
        let kv = KvParser::parse("a=1", "test", &["a", "b"]).unwrap();
        let err = kv.required_str("b").unwrap_err();
        assert!(err.contains("test"));
        assert!(err.contains("'b'"));
    }

    #[test]
    fn test_kv_parser_layer() {
        let kv = KvParser::parse("layer=over", "test", &["layer"]).unwrap();
        assert_eq!(kv.optional_layer().unwrap(), Some(true));
        let kv = KvParser::parse("layer=under", "test", &["layer"]).unwrap();
        assert_eq!(kv.optional_layer().unwrap(), Some(false));
        let kv = KvParser::parse("layer=middle", "test", &["layer"]).unwrap();
        assert!(kv.optional_layer().unwrap_err().contains("layer"));
    }

    #[test]
    fn test_kv_parser_trims_whitespace() {
        let kv = KvParser::parse("  a = 1 , b = hello  ", "test", &["a", "b"]).unwrap();
        assert_eq!(kv.get("a"), Some("1"));
        assert_eq!(kv.get("b"), Some("hello"));
    }
}
