use anyhow::{Context, Result, anyhow, bail};
use blake3;
use stbl_core::assets::{AssetIndex, AssetRelPath, AssetSourceId, ResolvedAsset};
use stbl_core::model::{BuildTask, SecurityConfig, SiteConfig, SvgSecurityMode, TaskKind};
use stbl_embedded_assets as embedded;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;
use xmltree::{Element, XMLNode};
const DEFAULT_THEME_VARIANT: &str = "stbl";

#[derive(Debug, Default, Clone)]
pub struct AssetSourceLookup {
    sources: BTreeMap<AssetSourceId, AssetSource>,
}

impl AssetSourceLookup {
    pub fn resolve(&self, source: &AssetSourceId) -> Option<&AssetSource> {
        self.sources.get(source)
    }
}

#[derive(Debug, Clone)]
pub enum AssetSource {
    File(PathBuf),
    Embedded(Vec<u8>),
}

#[allow(dead_code)]
pub fn embedded_default_template() -> Result<&'static embedded::Template> {
    embedded::template(DEFAULT_THEME_VARIANT).ok_or_else(|| {
        anyhow!(
            "embedded assets template '{}' not found",
            DEFAULT_THEME_VARIANT
        )
    })
}

pub fn iter_embedded_assets(template: &embedded::Template) -> Result<Vec<(String, Vec<u8>)>> {
    let mut assets = Vec::new();
    for entry in template.assets {
        let bytes = embedded::decompress_to_vec(&entry.hash)
            .ok_or_else(|| anyhow!("failed to decompress embedded asset {}", entry.path))?;
        assets.push((entry.path.to_string(), bytes));
    }
    Ok(assets)
}

#[allow(dead_code)]
pub fn discover_assets(site_root: &Path) -> Result<(AssetIndex, AssetSourceLookup)> {
    if site_root.join("stbl.yaml").is_file() {
        return discover_assets_for_theme(site_root, DEFAULT_THEME_VARIANT);
    }
    discover_assets_legacy(site_root, DEFAULT_THEME_VARIANT)
}

pub fn discover_assets_for_theme(
    project_root: &Path,
    theme_variant: &str,
) -> Result<(AssetIndex, AssetSourceLookup)> {
    let theme_variant = normalize_theme_variant(theme_variant);
    let mut resolved: BTreeMap<AssetRelPath, (AssetSourceId, AssetSource, String)> =
        BTreeMap::new();
    overlay_embedded_theme_assets(DEFAULT_THEME_VARIANT, &mut resolved, true)?;
    if theme_variant != DEFAULT_THEME_VARIANT {
        overlay_embedded_theme_assets(theme_variant, &mut resolved, false)?;
    }

    let stbl_root = project_root.join("stbl");
    collect_site_assets_mapped(
        &stbl_root.join("templates").join(theme_variant),
        "templates",
        &mut resolved,
    )?;
    collect_site_assets_mapped(
        &stbl_root.join("css").join(theme_variant),
        "css",
        &mut resolved,
    )?;
    collect_site_assets_mapped(
        &stbl_root.join("assets").join(theme_variant),
        "",
        &mut resolved,
    )?;
    collect_site_assets_mapped(&project_root.join("assets"), "", &mut resolved)?;

    let mut sources = BTreeMap::new();
    let assets = resolved
        .into_iter()
        .map(|(rel, (source, asset_source, content_hash))| {
            sources.insert(source.clone(), asset_source);
            ResolvedAsset {
                rel,
                source,
                content_hash,
            }
        })
        .collect::<Vec<_>>();

    Ok((AssetIndex { assets }, AssetSourceLookup { sources }))
}

#[allow(dead_code)]
fn discover_assets_legacy(
    site_root: &Path,
    theme_variant: &str,
) -> Result<(AssetIndex, AssetSourceLookup)> {
    let theme_variant = normalize_theme_variant(theme_variant);
    let mut resolved: BTreeMap<AssetRelPath, (AssetSourceId, AssetSource, String)> =
        BTreeMap::new();
    overlay_embedded_theme_assets(DEFAULT_THEME_VARIANT, &mut resolved, true)?;
    if theme_variant != DEFAULT_THEME_VARIANT {
        overlay_embedded_theme_assets(theme_variant, &mut resolved, false)?;
    }

    collect_site_assets_mapped(site_root, "", &mut resolved)?;

    let mut sources = BTreeMap::new();
    let assets = resolved
        .into_iter()
        .map(|(rel, (source, asset_source, content_hash))| {
            sources.insert(source.clone(), asset_source);
            ResolvedAsset {
                rel,
                source,
                content_hash,
            }
        })
        .collect::<Vec<_>>();

    Ok((AssetIndex { assets }, AssetSourceLookup { sources }))
}

fn overlay_embedded_theme_assets(
    theme_variant: &str,
    resolved: &mut BTreeMap<AssetRelPath, (AssetSourceId, AssetSource, String)>,
    required: bool,
) -> Result<()> {
    let template = match embedded::template(theme_variant) {
        Some(template) => template,
        None if required => {
            bail!("embedded assets template '{}' not found", theme_variant);
        }
        None => return Ok(()),
    };

    for (rel_path, bytes) in iter_embedded_assets(template)? {
        let rel = normalize_rel_path(Path::new(&rel_path))?;
        if rel.0 == "css/vars.css" {
            continue;
        }
        let content_hash = blake3::hash(&bytes).to_hex().to_string();
        let source_id = AssetSourceId(format!("embedded:{content_hash}"));
        resolved.insert(rel, (source_id, AssetSource::Embedded(bytes), content_hash));
    }
    Ok(())
}

fn normalize_theme_variant(theme_variant: &str) -> &str {
    let trimmed = theme_variant.trim();
    if trimmed.is_empty() || trimmed == "default" {
        DEFAULT_THEME_VARIANT
    } else {
        trimmed
    }
}

pub struct LogoAsset {
    pub rel: AssetRelPath,
    pub path: PathBuf,
}

pub fn resolve_site_logo(root: &Path, raw: &str) -> Result<LogoAsset> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        bail!("site.logo must not be empty");
    }
    if Path::new(trimmed).is_absolute() || trimmed.starts_with('/') {
        bail!("site.logo must be a relative path: {trimmed}");
    }
    let cleaned = trimmed.strip_prefix("./").unwrap_or(trimmed);
    let cleaned = cleaned.trim_end_matches('/');
    if cleaned.is_empty() {
        bail!("site.logo must not be empty");
    }
    let rel = normalize_rel_path(Path::new(cleaned))?;

    let mut candidates = Vec::new();
    if let Some(stripped) = cleaned.strip_prefix("assets/") {
        candidates.push(root.join("assets").join(stripped));
    } else {
        candidates.push(root.join(cleaned));
        candidates.push(root.join("assets").join(cleaned));
    }

    for candidate in candidates {
        if candidate.is_file() {
            return Ok(LogoAsset {
                rel,
                path: candidate,
            });
        }
    }

    bail!("site.logo not found: {}", cleaned);
}

pub fn include_site_logo(
    root: &Path,
    config: &SiteConfig,
    asset_index: &mut AssetIndex,
    lookup: &mut AssetSourceLookup,
) -> Result<()> {
    let Some(raw) = config.site.logo.as_deref() else {
        return Ok(());
    };
    let logo = resolve_site_logo(root, raw)?;
    add_file_asset(&logo.rel, &logo.path, asset_index, lookup)
}

#[allow(dead_code)]
pub fn execute_copy_tasks(
    tasks: &[BuildTask],
    out_dir: &Path,
    lookup: &AssetSourceLookup,
    security: &SecurityConfig,
) -> Result<()> {
    for task in tasks {
        if let TaskKind::CopyAsset {
            source, out_rel, ..
        } = &task.kind
        {
            copy_asset_to_out(out_dir, out_rel, source, lookup, security)?;
        }
    }
    Ok(())
}

pub fn copy_asset_to_out(
    out_dir: &Path,
    out_rel: &str,
    source: &AssetSourceId,
    lookup: &AssetSourceLookup,
    security: &SecurityConfig,
) -> Result<()> {
    let out_path = out_dir.join(out_rel);
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let bytes = match lookup
        .resolve(source)
        .ok_or_else(|| anyhow!("unknown asset source {}", source.0))?
    {
        AssetSource::File(src_path) => std::fs::read(src_path).with_context(|| {
            format!(
                "failed to read asset {} for copy to {}",
                src_path.display(),
                out_path.display()
            )
        })?,
        AssetSource::Embedded(bytes) => bytes.clone(),
    };
    let is_svg = out_rel.to_ascii_lowercase().ends_with(".svg");
    if is_svg {
        write_svg_asset(&out_path, out_rel, &bytes, security)?;
    } else {
        std::fs::write(&out_path, &bytes)
            .with_context(|| format!("failed to write {}", out_path.display()))?;
    }
    Ok(())
}

fn write_svg_asset(
    out_path: &Path,
    rel: &str,
    bytes: &[u8],
    security: &SecurityConfig,
) -> Result<()> {
    match security.svg.mode {
        SvgSecurityMode::Off => {
            std::fs::write(out_path, bytes)
                .with_context(|| format!("failed to write {}", out_path.display()))?;
            return Ok(());
        }
        _ => {}
    }
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) => match security.svg.mode {
            SvgSecurityMode::Fail => {
                bail!("svg security: {rel}: invalid UTF-8: {err}");
            }
            SvgSecurityMode::Warn | SvgSecurityMode::Sanitize => {
                eprintln!("warning: svg security: {rel}: invalid UTF-8: {err}");
                std::fs::write(out_path, bytes)
                    .with_context(|| format!("failed to write {}", out_path.display()))?;
                return Ok(());
            }
            SvgSecurityMode::Off => unreachable!(),
        },
    };
    let scan = match scan_svg(text) {
        Ok(scan) => scan,
        Err(err) => match security.svg.mode {
            SvgSecurityMode::Fail => {
                bail!("svg security: {rel}: failed to scan: {err}");
            }
            SvgSecurityMode::Warn | SvgSecurityMode::Sanitize => {
                eprintln!("warning: svg security: {rel}: failed to scan: {err}");
                std::fs::write(out_path, bytes)
                    .with_context(|| format!("failed to write {}", out_path.display()))?;
                return Ok(());
            }
            SvgSecurityMode::Off => unreachable!(),
        },
    };
    if scan.issues.is_empty() {
        std::fs::write(out_path, bytes)
            .with_context(|| format!("failed to write {}", out_path.display()))?;
        return Ok(());
    }
    match security.svg.mode {
        SvgSecurityMode::Warn => {
            eprintln!(
                "warning: svg security: {}: {}",
                rel,
                format_svg_issues(&scan)
            );
            std::fs::write(out_path, bytes)
                .with_context(|| format!("failed to write {}", out_path.display()))?;
        }
        SvgSecurityMode::Fail => {
            bail!("svg security: {rel}: {}", format_svg_issues(&scan));
        }
        SvgSecurityMode::Sanitize => {
            let sanitized = sanitize_svg_text(text)?;
            std::fs::write(out_path, sanitized)
                .with_context(|| format!("failed to write {}", out_path.display()))?;
        }
        SvgSecurityMode::Off => {}
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SvgIssueKind {
    ScriptElement,
    ForeignObjectElement,
    UnsafeHref,
    UnsafeUrl,
    StyleImport,
    StyleUrl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SvgIssue {
    kind: SvgIssueKind,
    detail: String,
}

#[derive(Debug, Default, Clone)]
struct SvgScanResult {
    issues: Vec<SvgIssue>,
}

fn scan_svg(text: &str) -> Result<SvgScanResult> {
    let doc = roxmltree::Document::parse(text).map_err(|err| anyhow!("invalid SVG XML: {err}"))?;
    let mut result = SvgScanResult::default();
    for node in doc.descendants().filter(|node| node.is_element()) {
        let name = node.tag_name().name();
        if name.eq_ignore_ascii_case("script") {
            result.issues.push(SvgIssue {
                kind: SvgIssueKind::ScriptElement,
                detail: "script element".to_string(),
            });
        }
        if name.eq_ignore_ascii_case("foreignObject") {
            result.issues.push(SvgIssue {
                kind: SvgIssueKind::ForeignObjectElement,
                detail: "foreignObject element".to_string(),
            });
        }
        for attr in node.attributes() {
            let value = attr.value();
            if is_href_attr(attr.name()) && is_unsafe_href_value(value) {
                result.issues.push(SvgIssue {
                    kind: SvgIssueKind::UnsafeHref,
                    detail: format!("unsafe href: {}", value.trim()),
                });
            }
            if has_unsafe_url_func(value) {
                result.issues.push(SvgIssue {
                    kind: SvgIssueKind::UnsafeUrl,
                    detail: format!("unsafe url(): {}", value.trim()),
                });
            }
        }
        if name.eq_ignore_ascii_case("style") {
            let mut css_text = String::new();
            for child in node.children().filter(|child| child.is_text()) {
                if let Some(text) = child.text() {
                    css_text.push_str(text);
                }
            }
            let css_lower = css_text.to_ascii_lowercase();
            if css_lower.contains("@import") {
                result.issues.push(SvgIssue {
                    kind: SvgIssueKind::StyleImport,
                    detail: "style @import".to_string(),
                });
            }
            if has_unsafe_url_func(&css_text) {
                result.issues.push(SvgIssue {
                    kind: SvgIssueKind::StyleUrl,
                    detail: "style url()".to_string(),
                });
            }
        }
    }
    Ok(result)
}

fn sanitize_svg_text(text: &str) -> Result<String> {
    let mut element =
        Element::parse(text.as_bytes()).map_err(|err| anyhow!("invalid SVG XML: {err}"))?;
    sanitize_element(&mut element);
    let mut out = Vec::new();
    element
        .write(&mut out)
        .map_err(|err| anyhow!("failed to write sanitized SVG: {err}"))?;
    let sanitized =
        String::from_utf8(out).map_err(|err| anyhow!("sanitized SVG is not UTF-8: {err}"))?;
    Ok(sanitized)
}

fn sanitize_element(element: &mut Element) {
    element.attributes.retain(|key, value| {
        if is_href_attr(key) && is_unsafe_href_value(value) {
            return false;
        }
        !has_unsafe_url_func(value)
    });

    let mut new_children = Vec::new();
    for child in element.children.drain(..) {
        match child {
            XMLNode::Element(mut child_elem) => {
                let name = child_elem.name.clone();
                if name.eq_ignore_ascii_case("script") || name.eq_ignore_ascii_case("foreignObject")
                {
                    continue;
                }
                if name.eq_ignore_ascii_case("style") {
                    sanitize_style_element(&mut child_elem);
                }
                sanitize_element(&mut child_elem);
                new_children.push(XMLNode::Element(child_elem));
            }
            XMLNode::Text(text) => new_children.push(XMLNode::Text(text)),
            other => new_children.push(other),
        }
    }
    element.children = new_children;
}

fn sanitize_style_element(element: &mut Element) {
    let mut new_children = Vec::new();
    for child in element.children.drain(..) {
        match child {
            XMLNode::Text(text) => {
                let cleaned = sanitize_css_text(&text);
                if !cleaned.trim().is_empty() {
                    new_children.push(XMLNode::Text(cleaned));
                }
            }
            other => new_children.push(other),
        }
    }
    element.children = new_children;
}

fn sanitize_css_text(text: &str) -> String {
    let mut cleaned_lines = Vec::new();
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("@import") {
            continue;
        }
        if has_unsafe_url_func(line) {
            continue;
        }
        cleaned_lines.push(line);
    }
    if cleaned_lines.is_empty() {
        String::new()
    } else {
        cleaned_lines.join("\n")
    }
}

fn is_href_attr(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower == "href" || lower.ends_with(":href")
}

fn is_unsafe_href_value(value: &str) -> bool {
    let trimmed = value.trim().to_ascii_lowercase();
    trimmed.starts_with("http:")
        || trimmed.starts_with("https:")
        || trimmed.starts_with("//")
        || trimmed.starts_with("data:")
}

fn has_unsafe_url_func(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(idx) = rest.find("url(") {
        let after = &rest[idx + 4..];
        let end = after.find(')');
        let Some(end) = end else {
            return true;
        };
        let mut inner = after[..end].trim();
        inner = inner.trim_matches('"').trim_matches('\'').trim();
        if !inner.starts_with('#') {
            return true;
        }
        rest = &after[end + 1..];
    }
    false
}

fn format_svg_issues(scan: &SvgScanResult) -> String {
    let mut parts = Vec::new();
    for issue in &scan.issues {
        parts.push(issue.detail.clone());
    }
    parts.join(", ")
}

fn add_file_asset(
    rel: &AssetRelPath,
    path: &Path,
    asset_index: &mut AssetIndex,
    lookup: &mut AssetSourceLookup,
) -> Result<()> {
    if asset_index.assets.iter().any(|asset| asset.rel == *rel) {
        return Ok(());
    }
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    let content_hash = blake3::hash(&bytes).to_hex().to_string();
    let source = AssetSourceId(path.to_string_lossy().to_string());
    lookup
        .sources
        .insert(source.clone(), AssetSource::File(path.to_path_buf()));
    asset_index.assets.push(ResolvedAsset {
        rel: rel.clone(),
        source,
        content_hash,
    });
    Ok(())
}

fn collect_site_assets_mapped(
    root: &Path,
    out_prefix: &str,
    out: &mut BTreeMap<AssetRelPath, (AssetSourceId, AssetSource, String)>,
) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(root).with_context(|| {
            format!(
                "failed to read asset relative path for {}",
                entry.path().display()
            )
        })?;
        let mut mapped = PathBuf::new();
        if !out_prefix.is_empty() {
            mapped.push(out_prefix);
        }
        mapped.push(rel);
        let rel = normalize_rel_path(&mapped)?;
        if rel.0 == "README.md" {
            continue;
        }
        if rel.0 == "css/vars.css" {
            continue;
        }
        let source = AssetSourceId(entry.path().to_string_lossy().to_string());
        let bytes = std::fs::read(entry.path())
            .with_context(|| format!("failed to read {}", entry.path().display()))?;
        let content_hash = blake3::hash(&bytes).to_hex().to_string();
        out.insert(
            rel,
            (
                source,
                AssetSource::File(entry.path().to_path_buf()),
                content_hash,
            ),
        );
    }
    Ok(())
}

fn normalize_rel_path(path: &Path) -> Result<AssetRelPath> {
    let raw = path.to_string_lossy().replace('\\', "/");
    let rel = Path::new(&raw);
    if rel.is_absolute() {
        bail!("asset rel path must be relative: {}", raw);
    }
    for comp in rel.components() {
        match comp {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                bail!("asset rel path must not contain parent/root: {}", raw);
            }
            _ => {}
        }
    }
    if raw.is_empty() {
        bail!("asset rel path must not be empty");
    }
    Ok(AssetRelPath(raw))
}

#[cfg(test)]
mod tests {
    use super::{SvgIssueKind, sanitize_svg_text, scan_svg};

    #[test]
    fn scan_svg_reports_unsafe_features() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
  <script>alert(1)</script>
  <foreignObject><div>hi</div></foreignObject>
  <image href="https://example.com/x.png" />
  <rect style="filter:url(http://evil)" />
  <style>@import url(http://evil); .a{fill:red;}</style>
</svg>"#;
        let result = scan_svg(svg).expect("scan svg");
        let kinds = result
            .issues
            .iter()
            .map(|issue| issue.kind.clone())
            .collect::<Vec<_>>();
        assert!(kinds.contains(&SvgIssueKind::ScriptElement));
        assert!(kinds.contains(&SvgIssueKind::ForeignObjectElement));
        assert!(kinds.contains(&SvgIssueKind::UnsafeHref));
        assert!(kinds.contains(&SvgIssueKind::UnsafeUrl));
        assert!(kinds.contains(&SvgIssueKind::StyleImport));
        assert!(kinds.contains(&SvgIssueKind::StyleUrl));
    }

    #[test]
    fn sanitize_svg_removes_unsafe_features() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg">
  <script>alert(1)</script>
  <foreignObject><div>hi</div></foreignObject>
  <image href="https://example.com/x.png" />
  <rect style="filter:url(http://evil)" />
  <style>@import url(http://evil); .a{fill:red;}</style>
  <rect fill="url(#safe)" />
</svg>"#;
        let sanitized = sanitize_svg_text(svg).expect("sanitize svg");
        let result = scan_svg(&sanitized).expect("scan sanitized");
        assert!(
            result.issues.is_empty(),
            "issues after sanitize: {:?}",
            result.issues
        );
        assert!(sanitized.contains("url(#safe)"));
    }
}
