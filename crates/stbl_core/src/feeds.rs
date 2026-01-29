use crate::header::TemplateId;
use crate::model::{Page, Project};
use crate::url::{UrlMapper, logical_key_from_source_path};
use chrono::{DateTime, Duration, Utc};
use std::collections::BTreeSet;

pub fn render_rss(project: &Project, mapper: &UrlMapper) -> String {
    let rss_config = match project.config.rss.as_ref() {
        Some(config) if config.enabled => config,
        _ => return String::new(),
    };

    let max_items = rss_config.max_items.unwrap_or(10);
    let cutoff = rss_config
        .ttl_days
        .map(|days| Utc::now() - Duration::days(i64::from(days)));

    let mut items: Vec<FeedItem> = collect_feed_pages(project)
        .into_iter()
        .filter_map(|feed_page| {
            FeedItem::from_page(project, mapper, feed_page.page, &feed_page.logical_key)
        })
        .filter(|item| {
            cutoff
                .map(|cutoff| item.published >= cutoff)
                .unwrap_or(true)
        })
        .collect();

    items.sort_by(|a, b| b.published.cmp(&a.published));
    items.truncate(max_items);

    let site_title = escape_xml(&project.config.site.title);
    let site_desc = escape_xml(project.config.site.abstract_text.as_deref().unwrap_or(""));
    let site_link = escape_xml(&project.config.site.base_url);
    let site_language = escape_xml(&project.config.site.language);

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<rss version=\"2.0\">\n");
    out.push_str("<channel>\n");
    out.push_str(&format!("<title>{site_title}</title>\n"));
    out.push_str(&format!("<link>{site_link}</link>\n"));
    out.push_str(&format!("<description>{site_desc}</description>\n"));
    out.push_str(&format!("<language>{site_language}</language>\n"));

    for item in items {
        out.push_str("<item>\n");
        out.push_str(&format!("<title>{}</title>\n", escape_xml(&item.title)));
        out.push_str(&format!("<link>{}</link>\n", escape_xml(&item.link)));
        out.push_str(&format!("<guid>{}</guid>\n", escape_xml(&item.link)));
        out.push_str(&format!("<pubDate>{}</pubDate>\n", item.pub_date));
        out.push_str(&format!(
            "<description>{}</description>\n",
            escape_xml(&item.description)
        ));
        if let Some(authors) = item.authors.as_deref() {
            out.push_str(&format!("<author>{}</author>\n", escape_xml(authors)));
        }
        out.push_str("</item>\n");
    }

    out.push_str("</channel>\n</rss>\n");
    out
}

pub fn render_sitemap(project: &Project, mapper: &UrlMapper) -> String {
    let mut entries = Vec::new();

    if let Some(entry) = sitemap_entry_for_key(project, mapper, "index", None) {
        entries.push(entry);
    }

    for feed_page in collect_feed_pages(project) {
        let lastmod = feed_page
            .page
            .header
            .updated
            .or(feed_page.page.header.published)
            .and_then(|value| DateTime::<Utc>::from_timestamp(value, 0));
        if let Some(entry) = sitemap_entry_for_key(project, mapper, &feed_page.logical_key, lastmod)
        {
            entries.push(entry);
        }
    }

    let mut tags = BTreeSet::new();
    for feed_page in collect_feed_pages(project) {
        for tag in &feed_page.page.header.tags {
            tags.insert(tag.clone());
        }
    }
    for tag in tags {
        let key = format!("tags/{tag}");
        if let Some(entry) = sitemap_entry_for_key(project, mapper, &key, None) {
            entries.push(entry);
        }
    }

    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    for entry in entries {
        out.push_str("<url>\n");
        out.push_str(&format!("<loc>{}</loc>\n", escape_xml(&entry.loc)));
        if let Some(lastmod) = entry.lastmod {
            out.push_str(&format!("<lastmod>{}</lastmod>\n", lastmod));
        }
        out.push_str("</url>\n");
    }
    out.push_str("</urlset>\n");
    out
}

struct FeedPage<'a> {
    page: &'a Page,
    logical_key: String,
}

fn collect_feed_pages(project: &Project) -> Vec<FeedPage<'_>> {
    let mut pages = Vec::new();
    for page in &project.content.pages {
        if page.header.template == Some(TemplateId::BlogIndex) {
            continue;
        }
        if crate::visibility::is_published_page(page) {
            pages.push(FeedPage {
                page,
                logical_key: logical_key_from_source_path(&page.source_path),
            });
        }
    }
    for series in &project.content.series {
        if crate::visibility::is_published_page(&series.index) {
            pages.push(FeedPage {
                page: &series.index,
                logical_key: logical_key_from_source_path(&series.dir_path),
            });
        }
        for part in &series.parts {
            if crate::visibility::is_published_page(&part.page) {
                pages.push(FeedPage {
                    page: &part.page,
                    logical_key: logical_key_from_source_path(&part.page.source_path),
                });
            }
        }
    }
    pages
}

fn base_url_join(base_url: &str, href: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let path = href.trim_start_matches('/');
    format!("{base}/{path}")
}

fn escape_xml(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

struct FeedItem {
    title: String,
    link: String,
    pub_date: String,
    published: DateTime<Utc>,
    description: String,
    authors: Option<String>,
}

impl FeedItem {
    fn from_page(
        project: &Project,
        mapper: &UrlMapper,
        page: &Page,
        logical_key: &str,
    ) -> Option<Self> {
        if !crate::visibility::is_published_page(page) {
            return None;
        }
        let published = page
            .header
            .published
            .or(page.header.updated)
            .and_then(|value| DateTime::<Utc>::from_timestamp(value, 0))?;
        let title = page
            .header
            .title
            .clone()
            .unwrap_or_else(|| "Untitled".to_string());
        let href = mapper.map(logical_key).href;
        let link = base_url_join(&project.config.site.base_url, &href);
        let pub_date = published.to_rfc2822();
        let description = page.header.abstract_text.clone().unwrap_or_default();
        let authors = page.header.authors.as_ref().map(|list| list.join(", "));
        Some(Self {
            title,
            link,
            pub_date,
            published,
            description,
            authors,
        })
    }
}

struct SitemapEntry {
    loc: String,
    lastmod: Option<String>,
}

fn sitemap_entry_for_key(
    project: &Project,
    mapper: &UrlMapper,
    logical_key: &str,
    lastmod: Option<DateTime<Utc>>,
) -> Option<SitemapEntry> {
    let href = mapper.map(logical_key).href;
    let loc = base_url_join(&project.config.site.base_url, &href);
    let lastmod = lastmod.map(|value| value.to_rfc3339());
    Some(SitemapEntry { loc, lastmod })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assemble::assemble_site;
    use crate::config::load_site_config;
    use crate::header::UnknownKeyPolicy;
    use crate::model::{Page, Project, SiteConfig, SiteContent, SiteMeta, UrlStyle};
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    fn fixture_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("site1")
    }

    fn scan_fixture(root: &Path) -> anyhow::Result<Vec<crate::model::DiscoveredDoc>> {
        let mut docs = Vec::new();
        let articles = root.join("articles");
        let entries = [
            articles.join("index.md"),
            articles.join("page1.md"),
            articles.join("page2.md"),
            articles.join("series/index.md"),
            articles.join("series/part1.md"),
            articles.join("series/part2.md"),
        ];

        for path in entries {
            let raw = std::fs::read_to_string(&path)?;
            let mtime = std::fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let (header_opt, body_slice) = extract_header_body(&raw);
            let header_present = header_opt.is_some();
            let header_text = header_opt.map(str::to_string);
            let body_markdown = body_slice.to_string();
            let header = match header_text.as_deref() {
                Some(text) => crate::header::parse_header(text, UnknownKeyPolicy::Error)?.header,
                None => crate::header::Header::default(),
            };

            let rel_path = to_relative_path(root, &path);
            let dir_path = to_relative_path(root, path.parent().unwrap_or_else(|| Path::new("")));
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("")
                .to_string();

            let parsed = crate::model::ParsedDoc {
                src: crate::model::SourceDoc {
                    source_path: rel_path.clone(),
                    dir_path: dir_path.clone(),
                    file_name,
                    raw,
                },
                header,
                body_markdown,
                header_present,
                mtime,
            };

            let (kind, series_dir) = classify_doc(root, &path, &articles);
            docs.push(crate::model::DiscoveredDoc {
                parsed,
                kind,
                series_dir,
            });
        }

        Ok(docs)
    }

    #[test]
    fn rss_excludes_unpublished_pages() {
        let config = SiteConfig {
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
            rss: Some(crate::model::RssConfig {
                enabled: true,
                max_items: Some(10),
                ttl_days: None,
            }),
            seo: None,
            comments: None,
            chroma: None,
            plyr: None,
        };

        let mut published_header = crate::header::Header::default();
        published_header.is_published = true;
        published_header.title = Some("Published".to_string());
        published_header.published = Some(1_704_153_600);

        let mut draft_header = crate::header::Header::default();
        draft_header.is_published = false;
        draft_header.title = Some("Draft".to_string());

        let published = Page {
            id: crate::model::DocId(blake3::hash(b"pub")),
            source_path: "articles/published.md".to_string(),
            header: published_header,
            body_markdown: "Body".to_string(),
            url_path: "published".to_string(),
            content_hash: blake3::hash(b"content"),
        };
        let draft = Page {
            id: crate::model::DocId(blake3::hash(b"draft")),
            source_path: "articles/draft.md".to_string(),
            header: draft_header,
            body_markdown: "Draft body".to_string(),
            url_path: "draft".to_string(),
            content_hash: blake3::hash(b"draft"),
        };

        let project = Project {
            root: PathBuf::from("/tmp"),
            config,
            content: SiteContent {
                pages: vec![published, draft],
                series: Vec::new(),
                diagnostics: Vec::new(),
                write_back: Default::default(),
            },
        };
        let mapper = UrlMapper::new(&project.config);
        let rss = render_rss(&project, &mapper);
        assert!(rss.contains("Published"));
        assert!(!rss.contains("Draft"));
    }

    fn to_relative_path(root: &Path, path: &Path) -> String {
        let rel = path.strip_prefix(root).unwrap_or(path);
        rel.to_string_lossy().replace('\\', "/")
    }

    fn extract_header_body(raw: &str) -> (Option<&str>, &str) {
        if let Some((header, body)) = extract_frontmatter(raw) {
            return (Some(header), body);
        }
        extract_plain_header(raw)
    }

    fn looks_like_header_line(line: &str) -> bool {
        let trimmed = line.trim_start();
        let (key, _) = match trimmed.split_once(':') {
            Some(value) => value,
            None => return false,
        };
        let key = key.trim_end();
        !key.is_empty()
            && key
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    }

    fn extract_frontmatter(raw: &str) -> Option<(&str, &str)> {
        let mut offset = 0;
        let mut iter = raw.split_inclusive('\n');
        let first = iter.next()?;
        if first.trim() != "---" {
            return None;
        }
        offset += first.len();
        let header_start = offset;
        for line in iter {
            if line.trim() == "---" {
                let header_end = offset;
                let body_start = offset + line.len();
                return Some((&raw[header_start..header_end], &raw[body_start..]));
            }
            offset += line.len();
        }
        None
    }

    fn extract_plain_header(raw: &str) -> (Option<&str>, &str) {
        let mut offset = 0;
        let mut saw_header_line = false;
        let mut header_end = 0;
        for line in raw.split_inclusive('\n') {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                header_end = offset + line.len();
                break;
            }
            if line.trim_start().starts_with('#') {
                offset += line.len();
                header_end = offset;
                continue;
            }
            if !looks_like_header_line(line) {
                return (None, raw);
            }
            saw_header_line = true;
            offset += line.len();
            header_end = offset;
        }
        if !saw_header_line {
            return (None, raw);
        }
        if header_end == 0 {
            header_end = raw.len();
        }
        (Some(&raw[..header_end]), &raw[header_end..])
    }

    fn classify_doc(
        root: &Path,
        path: &Path,
        articles_dir: &Path,
    ) -> (crate::model::DocKind, Option<String>) {
        let file_name = path.file_name().and_then(|name| name.to_str());
        let parent = path.parent().unwrap_or(articles_dir);
        if parent != articles_dir {
            let dir_path = to_relative_path(root, parent);
            if file_name == Some("index.md") {
                return (crate::model::DocKind::SeriesIndex, Some(dir_path));
            }
            return (crate::model::DocKind::SeriesPart, Some(dir_path));
        }
        (crate::model::DocKind::Page, None)
    }

    fn build_project(url_style: UrlStyle, rss_enabled: bool) -> Project {
        let root = fixture_root();
        let config = load_site_config(&root.join("stbl.yaml")).expect("load config");
        let mut config = config;
        config.site.url_style = url_style;
        config.rss = Some(crate::model::RssConfig {
            enabled: rss_enabled,
            max_items: Some(10),
            ttl_days: None,
        });
        let docs = scan_fixture(&root).expect("scan fixture");
        let content = assemble_site(docs).expect("assemble site");
        Project {
            root,
            config,
            content,
        }
    }

    #[test]
    fn render_rss_includes_latest_items() {
        let project = build_project(UrlStyle::Html, true);
        let mapper = UrlMapper::new(&project.config);
        let rss = render_rss(&project, &mapper);
        assert!(rss.contains("<rss version=\"2.0\">"));
        assert!(rss.contains("<channel>"));
        assert!(rss.contains("<item>"));
        let latest_href = mapper
            .map(&logical_key_from_source_path("articles/series/part2.md"))
            .href;
        let latest_link = base_url_join(&project.config.site.base_url, &latest_href);
        assert!(rss.contains(&latest_link));
        let first = rss.find("Part Two").expect("part two");
        let second = rss.find("Page One").expect("page one");
        assert!(first < second);
    }

    #[test]
    fn render_rss_contains_page1_link() {
        let project = build_project(UrlStyle::Html, true);
        let mapper = UrlMapper::new(&project.config);
        let rss = render_rss(&project, &mapper);
        let href = mapper
            .map(&logical_key_from_source_path("articles/page1.md"))
            .href;
        let link = base_url_join(&project.config.site.base_url, &href);
        assert!(rss.contains("<item>"));
        assert!(rss.contains(&link));
    }

    #[test]
    fn render_rss_empty_when_disabled() {
        let project = build_project(UrlStyle::Html, false);
        let mapper = UrlMapper::new(&project.config);
        let rss = render_rss(&project, &mapper);
        assert!(rss.is_empty());
    }

    #[test]
    fn render_sitemap_contains_frontpage_and_tags() {
        let project = build_project(UrlStyle::Html, true);
        let mapper = UrlMapper::new(&project.config);
        let sitemap = render_sitemap(&project, &mapper);
        let front_href = mapper.map("index").href;
        let front_loc = base_url_join(&project.config.site.base_url, &front_href);
        assert!(sitemap.contains(&front_loc));
        let tag_href = mapper.map("tags/rust").href;
        let tag_loc = base_url_join(&project.config.site.base_url, &tag_href);
        assert!(sitemap.contains(&tag_loc));
    }
}
