use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::color_presets::ColorPreset;

#[derive(Debug, Clone, Copy)]
struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

pub fn default_color_preview_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set for preview output path")?;
    Ok(PathBuf::from(home)
        .join(".cache")
        .join("stbl")
        .join("color-themes.html"))
}

pub fn write_color_theme_preview(
    out_path: &Path,
    presets: &BTreeMap<String, ColorPreset>,
) -> Result<()> {
    let html = render_color_theme_preview(presets)?;
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(out_path, html).with_context(|| format!("failed to write {}", out_path.display()))?;
    Ok(())
}

fn render_color_theme_preview(presets: &BTreeMap<String, ColorPreset>) -> Result<String> {
    let mut html = String::new();
    html.push_str(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Color themes</title>
  <style>
    :root {
      color-scheme: light dark;
    }
    body {
      margin: 0;
      padding: 2rem;
      background: #0b0b0c;
      color: #f5f5f5;
      font-family: "Iowan Old Style", "Palatino Linotype", "Times New Roman", serif;
    }
    h1 {
      margin: 0 0 1.5rem;
      font-size: 2rem;
      font-weight: 600;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(360px, 1fr));
      gap: 1.5rem;
    }
    .theme-card {
      background: #151518;
      border: 1px solid #2a2a2f;
      border-radius: 14px;
      padding: 1rem;
      display: grid;
      gap: 1rem;
    }
    .theme-header {
      display: flex;
      flex-direction: column;
      gap: 0.35rem;
    }
    .theme-name {
      font-size: 1.3rem;
      font-weight: 600;
    }
    .theme-meta {
      display: flex;
      flex-wrap: wrap;
      gap: 0.4rem;
      font-size: 0.85rem;
    }
    .badge {
      background: #23232a;
      color: #e6e6e6;
      border-radius: 999px;
      padding: 0.15rem 0.6rem;
      border: 1px solid #30303a;
    }
    .badge.warn {
      background: #4b2626;
      border-color: #6f2f2f;
      color: #ffd7d7;
    }
    .tokens {
      display: grid;
      gap: 0.35rem;
    }
    .token-row {
      display: grid;
      grid-template-columns: 1.1fr 1fr 48px;
      gap: 0.5rem;
      align-items: center;
      font-size: 0.85rem;
    }
    .token-group {
      font-size: 0.75rem;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      color: #a0a0a8;
      margin-top: 0.4rem;
    }
    .token-swatch {
      width: 44px;
      height: 20px;
      border-radius: 6px;
      border: 1px solid #444;
      background: transparent;
    }
    .preview {
      border-radius: 12px;
      border: 1px solid var(--c-border);
      background: var(--c-bg);
      color: var(--c-fg);
      padding: 0.9rem;
      display: grid;
      gap: 0.8rem;
    }
    .preview h2 {
      margin: 0;
      font-size: 1.1rem;
      color: var(--c-heading);
    }
    .preview p {
      margin: 0.15rem 0;
    }
    .preview a {
      color: var(--c-link);
      text-decoration: none;
    }
    .preview a:hover {
      color: var(--c-link-hover);
      text-decoration: underline;
    }
    .preview .muted {
      color: var(--c-muted);
    }
    .nav-preview {
      background: var(--c-nav-bg);
      color: var(--c-nav-fg);
      border: 1px solid var(--c-nav-border);
      border-radius: 8px;
      padding: 0.4rem 0.6rem;
      display: flex;
      gap: 0.8rem;
      font-size: 0.85rem;
    }
    .nav-preview a {
      color: var(--c-nav-fg);
    }
    .panel-preview {
      background: var(--c-surface);
      border: 1px solid var(--c-border);
      border-radius: 10px;
      padding: 0.6rem;
      display: grid;
      gap: 0.5rem;
    }
    .chip {
      display: inline-block;
      background: var(--c-accent);
      color: #fff;
      border-radius: 999px;
      padding: 0.1rem 0.6rem;
      font-size: 0.75rem;
    }
    .input-mock {
      border: 1px solid var(--c-border);
      border-radius: 6px;
      padding: 0.3rem 0.5rem;
      background: rgba(255,255,255,0.06);
      font-size: 0.8rem;
    }
    .quote-preview {
      background: var(--c-quote-bg);
      border-left: 3px solid var(--c-quote-border);
      padding: 0.5rem 0.7rem;
      border-radius: 6px;
      font-size: 0.85rem;
    }
    .code-preview {
      background: var(--c-code-bg);
      color: var(--c-code-fg);
      border-radius: 8px;
      padding: 0.5rem 0.7rem;
      font-family: "JetBrains Mono", "Fira Mono", ui-monospace, monospace;
      font-size: 0.78rem;
      line-height: 1.35;
      white-space: pre-wrap;
    }
    .wide-preview {
      background: var(--c-wide-bg);
      border-radius: 8px;
      padding: 0.4rem 0.6rem;
      font-size: 0.75rem;
      color: var(--c-fg);
      border: 1px solid var(--c-border);
    }
  </style>
</head>
<body>
  <h1>Color themes</h1>
  <div class="grid">
"#,
    );

    for (name, preset) in presets {
        let title = escape_html(name);
        let variant = preset
            .variant
            .as_ref()
            .map(|value| escape_html(value))
            .unwrap_or_default();
        let bg = preset.colors.bg.clone().unwrap_or_default();
        let fg = preset.colors.fg.clone().unwrap_or_default();
        let heading = preset.colors.heading.clone().unwrap_or_default();
        let title_fg = preset.colors.title_fg.clone().unwrap_or_default();
        let accent = preset.colors.accent.clone().unwrap_or_default();
        let link = preset.colors.link.clone().unwrap_or_default();
        let link_hover = preset.colors.link_hover.clone().unwrap_or_default();
        let muted = preset.colors.muted.clone().unwrap_or_default();
        let surface = preset.colors.surface.clone().unwrap_or_default();
        let border = preset.colors.border.clone().unwrap_or_default();
        let code_bg = preset.colors.code_bg.clone().unwrap_or_default();
        let code_fg = preset.colors.code_fg.clone().unwrap_or_default();
        let quote_bg = preset.colors.quote_bg.clone().unwrap_or_default();
        let quote_border = preset.colors.quote_border.clone().unwrap_or_default();
        let wide_bg = preset.colors.wide_bg.clone().unwrap_or_default();
        let nav_bg = preset.nav.bg.clone().unwrap_or_default();
        let nav_fg = preset.nav.fg.clone().unwrap_or_default();
        let nav_border = preset.nav.border.clone().unwrap_or_default();
        let wide_color = preset
            .wide_background
            .as_ref()
            .and_then(|wide| wide.color.clone())
            .unwrap_or_else(|| wide_bg.clone());
        let wide_opacity = preset
            .wide_background
            .as_ref()
            .and_then(|wide| wide.opacity)
            .map(f64::from);

        let mode_badge = if is_dark(&bg) { "dark" } else { "light" };

        let mut badges = Vec::new();
        badges.push(format!(r#"<span class="badge">{mode_badge}</span>"#));
        if !variant.is_empty() {
            badges.push(format!(r#"<span class="badge">variant: {variant}</span>"#));
        }

        if let Some(ratio) = contrast_ratio(&fg, &bg) {
            if ratio < 4.5 {
                badges.push(format!(
                    r#"<span class="badge warn">fg/bg {ratio:.2}</span>"#
                ));
            }
        }
        if let Some(ratio) = contrast_ratio(&link, &bg) {
            if ratio < 4.5 {
                badges.push(format!(
                    r#"<span class="badge warn">link/bg {ratio:.2}</span>"#
                ));
            }
        }
        if let Some(ratio) = contrast_ratio(&muted, &bg) {
            if ratio < 4.5 {
                badges.push(format!(
                    r#"<span class="badge warn">muted/bg {ratio:.2}</span>"#
                ));
            }
        }
        if let Some(ratio) = contrast_ratio(&code_fg, &code_bg) {
            if ratio < 4.5 {
                badges.push(format!(
                    r#"<span class="badge warn">code {ratio:.2}</span>"#
                ));
            }
        }

        let style = format!(
            "--c-bg:{};--c-fg:{};--c-heading:{};--c-accent:{};--c-link:{};--c-link-hover:{};--c-muted:{};--c-surface:{};--c-border:{};--c-code-bg:{};--c-code-fg:{};--c-quote-bg:{};--c-quote-border:{};--c-wide-bg:{};--c-nav-bg:{};--c-nav-fg:{};--c-nav-border:{};",
            bg,
            fg,
            heading,
            accent,
            link,
            link_hover,
            muted,
            surface,
            border,
            code_bg,
            code_fg,
            quote_bg,
            quote_border,
            wide_bg,
            nav_bg,
            nav_fg,
            nav_border
        );

        html.push_str(&format!(
            r##"<section class="theme-card" style="{style}">
  <div class="theme-header">
    <div class="theme-name">{title}</div>
    <div class="theme-meta">{}</div>
  </div>
  <div class="tokens">
    <div class="token-group">Colors</div>
    {}
    <div class="token-group">Nav</div>
    {}
    <div class="token-group">Wide background</div>
    {}
  </div>
  <div class="preview">
    <div class="nav-preview">
      <span>Home</span>
      <a href="#">Docs</a>
      <a href="#">Blog</a>
    </div>
    <div>
      <h2>Heading Sample</h2>
      <p>Body text uses <strong>foreground</strong> color and spacing.</p>
      <p class="muted">Muted text for metadata or hints.</p>
      <p><a href="#">Primary link</a> &middot; <a href="#">Secondary link</a></p>
    </div>
    <div class="panel-preview">
      <span class="chip">Accent</span>
      <div class="input-mock">Input / surface</div>
    </div>
    <div class="quote-preview">Quote block styling stays soft and readable.</div>
    <div class="code-preview">fn main() {{\n  println!(\"Hello\");\n}}</div>
    <div class="wide-preview" style="background: {};">Wide background</div>
  </div>
</section>
"##,
            badges.join(" "),
            token_rows(&[
                ("bg", &bg),
                ("fg", &fg),
                ("heading", &heading),
                ("title_fg", &title_fg),
                ("accent", &accent),
                ("link", &link),
                ("link_hover", &link_hover),
                ("muted", &muted),
                ("surface", &surface),
                ("border", &border),
                ("code_bg", &code_bg),
                ("code_fg", &code_fg),
                ("quote_bg", &quote_bg),
                ("quote_border", &quote_border),
                ("wide_bg", &wide_bg),
            ]),
            token_rows(&[
                ("nav.bg", &nav_bg),
                ("nav.fg", &nav_fg),
                ("nav.border", &nav_border),
            ]),
            token_rows(&[
                ("wide_background.color", &wide_color),
                (
                    "wide_background.opacity",
                    &wide_opacity
                        .map(|value| format!("{value:.2}"))
                        .unwrap_or_else(|| "(default)".to_string()),
                ),
            ]),
            wide_background_style(&wide_color, wide_opacity).unwrap_or_else(|| wide_color.clone())
        ));
    }

    html.push_str(
        r#"  </div>
</body>
</html>
"#,
    );

    Ok(html)
}

fn token_rows(entries: &[(&str, &String)]) -> String {
    let mut out = String::new();
    for (name, value) in entries {
        let display_value = if value.trim().is_empty() {
            "(default)".to_string()
        } else {
            value.to_string()
        };
        let swatch = if parse_hex_color(value).is_some() {
            format!(r#"style="background: {}""#, escape_html(value))
        } else {
            String::new()
        };
        out.push_str(&format!(
            r#"<div class="token-row">
  <div class="token-name">{}</div>
  <div class="token-value">{}</div>
  <div class="token-swatch" {}></div>
</div>
"#,
            escape_html(name),
            escape_html(&display_value),
            swatch
        ));
    }
    out
}

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

fn parse_hex_color(input: &str) -> Option<Rgb> {
    let value = input.trim().trim_start_matches('#');
    let expanded = match value.len() {
        6 => value.to_string(),
        3 => value
            .chars()
            .flat_map(|ch| std::iter::repeat(ch).take(2))
            .collect(),
        _ => return None,
    };
    let r = u8::from_str_radix(&expanded[0..2], 16).ok()?;
    let g = u8::from_str_radix(&expanded[2..4], 16).ok()?;
    let b = u8::from_str_radix(&expanded[4..6], 16).ok()?;
    Some(Rgb { r, g, b })
}

fn relative_luminance(rgb: Rgb) -> f64 {
    fn channel(value: u8) -> f64 {
        let v = value as f64 / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(rgb.r) + 0.7152 * channel(rgb.g) + 0.0722 * channel(rgb.b)
}

fn contrast_ratio(fg: &str, bg: &str) -> Option<f64> {
    let fg = parse_hex_color(fg)?;
    let bg = parse_hex_color(bg)?;
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (light, dark) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    Some((light + 0.05) / (dark + 0.05))
}

fn is_dark(color: &str) -> bool {
    parse_hex_color(color)
        .map(relative_luminance)
        .map(|lum| lum < 0.5)
        .unwrap_or(false)
}

fn wide_background_style(color: &str, opacity: Option<f64>) -> Option<String> {
    let opacity = opacity?;
    let rgb = parse_hex_color(color)?;
    Some(format!(
        "rgba({}, {}, {}, {:.2})",
        rgb.r, rgb.g, rgb.b, opacity
    ))
}

#[cfg(test)]
mod tests {
    use super::{contrast_ratio, is_dark, parse_hex_color};

    #[test]
    fn parse_hex_color_variants() {
        let rgb = parse_hex_color("#112233").expect("rgb");
        assert_eq!(rgb.r, 0x11);
        assert_eq!(rgb.g, 0x22);
        assert_eq!(rgb.b, 0x33);

        let rgb = parse_hex_color("abc").expect("rgb");
        assert_eq!(rgb.r, 0xaa);
        assert_eq!(rgb.g, 0xbb);
        assert_eq!(rgb.b, 0xcc);
    }

    #[test]
    fn contrast_ratio_is_symmetric() {
        let a = contrast_ratio("#000000", "#ffffff").expect("ratio");
        let b = contrast_ratio("#ffffff", "#000000").expect("ratio");
        assert!((a - b).abs() < 0.0001);
    }

    #[test]
    fn dark_detection() {
        assert!(is_dark("#000000"));
        assert!(!is_dark("#ffffff"));
    }
}
