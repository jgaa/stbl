use crate::header::TemplateId;
use crate::model::{DocId, Page, Project, Series, SeriesId};
use crate::render::render_markdown_to_html;
use crate::url::logical_key_from_source_path;

#[derive(Debug, Clone)]
pub enum FeedItem {
    Post(FeedPost),
    Series(FeedSeries),
}

#[derive(Debug, Clone)]
pub struct FeedPost {
    pub page_id: DocId,
    pub logical_key: String,
    pub title: String,
    pub published: Option<i64>,
    pub sort_date: i64,
    pub content_hash: blake3::Hash,
}

#[derive(Debug, Clone)]
pub struct FeedSeries {
    pub series_id: SeriesId,
    pub logical_key: String,
    pub title: String,
    pub published: Option<i64>,
    pub sort_date: i64,
    pub abstract_html: String,
    pub latest_parts: Vec<FeedSeriesPart>,
    pub index_id: DocId,
    pub index_hash: blake3::Hash,
    pub part_ids: Vec<DocId>,
    pub part_hashes: Vec<blake3::Hash>,
}

#[derive(Debug, Clone)]
pub struct FeedSeriesPart {
    pub title: String,
    pub logical_key: String,
    pub published: Option<i64>,
    pub sort_date: i64,
}

pub fn blog_page_size(project: &Project) -> usize {
    project
        .config
        .blog
        .as_ref()
        .map(|blog| blog.page_size)
        .unwrap_or(10)
        .max(1)
}

pub fn blog_latest_parts(project: &Project) -> usize {
    project
        .config
        .blog
        .as_ref()
        .map(|blog| blog.series.latest_parts)
        .unwrap_or(3)
        .max(1)
}

pub fn collect_blog_feed(project: &Project, source_page_id: DocId) -> Vec<FeedItem> {
    let mut items = Vec::new();

    for page in &project.content.pages {
        if !include_page(page, Some(source_page_id)) {
            continue;
        }
        items.push(FeedItem::Post(feed_post(page)));
    }

    let mut series_refs: Vec<&Series> = project.content.series.iter().collect();
    series_refs.sort_by(|a, b| a.dir_path.cmp(&b.dir_path));
    for series in series_refs {
        if let Some(item) = feed_series(project, series) {
            items.push(FeedItem::Series(item));
        }
    }

    items.sort_by(|a, b| {
        b.sort_date()
            .cmp(&a.sort_date())
            .then_with(|| a.tie_key().cmp(b.tie_key()))
    });
    items
}

impl FeedItem {
    pub fn sort_date(&self) -> i64 {
        match self {
            FeedItem::Post(post) => post.sort_date,
            FeedItem::Series(series) => series.sort_date,
        }
    }

    pub fn tie_key(&self) -> &str {
        match self {
            FeedItem::Post(post) => &post.logical_key,
            FeedItem::Series(series) => &series.logical_key,
        }
    }

    pub fn input_hashes(&self) -> Vec<blake3::Hash> {
        match self {
            FeedItem::Post(post) => vec![post.content_hash],
            FeedItem::Series(series) => {
                let mut hashes = Vec::with_capacity(1 + series.part_hashes.len());
                hashes.push(series.index_hash);
                hashes.extend(series.part_hashes.iter().copied());
                hashes
            }
        }
    }

    pub fn input_doc_ids(&self) -> Vec<DocId> {
        match self {
            FeedItem::Post(post) => vec![post.page_id],
            FeedItem::Series(series) => {
                let mut ids = Vec::with_capacity(1 + series.part_ids.len());
                ids.push(series.index_id);
                ids.extend(series.part_ids.iter().copied());
                ids
            }
        }
    }
}

fn include_page(page: &Page, source_page_id: Option<DocId>) -> bool {
    if let Some(source_id) = source_page_id {
        if page.id == source_id {
            return false;
        }
    }
    if !page.header.is_published {
        return false;
    }
    if page.header.exclude_from_blog {
        return false;
    }
    !matches!(
        page.header.template,
        Some(TemplateId::BlogIndex) | Some(TemplateId::Info)
    )
}

fn feed_post(page: &Page) -> FeedPost {
    let sort_date = page_sort_date(page);
    FeedPost {
        page_id: page.id,
        logical_key: logical_key_from_source_path(&page.source_path),
        title: page
            .header
            .title
            .clone()
            .unwrap_or_else(|| "Untitled".to_string()),
        published: timestamp_option(sort_date),
        sort_date,
        content_hash: page.content_hash,
    }
}

fn feed_series(project: &Project, series: &Series) -> Option<FeedSeries> {
    if !include_page(&series.index, None) {
        return None;
    }

    let mut part_candidates: Vec<SeriesPartCandidate<'_>> = Vec::new();
    let mut part_ids = Vec::new();
    let mut part_hashes = Vec::new();
    for part in &series.parts {
        if !include_page(&part.page, None) {
            continue;
        }
        let sort_date = page_sort_date(&part.page);
        let logical_key = logical_key_from_source_path(&part.page.source_path);
        part_candidates.push(SeriesPartCandidate {
            part_no: part.part_no,
            logical_key,
            title: part.page.header.title.as_deref(),
            published: timestamp_option(sort_date),
            sort_date,
        });
        part_ids.push(part.page.id);
        part_hashes.push(part.page.content_hash);
    }

    if part_candidates.is_empty() {
        return None;
    }

    part_candidates.sort_by(|a, b| {
        b.sort_date
            .cmp(&a.sort_date)
            .then_with(|| a.logical_key.cmp(&b.logical_key))
    });
    let latest_parts = part_candidates
        .iter()
        .take(blog_latest_parts(project))
        .map(|part| FeedSeriesPart {
            title: part
                .title
                .map(|value| value.to_string())
                .unwrap_or_else(|| format!("Part {}", part.part_no)),
            logical_key: part.logical_key.clone(),
            published: part.published,
            sort_date: part.sort_date,
        })
        .collect::<Vec<_>>();

    let sort_date = part_candidates
        .iter()
        .map(|part| part.sort_date)
        .max()
        .unwrap_or(0);
    let published = timestamp_option(sort_date);
    let abstract_html = extract_abstract_html(&series.index.body_markdown);

    Some(FeedSeries {
        series_id: series.id,
        logical_key: logical_key_from_source_path(&series.dir_path),
        title: series
            .index
            .header
            .title
            .clone()
            .unwrap_or_else(|| "Untitled".to_string()),
        published,
        sort_date,
        abstract_html,
        latest_parts,
        index_id: series.index.id,
        index_hash: series.index.content_hash,
        part_ids,
        part_hashes,
    })
}

fn page_sort_date(page: &Page) -> i64 {
    page.header.published.or(page.header.updated).unwrap_or(0)
}

fn timestamp_option(value: i64) -> Option<i64> {
    if value == 0 { None } else { Some(value) }
}

fn extract_abstract_html(markdown: &str) -> String {
    let mut selected = None;
    for chunk in markdown.split("\n\n") {
        let trimmed = chunk.trim();
        if !trimmed.is_empty() {
            selected = Some(trimmed);
            break;
        }
    }
    match selected {
        Some(text) => render_markdown_to_html(text),
        None => String::new(),
    }
}

#[derive(Debug)]
struct SeriesPartCandidate<'a> {
    part_no: i32,
    logical_key: String,
    title: Option<&'a str>,
    published: Option<i64>,
    sort_date: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BlogConfig, SiteConfig, SiteContent, SiteMeta, UrlStyle};
    use std::path::PathBuf;

    fn base_config() -> SiteConfig {
        SiteConfig {
            site: SiteMeta {
                id: "demo".to_string(),
                title: "Demo".to_string(),
                abstract_text: None,
                base_url: "https://example.com/".to_string(),
                language: "en".to_string(),
                timezone: None,
                url_style: UrlStyle::Html,
            },
            banner: None,
            menu: Vec::new(),
            people: None,
            blog: None,
            system: None,
            publish: None,
            rss: None,
            seo: None,
            comments: None,
            chroma: None,
            plyr: None,
        }
    }

    fn make_page(id_seed: &str, source_path: &str, mut header: crate::header::Header) -> Page {
        if header.title.is_none() {
            header.title = Some(id_seed.to_string());
        }
        Page {
            id: DocId(blake3::hash(id_seed.as_bytes())),
            source_path: source_path.to_string(),
            header,
            body_markdown: String::new(),
            url_path: logical_key_from_source_path(source_path),
            content_hash: blake3::hash(format!("content:{id_seed}").as_bytes()),
        }
    }

    fn project_with_pages(pages: Vec<Page>) -> Project {
        Project {
            root: PathBuf::from("/tmp"),
            config: base_config(),
            content: SiteContent {
                pages,
                series: Vec::new(),
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        }
    }

    #[test]
    fn exclude_from_blog_removes_page() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.exclude_from_blog = true;
        let excluded = make_page("excluded", "articles/excluded.md", header);

        let mut header = crate::header::Header::default();
        header.is_published = true;
        let included = make_page("included", "articles/included.md", header);

        let project = project_with_pages(vec![excluded, included.clone()]);
        let items = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        assert_eq!(items.len(), 1);
        match &items[0] {
            FeedItem::Post(post) => assert_eq!(post.logical_key, "included"),
            _ => panic!("expected post item"),
        }
    }

    #[test]
    fn template_info_is_excluded() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.template = Some(TemplateId::Info);
        let info = make_page("info", "articles/info.md", header);

        let project = project_with_pages(vec![info]);
        let items = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        assert!(items.is_empty());
    }

    #[test]
    fn series_rollup_orders_by_latest_part() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        let index_a = make_page(
            "series-a-index",
            "articles/series-a/index.md",
            header.clone(),
        );
        let index_b = make_page(
            "series-b-index",
            "articles/series-b/index.md",
            header.clone(),
        );

        let mut part_a1 = make_page(
            "series-a-part1",
            "articles/series-a/part1.md",
            header.clone(),
        );
        part_a1.header.published = Some(1);
        let mut part_b1 = make_page(
            "series-b-part1",
            "articles/series-b/part1.md",
            header.clone(),
        );
        part_b1.header.published = Some(10);

        let series_a = Series {
            id: SeriesId(blake3::hash(b"series-a")),
            dir_path: "articles/series-a".to_string(),
            index: index_a,
            parts: vec![crate::model::SeriesPart {
                part_no: 1,
                page: part_a1,
            }],
        };
        let series_b = Series {
            id: SeriesId(blake3::hash(b"series-b")),
            dir_path: "articles/series-b".to_string(),
            index: index_b,
            parts: vec![crate::model::SeriesPart {
                part_no: 1,
                page: part_b1,
            }],
        };

        let project = Project {
            root: PathBuf::from("/tmp"),
            config: base_config(),
            content: SiteContent {
                pages: Vec::new(),
                series: vec![series_a, series_b],
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };

        let items = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        assert_eq!(items.len(), 2);
        let first = items[0].tie_key().to_string();
        let second = items[1].tie_key().to_string();
        assert_eq!(first, "series-b");
        assert_eq!(second, "series-a");
    }

    #[test]
    fn latest_parts_default_and_configurable() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        let index = make_page("series-index", "articles/series/index.md", header.clone());

        let mut parts = Vec::new();
        for idx in 1..=4 {
            let part_no = idx as i32;
            let published = idx as i64;
            let mut part = make_page(
                &format!("part-{idx}"),
                &format!("articles/series/part{idx}.md"),
                header.clone(),
            );
            part.header.published = Some(published);
            parts.push(crate::model::SeriesPart {
                part_no,
                page: part,
            });
        }

        let series = Series {
            id: SeriesId(blake3::hash(b"series")),
            dir_path: "articles/series".to_string(),
            index,
            parts,
        };

        let mut config = base_config();
        let project = Project {
            root: PathBuf::from("/tmp"),
            config: config.clone(),
            content: SiteContent {
                pages: Vec::new(),
                series: vec![series.clone()],
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };

        let items = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        match &items[0] {
            FeedItem::Series(series_item) => assert_eq!(series_item.latest_parts.len(), 3),
            _ => panic!("expected series item"),
        }

        config.blog = Some(BlogConfig {
            page_size: 10,
            series: crate::model::BlogSeriesConfig { latest_parts: 1 },
        });
        let project = Project {
            root: PathBuf::from("/tmp"),
            config,
            content: SiteContent {
                pages: Vec::new(),
                series: vec![series],
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };

        let items = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        match &items[0] {
            FeedItem::Series(series_item) => assert_eq!(series_item.latest_parts.len(), 1),
            _ => panic!("expected series item"),
        }
    }

    #[test]
    fn deterministic_tie_breaker_by_logical_key() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.published = Some(10);
        let page_b = make_page("page-b", "articles/b.md", header.clone());
        let page_a = make_page("page-a", "articles/a.md", header.clone());
        let project = project_with_pages(vec![page_b, page_a]);

        let items = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        assert_eq!(items.len(), 2);
        let first = items[0].tie_key().to_string();
        let second = items[1].tie_key().to_string();
        assert_eq!(first, "a");
        assert_eq!(second, "b");
    }
}
