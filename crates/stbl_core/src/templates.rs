use anyhow::{Context, Result};
use chrono::{DateTime, FixedOffset, Utc};
use chrono_tz::Tz;
use minijinja::{AutoEscape, Environment, context};
use stbl_embedded_assets as embedded;
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use crate::assets::AssetManifest;
use crate::blog_index::{canonical_tag_map, collect_tag_list, iter_visible_posts, tag_key};
use crate::comments::{CommentTemplateProvider, render_comments_html};
use crate::macros::IncludeProvider;
use crate::model::{
    MenuAlign, NavItem, Page, PersonLink, Project, SystemConfig, ThemeHeaderLayout,
};
use crate::render::{RenderOptions, render_markdown_to_html_with_media};
use crate::url::UrlMapper;
use crate::visibility::{is_blog_index_excluded, is_cover_page};
use serde::Serialize;

const DEFAULT_THEME_VARIANT: &str = "stbl";
const REQUIRED_TEMPLATE_ASSETS: &[(&str, &str)] = &[
    ("base", "templates/base.html"),
    ("page", "templates/page.html"),
    ("blog_index", "templates/partials/blog_index.html"),
    ("tag_index", "templates/tag_index.html"),
    ("series_index", "templates/series_index.html"),
    ("list_item", "templates/partials/list_item.html"),
    ("header", "templates/partials/header.html"),
    ("footer", "templates/partials/footer.html"),
];
const DEFAULT_DATE_FORMAT: &str = "%B %-d, %Y at %H:%M %Z";
static TEMPLATE_ASSET_CACHE: OnceLock<Mutex<BTreeMap<String, TemplateAssetMap>>> = OnceLock::new();

pub fn templates_hash() -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"stbl2.templates.v3");
    let templates = template_assets_for_variant(DEFAULT_THEME_VARIANT)
        .expect("embedded stbl templates should always be available");
    for (name, path) in REQUIRED_TEMPLATE_ASSETS {
        let contents = templates
            .get(*path)
            .unwrap_or_else(|| panic!("missing required embedded template asset: {}", path));
        add_template_hash(&mut hasher, name, contents);
    }
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

type TemplateAssetMap = BTreeMap<String, &'static str>;

fn normalize_theme_variant(theme_variant: &str) -> &str {
    let trimmed = theme_variant.trim();
    if trimmed.is_empty() || trimmed == "default" {
        DEFAULT_THEME_VARIANT
    } else {
        trimmed
    }
}

fn template_assets_for_variant(theme_variant: &str) -> Result<TemplateAssetMap> {
    let normalized = normalize_theme_variant(theme_variant).to_string();
    let cache = TEMPLATE_ASSET_CACHE.get_or_init(|| Mutex::new(BTreeMap::new()));
    if let Some(cached) = cache
        .lock()
        .expect("template cache lock")
        .get(&normalized)
        .cloned()
    {
        return Ok(cached);
    }

    let mut merged = load_template_assets_from_embedded(DEFAULT_THEME_VARIANT)?;
    if normalized != DEFAULT_THEME_VARIANT {
        if let Some(overrides) = load_template_assets_from_embedded_optional(&normalized)? {
            for (path, contents) in overrides {
                merged.insert(path, contents);
            }
        }
    }

    {
        let mut guard = cache.lock().expect("template cache lock");
        guard.insert(normalized, merged.clone());
    }
    Ok(merged)
}

fn load_template_assets_from_embedded_optional(
    theme_variant: &str,
) -> Result<Option<TemplateAssetMap>> {
    let Some(template) = embedded::template(theme_variant) else {
        return Ok(None);
    };
    let mut out = TemplateAssetMap::new();
    for entry in template.assets {
        if !(entry.path.starts_with("templates/") && entry.path.ends_with(".html")) {
            continue;
        }
        let bytes = embedded::decompress_to_vec(&entry.hash).with_context(|| {
            format!(
                "failed to decompress embedded template asset {} for theme {}",
                entry.path, theme_variant
            )
        })?;
        let contents = String::from_utf8(bytes).with_context(|| {
            format!(
                "embedded template asset {} in theme {} is not utf-8",
                entry.path, theme_variant
            )
        })?;
        out.insert(
            entry.path.to_string(),
            Box::leak(contents.into_boxed_str()) as &'static str,
        );
    }
    Ok(Some(out))
}

fn load_template_assets_from_embedded(theme_variant: &str) -> Result<TemplateAssetMap> {
    load_template_assets_from_embedded_optional(theme_variant)?.with_context(|| {
        format!(
            "embedded theme '{}' not found while loading HTML templates",
            theme_variant
        )
    })
}

pub fn render_page(
    project: &Project,
    page: &Page,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    include_provider: Option<&dyn IncludeProvider>,
    comment_template_provider: Option<&dyn CommentTemplateProvider>,
) -> Result<String> {
    render_page_with_series_nav(
        project,
        page,
        asset_manifest,
        None,
        current_href,
        build_date_ymd,
        include_provider,
        comment_template_provider,
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
    comment_template_provider: Option<&dyn CommentTemplateProvider>,
) -> Result<String> {
    let rel = root_prefix_for_base_url(&project.config.site.base_url);
    let body_html = render_markdown_with_media(
        project,
        Some(page),
        &page.body_markdown,
        &rel,
        include_provider,
    );
    let banner_html = render_banner_html(project, page, &rel);
    let comments_html =
        render_comments_html(project, page, current_href, comment_template_provider);
    let page_title = page_title_or_filename(project, page);
    let show_page_title = !is_blog_index_excluded(page, None);
    let cover_page = is_cover_page(page);
    let show_authors = !cover_page;
    let authors = build_author_views(
        project,
        page.header.authors.as_ref(),
        asset_manifest,
        show_authors,
    );
    let (published, updated, published_raw, updated_raw, tags) = if cover_page {
        (None, None, None, None, Vec::new())
    } else {
        let canonical_tags = canonical_tag_map(project);
        let mut tag_links = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for tag in &page.header.tags {
            let key = tag_key(tag);
            if !seen.insert(key.clone()) {
                continue;
            }
            let label = canonical_tags
                .get(&key)
                .cloned()
                .unwrap_or_else(|| tag.clone());
            let href = UrlMapper::new(&project.config)
                .map(&format!("tags/{}", label))
                .href;
            tag_links.push(TagLink {
                label,
                href: resolve_root_href(&href, &rel),
            });
        }
        let published_ts =
            normalize_timestamp(page.header.published, project.config.system.as_ref());
        let updated_ts = effective_updated_timestamp(
            published_ts,
            normalize_timestamp(
                page.header.resolved_updated(),
                project.config.system.as_ref(),
            ),
        );
        let published = format_timestamp_display(
            published_ts,
            project.config.system.as_ref(),
            project.config.site.timezone.as_deref(),
        );
        let updated = format_timestamp_display(
            updated_ts,
            project.config.system.as_ref(),
            project.config.site.timezone.as_deref(),
        );
        let published_raw = format_timestamp_rfc3339(published_ts);
        let updated_raw = format_timestamp_rfc3339(updated_ts);
        (published, updated, published_raw, updated_raw, tag_links)
    };

    render_with_context(
        project,
        page_title,
        show_page_title,
        authors,
        published,
        updated,
        published_raw,
        updated_raw,
        tags,
        body_html,
        banner_html,
        comments_html,
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
    _comment_template_provider: Option<&dyn CommentTemplateProvider>,
) -> Result<String> {
    let rel = root_prefix_for_base_url(&project.config.site.base_url);
    let body_html =
        render_markdown_with_media(project, None, body_markdown, &rel, include_provider);
    render_with_context(
        project,
        title.to_string(),
        show_page_title,
        Vec::new(),
        None,
        None,
        None,
        None,
        Vec::new(),
        body_html,
        None,
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
    pub part_no: i32,
    pub title: String,
    pub href: String,
    pub published_display: Option<String>,
    pub published_raw: Option<String>,
    pub abstract_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TagLink {
    pub label: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorLinkView {
    pub name: String,
    pub href: String,
    pub icon: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthorView {
    pub name: String,
    pub links: Vec<AuthorLinkView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlogIndexItem {
    pub title: String,
    pub href: String,
    pub published_display: Option<String>,
    pub updated_display: Option<String>,
    pub published_raw: Option<String>,
    pub updated_raw: Option<String>,
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
    pub published_raw: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeriesNavLink {
    pub title: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeriesNavEntry {
    pub title: String,
    pub href: String,
    pub is_current: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeriesNavView {
    pub index: SeriesNavLink,
    pub parts: Vec<SeriesNavEntry>,
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
    first_href: Option<String>,
    last_href: Option<String>,
    page_no: u32,
    total_pages: u32,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    show_page_title: bool,
) -> Result<String> {
    let env =
        template_env(&project.config.theme.variant).context("failed to initialize templates")?;
    let template = env
        .get_template("blog_index.html")
        .context("missing blog_index template")?;

    let rel = root_prefix_for_base_url(&project.config.site.base_url);
    let nav_items = build_nav_view(project, current_href, &rel);
    let footer_show_stbl = project.config.footer.show_stbl;
    let footer_copyright = footer_copyright_text(&project.config.site, build_date_ymd);
    let rss_visible = project
        .config
        .rss
        .as_ref()
        .map(|rss| rss.enabled)
        .unwrap_or(false)
        && iter_visible_posts(project, None).next().is_some();
    let prev_href = prev_href.map(|href| resolve_root_href(&href, &rel));
    let next_href = next_href.map(|href| resolve_root_href(&href, &rel));
    let first_href = first_href.map(|href| resolve_root_href(&href, &rel));
    let last_href = last_href.map(|href| resolve_root_href(&href, &rel));

    let site = build_site_brand_view(project, asset_manifest, &rel);
    let menu_align = menu_align_value(project);
    let menu_align_class = menu_align_class(project);
    let header_layout = header_layout_value(project);
    let header_layout_class = header_layout_class(project);
    let tag_links = collect_tag_list(project)
        .into_iter()
        .map(|tag| {
            let href = UrlMapper::new(&project.config)
                .map(&format!("tags/{}", tag))
                .href;
            TagLink {
                label: tag,
                href: resolve_root_href(&href, &rel),
            }
        })
        .collect::<Vec<_>>();

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site => site,
            site_language => project.config.site.language.clone(),
            home_href => resolve_root_href(&UrlMapper::new(&project.config).map("index").href, &rel),
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
            first_href => first_href,
            last_href => last_href,
            page_no => page_no,
            total_pages => total_pages,
            tag_links => tag_links,
            build_date_ymd => build_date_ymd,
            footer_show_stbl => footer_show_stbl,
            footer_copyright => footer_copyright,
            rss_visible => rss_visible,
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
    let env =
        template_env(&project.config.theme.variant).context("failed to initialize templates")?;
    let template = env
        .get_template("tag_index.html")
        .context("missing tag_index template")?;

    let page_title = format!("Tag: {}", listing.tag);
    let rel = root_prefix_for_base_url(&project.config.site.base_url);
    let nav_items = build_nav_view(project, current_href, &rel);
    let footer_show_stbl = project.config.footer.show_stbl;
    let footer_copyright = footer_copyright_text(&project.config.site, build_date_ymd);
    let rss_visible = project
        .config
        .rss
        .as_ref()
        .map(|rss| rss.enabled)
        .unwrap_or(false)
        && iter_visible_posts(project, None).next().is_some();
    let site = build_site_brand_view(project, asset_manifest, &rel);
    let menu_align = menu_align_value(project);
    let menu_align_class = menu_align_class(project);
    let header_layout = header_layout_value(project);
    let header_layout_class = header_layout_class(project);
    let tag_links = collect_tag_list(project)
        .into_iter()
        .map(|tag| {
            let href = UrlMapper::new(&project.config)
                .map(&format!("tags/{}", tag))
                .href;
            TagLink {
                label: tag,
                href: resolve_root_href(&href, &rel),
            }
        })
        .collect::<Vec<_>>();

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site => site,
            site_language => project.config.site.language.clone(),
            home_href => resolve_root_href(&UrlMapper::new(&project.config).map("index").href, &rel),
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
            tag_links => tag_links,
            build_date_ymd => build_date_ymd,
            footer_show_stbl => footer_show_stbl,
            footer_copyright => footer_copyright,
            rss_visible => rss_visible,
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
    let env =
        template_env(&project.config.theme.variant).context("failed to initialize templates")?;
    let template = env
        .get_template("series_index.html")
        .context("missing series_index template")?;

    let rel = root_prefix_for_base_url(&project.config.site.base_url);
    let nav_items = build_nav_view(project, current_href, &rel);
    let footer_show_stbl = project.config.footer.show_stbl;
    let footer_copyright = footer_copyright_text(&project.config.site, build_date_ymd);
    let rss_visible = project
        .config
        .rss
        .as_ref()
        .map(|rss| rss.enabled)
        .unwrap_or(false)
        && iter_visible_posts(project, None).next().is_some();
    let site = build_site_brand_view(project, asset_manifest, &rel);
    let menu_align = menu_align_value(project);
    let menu_align_class = menu_align_class(project);
    let header_layout = header_layout_value(project);
    let header_layout_class = header_layout_class(project);
    let page_title = page_title_or_filename(project, index);
    let banner_html = render_banner_html(project, index, &rel);
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
            home_href => resolve_root_href(&UrlMapper::new(&project.config).map("index").href, &rel),
            rel => rel,
            asset_manifest => asset_manifest.entries.clone(),
            nav_items => nav_items,
            menu_align => menu_align,
            menu_align_class => menu_align_class,
            header_layout => header_layout,
            header_layout_class => header_layout_class,
            page_title => page_title,
            banner_html => banner_html,
            intro_html => intro_html,
            parts => parts,
            build_date_ymd => build_date_ymd,
            footer_show_stbl => footer_show_stbl,
            footer_copyright => footer_copyright,
            rss_visible => rss_visible,
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
        None,
    )
}

fn template_env(theme_variant: &str) -> Result<Environment<'static>> {
    let templates = template_assets_for_variant(theme_variant)?;
    let base_template = templates
        .get("templates/base.html")
        .context("missing base template asset")?;
    let page_template = templates
        .get("templates/page.html")
        .context("missing page template asset")?;
    let blog_index_template = templates
        .get("templates/partials/blog_index.html")
        .context("missing blog_index template asset")?;
    let tag_index_template = templates
        .get("templates/tag_index.html")
        .context("missing tag_index template asset")?;
    let series_index_template = templates
        .get("templates/series_index.html")
        .context("missing series_index template asset")?;
    let list_item_template = templates
        .get("templates/partials/list_item.html")
        .context("missing list_item template asset")?;
    let footer_template = templates
        .get("templates/partials/footer.html")
        .context("missing footer template asset")?;
    let header_template = templates
        .get("templates/partials/header.html")
        .context("missing header template asset")?;

    let mut env = Environment::new();
    env.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            AutoEscape::Html
        } else {
            AutoEscape::None
        }
    });
    env.add_template("base.html", base_template)?;
    env.add_template("page.html", page_template)?;
    env.add_template("blog_index.html", blog_index_template)?;
    env.add_template("tag_index.html", tag_index_template)?;
    env.add_template("series_index.html", series_index_template)?;
    env.add_template("partials/list_item.html", list_item_template)?;
    env.add_template("partials/header.html", header_template)?;
    env.add_template("partials/footer.html", footer_template)?;
    Ok(env)
}

fn render_with_context(
    project: &Project,
    page_title: String,
    show_page_title: bool,
    authors: Vec<AuthorView>,
    published: Option<String>,
    updated: Option<String>,
    published_raw: Option<String>,
    updated_raw: Option<String>,
    tags: Vec<TagLink>,
    body_html: String,
    banner_html: Option<String>,
    comments_html: Option<String>,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    series_nav: Option<SeriesNavView>,
    redirect_href: Option<String>,
) -> Result<String> {
    let env =
        template_env(&project.config.theme.variant).context("failed to initialize templates")?;
    let template = env
        .get_template("page.html")
        .context("missing page template")?;

    let rel = root_prefix_for_base_url(&project.config.site.base_url);
    let nav_items = build_nav_view(project, current_href, &rel);
    let footer_show_stbl = project.config.footer.show_stbl;
    let footer_copyright = footer_copyright_text(&project.config.site, build_date_ymd);
    let rss_visible = project
        .config
        .rss
        .as_ref()
        .map(|rss| rss.enabled)
        .unwrap_or(false)
        && iter_visible_posts(project, None).next().is_some();
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
            home_href => resolve_root_href(&UrlMapper::new(&project.config).map("index").href, &rel),
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
            published_raw => published_raw,
            updated_raw => updated_raw,
            tags => tags,
            body_html => body_html,
            banner_html => banner_html,
            comments_html => comments_html,
            series_nav => series_nav,
            build_date_ymd => build_date_ymd,
            footer_show_stbl => footer_show_stbl,
            footer_copyright => footer_copyright,
            rss_visible => rss_visible,
            redirect_href => redirect_href,
        })
        .context("failed to render page template")
}

fn build_author_views(
    project: &Project,
    author_ids: Option<&Vec<String>>,
    asset_manifest: &AssetManifest,
    show_authors: bool,
) -> Vec<AuthorView> {
    if !show_authors {
        return Vec::new();
    }
    let people = project.config.people.as_ref();
    let resolved_ids: Vec<String> = match author_ids {
        Some(ids) if !ids.is_empty() => ids.clone(),
        _ => people
            .map(|people| vec![people.default.clone()])
            .unwrap_or_default(),
    };
    resolved_ids
        .iter()
        .map(|author_id| {
            let (name, links) = match people.and_then(|people| people.entries.get(author_id)) {
                Some(person) => {
                    let links =
                        build_author_links(person.name.as_str(), &person.links, asset_manifest);
                    (person.name.clone(), links)
                }
                None => (author_id.clone(), Vec::new()),
            };
            AuthorView { name, links }
        })
        .collect()
}

fn build_author_links(
    person_name: &str,
    links: &[PersonLink],
    asset_manifest: &AssetManifest,
) -> Vec<AuthorLinkView> {
    let mut out = Vec::new();
    for link in links {
        match resolve_author_icon(asset_manifest, link) {
            Some(icon) => out.push(AuthorLinkView {
                name: link.name.clone(),
                href: link.url.clone(),
                icon,
            }),
            None => {
                eprintln!(
                    "author link icon not found for {} ({}): {}",
                    person_name, link.id, link.url
                );
            }
        }
    }
    out
}

fn resolve_author_icon(asset_manifest: &AssetManifest, link: &PersonLink) -> Option<String> {
    let mut candidates = Vec::new();
    if let Some(icon) = link.icon.as_deref() {
        push_icon_candidates_for_token(icon, &mut candidates);
    }
    let id = normalize_icon_id(&link.id);
    for base in icon_bases_for_id(&id) {
        candidates.extend(icon_candidates_for_base(&base));
    }
    for candidate in candidates {
        if asset_manifest.entries.contains_key(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn push_icon_candidates_for_token(token: &str, out: &mut Vec<String>) {
    let trimmed = token.trim().trim_start_matches("./");
    if trimmed.is_empty() {
        return;
    }
    if trimmed.contains('/') {
        out.push(trimmed.to_string());
        if !trimmed.ends_with(".svg") {
            out.push(format!("{trimmed}.svg"));
        }
        return;
    }
    let base = trimmed.trim_end_matches(".svg");
    out.extend(icon_candidates_for_base(base));
}

fn normalize_icon_id(id: &str) -> String {
    id.trim()
        .trim_start_matches('@')
        .to_ascii_lowercase()
        .replace([' ', '_'], "-")
}

fn icon_bases_for_id(id: &str) -> Vec<String> {
    match id {
        "email" | "e-mail" | "mail" => vec!["email".to_string(), "mail".to_string()],
        "linkedin" | "linked-in" => vec!["linkedin".to_string()],
        "reddit" | "reddir" => vec!["reddit".to_string()],
        "x" | "twitter" => vec!["x".to_string(), "twitter".to_string()],
        _ => vec![id.to_string()],
    }
}

fn icon_candidates_for_base(base: &str) -> Vec<String> {
    let base = base.trim();
    if base.is_empty() {
        return Vec::new();
    }
    vec![
        format!("assets/icons/{base}.svg"),
        format!("assets/{base}.svg"),
        format!("icons/{base}.svg"),
        format!("feather/{base}.svg"),
        format!("artifacts/icons/{base}.svg"),
        format!("artifacts/{base}.svg"),
    ]
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

fn build_nav_view(project: &Project, current_href: &str, rel: &str) -> Vec<NavItemView> {
    let mapper = UrlMapper::new(&project.config);
    let nav = resolved_nav_items(project);
    let mut active_taken = false;
    nav.iter()
        .map(|item| {
            let href = nav_item_href(item, &mapper, rel);
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

fn root_prefix_for_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }
    let without_scheme = trimmed.split("://").nth(1).unwrap_or(trimmed);
    let path = match without_scheme.find('/') {
        Some(idx) => &without_scheme[idx..],
        None => "",
    };
    let path = path.trim();
    if path.is_empty() || path == "/" {
        return "/".to_string();
    }
    let mut normalized = path.to_string();
    if !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }
    if !normalized.ends_with('/') {
        normalized.push('/');
    }
    normalized
}

fn nav_item_href(item: &NavItem, mapper: &UrlMapper, rel: &str) -> String {
    if is_external_href(&item.href) || is_absolute_or_fragment_href(&item.href) {
        return item.href.clone();
    }
    resolve_root_href(&mapper.map(&item.href).href, rel)
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

fn resolve_root_href(href: &str, rel: &str) -> String {
    if rel.is_empty() || is_external_href(href) || is_absolute_or_fragment_href(href) {
        return href.to_string();
    }
    format!("{rel}{}", href.trim_start_matches('/'))
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
    if let Some(title) = crate::title::deduce_title_from_source_path(&page.source_path) {
        return title;
    }
    project.config.site.title.clone()
}

pub fn format_timestamp_rfc3339(value: Option<i64>) -> Option<String> {
    let value = value?;
    let dt = DateTime::<Utc>::from_timestamp(value, 0)?;
    Some(dt.to_rfc3339())
}

pub fn format_timestamp_display(
    value: Option<i64>,
    system: Option<&SystemConfig>,
    timezone: Option<&str>,
) -> Option<String> {
    let value = normalize_timestamp(value, system)?;
    let date = system.and_then(|system| system.date.as_ref());
    let format = date
        .map(|date| date.format.trim())
        .filter(|format| !format.is_empty())
        .unwrap_or(DEFAULT_DATE_FORMAT);
    let dt = DateTime::<Utc>::from_timestamp(value, 0)?;
    match resolve_timezone(timezone) {
        ResolvedTimezone::Utc => Some(dt.with_timezone(&Utc).format(format).to_string()),
        ResolvedTimezone::Fixed(offset) => {
            Some(dt.with_timezone(&offset).format(format).to_string())
        }
        ResolvedTimezone::Named(tz) => Some(dt.with_timezone(&tz).format(format).to_string()),
    }
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

pub fn normalize_timestamp(value: Option<i64>, system: Option<&SystemConfig>) -> Option<i64> {
    let value = value?;
    let roundup = date_roundup_seconds(system);
    Some(round_timestamp(value, roundup))
}

pub fn effective_updated_timestamp(published: Option<i64>, updated: Option<i64>) -> Option<i64> {
    match (published, updated) {
        (Some(published), Some(updated)) if updated <= published => None,
        (_, other) => other,
    }
}

fn date_roundup_seconds(system: Option<&SystemConfig>) -> u32 {
    system
        .and_then(|system| system.date.as_ref())
        .map(|date| date.roundup_seconds)
        .unwrap_or(0)
}

fn round_timestamp(value: i64, roundup_seconds: u32) -> i64 {
    if roundup_seconds == 0 {
        return value;
    }
    let round = i64::from(roundup_seconds);
    value.div_euclid(round) * round
}

enum ResolvedTimezone {
    Utc,
    Fixed(FixedOffset),
    Named(Tz),
}

fn resolve_timezone(timezone: Option<&str>) -> ResolvedTimezone {
    let Some(value) = timezone.map(str::trim).filter(|value| !value.is_empty()) else {
        return ResolvedTimezone::Utc;
    };
    let upper = value.to_ascii_uppercase();
    if upper == "UTC" || upper == "GMT" || upper == "Z" {
        return ResolvedTimezone::Utc;
    }
    if let Ok(named) = value.parse::<Tz>() {
        return ResolvedTimezone::Named(named);
    }
    let (sign, rest) = match upper.as_bytes().first() {
        Some(b'+') => (1, &value[1..]),
        Some(b'-') => (-1, &value[1..]),
        _ => return ResolvedTimezone::Utc,
    };
    let digits: String = rest.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.len() != 2 && digits.len() != 4 {
        return ResolvedTimezone::Utc;
    }
    let (hours, minutes) = if digits.len() == 2 {
        (digits.parse::<i32>().ok(), Some(0))
    } else {
        let (h, m) = digits.split_at(2);
        (h.parse::<i32>().ok(), m.parse::<i32>().ok())
    };
    let (Some(hours), Some(minutes)) = (hours, minutes) else {
        return ResolvedTimezone::Utc;
    };
    if hours > 23 || minutes > 59 {
        return ResolvedTimezone::Utc;
    }
    let total = sign * (hours * 3600 + minutes * 60);
    FixedOffset::east_opt(total)
        .map(ResolvedTimezone::Fixed)
        .unwrap_or(ResolvedTimezone::Utc)
}

#[cfg(test)]
mod tests {
    use super::{
        BlogIndexItem, NavItemView, SeriesIndexPart, SeriesNavEntry, SeriesNavLink, SeriesNavView,
        SystemConfig, TagListingPage, build_nav_view, effective_updated_timestamp,
        format_timestamp_display, format_timestamp_ymd, render_blog_index, render_page,
        render_page_with_series_nav, render_series_index, render_tag_index,
    };
    use crate::assets::AssetManifest;
    use crate::config::load_site_config;
    use crate::header::Header;
    use crate::model::{DateConfig, DocId, Page, Project, SiteContent};
    use chrono::TimeZone;
    use std::fs;
    use std::path::PathBuf;
    use uuid::Uuid;

    #[test]
    fn format_timestamp_ymd_outputs_date_only() {
        let value = format_timestamp_ymd(Some(1_704_153_600)).expect("date");
        assert_eq!(value, "2024-01-02");
    }

    #[test]
    fn format_timestamp_display_defaults_and_rounds() {
        let ts = chrono::Utc
            .with_ymd_and_hms(2025, 7, 4, 17, 2, 29)
            .unwrap()
            .timestamp();
        let value = format_timestamp_display(Some(ts), None, None).expect("date");
        assert_eq!(value, "July 4, 2025 at 17:02 UTC");

        let config = SystemConfig {
            date: Some(DateConfig {
                format: "%Y-%m-%d %H:%M %Z".to_string(),
                roundup_seconds: 3600,
            }),
        };
        let value = format_timestamp_display(Some(ts), Some(&config), None).expect("date");
        assert_eq!(value, "2025-07-04 17:00 UTC");

        let value = format_timestamp_display(Some(ts), None, Some("+02:00")).expect("date");
        assert_eq!(value, "July 4, 2025 at 19:02 +02:00");

        let value = format_timestamp_display(Some(ts), None, Some("Europe/Oslo")).expect("date");
        assert_eq!(value, "July 4, 2025 at 19:02 CEST");
    }

    #[test]
    fn effective_updated_timestamp_requires_updated_after_published() {
        assert_eq!(effective_updated_timestamp(Some(200), Some(200)), None);
        assert_eq!(effective_updated_timestamp(Some(200), Some(199)), None);
        assert_eq!(effective_updated_timestamp(Some(200), Some(201)), Some(201));
        assert_eq!(effective_updated_timestamp(None, Some(201)), Some(201));
    }

    #[test]
    fn nav_ordering_preserves_config() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n  nav:\n    - label: \"Home\"\n      href: \"index\"\n    - label: \"Blog\"\n      href: \"blog\"\n",
            SiteContent::default(),
        );
        let items = build_nav_view(&project, "blog.html", "/");
        assert_nav_labels(&items, &["Home", "Blog"]);
        assert_eq!(items[0].href, "/index.html");
        assert_eq!(items[1].href, "/blog.html");
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
        let items = build_nav_view(&project, "index.html", "/");
        assert_nav_labels(&items, &["Home", "Blog", "Tags"]);
        assert_eq!(items[0].href, "/index.html");
        assert_eq!(items[1].href, "/index.html");
        assert_eq!(items[2].href, "/tags.html");
    }

    #[test]
    fn nav_active_is_exact_match_only() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n  nav:\n    - label: \"Home\"\n      href: \"index\"\n    - label: \"Blog\"\n      href: \"blog\"\n",
            SiteContent::default(),
        );
        let items = build_nav_view(&project, "blog/page1.html", "/");
        assert_eq!(active_count(&items), 0);

        let items = build_nav_view(&project, "blog.html", "/");
        assert_eq!(active_count(&items), 1);
        assert!(items[1].is_active);
    }

    #[test]
    fn nav_active_is_first_match_only() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n  nav:\n    - label: \"Home\"\n      href: \"index\"\n    - label: \"Home Duplicate\"\n      href: \"index\"\n",
            SiteContent::default(),
        );
        let items = build_nav_view(&project, "index.html", "/");
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
        let items = build_nav_view(&project, "index.html", "/");
        assert_nav_labels(&items, &["Home", "Blog", "Tags"]);
        assert_eq!(items[0].href, "/index.html");
        assert_eq!(items[1].href, "/index.html");
        assert_eq!(items[2].href, "/tags.html");
    }

    #[test]
    fn nav_href_mapping_rules() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n  nav:\n    - label: \"About\"\n      href: \"about\"\n    - label: \"Absolute\"\n      href: \"/about.html\"\n    - label: \"External\"\n      href: \"https://example.com/\"\n    - label: \"Top\"\n      href: \"#top\"\n",
            SiteContent::default(),
        );
        let items = build_nav_view(&project, "index.html", "/");
        assert_eq!(items[0].href, "/about.html");
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
            None,
        )
        .expect("render");
        assert!(html.contains("<div class=\"meta page-meta\">"));
        assert!(!html.contains("Tags:"));
        assert!(html.contains("class=\"tags page-tags\""));
    }

    #[test]
    fn page_meta_uses_updated_fallback_when_available() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent::default(),
        );
        let mut page = simple_page("Meta Fallback", "articles/meta-fallback.md");
        page.header.published = Some(1_704_067_200);
        page.header.updated_fallback = Some(1_704_153_600);
        let html = render_page(
            &project,
            &page,
            &default_manifest(),
            "meta-fallback.html",
            "2026-01-29",
            None,
            None,
        )
        .expect("render page");
        assert!(html.contains("Updated"));

        page.header.updated_disabled = true;
        let html = render_page(
            &project,
            &page,
            &default_manifest(),
            "meta-fallback.html",
            "2026-01-29",
            None,
            None,
        )
        .expect("render page");
        assert!(!html.contains("Updated"));
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
            None,
        )
        .expect("render");
        assert!(html.contains("<title>Download · Demo</title>"));
        assert!(!html.contains("<title>Untitled · Demo</title>"));
    }

    #[test]
    fn page_tags_use_canonical_case() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent {
                pages: Vec::new(),
                series: Vec::new(),
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        );
        let mut page_a = simple_page("A", "articles/a.md");
        page_a.header.is_published = true;
        page_a.header.tags = vec!["grpc".to_string()];
        let mut page_b = simple_page("B", "articles/b.md");
        page_b.header.is_published = true;
        page_b.header.tags = vec!["gRPC".to_string()];
        let mut project = project;
        project.content.pages = vec![page_a.clone(), page_b];

        let html = render_page(
            &project,
            &page_a,
            &default_manifest(),
            "a.html",
            "2026-01-29",
            None,
            None,
        )
        .expect("render page");
        assert!(html.contains(">gRPC<"));
    }

    #[test]
    fn cover_pages_hide_header_meta() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\npeople:\n  default: me\n  entries:\n    me:\n      name: \"Me\"\n",
            SiteContent::default(),
        );
        let mut page = simple_page("Home", "articles/index.md");
        page.header.published = Some(1_704_067_200);
        page.header.tags = vec!["rust".to_string()];
        let html = render_page(
            &project,
            &page,
            &default_manifest(),
            "index.html",
            "2026-01-29",
            None,
            None,
        )
        .expect("render page");
        assert!(!html.contains("Published"));
        assert!(!html.contains("meta-authors"));
        assert!(!html.contains("class=\"tags page-tags\""));

        let mut info_page = simple_page("Info", "articles/about.md");
        info_page.header.template = Some(crate::header::TemplateId::Info);
        info_page.header.published = Some(1_704_067_200);
        info_page.header.tags = vec!["about".to_string()];
        let html = render_page(
            &project,
            &info_page,
            &default_manifest(),
            "about.html",
            "2026-01-29",
            None,
            None,
        )
        .expect("render page");
        assert!(!html.contains("Published"));
        assert!(!html.contains("meta-authors"));
        assert!(!html.contains("class=\"tags page-tags\""));
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
            published_raw: None,
            updated_raw: None,
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
            None,
        )
        .expect("render page");
        assert!(!html.contains("class=\"series-nav\""));

        let nav = SeriesNavView {
            index: SeriesNavLink {
                title: "Series".to_string(),
                href: "series.html".to_string(),
            },
            parts: vec![
                SeriesNavEntry {
                    title: "Part 1 Title".to_string(),
                    href: "part1.html".to_string(),
                    is_current: true,
                },
                SeriesNavEntry {
                    title: "Part 2 Title".to_string(),
                    href: "part2.html".to_string(),
                    is_current: false,
                },
            ],
        };
        let html = render_page_with_series_nav(
            &project,
            &page,
            &default_manifest(),
            Some(nav),
            "part1.html",
            "2026-01-29",
            None,
            None,
        )
        .expect("render page");
        assert!(html.contains("class=\"series-nav\""));
        assert!(html.contains("series.html"));
        assert!(html.contains("part2.html"));
        assert!(html.contains("Part 1 Title"));
    }

    #[test]
    fn series_index_parts_render_in_order() {
        let project = project_with_config(
            "site:\n  id: \"demo\"\n  title: \"Demo\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
            SiteContent::default(),
        );
        let mut index = simple_page("Series", "articles/series/index.md");
        index.banner_name = Some("series-banner.svg".to_string());
        let parts = vec![
            SeriesIndexPart {
                title: "Part 1".to_string(),
                href: "part1.html".to_string(),
                published_display: Some("2024-01-01".to_string()),
                published_raw: None,
            },
            SeriesIndexPart {
                title: "Part 2".to_string(),
                href: "part2.html".to_string(),
                published_display: Some("2024-01-02".to_string()),
                published_raw: None,
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
        assert!(html.contains("<div class=\"banner\">"));
        assert!(html.contains("images/series-banner.svg"));
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
