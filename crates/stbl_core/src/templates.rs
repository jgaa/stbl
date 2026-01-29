use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use minijinja::{AutoEscape, Environment, context};

use crate::model::{MenuItem, Page, Project};
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

    let home_href = UrlMapper::new(&project.config).map("index").href;
    let menu = build_menu_view(&project.config.menu, current_href, &home_href);

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site_language => project.config.site.language.clone(),
            home_href => home_href,
            menu => menu,
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
    let home_href = UrlMapper::new(&project.config).map("index").href;
    let menu = build_menu_view(&project.config.menu, current_href, &home_href);

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site_language => project.config.site.language.clone(),
            home_href => home_href,
            menu => menu,
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

    let home_href = UrlMapper::new(&project.config).map("index").href;
    let menu = build_menu_view(&project.config.menu, current_href, &home_href);

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            site_language => project.config.site.language.clone(),
            home_href => home_href,
            menu => menu,
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
pub struct MenuItemView {
    pub title: String,
    pub href: String,
    pub is_active: bool,
}

fn build_menu_view(menu: &[MenuItem], current_href: &str, home_href: &str) -> Vec<MenuItemView> {
    menu.iter()
        .map(|item| MenuItemView {
            title: item.title.clone(),
            href: item.href.clone(),
            is_active: menu_item_active(&item.href, current_href, home_href),
        })
        .collect()
}

fn menu_item_active(menu_href: &str, current_href: &str, home_href: &str) -> bool {
    let mut menu_norm = normalize_href(menu_href);
    let current_norm = normalize_href(current_href);
    if menu_norm.is_empty() {
        menu_norm = normalize_href(home_href);
    }
    menu_norm == current_norm
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
    use super::format_timestamp_ymd;

    #[test]
    fn format_timestamp_ymd_outputs_date_only() {
        let value = format_timestamp_ymd(Some(1_704_153_600)).expect("date");
        assert_eq!(value, "2024-01-02");
    }
}
