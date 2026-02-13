use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use crate::color_presets;
use stbl_embedded_assets as embedded;

const CONFIG_TEMPLATE: &str = include_str!("../assets/stbl.template.yaml");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitKind {
    Blog,
    LandingPage,
}

#[derive(Debug, Clone)]
pub struct InitOptions {
    pub title: String,
    pub base_url: String,
    pub language: String,
    pub kind: InitKind,
    pub color_theme: Option<String>,
    pub copy_all: bool,
    pub target_dir: PathBuf,
}

pub fn init_site(options: InitOptions) -> Result<()> {
    let target_dir = options.target_dir;
    if !target_dir.exists() {
        fs::create_dir_all(&target_dir)
            .with_context(|| format!("failed to create {}", target_dir.display()))?;
    }

    if let Some(blocking) = first_blocking_path(&target_dir) {
        bail!("init aborted: {} already exists", blocking.display());
    }

    create_required_dirs(&target_dir, options.copy_all)?;

    let site_id = slugify_title(&options.title);
    let base_url = normalize_base_url(&options.base_url);
    let config = render_config(
        &site_id,
        &options.title,
        &base_url,
        &options.language,
        options.kind,
        options.color_theme.as_deref(),
    )?;
    let config_path = target_dir.join("stbl.yaml");
    fs::write(&config_path, config)
        .with_context(|| format!("failed to write {}", config_path.display()))?;

    write_article(
        &target_dir.join("articles/index.md"),
        "Home",
        match options.kind {
            InitKind::Blog => "blog_index",
            InitKind::LandingPage => "info",
        },
        "Welcome to your new site.\n",
    )?;
    write_article(
        &target_dir.join("articles/about.md"),
        "About",
        "info",
        "Write something about yourself and the site.\n",
    )?;
    write_article(
        &target_dir.join("articles/contact.md"),
        "Contact",
        "info",
        "How should people reach you?\n",
    )?;

    write_assets_readme(&target_dir.join("assets/README.md"))?;
    if options.copy_all {
        copy_embedded_assets(&target_dir.join("assets"))?;
    }

    Ok(())
}

fn create_required_dirs(target_dir: &Path, copy_all: bool) -> Result<()> {
    let mut dirs = vec![
        target_dir.join("articles"),
        target_dir.join("artifacts"),
        target_dir.join("images"),
        target_dir.join("assets"),
        target_dir.join("video"),
    ];
    if copy_all {
        dirs.push(target_dir.join("assets/css"));
    }
    for dir in dirs {
        fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create {}", dir.display()))?;
    }
    Ok(())
}

fn first_blocking_path(target_dir: &Path) -> Option<PathBuf> {
    let checks = [
        target_dir.join("stbl.yaml"),
        target_dir.join("articles"),
        target_dir.join("artifacts"),
        target_dir.join("images"),
        target_dir.join("assets"),
        target_dir.join("video"),
    ];
    checks.into_iter().find(|path| path.exists())
}

fn render_config(
    site_id: &str,
    title: &str,
    base_url: &str,
    language: &str,
    kind: InitKind,
    color_theme: Option<&str>,
) -> Result<String> {
    let title = yaml_string(title);
    let language = yaml_string(language);
    let base_url = yaml_string(base_url);
    let site_id = yaml_string(site_id);
    let mut out = CONFIG_TEMPLATE
        .replace("{{SITE_ID}}", &site_id)
        .replace("{{TITLE}}", &title)
        .replace("{{BASE_URL}}", &base_url)
        .replace("{{LANG}}", &language);

    let theme_block = match color_theme {
        Some(theme) => render_theme_preset_block(theme)?,
        None => render_commented_theme_block(),
    };
    out = out.replace("{{THEME_COLORS_BLOCK}}", &theme_block);

    let blog_block = match kind {
        InitKind::Blog => render_blog_pagination_block(),
        InitKind::LandingPage => render_commented_blog_pagination_block(),
    };
    out = out.replace("{{BLOG_PAGINATION_BLOCK}}", &blog_block);

    Ok(out)
}

fn render_theme_preset_block(theme: &str) -> Result<String> {
    let presets = color_presets::load_color_presets()?;
    let preset = presets.get(theme).ok_or_else(|| {
        anyhow!("unknown color preset '{theme}' (use apply-colors --list-presets)")
    })?;
    color_presets::validate_preset(theme, preset)?;

    let mut out = String::new();
    out.push_str("  colors:\n");
    push_color(&mut out, "bg", &preset.colors.bg, false);
    push_color(&mut out, "fg", &preset.colors.fg, false);
    push_color(&mut out, "heading", &preset.colors.heading, false);
    push_color(&mut out, "title_fg", &preset.colors.title_fg, true);
    push_color(&mut out, "accent", &preset.colors.accent, false);
    push_color(&mut out, "link", &preset.colors.link, false);
    push_color(&mut out, "muted", &preset.colors.muted, false);
    push_color(&mut out, "surface", &preset.colors.surface, false);
    push_color(&mut out, "border", &preset.colors.border, false);
    push_color(&mut out, "link_hover", &preset.colors.link_hover, false);
    push_color(&mut out, "code_bg", &preset.colors.code_bg, false);
    push_color(&mut out, "code_fg", &preset.colors.code_fg, false);
    push_color(&mut out, "quote_bg", &preset.colors.quote_bg, false);
    push_color(&mut out, "quote_border", &preset.colors.quote_border, false);
    push_color(&mut out, "wide_bg", &preset.colors.wide_bg, false);

    out.push_str("  nav:\n");
    push_color(&mut out, "bg", &preset.nav.bg, false);
    push_color(&mut out, "fg", &preset.nav.fg, false);
    push_color(&mut out, "border", &preset.nav.border, false);

    if let Some(wide) = preset.wide_background.as_ref() {
        out.push_str("  wide_background:\n");
        push_color(&mut out, "color", &wide.color, false);
        push_color(&mut out, "image", &wide.image, false);
        if let Some(style) = wide.style {
            let style = match style {
                stbl_core::model::WideBackgroundStyle::Cover => "cover",
                stbl_core::model::WideBackgroundStyle::Tile => "tile",
            };
            out.push_str(&format!("    style: \"{style}\"\n"));
        }
        if let Some(position) = wide.position.as_ref() {
            let position = yaml_string(position);
            out.push_str(&format!("    position: \"{position}\"\n"));
        }
        if let Some(opacity) = wide.opacity {
            out.push_str(&format!("    opacity: {opacity}\n"));
        }
    }

    let theme = yaml_string(theme);
    out.push_str("  color_scheme:\n");
    out.push_str(&format!("    name: \"{theme}\"\n"));
    out.push_str("    mode: auto\n");
    out.push_str("    source: preset\n");
    out.push_str("    # base:\n");
    out.push_str("    #   bg: \"#ffffff\"\n");
    out.push_str("    #   fg: \"#111111\"\n");
    out.push_str("    #   accent: \"#ff3366\"\n");
    out.push_str("    #   link: \"#0033cc\"\n");
    out.push_str("    #   heading: \"#111111\"\n");

    Ok(out)
}

fn render_commented_theme_block() -> String {
    let mut out = String::new();
    out.push_str("  # colors:\n");
    out.push_str("  #   bg: \"#ffffff\"\n");
    out.push_str("  #   fg: \"#111111\"\n");
    out.push_str("  #   heading: \"#111111\"\n");
    out.push_str("  #   title_fg: \"#111111\"\n");
    out.push_str("  #   accent: \"#ff3366\"\n");
    out.push_str("  #   link: \"#0033cc\"\n");
    out.push_str("  #   muted: \"#666666\"\n");
    out.push_str("  #   surface: \"#f5f5f5\"\n");
    out.push_str("  #   border: \"#dddddd\"\n");
    out.push_str("  #   link_hover: \"#002299\"\n");
    out.push_str("  #   code_bg: \"#f1f1f1\"\n");
    out.push_str("  #   code_fg: \"#111111\"\n");
    out.push_str("  #   quote_bg: \"#f7f7f7\"\n");
    out.push_str("  #   quote_border: \"#dddddd\"\n");
    out.push_str("  #   wide_bg: \"#fafafa\"\n");
    out.push_str("  # nav:\n");
    out.push_str("  #   bg: \"#ffffff\"\n");
    out.push_str("  #   fg: \"#111111\"\n");
    out.push_str("  #   border: \"#dddddd\"\n");
    out
}

fn render_blog_pagination_block() -> String {
    let mut out = String::new();
    out.push_str("  pagination:\n");
    out.push_str("    enabled: true\n");
    out.push_str("    page_size: 10\n");
    out
}

fn render_commented_blog_pagination_block() -> String {
    let mut out = String::new();
    out.push_str("  # pagination:\n");
    out.push_str("  #   enabled: false\n");
    out.push_str("  #   page_size: 10\n");
    out
}

fn push_color(out: &mut String, key: &str, value: &Option<String>, comment_if_missing: bool) {
    match value.as_ref() {
        Some(value) => {
            let value = yaml_string(value);
            out.push_str(&format!("    {key}: \"{value}\"\n"));
        }
        None if comment_if_missing => {
            out.push_str(&format!("    # {key}: \"#ffffff\"\n"));
        }
        None => {}
    }
}

fn write_article(path: &Path, title: &str, template: &str, body: &str) -> Result<()> {
    let contents = format!(
        "title: {}\n\
template: {}\n\
\n\
{}",
        title, template, body
    );
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn write_assets_readme(path: &Path) -> Result<()> {
    let contents = "\
# Assets\n\
\n\
This folder holds site-specific asset overrides. By default, stbl ships a minimal\n\
theme inside the binary. Only assets placed here override the embedded defaults.\n\
\n\
To see the effective assets, inspect the build output directory (the `artifacts/`\n\
folder inside `out/`).\n";
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn copy_embedded_assets(assets_root: &Path) -> Result<()> {
    let template = embedded::template("default")
        .ok_or_else(|| anyhow!("embedded assets template 'default' not found"))?;
    for asset in template.assets {
        if !asset.path.starts_with("css/") {
            continue;
        }
        let bytes = embedded::decompress_to_vec(&asset.hash)
            .ok_or_else(|| anyhow!("failed to decompress embedded asset {}", asset.path))?;
        let out_path = assets_root.join(asset.path);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&out_path, bytes)
            .with_context(|| format!("failed to write {}", out_path.display()))?;
    }
    Ok(())
}

fn normalize_base_url(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "http://localhost:8080/".to_string();
    }
    if trimmed.ends_with('/') {
        trimmed.to_string()
    } else {
        format!("{trimmed}/")
    }
}

fn slugify_title(value: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else {
            if !prev_dash {
                out.push('-');
                prev_dash = true;
            }
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "site".to_string()
    } else {
        trimmed
    }
}

fn yaml_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', " ")
}
