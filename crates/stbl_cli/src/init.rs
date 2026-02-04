use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use stbl_embedded_assets as embedded;

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
    );
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
) -> String {
    let title = yaml_string(title);
    let language = yaml_string(language);
    let base_url = yaml_string(base_url);
    let site_id = yaml_string(site_id);
    let mut out = String::new();
    out.push_str("site:\n");
    out.push_str(&format!("  id: \"{site_id}\"\n"));
    out.push_str(&format!("  title: \"{title}\"\n"));
    out.push_str(&format!("  base_url: \"{base_url}\"\n"));
    out.push_str(&format!("  language: \"{language}\"\n"));
    out.push_str("  url_style: html\n");
    out.push_str("menu:\n");
    out.push_str("  - title: \"Home\"\n");
    out.push_str("    href: \"./\"\n");
    out.push_str("  - title: \"About\"\n");
    out.push_str("    href: \"./about.html\"\n");
    out.push_str("  - title: \"Contact\"\n");
    out.push_str("    href: \"./contact.html\"\n");
    if kind == InitKind::Blog {
        out.push_str("blog:\n");
        out.push_str("  pagination:\n");
        out.push_str("    enabled: true\n");
        out.push_str("    page_size: 10\n");
    }
    out
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
