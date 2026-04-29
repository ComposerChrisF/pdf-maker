// Per-page drawing specs: watermarks, rectangles, lines, and images.

use std::path::PathBuf;
use std::str::FromStr;

use medpdf::{FontStyle, FontWeight, HAlign, PdfColor, Unit, VAlign};
use medpdf_image::ImageFit;

use super::parse::{
    parse_color, parse_font_style, parse_font_weight, strip_quotes, unescape_text, KvParser,
};

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

const WATERMARK_KEYS: &[&str] = &[
    "text", "font", "size", "x", "y", "units", "pages", "color", "alpha", "rotation",
    "h_align", "v_align", "strikeout", "underline", "layer", "weight", "style",
];

fn parse_h_align(v: &str) -> Result<HAlign, String> {
    match v {
        "left" => Ok(HAlign::Left),
        "center" => Ok(HAlign::Center),
        "right" => Ok(HAlign::Right),
        _ => Err(format!("Invalid h_align value: '{v}'. Use left, center, or right.")),
    }
}

fn parse_v_align(v: &str) -> Result<VAlign, String> {
    match v {
        "top" => Ok(VAlign::Top),
        "cap_top" => Ok(VAlign::CapTop),
        "center" => Ok(VAlign::Center),
        "baseline" => Ok(VAlign::Baseline),
        "descent_bottom" => Ok(VAlign::DescentBottom),
        "bottom" => Ok(VAlign::Bottom),
        _ => Err(format!(
            "Invalid v_align value: '{v}'. Use top, cap_top, center, baseline, descent_bottom, or bottom."
        )),
    }
}

impl FromStr for WatermarkSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kv = KvParser::parse(s, "watermark", WATERMARK_KEYS)?;

        let text_raw = kv.required_str("text")?;
        let text = unescape_text(strip_quotes(text_raw))?;
        let font = PathBuf::from(kv.required_str("font")?);
        let x = kv.required_parse::<f32>("x")?;
        let y = kv.required_parse::<f32>("y")?;

        let size = kv.optional_parse::<f32>("size")?.unwrap_or(48.0);
        let units = kv.optional_units()?.map(Unit::from).unwrap_or(Unit::In);
        let pages = kv.get("pages").map(str::to_string).unwrap_or_else(|| "all".to_string());
        let color_opt = kv.optional_with("color", parse_color)?;
        let alpha = kv.optional_parse::<f32>("alpha")?;
        let rotation = kv.optional_parse::<f32>("rotation")?.unwrap_or(0.0);
        let h_align = kv.optional_with("h_align", parse_h_align)?.unwrap_or(HAlign::Left);
        let v_align = kv.optional_with("v_align", parse_v_align)?.unwrap_or(VAlign::Baseline);
        let strikeout = kv.optional_with("strikeout", |v| {
            v.parse::<bool>()
                .map_err(|_| format!("Invalid strikeout value: '{v}'. Use true or false."))
        })?
        .unwrap_or(false);
        let underline = kv.optional_with("underline", |v| {
            v.parse::<bool>()
                .map_err(|_| format!("Invalid underline value: '{v}'. Use true or false."))
        })?
        .unwrap_or(false);
        let layer_over = kv.optional_layer()?.unwrap_or(true);
        let weight = kv.optional_with("weight", parse_font_weight)?.unwrap_or_default();
        let style = kv.optional_with("style", parse_font_style)?.unwrap_or_default();

        let mut final_color = color_opt.unwrap_or(PdfColor::BLACK);
        if let Some(a) = alpha {
            final_color.a = a;
        }

        Ok(WatermarkSpec {
            text,
            font,
            size,
            x,
            y,
            units,
            pages,
            color: final_color,
            rotation,
            h_align,
            v_align,
            strikeout,
            underline,
            layer_over,
            weight,
            style,
        })
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
        let kv = KvParser::parse(
            s,
            "draw-rect",
            &["x", "y", "w", "h", "color", "alpha", "pages", "units", "layer"],
        )?;
        let unit: Unit = kv.optional_units()?.map(Unit::from).unwrap_or(Unit::Pt);

        let x = kv.required_parse::<f32>("x")?;
        let y = kv.required_parse::<f32>("y")?;
        let w = kv.required_parse::<f32>("w")?;
        let h = kv.required_parse::<f32>("h")?;
        let color_opt = kv.optional_with("color", parse_color)?;
        let alpha = kv.optional_parse::<f32>("alpha")?;
        let pages = kv.get("pages").map(str::to_string).unwrap_or_else(|| "all".to_string());
        let layer_over = kv.optional_layer()?.unwrap_or(true);

        let mut final_color = color_opt.unwrap_or(PdfColor::BLACK);
        if let Some(a) = alpha {
            final_color.a = a;
        }

        Ok(DrawRectSpec {
            x: unit.to_points(x),
            y: unit.to_points(y),
            w: unit.to_points(w),
            h: unit.to_points(h),
            color: final_color,
            pages,
            layer_over,
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
        let kv = KvParser::parse(
            s,
            "draw-line",
            &["x1", "y1", "x2", "y2", "width", "color", "alpha", "pages", "units", "layer"],
        )?;
        let unit: Unit = kv.optional_units()?.map(Unit::from).unwrap_or(Unit::Pt);

        let x1 = kv.required_parse::<f32>("x1")?;
        let y1 = kv.required_parse::<f32>("y1")?;
        let x2 = kv.required_parse::<f32>("x2")?;
        let y2 = kv.required_parse::<f32>("y2")?;
        let width = kv.optional_parse::<f32>("width")?.unwrap_or(1.0);
        let color_opt = kv.optional_with("color", parse_color)?;
        let alpha = kv.optional_parse::<f32>("alpha")?;
        let pages = kv.get("pages").map(str::to_string).unwrap_or_else(|| "all".to_string());
        let layer_over = kv.optional_layer()?.unwrap_or(true);

        let mut final_color = color_opt.unwrap_or(PdfColor::BLACK);
        if let Some(a) = alpha {
            final_color.a = a;
        }

        Ok(DrawLineSpec {
            x1: unit.to_points(x1),
            y1: unit.to_points(y1),
            x2: unit.to_points(x2),
            y2: unit.to_points(y2),
            width,
            color: final_color,
            pages,
            layer_over,
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

fn parse_image_fit(v: &str) -> Result<ImageFit, String> {
    match v {
        "stretch" => Ok(ImageFit::Stretch),
        "contain" => Ok(ImageFit::Contain),
        "cover" => Ok(ImageFit::Cover),
        _ => Err(format!("Invalid fit value: '{v}'. Use stretch, contain, or cover.")),
    }
}

impl FromStr for DrawImageSpec {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let kv = KvParser::parse(
            s,
            "draw-image",
            &[
                "file", "x", "y", "w", "h", "fit", "max_dpi", "pages", "units", "layer", "alpha",
                "rotation",
            ],
        )?;
        let unit: Unit = kv.optional_units()?.map(Unit::from).unwrap_or(Unit::Pt);

        let file = PathBuf::from(kv.required_str("file")?);
        let x = kv.required_parse::<f32>("x")?;
        let y = kv.required_parse::<f32>("y")?;
        let w = kv.optional_parse::<f32>("w")?;
        let h = kv.optional_parse::<f32>("h")?;
        if w.is_none() && h.is_none() {
            return Err("draw-image requires at least one of 'w' or 'h'".to_string());
        }
        let fit = kv.optional_with("fit", parse_image_fit)?.unwrap_or(ImageFit::Contain);
        let max_dpi = kv.optional_parse::<f32>("max_dpi")?.unwrap_or(300.0);
        let pages = kv.get("pages").map(str::to_string).unwrap_or_else(|| "all".to_string());
        let layer_over = kv.optional_layer()?.unwrap_or(true);
        let alpha = kv.optional_parse::<f32>("alpha")?.unwrap_or(1.0);
        let rotation = kv.optional_parse::<f32>("rotation")?.unwrap_or(0.0);

        Ok(DrawImageSpec {
            file,
            x: unit.to_points(x),
            y: unit.to_points(y),
            w: w.map(|v| unit.to_points(v)),
            h: h.map(|v| unit.to_points(v)),
            fit,
            max_dpi,
            pages,
            layer_over,
            alpha,
            rotation,
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
        assert!((spec.size - 48.0).abs() < f32::EPSILON);
        assert_eq!(spec.pages, "all");
        assert_eq!(spec.h_align, HAlign::Left);
        assert_eq!(spec.v_align, VAlign::Baseline);
        assert!((spec.rotation - 0.0).abs() < f32::EPSILON);
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

    #[test]
    fn test_watermark_spec_escaped_comma() {
        let spec = WatermarkSpec::from_str(r"text=Hello\, World,font=@Helvetica,x=1,y=1").unwrap();
        assert_eq!(spec.text, "Hello, World");
    }

    #[test]
    fn test_watermark_alpha_overrides_hex_alpha() {
        let spec = WatermarkSpec::from_str(
            "text=X,font=@H,x=0,y=0,color=#FF0000FF,alpha=0.3"
        ).unwrap();
        assert!((spec.color.a - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn test_watermark_color_without_alpha_defaults_opaque() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,color=#00FF00").unwrap();
        assert!((spec.color.a - 1.0).abs() < f32::EPSILON);
    }

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

    #[test]
    fn test_watermark_spec_invalid_size() {
        assert!(WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,size=abc").is_err());
    }

    #[test]
    fn test_watermark_spec_invalid_x() {
        assert!(WatermarkSpec::from_str("text=X,font=@H,x=notanumber,y=0").is_err());
    }

    #[test]
    fn test_watermark_spec_invalid_alpha() {
        assert!(WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,alpha=nope").is_err());
    }

    #[test]
    fn test_watermark_spec_invalid_rotation() {
        assert!(WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,rotation=xyz").is_err());
    }

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
        assert!(WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,strikeout=yes").is_err());
    }

    #[test]
    fn test_watermark_spec_invalid_underline() {
        assert!(WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,underline=1").is_err());
    }

    #[test]
    fn test_watermark_spec_whitespace_in_key_value() {
        let spec = WatermarkSpec::from_str("text = DRAFT , font = @Helvetica , x = 1 , y = 1").unwrap();
        assert_eq!(spec.text, "DRAFT");
        assert_eq!(spec.font, PathBuf::from("@Helvetica"));
    }

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

    #[test]
    fn test_watermark_spec_units_cm() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=1,y=1,units=cm").unwrap();
        assert_eq!(spec.units, Unit::Cm);
    }

    #[test]
    fn test_watermark_spec_default_decorations_false() {
        let spec = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0").unwrap();
        assert!(!spec.strikeout);
        assert!(!spec.underline);
    }

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

    #[test]
    fn test_watermark_spec_quoted_text() {
        let spec = WatermarkSpec::from_str(r#"text="Hello, World",font=@Helvetica,x=1,y=1"#).unwrap();
        assert_eq!(spec.text, "Hello, World");
    }

    #[test]
    fn test_watermark_spec_unicode_text() {
        let spec = WatermarkSpec::from_str(r"text=em dash: —,font=@Helvetica,x=1,y=1").unwrap();
        assert_eq!(spec.text, "em dash: \u{2014}");
    }

    #[test]
    fn test_watermark_spec_quoted_plus_unicode() {
        let spec = WatermarkSpec::from_str(r#"text="curly: “”",font=@Helvetica,x=1,y=1"#).unwrap();
        assert_eq!(spec.text, "curly: \u{201C}\u{201D}");
    }

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
        assert!(WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=5000").is_err());
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

    #[test]
    fn test_watermark_spec_weight_extra_light_variants() {
        let spec1 = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=extra_light").unwrap();
        assert_eq!(spec1.weight, FontWeight::EXTRA_LIGHT);
        let spec2 = WatermarkSpec::from_str("text=X,font=@H,x=0,y=0,weight=extralight").unwrap();
        assert_eq!(spec2.weight, FontWeight::EXTRA_LIGHT);
    }

    // --- DrawRectSpec ---

    #[test]
    fn test_draw_rect_spec_minimal() {
        let spec = DrawRectSpec::from_str("x=30,y=680,w=550,h=0.5").unwrap();
        assert!((spec.x - 30.0).abs() < f32::EPSILON);
        assert!((spec.y - 680.0).abs() < f32::EPSILON);
        assert!((spec.w - 550.0).abs() < f32::EPSILON);
        assert!((spec.h - 0.5).abs() < f32::EPSILON);
        assert_eq!(spec.color, PdfColor::BLACK);
        assert_eq!(spec.pages, "all");
        assert!(spec.layer_over);
    }

    #[test]
    fn test_draw_rect_spec_full() {
        let spec = DrawRectSpec::from_str("x=1,y=2,w=3,h=4,color=red,pages=1-3,units=in,layer=under").unwrap();
        assert!((spec.x - 72.0).abs() < f32::EPSILON);
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
        assert!(DrawRectSpec::from_str("x=abc,y=0,w=10,h=10").is_err());
    }

    #[test]
    fn test_draw_rect_spec_units_cm() {
        let spec = DrawRectSpec::from_str("x=1,y=1,w=1,h=1,units=cm").unwrap();
        let expected_pt = 72.0 / 2.54;
        assert!((spec.x - expected_pt as f32).abs() < 0.01);
        assert!((spec.y - expected_pt as f32).abs() < 0.01);
    }

    // --- DrawLineSpec ---

    #[test]
    fn test_draw_line_spec_minimal() {
        let spec = DrawLineSpec::from_str("x1=100,y1=200,x2=400,y2=500").unwrap();
        assert!((spec.x1 - 100.0).abs() < f32::EPSILON);
        assert!((spec.y1 - 200.0).abs() < f32::EPSILON);
        assert!((spec.x2 - 400.0).abs() < f32::EPSILON);
        assert!((spec.y2 - 500.0).abs() < f32::EPSILON);
        assert!((spec.width - 1.0).abs() < f32::EPSILON);
        assert_eq!(spec.color, PdfColor::BLACK);
        assert_eq!(spec.pages, "all");
        assert!(spec.layer_over);
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
        assert!(DrawLineSpec::from_str("x1=abc,y1=0,x2=10,y2=10").is_err());
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
}
