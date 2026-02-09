use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use minijinja::{AutoEscape, Environment, context};

use crate::assets::AssetManifest;
use crate::model::{MenuAlign, NavItem, Page, Project, ThemeHeaderLayout};
use crate::macros::IncludeProvider;
use crate::render::{RenderOptions, render_markdown_to_html_with_media};
use crate::visibility::is_blog_index_excluded;
use crate::url::UrlMapper;
use serde::Serialize;

const BASE_TEMPLATE: &str = include_str!("templates/base.html");
const PAGE_TEMPLATE: &str = include_str!("templates/page.html");
const BLOG_INDEX_TEMPLATE: &str = include_str!("templates/blog_index.html");
const TAG_INDEX_TEMPLATE: &str = include_str!("templates/tag_index.html");
const SERIES_INDEX_TEMPLATE: &str = include_str!("templates/series_index.html");
const LIST_ITEM_TEMPLATE: &str = include_str!("templates/partials/list_item.html");

pub fn templates_hash() -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"stbl2.templates.v2");
    add_template_hash(&mut hasher, "base", BASE_TEMPLATE);
    add_template_hash(&mut hasher, "page", PAGE_TEMPLATE);
    add_template_hash(&mut hasher, "blog_index", BLOG_INDEX_TEMPLATE);
    add_template_hash(&mut hasher, "tag_index", TAG_INDEX_TEMPLATE);
    add_template_hash(&mut hasher, "series_index", SERIES_INDEX_TEMPLATE);
    add_template_hash(&mut hasher, "list_item", LIST_ITEM_TEMPLATE);
    *hasher.finalize().as_bytes()
}

fn add_template_hash(hasher: &mut blake3::Hasher, name: &str, contents: &str) {
    add_str(hasher, name);
    add_str(hasher, contents);
}

fn add_str(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

pub fn render_page(
    project: &Project,
    page: &Page,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    include_provider: Option<&dyn IncludeProvider>,
) -> Result<String> {
    render_page_with_series_nav(
        project,
        page,
        asset_manifest,
        None,
        current_href,
        build_date_ymd,
        include_provider,
    )
}

pub fn render_page_with_series_nav(
    project: &Project,
    page: &Page,
    asset_manifest: &AssetManifest,
    series_nav: Option<SeriesNavView>,
    current_href: &str,
    build_date_ymd: &str,
    include_provider: Option<&dyn IncludeProvider>,
) -> Result<String> {
    let rel = rel_prefix_for_href(current_href);
    let body_html = render_markdown_with_media(
        project,
        Some(page),
        &page.body_markdown,
        &rel,
        include_provider,
    );
    let banner_html = render_banner_html(project, page, &rel);
    let page_title = page_title_or_filename(project, page);
    let show_page_title = !is_blog_index_excluded(page, None);
    let authors = page.header.authors.clone();
    let tags = page.header.tags.clone();
    let published = format_timestamp_rfc3339(page.header.published);
    let updated = format_timestamp_rfc3339(page.header.updated);

    render_with_context(
        project,
        page_title,
        show_page_title,
        authors,
        published,
        updated,
        tags,
        body_html,
        banner_html,
        asset_manifest,
        current_href,
        build_date_ymd,
        series_nav,
        None,
    )
}

pub fn render_markdown_page(
    project: &Project,
    title: &str,
    body_markdown: &str,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    redirect_href: Option<&str>,
    show_page_title: bool,
    include_provider: Option<&dyn IncludeProvider>,
) -> Result<String> {
    let rel = rel_prefix_for_href(current_href);
    let body_html = render_markdown_with_media(
        project,
        None,
        body_markdown,
        &rel,
        include_provider,
    );
    render_with_context(
        project,
        title.to_string(),
        show_page_title,
        None,
        None,
        None,
        Vec::new(),
        body_html,
        None,
        asset_manifest,
        current_href,
        build_date_ymd,
        None,
        redirect_href.map(|value| value.to_string()),
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct BlogIndexPart {
    pub title: String,
    pub href: String,
    pub published_display: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagLink {
    pub label: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlogIndexItem {
    pub title: String,
    pub href: String,
    pub published_display: Option<String>,
    pub updated_display: Option<String>,
    pub kind_label: Option<String>,
    pub abstract_text: Option<String>,
    pub tags: Vec<TagLink>,
    pub latest_parts: Vec<BlogIndexPart>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeriesIndexPart {
    pub title: String,
    pub href: String,
    pub published_display: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeriesNavLink {
    pub title: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeriesNavView {
    pub prev: Option<SeriesNavLink>,
    pub index: SeriesNavLink,
    pub next: Option<SeriesNavLink>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagListingPage {
    pub tag: String,
    pub items: Vec<BlogIndexItem>,
}

pub fn render_blog_index(
    project: &Project,
    title: String,
    intro_html: Option<String>,
    banner_html: Option<String>,
    items: Vec<BlogIndexItem>,
    prev_href: Option<String>,
    next_href: Option<String>,
    page_no: u32,
    total_pages: u32,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    show_page_title: bool,
) -> Result<String> {
    let env = template_env().context("failed to initialize templates")?;
    let template = env
        .get_template("blog_index.html")
        .context("missing blog_index template")?;

    let nav_items = build_nav_view(project, current_href);
    let footer_show_stbl = project.config.footer.show_stbl;
    let footer_copyright = footer_copyright_text(&project.config.site, build_date_ymd);
    let rel = rel_prefix_for_href(current_href);

    let site = build_site_brand_view(project, asset_manifest, &rel);
    let menu_align = menu_align_value(project);
    let menu_align_class = menu_align_class(project);
    let header_layout = header_layout_value(project);
    let header_layout_class = header_layout_class(project);

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site => site,
            site_language => project.config.site.language.clone(),
            home_href => UrlMapper::new(&project.config).map("index").href,
            rel => rel,
            asset_manifest => asset_manifest.entries.clone(),
            nav_items => nav_items,
            menu_align => menu_align,
            menu_align_class => menu_align_class,
            header_layout => header_layout,
            header_layout_class => header_layout_class,
            page_title => title,
            show_page_title => show_page_title,
            intro_html => intro_html,
            banner_html => banner_html,
            items => items,
            prev_href => prev_href,
            next_href => next_href,
            page_no => page_no,
            total_pages => total_pages,
            build_date_ymd => build_date_ymd,
            footer_show_stbl => footer_show_stbl,
            footer_copyright => footer_copyright,
            redirect_href => Option::<String>::None,
        })
        .context("failed to render blog index template")
}

pub fn render_tag_index(
    project: &Project,
    listing: TagListingPage,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
) -> Result<String> {
    let env = template_env().context("failed to initialize templates")?;
    let template = env
        .get_template("tag_index.html")
        .context("missing tag_index template")?;

    let page_title = format!("Tag: {}", listing.tag);
    let nav_items = build_nav_view(project, current_href);
    let footer_show_stbl = project.config.footer.show_stbl;
    let footer_copyright = footer_copyright_text(&project.config.site, build_date_ymd);
    let rel = rel_prefix_for_href(current_href);
    let site = build_site_brand_view(project, asset_manifest, &rel);
    let menu_align = menu_align_value(project);
    let menu_align_class = menu_align_class(project);
    let header_layout = header_layout_value(project);
    let header_layout_class = header_layout_class(project);

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site => site,
            site_language => project.config.site.language.clone(),
            home_href => UrlMapper::new(&project.config).map("index").href,
            rel => rel,
            asset_manifest => asset_manifest.entries.clone(),
            nav_items => nav_items,
            menu_align => menu_align,
            menu_align_class => menu_align_class,
            header_layout => header_layout,
            header_layout_class => header_layout_class,
            page_title => page_title,
            tag => listing.tag,
            items => listing.items,
            build_date_ymd => build_date_ymd,
            footer_show_stbl => footer_show_stbl,
            footer_copyright => footer_copyright,
            redirect_href => Option::<String>::None,
        })
        .context("failed to render tag index template")
}

pub fn render_series_index(
    project: &Project,
    index: &Page,
    parts: Vec<SeriesIndexPart>,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    include_provider: Option<&dyn IncludeProvider>,
) -> Result<String> {
    let env = template_env().context("failed to initialize templates")?;
    let template = env
        .get_template("series_index.html")
        .context("missing series_index template")?;

    let nav_items = build_nav_view(project, current_href);
    let footer_show_stbl = project.config.footer.show_stbl;
    let footer_copyright = footer_copyright_text(&project.config.site, build_date_ymd);
    let rel = rel_prefix_for_href(current_href);
    let site = build_site_brand_view(project, asset_manifest, &rel);
    let menu_align = menu_align_value(project);
    let menu_align_class = menu_align_class(project);
    let header_layout = header_layout_value(project);
    let header_layout_class = header_layout_class(project);
    let page_title = page_title_or_filename(project, index);
    let intro_html = if index.body_markdown.trim().is_empty() {
        None
    } else {
        Some(render_markdown_with_media(
            project,
            Some(index),
            &index.body_markdown,
            &rel,
            include_provider,
        ))
    };

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site => site,
            site_language => project.config.site.language.clone(),
            home_href => UrlMapper::new(&project.config).map("index").href,
            rel => rel,
            asset_manifest => asset_manifest.entries.clone(),
            nav_items => nav_items,
            menu_align => menu_align,
            menu_align_class => menu_align_class,
            header_layout => header_layout,
            header_layout_class => header_layout_class,
            page_title => page_title,
            intro_html => intro_html,
            parts => parts,
            build_date_ymd => build_date_ymd,
            footer_show_stbl => footer_show_stbl,
            footer_copyright => footer_copyright,
            redirect_href => Option::<String>::None,
        })
        .context("failed to render series index template")
}

pub fn render_redirect_page(
    project: &Project,
    target_href: &str,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
) -> Result<String> {
    let body = format!("Redirecting to [{target_href}]({target_href}).\n");
    render_markdown_page(
        project,
        "Redirecting",
        &body,
        asset_manifest,
        current_href,
        build_date_ymd,
        Some(target_href),
        true,
        None,
    )
}

fn template_env() -> Result<Environment<'static>> {
    let mut env = Environment::new();
    env.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            AutoEscape::Html
        } else {
            AutoEscape::None
        }
    });
    env.add_template("base.html", BASE_TEMPLATE)?;
    env.add_template("page.html", PAGE_TEMPLATE)?;
    env.add_template("blog_index.html", BLOG_INDEX_TEMPLATE)?;
    env.add_template("tag_index.html", TAG_INDEX_TEMPLATE)?;
    env.add_template("series_index.html", SERIES_INDEX_TEMPLATE)?;
    env.add_template("partials/list_item.html", LIST_ITEM_TEMPLATE)?;
    Ok(env)
}

fn render_with_context(
    project: &Project,
    page_title: String,
    show_page_title: bool,
    authors: Option<Vec<String>>,
    published: Option<String>,
    updated: Option<String>,
    tags: Vec<String>,
    body_html: String,
    banner_html: Option<String>,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    series_nav: Option<SeriesNavView>,
    redirect_href: Option<String>,
) -> Result<String> {
    let env = template_env().context("failed to initialize templates")?;
    let template = env
        .get_template("page.html")
        .context("missing page template")?;

    let nav_items = build_nav_view(project, current_href);
    let footer_show_stbl = project.config.footer.show_stbl;
    let footer_copyright = footer_copyright_text(&project.config.site, build_date_ymd);
    let rel = rel_prefix_for_href(current_href);
    let site = build_site_brand_view(project, asset_manifest, &rel);
    let menu_align = menu_align_value(project);
    let menu_align_class = menu_align_class(project);
    let header_layout = header_layout_value(project);
    let header_layout_class = header_layout_class(project);

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site => site,
            site_language => project.config.site.language.clone(),
            home_href => UrlMapper::new(&project.config).map("index").href,
            rel => rel,
            asset_manifest => asset_manifest.entries.clone(),
            nav_items => nav_items,
            menu_align => menu_align,
            menu_align_class => menu_align_class,
            header_layout => header_layout,
            header_layout_class => header_layout_class,
            page_title => page_title,
            show_page_title => show_page_title,
            authors => authors,
            published => published,
            updated => updated,
            tags => tags,
            body_html => body_html,
            banner_html => banner_html,
            series_nav => series_nav,
            build_date_ymd => build_date_ymd,
            footer_show_stbl => footer_show_stbl,
            footer_copyright => footer_copyright,
            redirect_href => redirect_href,
        })
        .context("failed to render page template")
}

fn render_markdown_with_media(
    project: &Project,
    page: Option<&Page>,
    markdown: &str,
    rel: &str,
    include_provider: Option<&dyn IncludeProvider>,
) -> String {
    let options = RenderOptions {
        macro_project: Some(project),
        macro_page: page,
        macros_enabled: project.config.site.macros.enabled,
        include_provider,
        rel_prefix: rel,
        video_heights: &project.config.media.video.heights,
        image_widths: &project.config.media.images.widths,
        max_body_width: &project.config.theme.max_body_width,
        desktop_min: &project.config.theme.breakpoints.desktop_min,
        wide_min: &project.config.theme.breakpoints.wide_min,
        image_format_mode: project.config.media.images.format_mode,
        image_alpha: Some(&project.image_alpha),
        image_variants: Some(&project.image_variants),
        video_variants: Some(&project.video_variants),
        syntax_highlight: project.config.syntax.highlight,
        syntax_theme: &project.config.syntax.theme,
        syntax_line_numbers: project.config.syntax.line_numbers,
    };
    render_markdown_to_html_with_media(markdown, &options)
}

pub fn render_banner_html(project: &Project, page: &Page, rel: &str) -> Option<String> {
    let banner = page.banner_name.as_ref()?;
    let mut banner_path = banner.clone();
    if !banner_path.starts_with("images/") {
        if banner_path.contains('/') || banner_path.contains('\\') {
            return None;
        }
        banner_path = format!("images/{banner_path}");
    }
    let options = RenderOptions {
        macro_project: None,
        macro_page: None,
        macros_enabled: false,
        include_provider: None,
        rel_prefix: rel,
        video_heights: &project.config.media.video.heights,
        image_widths: &project.config.media.images.widths,
        max_body_width: &project.config.theme.max_body_width,
        desktop_min: &project.config.theme.breakpoints.desktop_min,
        wide_min: &project.config.theme.breakpoints.wide_min,
        image_format_mode: project.config.media.images.format_mode,
        image_alpha: Some(&project.image_alpha),
        image_variants: Some(&project.image_variants),
        video_variants: Some(&project.video_variants),
        syntax_highlight: project.config.syntax.highlight,
        syntax_theme: &project.config.syntax.theme,
        syntax_line_numbers: project.config.syntax.line_numbers,
    };
    let alt = page.header.title.clone().unwrap_or_default();
    Some(crate::render::render_image_html(
        &format!("{banner_path};banner"),
        &alt,
        &options,
    ))
}

#[derive(Debug, Clone, Serialize)]
pub struct NavItemView {
    pub label: String,
    pub href: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize)]
struct SiteBrandView {
    title: String,
    tagline: Option<String>,
    logo: Option<String>,
    logo_url: Option<String>,
}

fn build_nav_view(project: &Project, current_href: &str) -> Vec<NavItemView> {
    let mapper = UrlMapper::new(&project.config);
    let nav = resolved_nav_items(project);
    let mut active_taken = false;
    nav.iter()
        .map(|item| {
            let href = nav_item_href(item, &mapper);
            let is_active = !active_taken && href_matches(&href, current_href);
            if is_active {
                active_taken = true;
            }
            NavItemView {
                label: item.label.clone(),
                href,
                is_active,
            }
        })
        .collect()
}

fn resolved_nav_items(project: &Project) -> Vec<NavItem> {
    if !project.config.nav.is_empty() {
        return project.config.nav.clone();
    }
    vec![
        NavItem {
            label: "Home".to_string(),
            href: "index".to_string(),
        },
        NavItem {
            label: "Blog".to_string(),
            href: "index".to_string(),
        },
        NavItem {
            label: "Tags".to_string(),
            href: "tags".to_string(),
        },
    ]
}

fn build_site_brand_view(
    project: &Project,
    asset_manifest: &AssetManifest,
    rel: &str,
) -> SiteBrandView {
    let title = project.config.site.title.clone();
    let tagline = project.config.site.tagline.clone();
    let logo = project
        .config
        .site
        .logo
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string());
    let logo_url = logo.as_ref().and_then(|raw| {
        if is_absolute_or_fragment_href(raw) {
            return Some(raw.to_string());
        }
        let normalized = normalize_href(raw);
        let resolved = asset_manifest
            .entries
            .get(&normalized)
            .cloned()
            .unwrap_or(normalized);
        Some(format!("{rel}{resolved}"))
    });
    SiteBrandView {
        title,
        tagline,
        logo,
        logo_url,
    }
}

fn menu_align_class(project: &Project) -> String {
    match project.config.theme.header.menu_align {
        MenuAlign::Left => "menu-align-left".to_string(),
        MenuAlign::Center => "menu-align-center".to_string(),
        MenuAlign::Right => "menu-align-right".to_string(),
    }
}

fn menu_align_value(project: &Project) -> &'static str {
    match project.config.theme.header.menu_align {
        MenuAlign::Left => "left",
        MenuAlign::Center => "center",
        MenuAlign::Right => "right",
    }
}

fn header_layout_class(project: &Project) -> String {
    match project.config.theme.header.layout {
        ThemeHeaderLayout::Inline => "header-inline".to_string(),
        ThemeHeaderLayout::Stacked => "header-stacked".to_string(),
    }
}

fn header_layout_value(project: &Project) -> &'static str {
    match project.config.theme.header.layout {
        ThemeHeaderLayout::Inline => "inline",
        ThemeHeaderLayout::Stacked => "stacked",
    }
}

fn rel_prefix_for_href(href: &str) -> String {
    let trimmed = href.trim_start_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    let depth = if trimmed.ends_with('/') {
        let stripped = trimmed.trim_end_matches('/');
        if stripped.is_empty() {
            0
        } else {
            stripped.split('/').count()
        }
    } else if let Some((parent, _)) = trimmed.rsplit_once('/') {
        if parent.is_empty() {
            0
        } else {
            parent.split('/').count()
        }
    } else {
        0
    };
    "../".repeat(depth)
}

fn nav_item_href(item: &NavItem, mapper: &UrlMapper) -> String {
    if is_external_href(&item.href) || is_absolute_or_fragment_href(&item.href) {
        return item.href.clone();
    }
    mapper.map(&item.href).href
}

fn href_matches(nav_href: &str, current_href: &str) -> bool {
    normalize_href(nav_href) == normalize_href(current_href)
}

fn is_external_href(href: &str) -> bool {
    let href = href.trim();
    href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
}

fn is_absolute_or_fragment_href(href: &str) -> bool {
    let href = href.trim();
    href.starts_with('/') || href.starts_with('#') || href.starts_with('?')
}

fn normalize_href(value: &str) -> String {
    let trimmed = value.trim();
    let without_prefix = trimmed.strip_prefix("./").unwrap_or(trimmed);
    let without_prefix = without_prefix.strip_prefix('/').unwrap_or(without_prefix);
    let without_suffix = without_prefix.strip_suffix('/').unwrap_or(without_prefix);
    without_suffix.to_string()
}

fn footer_copyright_text(site: &crate::model::SiteMeta, build_date_ymd: &str) -> String {
    if let Some(text) = site
        .copyright
        .as_ref()
        .filter(|value| !value.trim().is_empty())
    {
        return text.to_string();
    }
    let year = extract_year(build_date_ymd).unwrap_or_else(|| "0000".to_string());
    format!("Copyright {year} by {}", site.title)
}

fn extract_year(value: &str) -> Option<String> {
    let mut last_year: Option<String> = None;
    let mut digits = String::new();
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            if digits.len() >= 4 {
                let start = digits.len().saturating_sub(4);
                last_year = Some(digits[start..].to_string());
            }
            digits.clear();
        }
    }
    if digits.len() >= 4 {
        let start = digits.len().saturating_sub(4);
        last_year = Some(digits[start..].to_string());
    }
    last_year
}

pub fn page_title_or_filename(project: &Project, page: &Page) -> String {
    if let Some(value) = page
        .header
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return value.to_string();
    }
    let logical_key = crate::url::logical_key_from_source_path(&page.source_path);
    if logical_key == "index" {
        return project.config.site.title.clone();
    }
    let stem = std::path::Path::new(&page.source_path)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Untitled");
    let mut chars = stem.chars();
    let Some(first) = chars.next() else {
        return project.config.site.title.clone();
    };
    format!("{}{}", first.to_uppercase(), chars.collect::<String>())
}

pub fn format_timestamp_rfc3339(value: Option<i64>) -> Option<String> {
    let value = value?;
    let dt = DateTime::<Utc>::from_timestamp(value, 0)?;
    Some(dt.to_rfc3339())
}

pub fn format_timestamp_ymd(value: Option<i64>) -> Option<String> {
    let value = value?;
    let dt = DateTime::<Utc>::from_timestamp(value, 0)?;
    Some(dt.format("%Y-%m-%d").to_string())
}

pub fn format_timestamp_long_date(value: Option<i64>) -> Option<String> {
    let value = value?;
    let dt = DateTime::<Utc>::from_timestamp(value, 0)?;
    Some(dt.format("%B %-d, %Y").to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        BlogIndexItem, NavItemView, SeriesIndexPart, SeriesNavLink, SeriesNavView, TagListingPage,
        build_nav_view, format_timestamp_ymd, render_blog_index, render_page,
        render_page_with_series_nav, render_series_index, render_tag_index,
    };
    use crate::assets::AssetManifest;
    use crate::config::load_site_config;
    use crate::header::Header;
    use crate::model::{DocId, Page, Project, SiteContent};
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn format_timestamp_ymd_outputs_date_only() {
        let value = format_timestamp_ymd(Some(1_704_153_600)).expect("date");
        assert_eq!(value, "2024-01-02");
    }

    #[test]
    fn nav_ordering_preserves_config() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n  nav:\n    - label: \"Home\"\n      href: \"index\"\n    - label: \"Blog\"\n      href: \"blog\"\n",
            SiteContent::default(),
        );
        let items = build_nav_view(&project, "blog.html");
        assert_nav_labels(&items, &["Home", "Blog"]);
        assert_eq!(items[0].href, "index.html");
        assert_eq!(items[1].href, "blog.html");
    }

    #[test]
    fn nav_defaults_do_not_scan_content() {
        let mut content = SiteContent::default();
        content
            .pages
            .push(fake_page("articles/blog/index.md", None, "Blog"));
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            content,
        );
        let items = build_nav_view(&project, "index.html");
        assert_nav_labels(&items, &["Home", "Blog", "Tags"]);
        assert_eq!(items[0].href, "index.html");
        assert_eq!(items[1].href, "index.html");
        assert_eq!(items[2].href, "tags.html");
    }

    #[test]
    fn nav_active_is_exact_match_only() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n  nav:\n    - label: \"Home\"\n      href: \"index\"\n    - label: \"Blog\"\n      href: \"blog\"\n",
            SiteContent::default(),
        );
        let items = build_nav_view(&project, "blog/page1.html");
        assert_eq!(active_count(&items), 0);

        let items = build_nav_view(&project, "blog.html");
        assert_eq!(active_count(&items), 1);
        assert!(items[1].is_active);
    }

    #[test]
    fn nav_active_is_first_match_only() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n  nav:\n    - label: \"Home\"\n      href: \"index\"\n    - label: \"Home Duplicate\"\n      href: \"index\"\n",
            SiteContent::default(),
        );
        let items = build_nav_view(&project, "index.html");
        assert_eq!(active_count(&items), 1);
        assert!(items[0].is_active);
        assert!(!items[1].is_active);
    }

    #[test]
    fn nav_default_is_deterministic_when_missing() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent::default(),
        );
        let items = build_nav_view(&project, "index.html");
        assert_nav_labels(&items, &["Home", "Blog", "Tags"]);
        assert_eq!(items[0].href, "index.html");
        assert_eq!(items[1].href, "index.html");
        assert_eq!(items[2].href, "tags.html");
    }

    #[test]
    fn nav_href_mapping_rules() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n  nav:\n    - label: \"About\"\n      href: \"about\"\n    - label: \"Absolute\"\n      href: \"/about.html\"\n    - label: \"External\"\n      href: \"https://example.com/\"\n    - label: \"Top\"\n      href: \"#top\"\n",
            SiteContent::default(),
        );
        let items = build_nav_view(&project, "index.html");
        assert_eq!(items[0].href, "about.html");
        assert_eq!(items[1].href, "/about.html");
        assert_eq!(items[2].href, "https://example.com/");
        assert_eq!(items[3].href, "#top");
    }

    #[test]
    fn meta_blocks_render_only_when_present() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent::default(),
        );
        let mut header = Header::default();
        header.title = Some("Meta Test".to_string());
        let page = Page {
            id: DocId(blake3::hash(b"meta-test")),
            source_path: "articles/meta-test.md".to_string(),
            header: header.clone(),
            body_markdown: "Body".to_string(),
            banner_name: None,
            media_refs: Vec::new(),
            url_path: "meta-test".to_string(),
            content_hash: blake3::hash(b"meta-test"),
        };
        let html = render_page(
            &project,
            &page,
            &default_manifest(),
            "meta-test.html",
            "2026-01-29",
            None,
        )
        .expect("render");
        assert!(!html.contains("<div class=\"meta\">"));
        assert!(!html.contains("Tags:"));
        assert!(!html.contains("class=\"tags\""));

        let mut header_with_tags = header;
        header_with_tags.tags = vec!["rust".to_string(), "stbl".to_string()];
        let page_with_tags = Page {
            header: header_with_tags,
            ..page
        };
        let html = render_page(
            &project,
            &page_with_tags,
            &default_manifest(),
            "meta-test.html",
            "2026-01-29",
            None,
        )
        .expect("render");
        assert!(html.contains("<div class=\"meta\">"));
        assert!(html.contains("Tags:"));
        assert!(html.contains("class=\"tags\""));
    }

    #[test]
    fn footer_defaults_and_overrides() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent::default(),
        );
        let page = simple_page("Footer Test", "articles/footer-test.md");
        let html = render_page(
            &project,
            &page,
            &default_manifest(),
            "footer.html",
            "2026-01-25",
            None,
        )
        .expect("render");
        assert!(html.contains("Copyright 2026 by Demo"));
        assert!(html.contains("Generated by"));
        assert!(html.contains("<a href=\"https://github.com/jgaa/stbl\">stbl</a>"));
        assert!(html.contains("on 2026-01-25"));

        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n  copyright: \"Copyright ACME\"\nfooter:\n  show_stbl: false\n",
            SiteContent::default(),
        );
        let html = render_page(
            &project,
            &page,
            &default_manifest(),
            "footer.html",
            "2026-01-25",
            None,
        )
        .expect("render");
        assert!(html.contains("Copyright ACME"));
        assert!(!html.contains("Copyright 2026 by Demo"));
        assert!(!html.contains("Generated by"));
    }

    #[test]
    fn page_title_falls_back_to_site_title() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent::default(),
        );
        let page = Page {
            id: DocId(blake3::hash(b"index")),
            source_path: "articles/index.md".to_string(),
            header: Header::default(),
            body_markdown: "Body".to_string(),
            banner_name: None,
            media_refs: Vec::new(),
            url_path: "index".to_string(),
            content_hash: blake3::hash(b"index"),
        };
        let html = render_page(
            &project,
            &page,
            &default_manifest(),
            "index.html",
            "2026-01-30",
            None,
        )
        .expect("render");
        assert!(html.contains("<title>Demo · Demo</title>"));
        assert!(!html.contains("<title>Untitled · Demo</title>"));
    }

    #[test]
    fn page_title_falls_back_to_filename() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent::default(),
        );
        let page = Page {
            id: DocId(blake3::hash(b"download")),
            source_path: "articles/download.md".to_string(),
            header: Header::default(),
            body_markdown: "Body".to_string(),
            banner_name: None,
            media_refs: Vec::new(),
            url_path: "download".to_string(),
            content_hash: blake3::hash(b"download"),
        };
        let html = render_page(
            &project,
            &page,
            &default_manifest(),
            "download.html",
            "2026-01-30",
            None,
        )
        .expect("render");
        assert!(html.contains("<title>Download · Demo</title>"));
        assert!(!html.contains("<title>Untitled · Demo</title>"));
    }

    #[test]
    fn listing_pages_use_shared_list_item_partial() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent::default(),
        );
        let item = BlogIndexItem {
            title: "Item".to_string(),
            href: "item.html".to_string(),
            published_display: Some("2024-01-01".to_string()),
            updated_display: None,
            kind_label: None,
            abstract_text: None,
            tags: Vec::new(),
            latest_parts: Vec::new(),
        };
        let html = render_blog_index(
            &project,
            "Blog".to_string(),
            None,
            None,
            vec![item.clone()],
            None,
            None,
            1,
            1,
            &default_manifest(),
            "index.html",
            "2026-01-29",
            false,
        )
        .expect("render blog");
        assert!(html.contains("class=\"list-item\""));

        let listing = TagListingPage {
            tag: "rust".to_string(),
            items: vec![item],
        };
        let html = render_tag_index(
            &project,
            listing,
            &default_manifest(),
            "tags/rust.html",
            "2026-01-29",
        )
        .expect("render tag");
        assert!(html.contains("class=\"list-item\""));
    }

    #[test]
    fn series_nav_block_renders_only_when_present() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent::default(),
        );
        let page = simple_page("Series Part", "articles/series/part1.md");
        let html = render_page(
            &project,
            &page,
            &default_manifest(),
            "part1.html",
            "2026-01-29",
            None,
        )
        .expect("render page");
        assert!(!html.contains("class=\"series-nav\""));

        let nav = SeriesNavView {
            prev: None,
            index: SeriesNavLink {
                title: "Series".to_string(),
                href: "series.html".to_string(),
            },
            next: Some(SeriesNavLink {
                title: "Part 2".to_string(),
                href: "part2.html".to_string(),
            }),
        };
        let html = render_page_with_series_nav(
            &project,
            &page,
            &default_manifest(),
            Some(nav),
            "part1.html",
            "2026-01-29",
            None,
        )
        .expect("render page");
        assert!(html.contains("class=\"series-nav\""));
        assert!(html.contains("series.html"));
        assert!(html.contains("part2.html"));
    }

    #[test]
    fn series_index_parts_render_in_order() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent::default(),
        );
        let index = simple_page("Series", "articles/series/index.md");
        let parts = vec![
            SeriesIndexPart {
                title: "Part 1".to_string(),
                href: "part1.html".to_string(),
                published_display: Some("2024-01-01".to_string()),
            },
            SeriesIndexPart {
                title: "Part 2".to_string(),
                href: "part2.html".to_string(),
                published_display: Some("2024-01-02".to_string()),
            },
        ];
        let html = render_series_index(
            &project,
            &index,
            parts,
            &default_manifest(),
            "series.html",
            "2026-01-29",
            None,
        )
        .expect("render series");
        let part1 = html.find("Part 1").expect("part1");
        let part2 = html.find("Part 2").expect("part2");
        assert!(part1 < part2);
    }

    fn project_with_config(config: &str, content: SiteContent) -> Project {
        let path = write_temp_config(config);
        let config = load_site_config(&path).expect("config");
        Project {
            root: PathBuf::from("."),
            config,
            content,
            image_alpha: std::collections::BTreeMap::new(),
            image_variants: Default::default(),
            video_variants: Default::default(),
        }
    }

    fn write_temp_config(contents: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("stbl-nav-{}.yaml", Uuid::new_v4()));
        fs::write(&path, contents).expect("write config");
        path
    }

    fn fake_page(
        source_path: &str,
        template: Option<crate::header::TemplateId>,
        title: &str,
    ) -> Page {
        let mut header = Header::default();
        header.title = Some(title.to_string());
        header.template = template;
        Page {
            id: DocId(blake3::hash(source_path.as_bytes())),
            source_path: source_path.to_string(),
            header,
            body_markdown: String::new(),
            banner_name: None,
            media_refs: Vec::new(),
            url_path: String::new(),
            content_hash: blake3::hash(source_path.as_bytes()),
        }
    }

    fn simple_page(title: &str, source_path: &str) -> Page {
        let mut header = Header::default();
        header.title = Some(title.to_string());
        Page {
            id: DocId(blake3::hash(title.as_bytes())),
            source_path: source_path.to_string(),
            header,
            body_markdown: "Body".to_string(),
            banner_name: None,
            media_refs: Vec::new(),
            url_path: "simple".to_string(),
            content_hash: blake3::hash(title.as_bytes()),
        }
    }

    fn assert_nav_labels(items: &[NavItemView], expected: &[&str]) {
        let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(labels, expected);
    }

    fn active_count(items: &[NavItemView]) -> usize {
        items.iter().filter(|item| item.is_active).count()
    }

    fn default_manifest() -> AssetManifest {
        let mut entries = std::collections::BTreeMap::new();
        entries.insert(
            "css/vars.css".to_string(),
            "artifacts/css/vars.css".to_string(),
        );
        entries.insert(
            "css/common.css".to_string(),
            "artifacts/css/common.css".to_string(),
        );
        entries.insert(
            "css/syntax.css".to_string(),
            "artifacts/css/syntax.css".to_string(),
        );
        entries.insert(
            "css/desktop.css".to_string(),
            "artifacts/css/desktop.css".to_string(),
        );
        entries.insert(
            "css/mobile.css".to_string(),
            "artifacts/css/mobile.css".to_string(),
        );
        entries.insert(
            "css/wide-desktop.css".to_string(),
            "artifacts/css/wide-desktop.css".to_string(),
        );
        AssetManifest { entries }
    }
}
