// src/spec_types.rs
// CLI argument spec types moved from main.rs for testability

use clap::ValueEnum;
use medpdf_image::ImageFit;
use std::path::PathBuf;
use std::str::FromStr;
use medpdf::{FontStyle, FontWeight, HAlign, PdfColor, Unit, VAlign};

/// CLI wrapper for Unit that implements ValueEnum for clap
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

#[derive(Debug, Clone)]
pub struct WatermarkSpec {
    pub text: String,
    pub font: PathBuf,
    pub size: f32,
    pub x: f32,
    pub y: f32,
    pub units: Unit,
    pub pages: String,
    pub color: PdfColor,
    pub rotation: f32,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub strikeout: bool,
    pub underline: bool,
    pub layer_over: bool,
    pub weight: FontWeight,
    pub style: FontStyle,
}

/// Parses a color string into a `PdfColor`.
///
/// Supports named colors (`black`, `white`, `red`, `blue`, `green`, `gray`/`grey`)
/// and hex formats (`#RGB`, `#RRGGBB`, `#RRGGBBAA`) with or without the `#` prefix.
fn parse_color(s: &str) -> Result<PdfColor, String> {
    match s.to_lowercase().as_str() {
        "black" => return Ok(PdfColor::BLACK),
        "white" => return Ok(PdfColor::WHITE),
        "red" => return Ok(PdfColor::RED),
        "blue" => return Ok(PdfColor::rgb(0.0, 0.0, 1.0)),
        "green" => return Ok(PdfColor::rgb(0.0, 0.5, 0.0)),
        "gray" | "grey" => return Ok(PdfColor::rgb(0.5, 0.5, 0.5)),
        _ => {}
    }

    let hex = s.strip_prefix('#').unwrap_or(s);
    let parse_hex = |h: &str| u8::from_str_radix(h, 16).map_err(|_| format!("Invalid hex color: '{s}'"));

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
            Ok(PdfColor::rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a as f32 / 255.0))
        }
        _ => Err(format!("Invalid color value: '{s}'. Use a named color or hex (#RGB, #RRGGBB, #RRGGBBAA).")),
    }
}

/// Splits a string by commas, treating `\,` as an escaped literal comma.
/// Commas inside double-quoted regions are treated as literal (not delimiters).
/// Quote characters are preserved in the output so callers can detect and strip them.
fn split_escaped_commas(s: &str) -> Vec<String> {
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

/// Processes escape sequences in text values:
/// - `\uXXXX` → Unicode character (4 hex digits, BMP)
/// - `\U{XXXXX}` → Unicode character (1-6 hex digits, full range)
/// - `\n` → newline, `\t` → tab
/// - `\\` → literal backslash
/// - Any other `\X` → left as-is (backward compatible)
fn unescape_text(s: &str) -> Result<String, String> {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            result.push(c);
            continue;
        }
        match chars.peek() {
            Some(&'n') => { chars.next(); result.push('\n'); }
            Some(&'t') => { chars.next(); result.push('\t'); }
            Some(&'\\') => { chars.next(); result.push('\\'); }
            Some(&'u') => {
                chars.next(); // consume 'u'
                let mut hex = String::with_capacity(4);
                for _ in 0..4 {
                    match chars.next() {
                        Some(h) if h.is_ascii_hexdigit() => hex.push(h),
                        Some(h) => return Err(format!("Invalid Unicode escape: \\u{hex}{h} (expected 4 hex digits)")),
                        None => return Err(format!("Incomplete Unicode escape: \\u{hex} (expected 4 hex digits)")),
                    }
                }
                let code = u32::from_str_radix(&hex, 16)
                    .map_err(|_| format!("Invalid hex in Unicode escape: \\u{hex}"))?;
                let ch = char::from_u32(code)
                    .ok_or_else(|| format!("Invalid Unicode code point: \\u{hex}"))?;
                result.push(ch);
            }
            Some(&'U') => {
                chars.next(); // consume 'U'
                match chars.next() {
                    Some('{') => {}
                    _ => { result.push('\\'); result.push('U'); continue; }
                }
                let mut hex = String::with_capacity(6);
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(h) if h.is_ascii_hexdigit() && hex.len() < 6 => hex.push(h),
                        Some(h) if h.is_ascii_hexdigit() => return Err(format!("Unicode escape too long: \\U{{{hex}{h}... (max 6 hex digits)")),
                        Some(h) => return Err(format!("Invalid character in Unicode escape: \\U{{{hex}{h}")),
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
                // Unknown escape — preserve as-is
                result.push('\\');
            }
        }
    }
    Ok(result)
}

fn parse_font_weight(s: &str) -> Result<FontWeight, String> {
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

fn parse_font_style(s: &str) -> Result<FontStyle, String> {
    match s.to_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique(14.0)),
        _ => Err(format!("Invalid style value: '{s}'. Use normal, italic, or oblique.")),
    }
}

impl FromStr for WatermarkSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut text = None;
        let mut font = None;
        let mut size = None;
        let mut x = None;
        let mut y = None;
        let mut units = None;
        let mut pages = None;
        let mut color = None;
        let mut alpha = None;
        let mut rotation = None;
        let mut h_align = None;
        let mut v_align = None;
        let mut strikeout = None;
        let mut underline = None;
        let mut layer = None;
        let mut weight = None;
        let mut style = None;
        for part in split_escaped_commas(s) {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| format!("Invalid key-value pair: '{}'. Expected 'key=value'.", part))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "text" => {
                    let v = value.strip_prefix('"')
                        .and_then(|s| s.strip_suffix('"'))
                        .unwrap_or(value);
                    text = Some(unescape_text(v)?);
                }
                "font" => font = Some(PathBuf::from(value)),
                "size" => size = Some(value.parse::<f32>().map_err(|_| format!("Invalid size value: '{}'", value))?),
                "x" => x = Some(value.parse::<f32>().map_err(|_| format!("Invalid x value: '{}'", value))?),
                "y" => y = Some(value.parse::<f32>().map_err(|_| format!("Invalid y value: '{}'", value))?),
                "units" => units = Some(CliUnit::from_str(value, true).map_err(|e| e.to_string())?),
                "pages" => pages = Some(value.to_string()),
                "color" => color = Some(parse_color(value)?),
                "alpha" => alpha = Some(value.parse::<f32>().map_err(|_| format!("Invalid alpha value: '{}'", value))?),
                "rotation" => rotation = Some(value.parse::<f32>().map_err(|_| format!("Invalid rotation value: '{}'", value))?),
                "h_align" => h_align = Some(match value {
                    "left" => HAlign::Left,
                    "center" => HAlign::Center,
                    "right" => HAlign::Right,
                    _ => return Err(format!("Invalid h_align value: '{}'. Use left, center, or right.", value)),
                }),
                "v_align" => v_align = Some(match value {
                    "top" => VAlign::Top,
                    "cap_top" => VAlign::CapTop,
                    "center" => VAlign::Center,
                    "baseline" => VAlign::Baseline,
                    "descent_bottom" => VAlign::DescentBottom,
                    "bottom" => VAlign::Bottom,
                    _ => return Err(format!("Invalid v_align value: '{}'. Use top, cap_top, center, baseline, descent_bottom, or bottom.", value)),
                }),
                "strikeout" => strikeout = Some(value.parse::<bool>().map_err(|_| format!("Invalid strikeout value: '{}'. Use true or false.", value))?),
                "underline" => underline = Some(value.parse::<bool>().map_err(|_| format!("Invalid underline value: '{}'. Use true or false.", value))?),
                "layer" => layer = Some(match value {
                    "over" => true,
                    "under" => false,
                    _ => return Err(format!("Invalid layer value: '{}'. Use over or under.", value)),
                }),
                "weight" => weight = Some(parse_font_weight(value)?),
                "style" => style = Some(parse_font_style(value)?),
                _ => return Err(format!("Unknown watermark key: '{}'", key)),
            }
        }

        // If both color and alpha are specified, alpha overrides the color's alpha channel
        let mut final_color = color.unwrap_or(PdfColor::BLACK);
        if let Some(a) = alpha {
            final_color.a = a;
        }

        Ok(WatermarkSpec {
            text: text.ok_or("Watermark 'text' is required")?,
            font: font.ok_or("Watermark 'font' is required")?,
            size: size.unwrap_or(48.0),
            x: x.ok_or("Watermark 'x' coordinate is required")?,
            y: y.ok_or("Watermark 'y' coordinate is required")?,
            units: units.map(Unit::from).unwrap_or(Unit::In),
            pages: pages.unwrap_or_else(|| "all".to_string()),
            color: final_color,
            rotation: rotation.unwrap_or(0.0),
            h_align: h_align.unwrap_or(HAlign::Left),
            v_align: v_align.unwrap_or(VAlign::Baseline),
            strikeout: strikeout.unwrap_or(false),
            underline: underline.unwrap_or(false),
            layer_over: layer.unwrap_or(true),
            weight: weight.unwrap_or_default(),
            style: style.unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct OverlaySpec {
    pub file: PathBuf,
    pub src_page: u32,
    pub target_pages: String,
}

impl FromStr for OverlaySpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut file = None;
        let mut from_page = None;
        let mut pages = None;
        for part in split_escaped_commas(s) {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| format!("Invalid key-value pair: '{}'. Expected 'key=value'.", part))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "file" => file = Some(PathBuf::from(value)),
                "src_page" => from_page = Some(value.parse::<u32>().map_err(|_| format!("Invalid src_page value: '{}'", value))?),
                "target_pages" => pages = Some(value.to_string()),
                _ => return Err(format!("Unknown overlay key: '{}'", key)),
            }
        }
        Ok(OverlaySpec {
            file: file.ok_or("Overlay 'file' is required")?,
            src_page: from_page.ok_or("Overlay 'src_page' is required")?,
            target_pages: pages.unwrap_or_else(|| "all".to_string()),
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
        let mut file = None;
        let mut page = None;
        for part in split_escaped_commas(s) {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| format!("Invalid key-value pair: '{part}'."))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "file" => file = Some(PathBuf::from(value)),
                "page" => page = Some(value.parse::<u32>().map_err(|e| e.to_string())?),
                _ => return Err(format!("Unknown pad-file key: '{key}'")),
            }
        }
        Ok(PadFileSpec {
            file: file.ok_or("pad-file 'file' is required")?,
            page: page.unwrap_or(1),
        })
    }
}

fn parse_paper_size(name: &str) -> Result<(f32, f32), String> {
    match name.to_lowercase().as_str() {
        "letter" => Ok((612.0, 792.0)),
        "a4" => Ok((595.28, 841.89)),
        "legal" => Ok((612.0, 1008.0)),
        _ => Err(format!("Unknown paper size: '{}'. Use letter, a4, or legal.", name)),
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
        // Check for named sizes (no '=' means it's a name, not key-value pairs)
        if !trimmed.contains('=') {
            let (w, h) = parse_paper_size(trimmed)
                .map_err(|_| format!("Unknown page size: '{}'. Use letter, a4, legal, or w=...,h=...", trimmed))?;
            return Ok(BlankPageSpec { width: w, height: h, count: 1 });
        }

        let mut w = None;
        let mut h = None;
        let mut units = None;
        let mut count = None;
        for part in split_escaped_commas(trimmed) {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| format!("Invalid key-value pair: '{}'. Expected 'key=value'.", part))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "w" => w = Some(value.parse::<f32>().map_err(|_| format!("Invalid w value: '{}'", value))?),
                "h" => h = Some(value.parse::<f32>().map_err(|_| format!("Invalid h value: '{}'", value))?),
                "units" => units = Some(CliUnit::from_str(value, true).map_err(|e| e.to_string())?),
                "count" => count = Some(value.parse::<u32>().map_err(|_| format!("Invalid count value: '{}'", value))?),
                _ => return Err(format!("Unknown blank-page key: '{}'", key)),
            }
        }

        let unit: Unit = units.map(Unit::from).unwrap_or(Unit::Pt);
        let w = unit.to_points(w.ok_or("blank-page 'w' is required")?);
        let h = unit.to_points(h.ok_or("blank-page 'h' is required")?);
        let count = count.unwrap_or(1);
        if count == 0 {
            return Err("blank-page 'count' must be greater than 0".to_string());
        }

        Ok(BlankPageSpec { width: w, height: h, count })
    }
}

#[derive(Debug, Clone)]
pub struct DrawRectSpec {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: PdfColor,
    pub pages: String,
    pub layer_over: bool,
}

impl FromStr for DrawRectSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut x = None;
        let mut y = None;
        let mut w = None;
        let mut h = None;
        let mut color = None;
        let mut alpha = None;
        let mut pages = None;
        let mut units = None;
        let mut layer = None;
        for part in split_escaped_commas(s) {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| format!("Invalid key-value pair: '{}'. Expected 'key=value'.", part))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "x" => x = Some(value.parse::<f32>().map_err(|_| format!("Invalid x value: '{}'", value))?),
                "y" => y = Some(value.parse::<f32>().map_err(|_| format!("Invalid y value: '{}'", value))?),
                "w" => w = Some(value.parse::<f32>().map_err(|_| format!("Invalid w value: '{}'", value))?),
                "h" => h = Some(value.parse::<f32>().map_err(|_| format!("Invalid h value: '{}'", value))?),
                "color" => color = Some(parse_color(value)?),
                "alpha" => alpha = Some(value.parse::<f32>().map_err(|_| format!("Invalid alpha value: '{}'", value))?),
                "pages" => pages = Some(value.to_string()),
                "units" => units = Some(CliUnit::from_str(value, true).map_err(|e| e.to_string())?),
                "layer" => layer = Some(match value {
                    "over" => true,
                    "under" => false,
                    _ => return Err(format!("Invalid layer value: '{}'. Use over or under.", value)),
                }),
                _ => return Err(format!("Unknown draw-rect key: '{}'", key)),
            }
        }

        let mut final_color = color.unwrap_or(PdfColor::BLACK);
        if let Some(a) = alpha {
            final_color.a = a;
        }

        let unit = units.map(Unit::from).unwrap_or(Unit::Pt);
        Ok(DrawRectSpec {
            x: unit.to_points(x.ok_or("draw-rect 'x' is required")?),
            y: unit.to_points(y.ok_or("draw-rect 'y' is required")?),
            w: unit.to_points(w.ok_or("draw-rect 'w' is required")?),
            h: unit.to_points(h.ok_or("draw-rect 'h' is required")?),
            color: final_color,
            pages: pages.unwrap_or_else(|| "all".to_string()),
            layer_over: layer.unwrap_or(true),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DrawLineSpec {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub width: f32,
    pub color: PdfColor,
    pub pages: String,
    pub layer_over: bool,
}

impl FromStr for DrawLineSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut x1 = None;
        let mut y1 = None;
        let mut x2 = None;
        let mut y2 = None;
        let mut width = None;
        let mut color = None;
        let mut alpha = None;
        let mut pages = None;
        let mut units = None;
        let mut layer = None;
        for part in split_escaped_commas(s) {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| format!("Invalid key-value pair: '{}'. Expected 'key=value'.", part))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "x1" => x1 = Some(value.parse::<f32>().map_err(|_| format!("Invalid x1 value: '{}'", value))?),
                "y1" => y1 = Some(value.parse::<f32>().map_err(|_| format!("Invalid y1 value: '{}'", value))?),
                "x2" => x2 = Some(value.parse::<f32>().map_err(|_| format!("Invalid x2 value: '{}'", value))?),
                "y2" => y2 = Some(value.parse::<f32>().map_err(|_| format!("Invalid y2 value: '{}'", value))?),
                "width" => width = Some(value.parse::<f32>().map_err(|_| format!("Invalid width value: '{}'", value))?),
                "color" => color = Some(parse_color(value)?),
                "alpha" => alpha = Some(value.parse::<f32>().map_err(|_| format!("Invalid alpha value: '{}'", value))?),
                "pages" => pages = Some(value.to_string()),
                "units" => units = Some(CliUnit::from_str(value, true).map_err(|e| e.to_string())?),
                "layer" => layer = Some(match value {
                    "over" => true,
                    "under" => false,
                    _ => return Err(format!("Invalid layer value: '{}'. Use over or under.", value)),
                }),
                _ => return Err(format!("Unknown draw-line key: '{}'", key)),
            }
        }

        let mut final_color = color.unwrap_or(PdfColor::BLACK);
        if let Some(a) = alpha {
            final_color.a = a;
        }

        let unit = units.map(Unit::from).unwrap_or(Unit::Pt);
        Ok(DrawLineSpec {
            x1: unit.to_points(x1.ok_or("draw-line 'x1' is required")?),
            y1: unit.to_points(y1.ok_or("draw-line 'y1' is required")?),
            x2: unit.to_points(x2.ok_or("draw-line 'x2' is required")?),
            y2: unit.to_points(y2.ok_or("draw-line 'y2' is required")?),
            width: width.unwrap_or(1.0),
            color: final_color,
            pages: pages.unwrap_or_else(|| "all".to_string()),
            layer_over: layer.unwrap_or(true),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DrawImageSpec {
    pub file: PathBuf,
    pub x: f32,
    pub y: f32,
    pub w: Option<f32>,
    pub h: Option<f32>,
    pub fit: ImageFit,
    pub max_dpi: f32,
    pub pages: String,
    pub layer_over: bool,
    pub alpha: f32,
    pub rotation: f32,
}

impl FromStr for DrawImageSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut file = None;
        let mut x = None;
        let mut y = None;
        let mut w = None;
        let mut h = None;
        let mut fit = None;
        let mut max_dpi = None;
        let mut pages = None;
        let mut units = None;
        let mut layer = None;
        let mut alpha = None;
        let mut rotation = None;
        for part in split_escaped_commas(s) {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| format!("Invalid key-value pair: '{}'. Expected 'key=value'.", part))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "file" => file = Some(PathBuf::from(value)),
                "x" => x = Some(value.parse::<f32>().map_err(|_| format!("Invalid x value: '{}'", value))?),
                "y" => y = Some(value.parse::<f32>().map_err(|_| format!("Invalid y value: '{}'", value))?),
                "w" => w = Some(value.parse::<f32>().map_err(|_| format!("Invalid w value: '{}'", value))?),
                "h" => h = Some(value.parse::<f32>().map_err(|_| format!("Invalid h value: '{}'", value))?),
                "fit" => fit = Some(match value {
                    "stretch" => ImageFit::Stretch,
                    "contain" => ImageFit::Contain,
                    "cover" => ImageFit::Cover,
                    _ => return Err(format!("Invalid fit value: '{}'. Use stretch, contain, or cover.", value)),
                }),
                "max_dpi" => max_dpi = Some(value.parse::<f32>().map_err(|_| format!("Invalid max_dpi value: '{}'", value))?),
                "pages" => pages = Some(value.to_string()),
                "units" => units = Some(CliUnit::from_str(value, true).map_err(|e| e.to_string())?),
                "layer" => layer = Some(match value {
                    "over" => true,
                    "under" => false,
                    _ => return Err(format!("Invalid layer value: '{}'. Use over or under.", value)),
                }),
                "alpha" => alpha = Some(value.parse::<f32>().map_err(|_| format!("Invalid alpha value: '{}'", value))?),
                "rotation" => rotation = Some(value.parse::<f32>().map_err(|_| format!("Invalid rotation value: '{}'", value))?),
                _ => return Err(format!("Unknown draw-image key: '{}'", key)),
            }
        }

        if w.is_none() && h.is_none() {
            return Err("draw-image requires at least one of 'w' or 'h'".to_string());
        }

        let unit = units.map(Unit::from).unwrap_or(Unit::Pt);
        Ok(DrawImageSpec {
            file: file.ok_or("draw-image 'file' is required")?,
            x: unit.to_points(x.ok_or("draw-image 'x' is required")?),
            y: unit.to_points(y.ok_or("draw-image 'y' is required")?),
            w: w.map(|v| unit.to_points(v)),
            h: h.map(|v| unit.to_points(v)),
            fit: fit.unwrap_or(ImageFit::Contain),
            max_dpi: max_dpi.unwrap_or(300.0),
            pages: pages.unwrap_or_else(|| "all".to_string()),
            layer_over: layer.unwrap_or(true),
            alpha: alpha.unwrap_or(1.0),
            rotation: rotation.unwrap_or(0.0),
        })
    }
}

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

fn auto_grid(n: u32) -> (u32, u32) {
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

fn resolve_paper_dims(
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

fn apply_orientation(w: f32, h: f32, orientation: Orientation, cols: u32, rows: u32) -> (f32, f32) {
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

impl FromStr for NupSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut n = None;
        let mut cols = None;
        let mut rows = None;
        let mut paper = None;
        let mut paper_w = None;
        let mut paper_h = None;
        let mut orientation = None;
        let mut margin = None;
        let mut gutter = None;
        let mut units = None;
        let mut order = None;
        let mut border = None;
        let mut repeat = None;

        for part in split_escaped_commas(s) {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| format!("Invalid key-value pair: '{}'. Expected 'key=value'.", part))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "n" => n = Some(value.parse::<u32>().map_err(|_| format!("Invalid n value: '{}'", value))?),
                "cols" => cols = Some(value.parse::<u32>().map_err(|_| format!("Invalid cols value: '{}'", value))?),
                "rows" => rows = Some(value.parse::<u32>().map_err(|_| format!("Invalid rows value: '{}'", value))?),
                "paper" => paper = Some(value.to_string()),
                "paper_w" => paper_w = Some(value.parse::<f32>().map_err(|_| format!("Invalid paper_w value: '{}'", value))?),
                "paper_h" => paper_h = Some(value.parse::<f32>().map_err(|_| format!("Invalid paper_h value: '{}'", value))?),
                "orientation" => orientation = Some(match value.to_lowercase().as_str() {
                    "auto" => Orientation::Auto,
                    "landscape" => Orientation::Landscape,
                    "portrait" => Orientation::Portrait,
                    _ => return Err(format!("Invalid orientation: '{}'. Use auto, landscape, or portrait.", value)),
                }),
                "margin" => margin = Some(value.parse::<f32>().map_err(|_| format!("Invalid margin value: '{}'", value))?),
                "gutter" => gutter = Some(value.parse::<f32>().map_err(|_| format!("Invalid gutter value: '{}'", value))?),
                "units" => units = Some(CliUnit::from_str(value, true).map_err(|e| e.to_string())?),
                "order" => order = Some(match value.to_lowercase().as_str() {
                    "lrtb" => GridOrder::LeftToRightTopToBottom,
                    "rltb" => GridOrder::RightToLeftTopToBottom,
                    "tblr" => GridOrder::TopToBottomLeftToRight,
                    "tbrl" => GridOrder::TopToBottomRightToLeft,
                    _ => return Err(format!("Invalid order: '{}'. Use lrtb, rltb, tblr, or tbrl.", value)),
                }),
                "border" => border = Some(value.parse::<bool>().map_err(|_| format!("Invalid border value: '{}'. Use true or false.", value))?),
                "repeat" => repeat = Some(match value.to_lowercase().as_str() {
                    "auto" => 0u32, // sentinel, resolved after cols/rows are known
                    _ => {
                        let v = value.parse::<u32>().map_err(|_| format!("Invalid repeat value: '{}'. Use a positive integer or 'auto'.", value))?;
                        if v == 0 {
                            return Err("repeat must be a positive integer or 'auto'".to_string());
                        }
                        v
                    }
                }),
                _ => return Err(format!("Unknown nup key: '{}'", key)),
            }
        }

        let (cols, rows) = match (n, cols, rows) {
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

        let unit: Unit = units.map(Unit::from).unwrap_or(Unit::In);

        let (pw, ph) = resolve_paper_dims(&paper, paper_w, paper_h, unit, (612.0, 792.0))?;
        let (pw, ph) = apply_orientation(pw, ph, orientation.unwrap_or_default(), cols, rows);

        // Resolve repeat: 0 sentinel means "auto" = cols * rows
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

impl FromStr for BookletSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.trim().is_empty() {
            return Ok(BookletSpec {
                paper_width: 792.0,
                paper_height: 612.0,
                binding_margin: 0.0,
                flip: DuplexFlip::None,
                back: 0,
            });
        }

        let mut paper = None;
        let mut paper_w = None;
        let mut paper_h = None;
        let mut binding_margin = None;
        let mut units = None;
        let mut flip = None;
        let mut back = None;

        for part in split_escaped_commas(s) {
            let (key, value) = part.split_once('=')
                .ok_or_else(|| format!("Invalid key-value pair: '{}'. Expected 'key=value'.", part))?;
            let key = key.trim();
            let value = value.trim();
            match key {
                "paper" => paper = Some(value.to_string()),
                "paper_w" => paper_w = Some(value.parse::<f32>().map_err(|_| format!("Invalid paper_w value: '{}'", value))?),
                "paper_h" => paper_h = Some(value.parse::<f32>().map_err(|_| format!("Invalid paper_h value: '{}'", value))?),
                "binding_margin" => binding_margin = Some(value.parse::<f32>().map_err(|_| format!("Invalid binding_margin value: '{}'", value))?),
                "units" => units = Some(CliUnit::from_str(value, true).map_err(|e| e.to_string())?),
                "flip" => flip = Some(match value.to_lowercase().as_str() {
                    "none" => DuplexFlip::None,
                    "short_edge" => DuplexFlip::ShortEdge,
                    "long_edge" => DuplexFlip::LongEdge,
                    _ => return Err(format!("Invalid flip value: '{}'. Use none, short_edge, or long_edge.", value)),
                }),
                "back" => back = Some(value.parse::<u32>().map_err(|_| format!("Invalid back value: '{}'. Must be a non-negative integer.", value))?),
                _ => return Err(format!("Unknown booklet key: '{}'", key)),
            }
        }

        let unit: Unit = units.map(Unit::from).unwrap_or(Unit::In);

        let (pw, ph) = resolve_paper_dims(&paper, paper_w, paper_h, unit, (792.0, 612.0))?;
        // For named paper sizes, ensure landscape orientation
        let (pw, ph) = if paper.is_some() && ph > pw { (ph, pw) } else { (pw, ph) };

        Ok(BookletSpec {
            paper_width: pw,
            paper_height: ph,
            binding_margin: unit.to_points(binding_margin.unwrap_or(0.0)),
            flip: flip.unwrap_or_default(),
            back: back.unwrap_or(0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- WatermarkSpec ---

    #[test]
    fn test_watermark_spec_minimal() {
        let spec = WatermarkSpec::from_str("text=DRAFT,font=@Helvetica,x=1,y=1").unwrap();
        assert_eq!(spec.text, "DRAFT");
        assert_eq!(spec.font, PathBuf::from("@Helvetica"));
        assert!((spec.x - 1.0).abs() < f32::EPSILON);
        assert!((spec.y - 1.0).abs() < f32::EPSILON);
        assert!((spec.size - 48.0).abs() < f32::EPSILON); // default
        assert_eq!(spec.pages, "all"); // default
        assert_eq!(spec.h_align, HAlign::Left); // default
        assert_eq!(spec.v_align, VAlign::Baseline); // default
        assert!((spec.rotation - 0.0).abs() < f32::EPSILON); // default
    }

    #[test]
    fn test_watermark_spec_full() {
        let spec = WatermarkSpec::from_str(
            "text=Hello,font=@Courier,size=24,x=2,y=3,units=mm,pages=1-3,color=#FF0000,alpha=0.5,rotation=45,h_align=center,v_align=top,strikeout=true,underline=true"
        ).unwrap();
        assert_eq!(spec.text, "Hello");
        assert!((spec.size - 24.0).abs() < f32::EPSILON);
        assert!((spec.x - 2.0).abs() < f32::EPSILON);
        assert!((spec.y - 3.0).abs() < f32::EPSILON);
        assert_eq!(spec.units, Unit::Mm);
        assert_eq!(spec.pages, "1-3");
        assert!((spec.color.r - 1.0).abs() < f32::EPSILON);
        assert!((spec.color.a - 0.5).abs() < f32::EPSILON);
        assert!((spec.rotation - 45.0).abs() < f32::EPSILON);
        assert_eq!(spec.h_align, HAlign::Center);
        assert_eq!(spec.v_align, VAlign::Top);
        assert!(spec.strikeout);
        assert!(spec.underline);
    }

    #[test]
    fn test_watermark_spec_missing_text() {
        let result = WatermarkSpec::from_str("font=@Helvetica,x=1,y=1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("text"));
    }

    #[test]
    fn test_watermark_spec_missing_font() {
        let result = WatermarkSpec::from_str("text=DRAFT,x=1,y=1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("font"));
    }

    #[test]
    fn test_watermark_spec_invalid_kv() {
        let result = WatermarkSpec::from_str("text=DRAFT,badinput,font=@Helvetica,x=1,y=1");
        assert!(result.is_err());
    }

    #[test]
    fn test_watermark_spec_unknown_key() {
        let result = WatermarkSpec::from_str("text=DRAFT,font=@Helvetica,x=1,y=1,bogus=val");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bogus"));
    }

    #[test]
    fn test_watermark_spec_color_named() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,color=red").unwrap();
        assert!((spec.color.r - 1.0).abs() < f32::EPSILON);
        assert!((spec.color.g - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_watermark_spec_color_hex_short() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,color=#F00").unwrap();
        assert!((spec.color.r - 1.0).abs() < f32::EPSILON);
        assert!((spec.color.g - 0.0).abs() < f32::EPSILON);
    }

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
    }

    #[test]
    fn test_overlay_spec_missing_src_page() {
        let result = OverlaySpec::from_str("file=overlay.pdf,target_pages=all");
        assert!(result.is_err());
    }

    #[test]
    fn test_overlay_spec_unknown_key() {
        let result = OverlaySpec::from_str("file=overlay.pdf,src_page=1,bogus=val");
        assert!(result.is_err());
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

    // --- Escaped comma ---

    #[test]
    fn test_watermark_spec_escaped_comma() {
        let spec = WatermarkSpec::from_str(r"text=Hello\, World,font=@Helvetica,x=1,y=1").unwrap();
        assert_eq!(spec.text, "Hello, World");
    }

    // --- split_escaped_commas edge cases ---

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
        // A trailing backslash not followed by comma is preserved
        let parts = split_escaped_commas(r"text=hello\");
        assert_eq!(parts, vec!["text=hello\\"]);
    }

    #[test]
    fn test_split_escaped_commas_backslash_not_before_comma() {
        // Backslash not followed by comma is preserved as-is
        let parts = split_escaped_commas(r"path=C:\Users\test,key=val");
        assert_eq!(parts, vec![r"path=C:\Users\test", "key=val"]);
    }

    #[test]
    fn test_split_escaped_commas_consecutive_commas() {
        let parts = split_escaped_commas("a,,b");
        assert_eq!(parts, vec!["a", "", "b"]);
    }

    // --- parse_color edge cases ---

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
        // 5 hex chars - not 3, 6, or 8
        let result = parse_color("#12345");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid color value"));
    }

    #[test]
    fn test_parse_color_unknown_name() {
        // "purple" is not a named color in the parser
        let result = parse_color("purple");
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

    // --- Alpha override behavior ---

    #[test]
    fn test_watermark_alpha_overrides_hex_alpha() {
        // Color has alpha from RRGGBBAA, but explicit alpha= should override
        let spec = WatermarkSpec::from_str(
            "text=X,font=@H,x=0,y=0,color=#FF0000FF,alpha=0.3"
        ).unwrap();
        // alpha=0.3 should override the FF (1.0) from color
        assert!((spec.color.a - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_watermark_color_without_alpha_defaults_opaque() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,color=#00FF00").unwrap();
        assert!((spec.color.a - 1.0).abs() < f32::EPSILON);
    }

    // --- Missing required fields ---

    #[test]
    fn test_watermark_spec_missing_x() {
        let result = WatermarkSpec::from_str("text=DRAFT,font=@H,y=1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("x"));
    }

    #[test]
    fn test_watermark_spec_missing_y() {
        let result = WatermarkSpec::from_str("text=DRAFT,font=@H,x=1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("y"));
    }

    // --- Invalid numeric values ---

    #[test]
    fn test_watermark_spec_invalid_size() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,size=abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_watermark_spec_invalid_x() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=notanumber,y=0");
        assert!(result.is_err());
    }

    #[test]
    fn test_watermark_spec_invalid_alpha() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,alpha=nope");
        assert!(result.is_err());
    }

    #[test]
    fn test_watermark_spec_invalid_rotation() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,rotation=xyz");
        assert!(result.is_err());
    }

    // --- Invalid enum values ---

    #[test]
    fn test_watermark_spec_invalid_h_align() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,h_align=middle");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("h_align"));
    }

    #[test]
    fn test_watermark_spec_invalid_v_align() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,v_align=middle");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("v_align"));
    }

    #[test]
    fn test_watermark_spec_invalid_strikeout() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,strikeout=yes");
        assert!(result.is_err());
    }

    #[test]
    fn test_watermark_spec_invalid_underline() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,underline=1");
        assert!(result.is_err());
    }

    // --- OverlaySpec edge cases ---

    #[test]
    fn test_overlay_spec_invalid_src_page_value() {
        let result = OverlaySpec::from_str("file=f.pdf,src_page=abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_overlay_spec_empty_string() {
        let result = OverlaySpec::from_str("");
        assert!(result.is_err());
    }

    // --- PadToSpec edge cases ---

    #[test]
    fn test_pad_to_spec_large_value() {
        let spec = PadToSpec::from_str("1000").unwrap();
        assert_eq!(spec.pages, 1000);
    }

    #[test]
    fn test_pad_to_spec_one() {
        let spec = PadToSpec::from_str("1").unwrap();
        assert_eq!(spec.pages, 1);
    }

    #[test]
    fn test_pad_to_spec_float_fails() {
        assert!(PadToSpec::from_str("1.5").is_err());
    }

    #[test]
    fn test_pad_to_spec_empty_fails() {
        assert!(PadToSpec::from_str("").is_err());
    }

    // --- PadFileSpec edge cases ---

    #[test]
    fn test_pad_file_spec_unknown_key() {
        let result = PadFileSpec::from_str("file=f.pdf,bogus=val");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bogus"));
    }

    #[test]
    fn test_pad_file_spec_invalid_page() {
        let result = PadFileSpec::from_str("file=f.pdf,page=abc");
        assert!(result.is_err());
    }

    // --- WatermarkSpec with whitespace in values ---

    #[test]
    fn test_watermark_spec_whitespace_in_key_value() {
        // Keys and values are trimmed
        let spec = WatermarkSpec::from_str("text = DRAFT , font = @Helvetica , x = 1 , y = 1").unwrap();
        assert_eq!(spec.text, "DRAFT");
        assert_eq!(spec.font, PathBuf::from("@Helvetica"));
    }

    // --- Default units ---

    #[test]
    fn test_watermark_spec_default_units_is_inches() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=1,y=1").unwrap();
        assert_eq!(spec.units, Unit::In);
    }

    #[test]
    fn test_watermark_spec_units_mm() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=1,y=1,units=mm").unwrap();
        assert_eq!(spec.units, Unit::Mm);
    }

    // --- Default strikeout/underline ---

    #[test]
    fn test_watermark_spec_default_decorations_false() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0").unwrap();
        assert!(!spec.strikeout);
        assert!(!spec.underline);
    }

    // --- WatermarkSpec layer ---

    #[test]
    fn test_watermark_spec_default_layer_over() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0").unwrap();
        assert!(spec.layer_over);
    }

    #[test]
    fn test_watermark_spec_layer_under() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,layer=under").unwrap();
        assert!(!spec.layer_over);
    }

    #[test]
    fn test_watermark_spec_layer_over_explicit() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,layer=over").unwrap();
        assert!(spec.layer_over);
    }

    #[test]
    fn test_watermark_spec_layer_invalid() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,layer=middle");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("layer"));
    }

    // --- DrawRectSpec ---

    #[test]
    fn test_draw_rect_spec_minimal() {
        let spec = DrawRectSpec::from_str("x=30,y=680,w=550,h=0.5").unwrap();
        assert!((spec.x - 30.0).abs() < f32::EPSILON);
        assert!((spec.y - 680.0).abs() < f32::EPSILON);
        assert!((spec.w - 550.0).abs() < f32::EPSILON);
        assert!((spec.h - 0.5).abs() < f32::EPSILON);
        assert_eq!(spec.color, PdfColor::BLACK); // default
        assert_eq!(spec.pages, "all"); // default
        assert!(spec.layer_over); // default
    }

    #[test]
    fn test_draw_rect_spec_full() {
        let spec = DrawRectSpec::from_str("x=1,y=2,w=3,h=4,color=red,pages=1-3,units=in,layer=under").unwrap();
        assert!((spec.x - 72.0).abs() < f32::EPSILON); // 1 inch = 72pt
        assert!((spec.y - 144.0).abs() < f32::EPSILON);
        assert!((spec.w - 216.0).abs() < f32::EPSILON);
        assert!((spec.h - 288.0).abs() < f32::EPSILON);
        assert_eq!(spec.color, PdfColor::RED);
        assert_eq!(spec.pages, "1-3");
        assert!(!spec.layer_over);
    }

    #[test]
    fn test_draw_rect_spec_with_alpha() {
        let spec = DrawRectSpec::from_str("x=0,y=0,w=10,h=10,color=#FF0000,alpha=0.5").unwrap();
        assert!((spec.color.a - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_draw_rect_spec_missing_x() {
        let result = DrawRectSpec::from_str("y=0,w=10,h=10");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("x"));
    }

    #[test]
    fn test_draw_rect_spec_missing_w() {
        let result = DrawRectSpec::from_str("x=0,y=0,h=10");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("w"));
    }

    #[test]
    fn test_draw_rect_spec_unknown_key() {
        let result = DrawRectSpec::from_str("x=0,y=0,w=10,h=10,bogus=val");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bogus"));
    }

    #[test]
    fn test_draw_rect_spec_invalid_value() {
        let result = DrawRectSpec::from_str("x=abc,y=0,w=10,h=10");
        assert!(result.is_err());
    }

    // --- DrawLineSpec ---

    #[test]
    fn test_draw_line_spec_minimal() {
        let spec = DrawLineSpec::from_str("x1=100,y1=200,x2=400,y2=500").unwrap();
        assert!((spec.x1 - 100.0).abs() < f32::EPSILON);
        assert!((spec.y1 - 200.0).abs() < f32::EPSILON);
        assert!((spec.x2 - 400.0).abs() < f32::EPSILON);
        assert!((spec.y2 - 500.0).abs() < f32::EPSILON);
        assert!((spec.width - 1.0).abs() < f32::EPSILON); // default
        assert_eq!(spec.color, PdfColor::BLACK); // default
        assert_eq!(spec.pages, "all"); // default
        assert!(spec.layer_over); // default
    }

    #[test]
    fn test_draw_line_spec_full() {
        let spec = DrawLineSpec::from_str("x1=1,y1=2,x2=3,y2=4,width=2.5,color=blue,pages=1,units=in,layer=under").unwrap();
        assert!((spec.x1 - 72.0).abs() < f32::EPSILON);
        assert!((spec.y1 - 144.0).abs() < f32::EPSILON);
        assert!((spec.x2 - 216.0).abs() < f32::EPSILON);
        assert!((spec.y2 - 288.0).abs() < f32::EPSILON);
        assert!((spec.width - 2.5).abs() < f32::EPSILON);
        assert_eq!(spec.pages, "1");
        assert!(!spec.layer_over);
    }

    #[test]
    fn test_draw_line_spec_with_alpha() {
        let spec = DrawLineSpec::from_str("x1=0,y1=0,x2=10,y2=10,alpha=0.3").unwrap();
        assert!((spec.color.a - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_draw_line_spec_missing_x1() {
        let result = DrawLineSpec::from_str("y1=0,x2=10,y2=10");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("x1"));
    }

    #[test]
    fn test_draw_line_spec_missing_y2() {
        let result = DrawLineSpec::from_str("x1=0,y1=0,x2=10");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("y2"));
    }

    #[test]
    fn test_draw_line_spec_unknown_key() {
        let result = DrawLineSpec::from_str("x1=0,y1=0,x2=10,y2=10,bogus=val");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bogus"));
    }

    #[test]
    fn test_draw_line_spec_invalid_value() {
        let result = DrawLineSpec::from_str("x1=abc,y1=0,x2=10,y2=10");
        assert!(result.is_err());
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
        let result = BlankPageSpec::from_str("w=abc,h=100");
        assert!(result.is_err());
    }

    // --- unescape_text ---

    #[test]
    fn test_unescape_text_ascii() {
        assert_eq!(unescape_text(r"\u0041").unwrap(), "A");
    }

    #[test]
    fn test_unescape_text_em_dash() {
        assert_eq!(unescape_text(r"\u2014").unwrap(), "\u{2014}");
    }

    #[test]
    fn test_unescape_text_accented() {
        assert_eq!(unescape_text(r"\u00E9").unwrap(), "é");
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
        let result = unescape_text(r"\u00GG");
        assert!(result.is_err());
    }

    #[test]
    fn test_unescape_text_incomplete() {
        let result = unescape_text(r"\u00");
        assert!(result.is_err());
    }

    #[test]
    fn test_unescape_text_empty_u_braces() {
        let result = unescape_text(r"\U{}");
        assert!(result.is_err());
    }

    #[test]
    fn test_unescape_text_mixed() {
        assert_eq!(
            unescape_text(r"Hello \u2014 World \U{1F600}").unwrap(),
            "Hello \u{2014} World \u{1F600}"
        );
    }

    // --- split_escaped_commas with quotes ---

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
        // Unclosed quote — rest of string included
        let parts = split_escaped_commas(r#"text="hello,x=1"#);
        assert_eq!(parts, vec![r#"text="hello,x=1"#]);
    }

    // --- WatermarkSpec with quotes and unicode ---

    #[test]
    fn test_watermark_spec_quoted_text() {
        let spec = WatermarkSpec::from_str(r#"text="Hello, World",font=@Helvetica,x=1,y=1"#).unwrap();
        assert_eq!(spec.text, "Hello, World");
    }

    #[test]
    fn test_watermark_spec_unicode_text() {
        let spec = WatermarkSpec::from_str(r"text=em dash: \u2014,font=@Helvetica,x=1,y=1").unwrap();
        assert_eq!(spec.text, "em dash: \u{2014}");
    }

    #[test]
    fn test_watermark_spec_quoted_plus_unicode() {
        let spec = WatermarkSpec::from_str(r#"text="curly: \u201C\u201D",font=@Helvetica,x=1,y=1"#).unwrap();
        assert_eq!(spec.text, "curly: \u{201C}\u{201D}");
    }

    // --- DrawImageSpec ---

    #[test]
    fn test_draw_image_spec_minimal_w_only() {
        let spec = DrawImageSpec::from_str("file=logo.png,x=100,y=200,w=150").unwrap();
        assert_eq!(spec.file, PathBuf::from("logo.png"));
        assert!((spec.x - 100.0).abs() < f32::EPSILON);
        assert!((spec.y - 200.0).abs() < f32::EPSILON);
        assert!((spec.w.unwrap() - 150.0).abs() < f32::EPSILON);
        assert!(spec.h.is_none());
        assert_eq!(spec.fit, ImageFit::Contain);
        assert!((spec.max_dpi - 300.0).abs() < f32::EPSILON);
        assert_eq!(spec.pages, "all");
        assert!(spec.layer_over);
        assert!((spec.alpha - 1.0).abs() < f32::EPSILON);
        assert!((spec.rotation - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_draw_image_spec_h_only() {
        let spec = DrawImageSpec::from_str("file=logo.png,x=0,y=0,h=200").unwrap();
        assert!(spec.w.is_none());
        assert!((spec.h.unwrap() - 200.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_draw_image_spec_full() {
        let spec = DrawImageSpec::from_str(
            "file=photo.jpg,x=1,y=2,w=3,h=4,fit=cover,max_dpi=150,pages=1-3,layer=under,alpha=0.5,rotation=45,units=in"
        ).unwrap();
        assert_eq!(spec.file, PathBuf::from("photo.jpg"));
        assert!((spec.x - 72.0).abs() < f32::EPSILON);
        assert!((spec.y - 144.0).abs() < f32::EPSILON);
        assert!((spec.w.unwrap() - 216.0).abs() < f32::EPSILON);
        assert!((spec.h.unwrap() - 288.0).abs() < f32::EPSILON);
        assert_eq!(spec.fit, ImageFit::Cover);
        assert!((spec.max_dpi - 150.0).abs() < f32::EPSILON);
        assert_eq!(spec.pages, "1-3");
        assert!(!spec.layer_over);
        assert!((spec.alpha - 0.5).abs() < f32::EPSILON);
        assert!((spec.rotation - 45.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_draw_image_spec_missing_w_and_h() {
        let result = DrawImageSpec::from_str("file=logo.png,x=0,y=0");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("w") || err.contains("h"));
    }

    #[test]
    fn test_draw_image_spec_missing_file() {
        let result = DrawImageSpec::from_str("x=0,y=0,w=100");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("file"));
    }

    #[test]
    fn test_draw_image_spec_missing_x() {
        let result = DrawImageSpec::from_str("file=logo.png,y=0,w=100");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("x"));
    }

    #[test]
    fn test_draw_image_spec_missing_y() {
        let result = DrawImageSpec::from_str("file=logo.png,x=0,w=100");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("y"));
    }

    #[test]
    fn test_draw_image_spec_unknown_key() {
        let result = DrawImageSpec::from_str("file=logo.png,x=0,y=0,w=100,bogus=val");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("bogus"));
    }

    #[test]
    fn test_draw_image_spec_invalid_fit() {
        let result = DrawImageSpec::from_str("file=logo.png,x=0,y=0,w=100,fit=zoom");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("fit"));
    }

    #[test]
    fn test_draw_image_spec_max_dpi_zero() {
        let spec = DrawImageSpec::from_str("file=logo.png,x=0,y=0,w=100,max_dpi=0").unwrap();
        assert!((spec.max_dpi - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_draw_image_spec_fit_stretch() {
        let spec = DrawImageSpec::from_str("file=logo.png,x=0,y=0,w=100,h=100,fit=stretch").unwrap();
        assert_eq!(spec.fit, ImageFit::Stretch);
    }

    #[test]
    fn test_draw_image_spec_fit_contain() {
        let spec = DrawImageSpec::from_str("file=logo.png,x=0,y=0,w=100,h=100,fit=contain").unwrap();
        assert_eq!(spec.fit, ImageFit::Contain);
    }

    #[test]
    fn test_draw_image_spec_with_units() {
        let spec = DrawImageSpec::from_str("file=logo.png,x=1,y=1,w=2,units=in").unwrap();
        assert!((spec.x - 72.0).abs() < f32::EPSILON);
        assert!((spec.y - 72.0).abs() < f32::EPSILON);
        assert!((spec.w.unwrap() - 144.0).abs() < f32::EPSILON);
    }

    // --- Weight and Style parsing ---

    #[test]
    fn test_watermark_spec_default_weight_style() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0").unwrap();
        assert_eq!(spec.weight, FontWeight::NORMAL);
        assert_eq!(spec.style, FontStyle::Normal);
    }

    #[test]
    fn test_watermark_spec_weight_named() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=bold").unwrap();
        assert_eq!(spec.weight, FontWeight::BOLD);
    }

    #[test]
    fn test_watermark_spec_weight_named_variants() {
        for (name, expected) in [
            ("thin", FontWeight::THIN),
            ("light", FontWeight::LIGHT),
            ("normal", FontWeight::NORMAL),
            ("regular", FontWeight::NORMAL),
            ("medium", FontWeight::MEDIUM),
            ("semi_bold", FontWeight::SEMI_BOLD),
            ("semibold", FontWeight::SEMI_BOLD),
            ("bold", FontWeight::BOLD),
            ("extra_bold", FontWeight::EXTRA_BOLD),
            ("extrabold", FontWeight::EXTRA_BOLD),
            ("black", FontWeight::BLACK),
        ] {
            let spec = WatermarkSpec::from_str(&format!("text=X,font=@H,x=0,y=0,weight={name}")).unwrap();
            assert_eq!(spec.weight, expected, "weight={name}");
        }
    }

    #[test]
    fn test_watermark_spec_weight_numeric() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=700").unwrap();
        assert_eq!(spec.weight, FontWeight::BOLD);
    }

    #[test]
    fn test_watermark_spec_weight_numeric_custom() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=450").unwrap();
        assert_eq!(spec.weight, FontWeight(450));
    }

    #[test]
    fn test_watermark_spec_weight_invalid() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=superduper");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("weight"));
    }

    #[test]
    fn test_watermark_spec_weight_out_of_range() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=5000");
        assert!(result.is_err());
    }

    #[test]
    fn test_watermark_spec_style_italic() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,style=italic").unwrap();
        assert_eq!(spec.style, FontStyle::Italic);
    }

    #[test]
    fn test_watermark_spec_style_oblique() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,style=oblique").unwrap();
        assert!(matches!(spec.style, FontStyle::Oblique(_)));
    }

    #[test]
    fn test_watermark_spec_style_normal() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,style=normal").unwrap();
        assert_eq!(spec.style, FontStyle::Normal);
    }

    #[test]
    fn test_watermark_spec_style_invalid() {
        let result = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,style=cursive");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("style"));
    }

    #[test]
    fn test_watermark_spec_weight_and_style_combined() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=bold,style=italic").unwrap();
        assert_eq!(spec.weight, FontWeight::BOLD);
        assert_eq!(spec.style, FontStyle::Italic);
    }

    #[test]
    fn test_watermark_spec_weight_case_insensitive() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=BOLD").unwrap();
        assert_eq!(spec.weight, FontWeight::BOLD);
    }

    #[test]
    fn test_watermark_spec_style_case_insensitive() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,style=ITALIC").unwrap();
        assert_eq!(spec.style, FontStyle::Italic);
    }

    // --- parse_paper_size ---

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
        // 3 -> ceil(sqrt(3))=2 cols, ceil(3/2)=2 rows
        assert_eq!(auto_grid(3), (2, 2));
        // 5 -> ceil(sqrt(5))=3 cols, ceil(5/3)=2 rows
        assert_eq!(auto_grid(5), (3, 2));
        // 7 -> ceil(sqrt(7))=3 cols, ceil(7/3)=3 rows
        assert_eq!(auto_grid(7), (3, 3));
        // 1 -> ceil(sqrt(1))=1 cols, ceil(1/1)=1 rows
        assert_eq!(auto_grid(1), (1, 1));
    }

    // --- NupSpec ---

    #[test]
    fn test_nup_spec_with_n() {
        let spec = NupSpec::from_str("n=4").unwrap();
        assert_eq!(spec.cols, 2);
        assert_eq!(spec.rows, 2);
        // Default paper = letter, auto orientation: cols==rows -> portrait
        assert!((spec.paper_width - 612.0).abs() < f32::EPSILON);
        assert!((spec.paper_height - 792.0).abs() < f32::EPSILON);
        assert!(!spec.border);
    }

    #[test]
    fn test_nup_spec_explicit_grid() {
        let spec = NupSpec::from_str("cols=3,rows=2,paper=a4").unwrap();
        assert_eq!(spec.cols, 3);
        assert_eq!(spec.rows, 2);
        // cols > rows -> auto selects landscape -> swap a4 dims
        assert!((spec.paper_width - 841.89).abs() < 0.01);
        assert!((spec.paper_height - 595.28).abs() < 0.01);
    }

    #[test]
    fn test_nup_spec_with_options() {
        let spec = NupSpec::from_str("n=4,margin=0.5,gutter=0.25,units=in,border=true,order=rltb").unwrap();
        assert!((spec.margin - 36.0).abs() < f32::EPSILON); // 0.5in = 36pt
        assert!((spec.gutter - 18.0).abs() < f32::EPSILON); // 0.25in = 18pt
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
        assert!((spec.paper_width - 792.0).abs() < f32::EPSILON); // 11in
        assert!((spec.paper_height - 1224.0).abs() < f32::EPSILON); // 17in
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
        let spec = NupSpec::from_str("n=4").unwrap();
        assert_eq!(spec.repeat, 1);
    }

    #[test]
    fn test_nup_spec_repeat_auto() {
        let spec = NupSpec::from_str("n=4,repeat=auto").unwrap();
        assert_eq!(spec.repeat, 4); // 2x2 = 4
    }

    #[test]
    fn test_nup_spec_repeat_auto_3x2() {
        let spec = NupSpec::from_str("cols=3,rows=2,repeat=auto").unwrap();
        assert_eq!(spec.repeat, 6); // 3x2 = 6
    }

    #[test]
    fn test_nup_spec_repeat_explicit() {
        let spec = NupSpec::from_str("n=4,repeat=3").unwrap();
        assert_eq!(spec.repeat, 3);
    }

    #[test]
    fn test_nup_spec_repeat_zero_error() {
        assert!(NupSpec::from_str("n=4,repeat=0").is_err());
    }

    // --- BookletSpec ---

    #[test]
    fn test_booklet_spec_defaults() {
        let spec = BookletSpec::from_str("").unwrap();
        assert!((spec.paper_width - 792.0).abs() < f32::EPSILON); // letter landscape
        assert!((spec.paper_height - 612.0).abs() < f32::EPSILON);
        assert!((spec.binding_margin - 0.0).abs() < f32::EPSILON);
        assert_eq!(spec.flip, DuplexFlip::None);
    }

    #[test]
    fn test_booklet_spec_with_paper() {
        let spec = BookletSpec::from_str("paper=a4").unwrap();
        // a4 in landscape: 841.89 x 595.28
        assert!((spec.paper_width - 841.89).abs() < 0.01);
        assert!((spec.paper_height - 595.28).abs() < 0.01);
    }

    #[test]
    fn test_booklet_spec_with_options() {
        let spec = BookletSpec::from_str("binding_margin=0.25,units=in,flip=short_edge").unwrap();
        assert!((spec.binding_margin - 18.0).abs() < f32::EPSILON); // 0.25in = 18pt
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
        let spec = BookletSpec::from_str("").unwrap();
        assert_eq!(spec.back, 0);
    }

    #[test]
    fn test_booklet_spec_with_back() {
        let spec = BookletSpec::from_str("back=3").unwrap();
        assert_eq!(spec.back, 3);
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

    // --- Missing coverage: extra_light weight variants (#8) ---

    #[test]
    fn test_watermark_spec_weight_extra_light_variants() {
        let spec1 = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=extra_light").unwrap();
        assert_eq!(spec1.weight, FontWeight::EXTRA_LIGHT);
        let spec2 = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=extralight").unwrap();
        assert_eq!(spec2.weight, FontWeight::EXTRA_LIGHT);
    }

    // --- Missing coverage: NupSpec invalid orientation (#6) ---

    #[test]
    fn test_nup_spec_invalid_orientation() {
        let result = NupSpec::from_str("n=4,orientation=bogus");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("orientation"));
    }

    // --- Missing coverage: BookletSpec paper_w without paper_h (#7) ---

    #[test]
    fn test_booklet_spec_paper_w_without_paper_h() {
        assert!(BookletSpec::from_str("paper_w=100").is_err());
    }

    // --- Missing coverage: units=cm (#11) ---

    #[test]
    fn test_watermark_spec_units_cm() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=1,y=1,units=cm").unwrap();
        assert_eq!(spec.units, Unit::Cm);
    }

    #[test]
    fn test_draw_rect_spec_units_cm() {
        // 1cm = 72/2.54 ≈ 28.3465 pt
        let spec = DrawRectSpec::from_str("x=1,y=1,w=1,h=1,units=cm").unwrap();
        let expected_pt = 72.0 / 2.54;
        assert!((spec.x - expected_pt as f32).abs() < 0.01);
        assert!((spec.y - expected_pt as f32).abs() < 0.01);
    }
}
