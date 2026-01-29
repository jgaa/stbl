use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use stbl_core::blog_index::{
    FeedItem, blog_index_page_logical_key, blog_pagination_settings, collect_blog_feed,
    collect_tag_feed, paginate_blog_index,
};
use stbl_core::feeds::{render_rss, render_sitemap};
use stbl_core::model::{BuildPlan, DocId, Page, Project, Series, TaskKind};
use stbl_core::render::render_markdown_to_html;
use stbl_core::templates::{
    BlogIndexItem, BlogIndexPart, TagListingPage, format_timestamp_ymd, render_blog_index,
    render_markdown_page, render_page, render_tag_index,
};
use stbl_core::url::{UrlMapper, logical_key_from_source_path};

#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct ExecReport {
    pub executed: usize,
    pub skipped: usize,
}

pub fn execute_plan(project: &Project, plan: &BuildPlan, out_dir: &PathBuf) -> Result<ExecReport> {
    let mut report = ExecReport::default();
    let mapper = UrlMapper::new(&project.config);
    for task in &plan.tasks {
        if matches!(task.kind, TaskKind::GenerateRss)
            && !project.config.rss.as_ref().is_some_and(|rss| rss.enabled)
        {
            continue;
        }
        for output in &task.outputs {
            let out_path = out_dir.join(&output.path);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            let contents = render_output(project, &mapper, &task.kind, &output.path)
                .with_context(|| format!("failed to render {}", out_path.display()))?;
            fs::write(&out_path, contents)
                .with_context(|| format!("failed to write {}", out_path.display()))?;
            report.executed += 1;
        }
    }
    Ok(report)
}

fn render_output(
    project: &Project,
    mapper: &UrlMapper,
    kind: &TaskKind,
    output_path: &PathBuf,
) -> Result<String> {
    match output_path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => render_html_output(project, mapper, kind, output_path),
        Some("xml") => render_xml_output(project, mapper, kind),
        _ => Ok(String::new()),
    }
}

fn render_html_output(
    project: &Project,
    mapper: &UrlMapper,
    kind: &TaskKind,
    output_path: &PathBuf,
) -> Result<String> {
    if let Some(mapping) = mapping_for_task(project, mapper, kind)? {
        if output_path == &mapping.primary_output {
            return render_primary_html(project, kind);
        }
        if mapping
            .fallback
            .as_ref()
            .is_some_and(|redirect| output_path == &redirect.from)
        {
            return Ok(render_redirect_stub(&mapping.href));
        }
    }

    render_primary_html(project, kind)
}

fn render_xml_output(project: &Project, mapper: &UrlMapper, kind: &TaskKind) -> Result<String> {
    match kind {
        TaskKind::GenerateRss => Ok(render_rss(project, mapper)),
        TaskKind::GenerateSitemap => Ok(render_sitemap(project, mapper)),
        _ => Ok(String::new()),
    }
}

fn render_primary_html(project: &Project, kind: &TaskKind) -> Result<String> {
    match kind {
        TaskKind::RenderPage { page } => render_page_by_id(project, *page),
        TaskKind::RenderBlogIndex {
            source_page,
            page_no,
        } => render_blog_index_page(project, source_page, *page_no),
        TaskKind::RenderSeries { series } => render_series(project, *series),
        TaskKind::RenderTagIndex { tag } => render_tag_index_page(project, tag),
        TaskKind::RenderTagsIndex => {
            render_markdown_page(project, Some("Tags"), "*Not implemented.*\n")
        }
        TaskKind::RenderFrontPage => {
            let title = project.config.site.title.clone();
            render_markdown_page(project, Some(&title), "*Not implemented.*\n")
        }
        _ => render_markdown_page(project, Some("Not implemented"), "*Not implemented.*\n"),
    }
}

fn render_page_by_id(project: &Project, page_id: DocId) -> Result<String> {
    let page =
        find_page(project, page_id).ok_or_else(|| anyhow!("page not found for render task"))?;
    render_page(project, page)
}

fn render_series(project: &Project, series_id: stbl_core::model::SeriesId) -> Result<String> {
    let series = find_series(project, series_id)
        .ok_or_else(|| anyhow!("series not found for render task"))?;
    render_page(project, &series.index)
}

fn find_page(project: &Project, page_id: DocId) -> Option<&Page> {
    if let Some(page) = project.content.pages.iter().find(|page| page.id == page_id) {
        return Some(page);
    }
    for series in &project.content.series {
        if series.index.id == page_id {
            return Some(&series.index);
        }
        if let Some(page) = series
            .parts
            .iter()
            .find(|part| part.page.id == page_id)
            .map(|part| &part.page)
        {
            return Some(page);
        }
    }
    None
}

fn find_series(project: &Project, series_id: stbl_core::model::SeriesId) -> Option<&Series> {
    project
        .content
        .series
        .iter()
        .find(|series| series.id == series_id)
}

fn mapping_for_task(
    project: &Project,
    mapper: &UrlMapper,
    kind: &TaskKind,
) -> Result<Option<stbl_core::url::UrlMapping>> {
    let logical_key = match kind {
        TaskKind::RenderPage { page } => {
            let page = find_page(project, *page)
                .ok_or_else(|| anyhow!("page not found for render task"))?;
            logical_key_from_source_path(&page.source_path)
        }
        TaskKind::RenderBlogIndex {
            source_page,
            page_no,
        } => {
            let page = find_page(project, *source_page)
                .ok_or_else(|| anyhow!("blog index page not found"))?;
            let base_key = logical_key_from_source_path(&page.source_path);
            blog_index_page_logical_key(&base_key, *page_no)
        }
        TaskKind::RenderSeries { series } => {
            let series = find_series(project, *series)
                .ok_or_else(|| anyhow!("series not found for render task"))?;
            logical_key_from_source_path(&series.dir_path)
        }
        TaskKind::RenderTagIndex { tag } => format!("tags/{tag}"),
        TaskKind::RenderTagsIndex => "tags".to_string(),
        TaskKind::RenderFrontPage => "index".to_string(),
        _ => return Ok(None),
    };
    Ok(Some(mapper.map(&logical_key)))
}

fn render_redirect_stub(href: &str) -> String {
    let target = format!("/{}", href.trim_start_matches('/'));
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n  <meta charset=\"utf-8\">\n  <meta http-equiv=\"refresh\" content=\"0; url={target}\">\n</head>\n<body>\n  <a href=\"{target}\">{target}</a>\n</body>\n</html>\n"
    )
}

fn render_blog_index_page(
    project: &Project,
    source_page_id: &DocId,
    page_no: u32,
) -> Result<String> {
    let mapper = UrlMapper::new(&project.config);
    let source_page =
        find_page(project, *source_page_id).ok_or_else(|| anyhow!("blog index page not found"))?;

    let feed_items = collect_blog_feed(project, source_page.id);
    let base_key = logical_key_from_source_path(&source_page.source_path);
    let pagination = blog_pagination_settings(project);
    let page_ranges = paginate_blog_index(pagination, &base_key, feed_items.len());
    let page_range = page_ranges
        .iter()
        .find(|page| page.page_no == page_no)
        .ok_or_else(|| anyhow!("blog index page out of range"))?;
    let (start, end) = (page_range.start, page_range.end);
    let items = feed_items[start..end]
        .iter()
        .map(|item| map_feed_item(item, &mapper))
        .collect::<Vec<_>>();

    let intro_html = if page_no == 1 && !source_page.body_markdown.trim().is_empty() {
        Some(render_markdown_to_html(&source_page.body_markdown))
    } else {
        None
    };

    let title = source_page.header.title.clone();
    let prev_href = page_range.prev_key.as_ref().map(|key| mapper.map(key).href);
    let next_href = page_range.next_key.as_ref().map(|key| mapper.map(key).href);

    render_blog_index(
        project,
        title,
        intro_html,
        items,
        prev_href,
        next_href,
        page_range.page_no,
        page_range.total_pages,
    )
}

fn render_tag_index_page(project: &Project, tag: &str) -> Result<String> {
    let mapper = UrlMapper::new(&project.config);
    let feed_items = collect_tag_feed(project, tag);
    let items = feed_items
        .iter()
        .map(|item| map_feed_item(item, &mapper))
        .collect::<Vec<_>>();
    let listing = TagListingPage {
        tag: tag.to_string(),
        items,
    };
    render_tag_index(project, listing)
}

fn map_feed_item(item: &FeedItem, mapper: &UrlMapper) -> BlogIndexItem {
    match item {
        FeedItem::Post(post) => BlogIndexItem {
            title: post.title.clone(),
            href: mapper.map(&post.logical_key).href,
            published_display: format_timestamp_ymd(post.published),
            kind_label: None,
            abstract_text: post.abstract_text.clone(),
            latest_parts: Vec::new(),
        },
        FeedItem::Series(series) => BlogIndexItem {
            title: series.title.clone(),
            href: mapper.map(&series.logical_key).href,
            published_display: format_timestamp_ymd(series.published),
            kind_label: Some("Series".to_string()),
            abstract_text: series.abstract_text.clone(),
            latest_parts: series
                .latest_parts
                .iter()
                .map(|part| BlogIndexPart {
                    title: part.title.clone(),
                    href: mapper.map(&part.logical_key).href,
                    published_display: format_timestamp_ymd(part.published),
                })
                .collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stbl_core::assemble::assemble_site;
    use stbl_core::config::load_site_config;
    use stbl_core::header::UnknownKeyPolicy;
    use stbl_core::model::{Project, UrlStyle};
    use tempfile::TempDir;

    fn fixture_root(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("stbl_core")
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn build_project_at(root: PathBuf, url_style: UrlStyle) -> Project {
        let config_path = root.join("stbl.yaml");
        let mut config = load_site_config(&config_path).expect("load config");
        config.site.url_style = url_style;
        let docs =
            crate::walk::walk_content(&root, &root.join("articles"), UnknownKeyPolicy::Error)
                .expect("walk content");
        let content = assemble_site(docs).expect("assemble site");
        Project {
            root,
            config,
            content,
        }
    }

    fn build_project(url_style: UrlStyle) -> Project {
        build_project_at(fixture_root("site1"), url_style)
    }

    fn build_into_temp(url_style: UrlStyle) -> (TempDir, PathBuf) {
        let project = build_project(url_style);
        let plan = stbl_core::plan::build_plan(&project);
        let temp = TempDir::new().expect("tempdir");
        let out_dir = temp.path().join("out");
        execute_plan(&project, &plan, &out_dir).expect("execute plan");
        (temp, out_dir)
    }

    #[test]
    fn html_style_writes_flat_html() {
        let (_temp, out_dir) = build_into_temp(UrlStyle::Html);
        assert!(out_dir.join("page1.html").exists());
        assert!(!out_dir.join("page1").join("index.html").exists());
    }

    #[test]
    fn blog_index_lists_pages() {
        let (_temp, out_dir) = build_into_temp(UrlStyle::Html);
        let index_html = fs::read_to_string(out_dir.join("index.html")).expect("read index");
        assert!(index_html.contains("page1.html"));
        assert!(index_html.contains("page2.html"));
        assert!(!index_html.contains("info.html"));
    }

    #[test]
    fn pretty_style_writes_index_html() {
        let (_temp, out_dir) = build_into_temp(UrlStyle::Pretty);
        assert!(out_dir.join("page1").join("index.html").exists());
        assert!(!out_dir.join("page1.html").exists());
    }

    #[test]
    fn pretty_with_fallback_writes_redirect_stub() {
        let (_temp, out_dir) = build_into_temp(UrlStyle::PrettyWithFallback);
        let index_path = out_dir.join("page1").join("index.html");
        let fallback_path = out_dir.join("page1.html");
        assert!(index_path.exists());
        assert!(fallback_path.exists());
        let contents = fs::read_to_string(fallback_path).expect("read fallback");
        assert!(contents.contains("http-equiv=\"refresh\""));
        assert!(contents.contains("href=\"/page1/\""));
    }

    #[test]
    fn pagination_fixture_generates_multiple_blog_pages() {
        let project = build_project_at(fixture_root("site-pagination"), UrlStyle::Html);
        let plan = stbl_core::plan::build_plan(&project);
        let temp = TempDir::new().expect("tempdir");
        let out_dir = temp.path().join("out");
        execute_plan(&project, &plan, &out_dir).expect("execute plan");

        assert!(out_dir.join("index.html").exists());
        assert!(out_dir.join("page/2.html").exists());
        assert!(out_dir.join("page/3.html").exists());
        assert!(out_dir.join("page/4.html").exists());

        let index_html = fs::read_to_string(out_dir.join("index.html")).expect("read index");
        assert!(index_html.contains("series.html"));
        assert!(index_html.contains("series&#x2f;part5.html"));
        assert!(index_html.contains("Part 5"));
        assert!(index_html.contains("Part 4"));
        assert!(index_html.contains("Part 3"));
        assert!(!index_html.contains("Part 2"));
        assert!(index_html.contains("Series abstract override."));
        assert!(index_html.contains("2024-01-15"));
        assert!(!index_html.contains("T10:00:00"));
        assert!(!index_html.contains("<span class=\"meta\"></span>"));

        let page2_html = fs::read_to_string(out_dir.join("page/2.html")).expect("read page2");
        assert!(page2_html.contains("page&#x2f;3.html"));
        assert!(page2_html.contains("index.html"));
        assert!(!page2_html.contains("series.html"));
        assert!(!page2_html.contains("<span class=\"meta\"></span>"));

        let page4_html = fs::read_to_string(out_dir.join("page/4.html")).expect("read page4");
        assert!(page4_html.contains("Custom abstract for page 1"));
        assert!(page4_html.contains("First paragraph for auto-abstract."));
        assert!(!page4_html.contains("Series"));
    }
}
