use anyhow::{Result, bail};
use stbl_core::model::{
    ThemeColorOverrides, ThemeColorSchemeMode, ThemeNavOverrides, ThemeWideBackgroundOverrides,
};

#[derive(Debug, Clone)]
pub struct BaseColorsInput {
    pub bg: String,
    pub fg: Option<String>,
    pub accent: String,
    pub link: Option<String>,
    pub heading: Option<String>,
    pub mode: ThemeColorSchemeMode,
    pub brand: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct BaseColorsResolved {
    pub bg: String,
    pub fg: String,
    pub accent: String,
    pub link: Option<String>,
    pub heading: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DerivedScheme {
    pub colors: ThemeColorOverrides,
    pub nav: ThemeNavOverrides,
    pub wide_background: ThemeWideBackgroundOverrides,
    pub mode: ThemeColorSchemeMode,
    pub base: BaseColorsResolved,
}

#[derive(Debug, Clone, Copy)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

impl Rgb {
    fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

pub fn derive_from_base(input: BaseColorsInput) -> Result<DerivedScheme> {
    let _ = input.brand.len();
    let bg = parse_hex(&input.bg)?;
    let accent = parse_hex(&input.accent)?;
    let mode = resolve_mode(input.mode, bg);

    let fg = if let Some(fg) = input.fg.as_ref() {
        parse_hex(fg)?
    } else {
        default_fg_for_mode(mode)
    };
    let fg = ensure_contrast(fg, bg, 4.5, prefers_lighter(mode))?;

    let heading = if let Some(heading) = input.heading.as_ref() {
        parse_hex(heading)?
    } else if prefers_lighter(mode) {
        blend(fg, Rgb::new(255, 255, 255), 0.08)
    } else {
        blend(fg, Rgb::new(0, 0, 0), 0.08)
    };

    let muted = ensure_contrast(blend(fg, bg, 0.5), bg, 3.0, prefers_lighter(mode))?;

    let surface = if prefers_lighter(mode) {
        blend(bg, fg, 0.12)
    } else {
        blend(bg, fg, 0.04)
    };

    let border = ensure_contrast(
        if prefers_lighter(mode) {
            blend(fg, bg, 0.75)
        } else {
            blend(fg, bg, 0.85)
        },
        bg,
        1.5,
        prefers_lighter(mode),
    )?;

    let link = if let Some(link) = input.link.as_ref() {
        parse_hex(link)?
    } else {
        accent
    };
    let link = ensure_contrast(link, bg, 4.5, prefers_lighter(mode))?;
    let link_hover = if prefers_lighter(mode) {
        blend(link, Rgb::new(255, 255, 255), 0.15)
    } else {
        blend(link, Rgb::new(0, 0, 0), 0.15)
    };

    let code_bg = if prefers_lighter(mode) {
        blend(bg, fg, 0.16)
    } else {
        blend(bg, fg, 0.08)
    };
    let code_fg = ensure_contrast(fg, code_bg, 4.5, prefers_lighter(mode))?;

    let quote_bg = if prefers_lighter(mode) {
        blend(bg, fg, 0.12)
    } else {
        blend(bg, fg, 0.05)
    };
    let quote_border = blend(accent, bg, 0.7);

    let wide_bg = bg;

    let nav_bg = if prefers_lighter(mode) {
        blend(bg, fg, 0.14)
    } else {
        blend(bg, fg, 0.06)
    };
    let nav_fg = ensure_contrast(fg, nav_bg, 4.5, prefers_lighter(mode))?;
    let nav_border = border;

    let colors = ThemeColorOverrides {
        bg: Some(bg.to_hex()),
        fg: Some(fg.to_hex()),
        heading: Some(heading.to_hex()),
        title_fg: Some(heading.to_hex()),
        accent: Some(accent.to_hex()),
        link: Some(link.to_hex()),
        muted: Some(muted.to_hex()),
        surface: Some(surface.to_hex()),
        border: Some(border.to_hex()),
        link_hover: Some(link_hover.to_hex()),
        code_bg: Some(code_bg.to_hex()),
        code_fg: Some(code_fg.to_hex()),
        quote_bg: Some(quote_bg.to_hex()),
        quote_border: Some(quote_border.to_hex()),
        wide_bg: Some(wide_bg.to_hex()),
    };

    let nav = ThemeNavOverrides {
        bg: Some(nav_bg.to_hex()),
        fg: Some(nav_fg.to_hex()),
        border: Some(nav_border.to_hex()),
    };

    let wide_background = ThemeWideBackgroundOverrides {
        color: Some(wide_bg.to_hex()),
        image: None,
        style: None,
        position: None,
        opacity: None,
    };

    let base = BaseColorsResolved {
        bg: bg.to_hex(),
        fg: fg.to_hex(),
        accent: accent.to_hex(),
        link: input.link.clone(),
        heading: input.heading.clone(),
    };

    Ok(DerivedScheme {
        colors,
        nav,
        wide_background,
        mode,
        base,
    })
}

fn resolve_mode(mode: ThemeColorSchemeMode, bg: Rgb) -> ThemeColorSchemeMode {
    match mode {
        ThemeColorSchemeMode::Auto => {
            if relative_luminance(bg) < 0.5 {
                ThemeColorSchemeMode::Dark
            } else {
                ThemeColorSchemeMode::Light
            }
        }
        other => other,
    }
}

fn prefers_lighter(mode: ThemeColorSchemeMode) -> bool {
    matches!(mode, ThemeColorSchemeMode::Dark)
}

fn default_fg_for_mode(mode: ThemeColorSchemeMode) -> Rgb {
    match mode {
        ThemeColorSchemeMode::Dark => Rgb::new(245, 245, 245),
        _ => Rgb::new(17, 17, 17),
    }
}

fn ensure_contrast(color: Rgb, bg: Rgb, target: f32, lighten: bool) -> Result<Rgb> {
    if contrast_ratio(color, bg) >= target {
        return Ok(color);
    }
    let target_color = if lighten {
        Rgb::new(255, 255, 255)
    } else {
        Rgb::new(0, 0, 0)
    };
    let mut best = color;
    for step in 1..=20 {
        let t = step as f32 / 20.0;
        let candidate = blend(color, target_color, t);
        best = candidate;
        if contrast_ratio(candidate, bg) >= target {
            return Ok(candidate);
        }
    }
    Ok(best)
}

fn contrast_ratio(a: Rgb, b: Rgb) -> f32 {
    let la = relative_luminance(a);
    let lb = relative_luminance(b);
    if la >= lb {
        (la + 0.05) / (lb + 0.05)
    } else {
        (lb + 0.05) / (la + 0.05)
    }
}

fn relative_luminance(rgb: Rgb) -> f32 {
    fn channel(c: u8) -> f32 {
        let v = c as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b)
}

fn parse_hex(input: &str) -> Result<Rgb> {
    let hex = input.trim().trim_start_matches('#');
    let bytes = match hex.len() {
        3 => {
            let mut out = [0u8; 3];
            for (idx, ch) in hex.chars().enumerate() {
                let digit = ch
                    .to_digit(16)
                    .ok_or_else(|| anyhow::anyhow!("invalid hex digit"))?
                    as u8;
                out[idx] = digit * 17;
            }
            out
        }
        6 => {
            let parsed =
                u32::from_str_radix(hex, 16).map_err(|_| anyhow::anyhow!("invalid hex color"))?;
            [
                ((parsed >> 16) & 0xFF) as u8,
                ((parsed >> 8) & 0xFF) as u8,
                (parsed & 0xFF) as u8,
            ]
        }
        _ => bail!("invalid hex color '{}'", input),
    };
    Ok(Rgb::new(bytes[0], bytes[1], bytes[2]))
}

fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| ((a as f32 * (1.0 - t)) + (b as f32 * t)).round() as u8;
    Rgb::new(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_accepts_short_and_long() {
        let short = parse_hex("#fff").expect("short hex");
        assert_eq!(short.to_hex(), "#ffffff");
        let long = parse_hex("0b1020").expect("long hex");
        assert_eq!(long.to_hex(), "#0b1020");
    }

    #[test]
    fn ensure_contrast_improves_low_contrast() {
        let bg = parse_hex("#ffffff").unwrap();
        let fg = parse_hex("#f0f0f0").unwrap();
        let adjusted = ensure_contrast(fg, bg, 4.5, false).unwrap();
        assert!(contrast_ratio(adjusted, bg) >= 4.5);
    }

    #[test]
    fn derive_snapshot_matches_expected_tokens() {
        let derived = derive_from_base(BaseColorsInput {
            bg: "#0b1020".to_string(),
            fg: Some("#e9eefc".to_string()),
            accent: "#ff4fd8".to_string(),
            link: None,
            heading: None,
            mode: ThemeColorSchemeMode::Auto,
            brand: Vec::new(),
        })
        .expect("derive");

        assert_eq!(derived.colors.bg.as_deref(), Some("#0b1020"));
        assert_eq!(derived.colors.fg.as_deref(), Some("#e9eefc"));
        assert_eq!(derived.colors.accent.as_deref(), Some("#ff4fd8"));
        assert_eq!(derived.colors.link.as_deref(), Some("#ff4fd8"));
        assert_eq!(derived.colors.code_fg.as_deref(), Some("#e9eefc"));
        assert_eq!(derived.nav.bg.as_deref(), Some("#2a2f3f"));
        assert_eq!(derived.nav.fg.as_deref(), Some("#e9eefc"));
    }
}
