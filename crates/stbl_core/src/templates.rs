use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use minijinja::{AutoEscape, Environment, context};

use crate::model::{NavItem, Page, Project};
use crate::render::render_markdown_to_html;
use crate::url::UrlMapper;
use serde::Serialize;

const BASE_TEMPLATE: &str = include_str!("templates/base.html");
const PAGE_TEMPLATE: &str = include_str!("templates/page.html");
const BLOG_INDEX_TEMPLATE: &str = include_str!("templates/blog_index.html");
const TAG_INDEX_TEMPLATE: &str = include_str!("templates/tag_index.html");

pub fn render_page(
    project: &Project,
    page: &Page,
    current_href: &str,
    build_date_ymd: &str,
) -> Result<String> {
    let body_html = render_markdown_to_html(&page.body_markdown);
    let page_title = page
        .header
        .title
        .clone()
        .unwrap_or_else(|| "Untitled".to_string());
    let authors = page.header.authors.clone();
    let tags = page.header.tags.clone();
    let published = format_timestamp_rfc3339(page.header.published);
    let updated = format_timestamp_rfc3339(page.header.updated);

    render_with_context(
        project,
        page_title,
        authors,
        published,
        updated,
        tags,
        body_html,
        current_href,
        build_date_ymd,
        None,
    )
}

pub fn render_markdown_page(
    project: &Project,
    title: &str,
    body_markdown: &str,
    current_href: &str,
    build_date_ymd: &str,
    redirect_href: Option<&str>,
) -> Result<String> {
    let body_html = render_markdown_to_html(body_markdown);
    render_with_context(
        project,
        title.to_string(),
        None,
        None,
        None,
        Vec::new(),
        body_html,
        current_href,
        build_date_ymd,
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
pub struct BlogIndexItem {
    pub title: String,
    pub href: String,
    pub published_display: Option<String>,
    pub kind_label: Option<String>,
    pub abstract_text: Option<String>,
    pub latest_parts: Vec<BlogIndexPart>,
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
    items: Vec<BlogIndexItem>,
    prev_href: Option<String>,
    next_href: Option<String>,
    page_no: u32,
    total_pages: u32,
    current_href: &str,
    build_date_ymd: &str,
) -> Result<String> {
    let env = template_env().context("failed to initialize templates")?;
    let template = env
        .get_template("blog_index.html")
        .context("missing blog_index template")?;

    let nav_items = build_nav_view(project, current_href);

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site_language => project.config.site.language.clone(),
            home_href => UrlMapper::new(&project.config).map("index").href,
            nav_items => nav_items,
            page_title => title,
            intro_html => intro_html,
            items => items,
            prev_href => prev_href,
            next_href => next_href,
            page_no => page_no,
            total_pages => total_pages,
            build_date_ymd => build_date_ymd,
            redirect_href => Option::<String>::None,
        })
        .context("failed to render blog index template")
}

pub fn render_tag_index(
    project: &Project,
    listing: TagListingPage,
    current_href: &str,
    build_date_ymd: &str,
) -> Result<String> {
    let env = template_env().context("failed to initialize templates")?;
    let template = env
        .get_template("tag_index.html")
        .context("missing tag_index template")?;

    let page_title = format!("Tag: {}", listing.tag);
    let nav_items = build_nav_view(project, current_href);

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site_language => project.config.site.language.clone(),
            home_href => UrlMapper::new(&project.config).map("index").href,
            nav_items => nav_items,
            page_title => page_title,
            tag => listing.tag,
            items => listing.items,
            build_date_ymd => build_date_ymd,
            redirect_href => Option::<String>::None,
        })
        .context("failed to render tag index template")
}

pub fn render_redirect_page(
    project: &Project,
    target_href: &str,
    current_href: &str,
    build_date_ymd: &str,
) -> Result<String> {
    let body = format!("Redirecting to [{target_href}]({target_href}).\n");
    render_markdown_page(
        project,
        "Redirecting",
        &body,
        current_href,
        build_date_ymd,
        Some(target_href),
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
    Ok(env)
}

fn render_with_context(
    project: &Project,
    page_title: String,
    authors: Option<Vec<String>>,
    published: Option<String>,
    updated: Option<String>,
    tags: Vec<String>,
    body_html: String,
    current_href: &str,
    build_date_ymd: &str,
    redirect_href: Option<String>,
) -> Result<String> {
    let env = template_env().context("failed to initialize templates")?;
    let template = env
        .get_template("page.html")
        .context("missing page template")?;

    let nav_items = build_nav_view(project, current_href);

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site_language => project.config.site.language.clone(),
            home_href => UrlMapper::new(&project.config).map("index").href,
            nav_items => nav_items,
            page_title => page_title,
            authors => authors,
            published => published,
            updated => updated,
            tags => tags,
            body_html => body_html,
            build_date_ymd => build_date_ymd,
            redirect_href => redirect_href,
        })
        .context("failed to render page template")
}

#[derive(Debug, Clone, Serialize)]
pub struct NavItemView {
    pub label: String,
    pub href: String,
    pub is_active: bool,
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

#[cfg(test)]
mod tests {
    use super::{NavItemView, build_nav_view, format_timestamp_ymd};
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

    fn project_with_config(config: &str, content: SiteContent) -> Project {
        let path = write_temp_config(config);
        let config = load_site_config(&path).expect("config");
        Project {
            root: PathBuf::from("."),
            config,
            content,
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
            url_path: String::new(),
            content_hash: blake3::hash(source_path.as_bytes()),
        }
    }

    fn assert_nav_labels(items: &[NavItemView], expected: &[&str]) {
        let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
        assert_eq!(labels, expected);
    }

    fn active_count(items: &[NavItemView]) -> usize {
        items.iter().filter(|item| item.is_active).count()
    }
}
