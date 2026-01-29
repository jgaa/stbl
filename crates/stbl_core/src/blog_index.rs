use crate::abstracts::derive_abstract_from_markdown;
use crate::model::{DocId, Page, Project, Series, SeriesId};
use crate::url::logical_key_from_source_path;
use crate::visibility::is_blog_index_excluded;
use std::collections::{BTreeMap, BTreeSet};

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
    pub abstract_text: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FeedSeries {
    pub series_id: SeriesId,
    pub logical_key: String,
    pub title: String,
    pub published: Option<i64>,
    pub sort_date: i64,
    pub abstract_text: Option<String>,
    pub tags: Vec<String>,
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

#[derive(Debug, Clone, Copy)]
pub struct BlogPaginationSettings {
    pub enabled: bool,
    pub page_size: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct BlogAbstractSettings {
    pub enabled: bool,
    pub max_chars: usize,
}

#[derive(Debug, Clone)]
pub struct BlogIndexPageRange {
    pub page_no: u32,
    pub total_pages: u32,
    pub start: usize,
    pub end: usize,
    pub prev_key: Option<String>,
    pub next_key: Option<String>,
}

pub fn blog_pagination_settings(project: &Project) -> BlogPaginationSettings {
    BlogPaginationSettings {
        enabled: blog_pagination_enabled(project),
        page_size: blog_page_size(project),
    }
}

pub fn blog_abstract_settings(project: &Project) -> BlogAbstractSettings {
    let default = BlogAbstractSettings {
        enabled: true,
        max_chars: 200,
    };
    let Some(blog) = project.config.blog.as_ref() else {
        return default;
    };
    BlogAbstractSettings {
        enabled: blog.abstract_cfg.enabled,
        max_chars: blog.abstract_cfg.max_chars,
    }
}

pub fn blog_pagination_enabled(project: &Project) -> bool {
    project
        .config
        .blog
        .as_ref()
        .is_some_and(|blog| blog.pagination.enabled)
}

pub fn blog_page_size(project: &Project) -> usize {
    project
        .config
        .blog
        .as_ref()
        .map(|blog| blog.pagination.page_size)
        .unwrap_or(10)
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
    collect_blog_feed_internal(project, Some(source_page_id))
}

pub fn collect_blog_feed_for_tags(project: &Project) -> Vec<FeedItem> {
    collect_blog_feed_internal(project, None)
}

pub fn collect_tag_feed(project: &Project, tag: &str) -> Vec<FeedItem> {
    collect_blog_feed_internal(project, None)
        .into_iter()
        .filter(|item| item.has_tag(tag))
        .collect()
}

pub fn collect_tag_map(project: &Project) -> BTreeMap<String, Vec<FeedItem>> {
    let items = collect_blog_feed_internal(project, None);
    let mut tag_map: BTreeMap<String, Vec<FeedItem>> = BTreeMap::new();
    for item in &items {
        for tag in item.tags() {
            tag_map.entry(tag.clone()).or_default().push(item.clone());
        }
    }
    tag_map
}

pub fn collect_tag_list(project: &Project) -> Vec<String> {
    let items = collect_blog_feed_internal(project, None);
    let mut tags = BTreeSet::new();
    for item in &items {
        for tag in item.tags() {
            tags.insert(tag.clone());
        }
    }
    tags.into_iter().collect()
}

fn collect_blog_feed_internal(project: &Project, source_page_id: Option<DocId>) -> Vec<FeedItem> {
    let mut items = Vec::new();

    for page in &project.content.pages {
        if !include_page(page, source_page_id) {
            continue;
        }
        items.push(FeedItem::Post(feed_post(project, page)));
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

pub fn blog_index_page_logical_key(base_logical_key: &str, page_no: u32) -> String {
    if page_no <= 1 {
        return base_logical_key.to_string();
    }
    let base = base_logical_key.trim_matches('/');
    let suffix = format!("page/{}", page_no);
    if base.is_empty() || base == "index" {
        suffix
    } else {
        format!("{base}/{suffix}")
    }
}

pub fn paginate_blog_index(
    pagination: BlogPaginationSettings,
    base_logical_key: &str,
    total_items: usize,
) -> Vec<BlogIndexPageRange> {
    if !pagination.enabled {
        return vec![BlogIndexPageRange {
            page_no: 1,
            total_pages: 1,
            start: 0,
            end: total_items,
            prev_key: None,
            next_key: None,
        }];
    }

    let total_pages = total_pages(total_items, pagination.page_size);
    let mut pages = Vec::with_capacity(total_pages as usize);
    for page_no in 1..=total_pages {
        let (start, end) = page_slice_bounds(page_no, pagination.page_size, total_items);
        let prev_key = if page_no > 1 {
            Some(blog_index_page_logical_key(base_logical_key, page_no - 1))
        } else {
            None
        };
        let next_key = if page_no < total_pages {
            Some(blog_index_page_logical_key(base_logical_key, page_no + 1))
        } else {
            None
        };
        pages.push(BlogIndexPageRange {
            page_no,
            total_pages,
            start,
            end,
            prev_key,
            next_key,
        });
    }
    pages
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

    pub fn tags(&self) -> &[String] {
        match self {
            FeedItem::Post(post) => &post.tags,
            FeedItem::Series(series) => &series.tags,
        }
    }

    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags().iter().any(|value| value == tag)
    }
}

fn include_page(page: &Page, source_page_id: Option<DocId>) -> bool {
    !is_blog_index_excluded(page, source_page_id)
}

fn feed_post(project: &Project, page: &Page) -> FeedPost {
    let sort_date = page_sort_date(page);
    let abstract_text = select_abstract_text(
        project,
        page.header.abstract_text.as_deref(),
        &page.body_markdown,
    );
    let mut tags = page.header.tags.clone();
    tags.sort();
    tags.dedup();
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
        abstract_text,
        tags,
    }
}

fn feed_series(project: &Project, series: &Series) -> Option<FeedSeries> {
    if !include_page(&series.index, None) {
        return None;
    }

    let mut part_candidates: Vec<SeriesPartCandidate<'_>> = Vec::new();
    let mut part_ids = Vec::new();
    let mut part_hashes = Vec::new();
    let mut tag_set: BTreeSet<String> = BTreeSet::new();
    for tag in &series.index.header.tags {
        tag_set.insert(tag.clone());
    }
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
        for tag in &part.page.header.tags {
            tag_set.insert(tag.clone());
        }
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
    let abstract_text = select_abstract_text(
        project,
        series.index.header.abstract_text.as_deref(),
        &series.index.body_markdown,
    );
    let tags = tag_set.into_iter().collect::<Vec<_>>();

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
        abstract_text,
        tags,
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

fn select_abstract_text(
    project: &Project,
    header_abstract: Option<&str>,
    markdown: &str,
) -> Option<String> {
    let settings = blog_abstract_settings(project);
    if !settings.enabled {
        return None;
    }
    if let Some(value) = header_abstract {
        if !value.trim().is_empty() {
            return Some(value.to_string());
        }
    }
    derive_abstract_from_markdown(markdown, settings.max_chars)
}

fn total_pages(total_items: usize, page_size: usize) -> u32 {
    let size = page_size.max(1);
    let pages = if total_items == 0 {
        1
    } else {
        (total_items + size - 1) / size
    };
    pages as u32
}

fn page_slice_bounds(page_no: u32, page_size: usize, total_items: usize) -> (usize, usize) {
    if total_items == 0 {
        return (0, 0);
    }
    let size = page_size.max(1);
    let start = ((page_no.saturating_sub(1)) as usize).saturating_mul(size);
    let end = usize::min(start + size, total_items);
    if start >= total_items {
        return (total_items, total_items);
    }
    (start, end)
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
    use crate::header::TemplateId;
    use crate::model::{
        BlogConfig, BlogPaginationConfig, SiteConfig, SiteContent, SiteMeta, UrlStyle,
    };
    use crate::url::UrlMapper;
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
            abstract_cfg: crate::model::BlogAbstractConfig {
                enabled: true,
                max_chars: 200,
            },
            pagination: BlogPaginationConfig {
                enabled: false,
                page_size: 10,
            },
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

    #[test]
    fn post_abstract_prefers_header_over_body() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.abstract_text = Some("Header abstract".to_string());
        let mut page = make_page("page", "articles/page.md", header);
        page.body_markdown = "Body paragraph.\n\nSecond paragraph.".to_string();

        let project = project_with_pages(vec![page]);
        let items = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        match &items[0] {
            FeedItem::Post(post) => {
                assert_eq!(post.abstract_text.as_deref(), Some("Header abstract"));
            }
            _ => panic!("expected post item"),
        }
    }

    #[test]
    fn series_abstract_uses_body_when_header_missing() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        let mut index = make_page("series-index", "articles/series/index.md", header.clone());
        index.body_markdown = "Series first paragraph.\n\nSecond paragraph.".to_string();

        let mut part = make_page("series-part", "articles/series/part1.md", header);
        part.header.published = Some(10);
        let series = Series {
            id: SeriesId(blake3::hash(b"series")),
            dir_path: "articles/series".to_string(),
            index,
            parts: vec![crate::model::SeriesPart {
                part_no: 1,
                page: part,
            }],
        };

        let project = Project {
            root: PathBuf::from("/tmp"),
            config: base_config(),
            content: SiteContent {
                pages: Vec::new(),
                series: vec![series],
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };

        let items = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        match &items[0] {
            FeedItem::Series(series_item) => {
                assert_eq!(
                    series_item.abstract_text.as_deref(),
                    Some("Series first paragraph.")
                );
            }
            _ => panic!("expected series item"),
        }
    }

    #[test]
    fn series_abstract_prefers_header_override() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.abstract_text = Some("Series header abstract".to_string());
        let mut index = make_page("series-index", "articles/series/index.md", header.clone());
        index.body_markdown = "Series body text.".to_string();

        let mut part = make_page("series-part", "articles/series/part1.md", header);
        part.header.published = Some(10);
        let series = Series {
            id: SeriesId(blake3::hash(b"series")),
            dir_path: "articles/series".to_string(),
            index,
            parts: vec![crate::model::SeriesPart {
                part_no: 1,
                page: part,
            }],
        };

        let project = Project {
            root: PathBuf::from("/tmp"),
            config: base_config(),
            content: SiteContent {
                pages: Vec::new(),
                series: vec![series],
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };

        let items = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        match &items[0] {
            FeedItem::Series(series_item) => {
                assert_eq!(
                    series_item.abstract_text.as_deref(),
                    Some("Series header abstract")
                );
            }
            _ => panic!("expected series item"),
        }
    }

    #[test]
    fn pagination_disabled_returns_single_page() {
        let project = project_with_pages(Vec::new());
        let pagination = blog_pagination_settings(&project);
        assert!(!pagination.enabled);
        let pages = paginate_blog_index(pagination, "index", 5);
        assert_eq!(pages.len(), 1);
        let page = &pages[0];
        assert_eq!(page.page_no, 1);
        assert_eq!(page.total_pages, 1);
        assert_eq!(page.start, 0);
        assert_eq!(page.end, 5);
        assert!(page.prev_key.is_none());
        assert!(page.next_key.is_none());
    }

    #[test]
    fn pagination_enabled_slices_and_links() {
        let mut config = base_config();
        config.blog = Some(BlogConfig {
            abstract_cfg: crate::model::BlogAbstractConfig {
                enabled: true,
                max_chars: 200,
            },
            pagination: BlogPaginationConfig {
                enabled: true,
                page_size: 2,
            },
            series: crate::model::BlogSeriesConfig { latest_parts: 3 },
        });
        let project = Project {
            root: PathBuf::from("/tmp"),
            config,
            content: SiteContent {
                pages: Vec::new(),
                series: Vec::new(),
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };
        let pages = paginate_blog_index(blog_pagination_settings(&project), "blog", 5);
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].start, 0);
        assert_eq!(pages[0].end, 2);
        assert_eq!(pages[0].prev_key, None);
        assert_eq!(pages[0].next_key.as_deref(), Some("blog/page/2"));
        assert_eq!(pages[1].start, 2);
        assert_eq!(pages[1].end, 4);
        assert_eq!(pages[1].prev_key.as_deref(), Some("blog"));
        assert_eq!(pages[1].next_key.as_deref(), Some("blog/page/3"));
        assert_eq!(pages[2].start, 4);
        assert_eq!(pages[2].end, 5);
        assert_eq!(pages[2].prev_key.as_deref(), Some("blog/page/2"));
        assert_eq!(pages[2].next_key, None);
    }

    #[test]
    fn pagination_key_for_root_index_avoids_double_slashes() {
        assert_eq!(blog_index_page_logical_key("index", 2), "page/2");
        assert_eq!(blog_index_page_logical_key("", 2), "page/2");
        assert_eq!(blog_index_page_logical_key("blog", 2), "blog/page/2");
    }

    #[test]
    fn pagination_keeps_page1_key_canonical() {
        assert_eq!(blog_index_page_logical_key("index", 1), "index");
        assert_eq!(blog_index_page_logical_key("blog", 1), "blog");
    }

    #[test]
    fn pagination_href_respects_url_style() {
        let mut config = base_config();
        config.site.url_style = UrlStyle::Html;
        let mapper = UrlMapper::new(&config);
        let key = blog_index_page_logical_key("index", 2);
        let mapping = mapper.map(&key);
        assert_eq!(mapping.href, "page/2.html");

        let mut config = base_config();
        config.site.url_style = UrlStyle::Pretty;
        let mapper = UrlMapper::new(&config);
        let mapping = mapper.map(&key);
        assert_eq!(mapping.href, "page/2/");
    }

    #[test]
    fn pagination_preserves_feed_order_across_pages() {
        let mut config = base_config();
        config.blog = Some(BlogConfig {
            abstract_cfg: crate::model::BlogAbstractConfig {
                enabled: true,
                max_chars: 200,
            },
            pagination: BlogPaginationConfig {
                enabled: true,
                page_size: 1,
            },
            series: crate::model::BlogSeriesConfig { latest_parts: 3 },
        });
        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.published = Some(10);
        let page_b = make_page("page-b", "articles/b.md", header.clone());
        let page_a = make_page("page-a", "articles/a.md", header.clone());
        let project = Project {
            root: PathBuf::from("/tmp"),
            config,
            content: SiteContent {
                pages: vec![page_b, page_a],
                series: Vec::new(),
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };
        let feed = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        let pages = paginate_blog_index(blog_pagination_settings(&project), "index", feed.len());
        let first = feed[pages[0].start].tie_key().to_string();
        let second = feed[pages[1].start].tie_key().to_string();
        assert_eq!(first, "a");
        assert_eq!(second, "b");
    }

    #[test]
    fn series_rollup_latest_parts_not_affected_by_pagination() {
        let mut config = base_config();
        config.blog = Some(BlogConfig {
            abstract_cfg: crate::model::BlogAbstractConfig {
                enabled: true,
                max_chars: 200,
            },
            pagination: BlogPaginationConfig {
                enabled: true,
                page_size: 1,
            },
            series: crate::model::BlogSeriesConfig { latest_parts: 3 },
        });

        let mut header = crate::header::Header::default();
        header.is_published = true;

        let mut index_header = header.clone();
        index_header.title = Some("Series".to_string());
        let index = make_page("series-index", "articles/series/index.md", index_header);

        let mut parts = Vec::new();
        for part_no in 1..=5 {
            let mut part_header = header.clone();
            part_header.title = Some(format!("Part {}", part_no));
            part_header.published = Some(10 + part_no as i64);
            let part = make_page(
                &format!("series-part-{part_no}"),
                &format!("articles/series/part{part_no}.md"),
                part_header,
            );
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

        let mut post_header = header.clone();
        post_header.published = Some(100);
        let post = make_page("post", "articles/post.md", post_header);

        let project = Project {
            root: PathBuf::from("/tmp"),
            config,
            content: SiteContent {
                pages: vec![post],
                series: vec![series],
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };

        let feed = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        let pages = paginate_blog_index(blog_pagination_settings(&project), "index", feed.len());
        let mut series_count = 0;
        for page in &pages {
            for item in &feed[page.start..page.end] {
                if matches!(item, FeedItem::Series(_)) {
                    series_count += 1;
                }
            }
        }
        assert_eq!(series_count, 1);

        let series_item = feed
            .iter()
            .find_map(|item| match item {
                FeedItem::Series(series) => Some(series),
                _ => None,
            })
            .expect("expected series item");
        assert_eq!(series_item.latest_parts.len(), 3);
        assert_eq!(series_item.latest_parts[0].logical_key, "series/part5");
        assert_eq!(series_item.latest_parts[1].logical_key, "series/part4");
        assert_eq!(series_item.latest_parts[2].logical_key, "series/part3");
        assert_eq!(series_item.sort_date, 15);
    }

    #[test]
    fn series_latest_parts_tie_breaker_is_logical_key() {
        let mut config = base_config();
        config.blog = Some(BlogConfig {
            abstract_cfg: crate::model::BlogAbstractConfig {
                enabled: true,
                max_chars: 200,
            },
            pagination: BlogPaginationConfig {
                enabled: true,
                page_size: 2,
            },
            series: crate::model::BlogSeriesConfig { latest_parts: 2 },
        });

        let mut header = crate::header::Header::default();
        header.is_published = true;

        let index = make_page("series-index", "articles/series/index.md", header.clone());

        let mut part_a_header = header.clone();
        part_a_header.published = Some(20);
        let part_a = make_page("part-a", "articles/series/a.md", part_a_header);
        let mut part_b_header = header.clone();
        part_b_header.published = Some(20);
        let part_b = make_page("part-b", "articles/series/b.md", part_b_header);

        let series = Series {
            id: SeriesId(blake3::hash(b"series")),
            dir_path: "articles/series".to_string(),
            index,
            parts: vec![
                crate::model::SeriesPart {
                    part_no: 1,
                    page: part_b,
                },
                crate::model::SeriesPart {
                    part_no: 2,
                    page: part_a,
                },
            ],
        };

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

        let feed = collect_blog_feed(&project, DocId(blake3::hash(b"source")));
        let series_item = feed
            .iter()
            .find_map(|item| match item {
                FeedItem::Series(series) => Some(series),
                _ => None,
            })
            .expect("expected series item");
        assert_eq!(series_item.latest_parts.len(), 2);
        assert_eq!(series_item.latest_parts[0].logical_key, "series/a");
        assert_eq!(series_item.latest_parts[1].logical_key, "series/b");
    }

    #[test]
    fn tag_feed_includes_series_when_part_has_tag() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        let index = make_page("series-index", "articles/series/index.md", header.clone());

        let mut part = make_page("series-part", "articles/series/part1.md", header.clone());
        part.header.published = Some(10);
        part.header.tags = vec!["series-only".to_string()];

        let series = Series {
            id: SeriesId(blake3::hash(b"series")),
            dir_path: "articles/series".to_string(),
            index,
            parts: vec![crate::model::SeriesPart {
                part_no: 1,
                page: part,
            }],
        };

        let mut post_header = crate::header::Header::default();
        post_header.is_published = true;
        post_header.tags = vec!["rust".to_string()];
        let mut post = make_page("post", "articles/post.md", post_header);
        post.header.published = Some(5);

        let project = Project {
            root: PathBuf::from("/tmp"),
            config: base_config(),
            content: SiteContent {
                pages: vec![post],
                series: vec![series],
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };

        let items = collect_tag_feed(&project, "series-only");
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], FeedItem::Series(_)));

        let items = collect_tag_feed(&project, "rust");
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], FeedItem::Post(_)));
    }

    #[test]
    fn tag_feed_includes_series_index_tag() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.tags = vec!["series-index".to_string()];
        let index = make_page("series-index", "articles/series/index.md", header.clone());

        let mut part = make_page("series-part", "articles/series/part1.md", header);
        part.header.published = Some(10);

        let series = Series {
            id: SeriesId(blake3::hash(b"series")),
            dir_path: "articles/series".to_string(),
            index,
            parts: vec![crate::model::SeriesPart {
                part_no: 1,
                page: part,
            }],
        };

        let project = Project {
            root: PathBuf::from("/tmp"),
            config: base_config(),
            content: SiteContent {
                pages: Vec::new(),
                series: vec![series],
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };

        let items = collect_tag_feed(&project, "series-index");
        assert_eq!(items.len(), 1);
        assert!(matches!(items[0], FeedItem::Series(_)));
    }

    #[test]
    fn tag_map_excludes_info_and_excluded_pages() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.template = Some(TemplateId::Info);
        header.tags = vec!["hidden-info".to_string()];
        let info = make_page("info", "articles/info.md", header);

        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.exclude_from_blog = true;
        header.tags = vec!["hidden-excluded".to_string()];
        let excluded = make_page("excluded", "articles/excluded.md", header);

        let project = project_with_pages(vec![info, excluded]);
        let tag_map = collect_tag_map(&project);
        assert!(!tag_map.contains_key("hidden-info"));
        assert!(!tag_map.contains_key("hidden-excluded"));
    }

    #[test]
    fn tag_feed_orders_by_sort_date_then_logical_key() {
        let mut header = crate::header::Header::default();
        header.is_published = true;
        header.tags = vec!["rust".to_string()];
        header.published = Some(10);
        let page_b = make_page("page-b", "articles/b.md", header.clone());
        let page_a = make_page("page-a", "articles/a.md", header);

        let project = project_with_pages(vec![page_b, page_a]);
        let items = collect_tag_feed(&project, "rust");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].tie_key(), "a");
        assert_eq!(items[1].tie_key(), "b");
    }
}
