use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use minijinja::{AutoEscape, Environment, context};

use crate::model::{MenuItem, Page, Project};
use crate::render::render_markdown_to_html;
use serde::Serialize;

const BASE_TEMPLATE: &str = include_str!("templates/base.html");
const PAGE_TEMPLATE: &str = include_str!("templates/page.html");
const BLOG_INDEX_TEMPLATE: &str = include_str!("templates/blog_index.html");

pub fn render_page(project: &Project, page: &Page) -> Result<String> {
    let body_html = render_markdown_to_html(&page.body_markdown);
    let menu: Vec<MenuItem> = project.config.menu.clone();
    let page_title = page.header.title.clone();
    let authors = page.header.authors.clone();
    let tags = page.header.tags.clone();
    let published = format_timestamp_rfc3339(page.header.published);
    let updated = format_timestamp_rfc3339(page.header.updated);

    render_with_context(
        project, menu, page_title, authors, published, updated, tags, body_html,
    )
}

pub fn render_markdown_page(
    project: &Project,
    title: Option<&str>,
    body_markdown: &str,
) -> Result<String> {
    let body_html = render_markdown_to_html(body_markdown);
    let menu: Vec<MenuItem> = project.config.menu.clone();
    let page_title = title.map(|value| value.to_string());
    render_with_context(
        project,
        menu,
        page_title,
        None,
        None,
        None,
        Vec::new(),
        body_html,
    )
}

#[derive(Debug, Clone, Serialize)]
pub struct BlogIndexPart {
    pub title: String,
    pub href: String,
    pub published: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlogIndexItem {
    pub title: String,
    pub href: String,
    pub published: String,
    pub kind_label: String,
    pub summary_html: String,
    pub latest_parts: Vec<BlogIndexPart>,
}

pub fn render_blog_index(
    project: &Project,
    title: Option<String>,
    intro_html: Option<String>,
    items: Vec<BlogIndexItem>,
    prev_href: Option<String>,
    next_href: Option<String>,
    page_no: u32,
    total_pages: u32,
) -> Result<String> {
    let menu: Vec<MenuItem> = project.config.menu.clone();
    let env = template_env().context("failed to initialize templates")?;
    let template = env
        .get_template("blog_index.html")
        .context("missing blog_index template")?;

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            menu => menu,
            page_title => title,
            intro_html => intro_html,
            items => items,
            prev_href => prev_href,
            next_href => next_href,
            page_no => page_no,
            total_pages => total_pages,
        })
        .context("failed to render blog index template")
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
    Ok(env)
}

fn render_with_context(
    project: &Project,
    menu: Vec<MenuItem>,
    page_title: Option<String>,
    authors: Option<Vec<String>>,
    published: Option<String>,
    updated: Option<String>,
    tags: Vec<String>,
    body_html: String,
) -> Result<String> {
    let env = template_env().context("failed to initialize templates")?;
    let template = env
        .get_template("page.html")
        .context("missing page template")?;

    template
        .render(context! {
            site_title => project.config.site.title.clone(),
            menu => menu,
            page_title => page_title,
            authors => authors,
            published => published,
            updated => updated,
            tags => tags,
            body_html => body_html,
        })
        .context("failed to render page template")
}

pub fn format_timestamp_rfc3339(value: Option<i64>) -> Option<String> {
    let value = value?;
    let dt = DateTime::<Utc>::from_timestamp(value, 0)?;
    Some(dt.to_rfc3339())
}
