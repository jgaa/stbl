use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::model::{SiteConfig, ThemeColorOverrides, ThemeWideBackgroundOverrides, WideBackgroundStyle};

#[derive(Debug, Clone)]
pub struct ResolvedThemeVars {
    pub max_body_width: String,
    pub desktop_min: String,
    pub wide_min: String,
    pub header_title_size: String,
    pub header_tagline_size: String,
    pub c_bg: String,
    pub c_fg: String,
    pub c_heading: String,
    pub c_title_fg: String,
    pub c_muted: String,
    pub c_surface: String,
    pub c_border: String,
    pub c_link: String,
    pub c_link_hover: String,
    pub c_accent: String,
    pub c_nav_bg: String,
    pub c_nav_fg: String,
    pub c_nav_border: String,
    pub c_code_bg: String,
    pub c_code_fg: String,
    pub c_quote_bg: String,
    pub c_quote_border: String,
    pub c_wide_bg: String,
    pub wide_bg_image: String,
    pub wide_bg_repeat: String,
    pub wide_bg_size: String,
    pub wide_bg_position: String,
    pub wide_bg_opacity: String,
}

#[derive(Debug, Deserialize)]
struct ThemeDefaultsYaml {
    base: Option<ThemeDefaultsBase>,
    nav: Option<ThemeDefaultsNav>,
    wide_background: Option<ThemeDefaultsWideBackground>,
}

#[derive(Debug, Deserialize)]
struct ThemeDefaultsBase {
    bg: Option<String>,
    fg: Option<String>,
    heading: Option<String>,
    accent: Option<String>,
    link: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ThemeDefaultsNav {
    bg: Option<String>,
    fg: Option<String>,
    border: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ThemeDefaultsWideBackground {
    color: Option<String>,
    image: Option<String>,
    style: Option<String>,
    position: Option<String>,
    opacity: Option<f32>,
}

#[derive(Debug, Clone, Copy)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

pub fn resolve_theme_vars(defaults_yaml: &[u8], config: &SiteConfig) -> Result<ResolvedThemeVars> {
    let defaults: ThemeDefaultsYaml = serde_yaml::from_slice(defaults_yaml)
        .context("failed to parse theme defaults yaml")?;
    let base_defaults = defaults.base.unwrap_or(ThemeDefaultsBase {
        bg: None,
        fg: None,
        heading: None,
        accent: None,
        link: None,
    });
    let nav_defaults = defaults.nav.unwrap_or(ThemeDefaultsNav {
        bg: None,
        fg: None,
        border: None,
    });
    let wide_defaults = defaults.wide_background.unwrap_or(ThemeDefaultsWideBackground {
        color: None,
        image: None,
        style: None,
        position: None,
        opacity: None,
    });

    let overrides = &config.theme.colors;
    let nav_overrides = &config.theme.nav;
    let wide_overrides = &config.theme.wide_background;

    let bg = pick_color(overrides.bg.as_ref(), base_defaults.bg.as_ref(), "bg", Rgb::new(255, 255, 255))?;
    let fg = pick_color(overrides.fg.as_ref(), base_defaults.fg.as_ref(), "fg", Rgb::new(0, 0, 0))?;

    let accent = pick_color_optional(overrides.accent.as_ref(), base_defaults.accent.as_ref(), "accent")?
        .unwrap_or(fg);
    let heading = pick_color_optional(overrides.heading.as_ref(), base_defaults.heading.as_ref(), "heading")?
        .unwrap_or(accent);
    let title_fg = pick_color_optional(overrides.title_fg.as_ref(), None, "title_fg")?
        .unwrap_or(heading);
    let link = pick_color_optional(overrides.link.as_ref(), base_defaults.link.as_ref(), "link")?
        .unwrap_or(accent);

    let muted = pick_color_optional(overrides.muted.as_ref(), None, "muted")?
        .unwrap_or_else(|| blend(fg, bg, 0.55));
    let border = pick_color_optional(overrides.border.as_ref(), None, "border")?
        .unwrap_or_else(|| blend(fg, bg, 0.85));
    let surface = pick_color_optional(overrides.surface.as_ref(), None, "surface")?
        .unwrap_or_else(|| blend(bg, fg, 0.04));
    let link_hover = pick_color_optional(overrides.link_hover.as_ref(), None, "link_hover")?
        .unwrap_or_else(|| blend(link, fg, 0.15));
    let quote_bg = pick_color_optional(overrides.quote_bg.as_ref(), None, "quote_bg")?
        .unwrap_or_else(|| blend(bg, fg, 0.03));
    let quote_border = pick_color_optional(overrides.quote_border.as_ref(), None, "quote_border")?
        .unwrap_or(border);

    let wide_bg = resolve_wide_bg_color(overrides, wide_overrides, &wide_defaults, surface)?;

    let (code_bg, code_fg) = resolve_code_colors(overrides, bg, fg)?;

    let nav_bg = resolve_nav_color(
        nav_overrides.bg.as_ref(),
        nav_defaults.bg.as_ref(),
        "nav.bg",
    )?
    .unwrap_or(fg);
    let nav_fg = resolve_nav_color(
        nav_overrides.fg.as_ref(),
        nav_defaults.fg.as_ref(),
        "nav.fg",
    )?
    .unwrap_or(bg);
    let nav_border = resolve_nav_color(
        nav_overrides.border.as_ref(),
        nav_defaults.border.as_ref(),
        "nav.border",
    )?
    .unwrap_or(nav_fg);

    let (wide_bg_repeat, wide_bg_size) = resolve_wide_background_style(wide_overrides, &wide_defaults)?;
    let wide_bg_position = resolve_wide_background_position(wide_overrides, &wide_defaults)?;
    let wide_bg_opacity = resolve_wide_background_opacity(wide_overrides, &wide_defaults)?;
    let wide_bg_image = resolve_wide_background_image(wide_overrides, &wide_defaults)?;

    Ok(ResolvedThemeVars {
        max_body_width: config.theme.max_body_width.clone(),
        desktop_min: config.theme.breakpoints.desktop_min.clone(),
        wide_min: config.theme.breakpoints.wide_min.clone(),
        header_title_size: config.theme.header.title_size.clone(),
        header_tagline_size: config.theme.header.tagline_size.clone(),
        c_bg: bg.to_hex(),
        c_fg: fg.to_hex(),
        c_heading: heading.to_hex(),
        c_title_fg: title_fg.to_hex(),
        c_muted: muted.to_hex(),
        c_surface: surface.to_hex(),
        c_border: border.to_hex(),
        c_link: link.to_hex(),
        c_link_hover: link_hover.to_hex(),
        c_accent: accent.to_hex(),
        c_nav_bg: nav_bg.to_hex(),
        c_nav_fg: nav_fg.to_hex(),
        c_nav_border: nav_border.to_hex(),
        c_code_bg: code_bg.to_hex(),
        c_code_fg: code_fg.to_hex(),
        c_quote_bg: quote_bg.to_hex(),
        c_quote_border: quote_border.to_hex(),
        c_wide_bg: wide_bg.to_hex(),
        wide_bg_image,
        wide_bg_repeat,
        wide_bg_size,
        wide_bg_position,
        wide_bg_opacity,
    })
}

fn resolve_wide_bg_color(
    overrides: &ThemeColorOverrides,
    wide_overrides: &ThemeWideBackgroundOverrides,
    defaults: &ThemeDefaultsWideBackground,
    surface: Rgb,
) -> Result<Rgb> {
    if let Some(color) = overrides.wide_bg.as_ref() {
        return parse_color(color, "wide_bg");
    }
    if let Some(color) = wide_overrides.color.as_ref() {
        return parse_color(color, "wide_background.color");
    }
    if let Some(color) = defaults.color.as_ref() {
        return parse_color(color, "wide_background.color");
    }
    Ok(surface)
}

fn resolve_code_colors(overrides: &ThemeColorOverrides, bg: Rgb, fg: Rgb) -> Result<(Rgb, Rgb)> {
    let code_bg = if let Some(value) = overrides.code_bg.as_ref() {
        parse_color(value, "code_bg")?
    } else {
        blend(bg, fg, 0.08)
    };

    let code_fg = if let Some(value) = overrides.code_fg.as_ref() {
        parse_color(value, "code_fg")?
    } else {
        fg
    };

    Ok((code_bg, code_fg))
}

fn resolve_nav_color(
    override_value: Option<&String>,
    default_value: Option<&String>,
    label: &str,
) -> Result<Option<Rgb>> {
    if let Some(value) = override_value {
        return Ok(Some(parse_color(value, label)?));
    }
    if let Some(value) = default_value {
        return Ok(Some(parse_color(value, label)?));
    }
    Ok(None)
}

fn resolve_wide_background_style(
    overrides: &ThemeWideBackgroundOverrides,
    defaults: &ThemeDefaultsWideBackground,
) -> Result<(String, String)> {
    let style = if let Some(style) = overrides.style {
        style
    } else if let Some(value) = defaults.style.as_ref() {
        parse_style(value)?
    } else {
        WideBackgroundStyle::Cover
    };
    match style {
        WideBackgroundStyle::Cover => Ok(("no-repeat".to_string(), "cover".to_string())),
        WideBackgroundStyle::Tile => Ok(("repeat".to_string(), "auto".to_string())),
    }
}

fn resolve_wide_background_position(
    overrides: &ThemeWideBackgroundOverrides,
    defaults: &ThemeDefaultsWideBackground,
) -> Result<String> {
    if let Some(value) = overrides.position.as_ref() {
        return Ok(value.clone());
    }
    if let Some(value) = defaults.position.as_ref() {
        return Ok(value.clone());
    }
    Ok("center top".to_string())
}

fn resolve_wide_background_opacity(
    overrides: &ThemeWideBackgroundOverrides,
    defaults: &ThemeDefaultsWideBackground,
) -> Result<String> {
    let value = overrides
        .opacity
        .or(defaults.opacity)
        .unwrap_or(1.0);
    if !(0.0..=1.0).contains(&value) || !value.is_finite() {
        bail!("wide_background.opacity must be between 0.0 and 1.0");
    }
    Ok(format_opacity(value))
}

fn resolve_wide_background_image(
    overrides: &ThemeWideBackgroundOverrides,
    defaults: &ThemeDefaultsWideBackground,
) -> Result<String> {
    let value = overrides
        .image
        .as_ref()
        .or_else(|| defaults.image.as_ref());
    match value {
        Some(raw) if !raw.trim().is_empty() => {
            let normalized = normalize_wide_background_image(raw.trim());
            Ok(format!("url(\"{}\")", normalized))
        }
        _ => Ok("none".to_string()),
    }
}

fn normalize_wide_background_image(raw: &str) -> String {
    if raw.starts_with("url(") {
        return raw.to_string();
    }
    if raw.starts_with('/')
        || raw.starts_with("http://")
        || raw.starts_with("https://")
        || raw.starts_with("data:")
    {
        return raw.replace('"', "\\\"");
    }
    let trimmed = raw.trim_start_matches("./");
    format!("/{}", trimmed).replace('"', "\\\"")
}

fn pick_color(
    override_value: Option<&String>,
    default_value: Option<&String>,
    label: &str,
    fallback: Rgb,
) -> Result<Rgb> {
    if let Some(value) = override_value {
        return parse_color(value, label);
    }
    if let Some(value) = default_value {
        return parse_color(value, label);
    }
    Ok(fallback)
}

fn pick_color_optional(
    override_value: Option<&String>,
    default_value: Option<&String>,
    label: &str,
) -> Result<Option<Rgb>> {
    if let Some(value) = override_value {
        return Ok(Some(parse_color(value, label)?));
    }
    if let Some(value) = default_value {
        return Ok(Some(parse_color(value, label)?));
    }
    Ok(None)
}

fn parse_style(value: &str) -> Result<WideBackgroundStyle> {
    match value.trim() {
        "cover" => Ok(WideBackgroundStyle::Cover),
        "tile" => Ok(WideBackgroundStyle::Tile),
        _ => bail!("wide_background.style must be 'cover' or 'tile'"),
    }
}

impl Rgb {
    fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

fn parse_color(input: &str, label: &str) -> Result<Rgb> {
    let value = input.trim();
    let hex = value.strip_prefix('#').unwrap_or(value);
    let bytes = match hex.len() {
        3 => {
            let mut out = [0u8; 3];
            for (idx, ch) in hex.chars().enumerate() {
                let digit = ch.to_digit(16).context("invalid hex digit")? as u8;
                out[idx] = digit * 17;
            }
            out
        }
        6 => {
            let parsed = u32::from_str_radix(hex, 16).context("invalid hex color")?;
            [((parsed >> 16) & 0xFF) as u8, ((parsed >> 8) & 0xFF) as u8, (parsed & 0xFF) as u8]
        }
        _ => bail!("invalid color for {}: {}", label, input),
    };
    Ok(Rgb::new(bytes[0], bytes[1], bytes[2]))
}

fn blend(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| ((a as f32 * (1.0 - t)) + (b as f32 * t)).round() as u8;
    Rgb::new(mix(a.r, b.r), mix(a.g, b.g), mix(a.b, b.b))
}

fn format_opacity(value: f32) -> String {
    let formatted = format!("{:.3}", value);
    let trimmed = formatted.trim_end_matches('0').trim_end_matches('.');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_wide_background_image_prefixes_root_for_relative_paths() {
        assert_eq!(
            normalize_wide_background_image("images/background-image.jpg"),
            "/images/background-image.jpg"
        );
        assert_eq!(
            normalize_wide_background_image("./images/background-image.jpg"),
            "/images/background-image.jpg"
        );
    }

    #[test]
    fn normalize_wide_background_image_keeps_absolute_and_url_values() {
        assert_eq!(
            normalize_wide_background_image("/images/background-image.jpg"),
            "/images/background-image.jpg"
        );
        assert_eq!(
            normalize_wide_background_image("https://example.com/bg.jpg"),
            "https://example.com/bg.jpg"
        );
        assert_eq!(
            normalize_wide_background_image("url(\"/images/background-image.jpg\")"),
            "url(\"/images/background-image.jpg\")"
        );
    }

    #[test]
    fn wide_background_opacity_defaults_to_full_when_unset() {
        let overrides = ThemeWideBackgroundOverrides::default();
        let defaults = ThemeDefaultsWideBackground {
            color: None,
            image: None,
            style: None,
            position: None,
            opacity: None,
        };
        let value = resolve_wide_background_opacity(&overrides, &defaults).expect("opacity");
        assert_eq!(value, "1");
    }
}
