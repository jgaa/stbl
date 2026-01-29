//! Core document, site, and planning models.
//!
//! This module is intentionally "boring":
//! - No filesystem I/O
//! - No rendering details
//! - Just data structures used across scan/assemble/plan

use crate::header::Header;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::SystemTime;

// ----------------------------
// Discovery / parsing stage
// ----------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocKind {
    /// A standalone page/post.
    Page,
    /// The index.md of a series directory.
    SeriesIndex,
    /// A series part (non-index file in a series dir).
    SeriesPart,
}

/// Raw source document as read from disk.
#[derive(Debug, Clone)]
pub struct SourceDoc {
    /// Path relative to project root (preferred), or absolute if you decided so in CLI.
    pub source_path: String,
    pub dir_path: String,
    pub file_name: String,
    pub raw: String,
}

/// Parsed document = header + body markdown.
#[derive(Debug, Clone)]
pub struct ParsedDoc {
    pub src: SourceDoc,
    pub header: Header,
    pub body_markdown: String,
    pub header_present: bool,
    pub mtime: SystemTime,
}

/// Discovered document = parsed doc + discovery classification.
#[derive(Debug, Clone)]
pub struct DiscoveredDoc {
    pub parsed: ParsedDoc,
    pub kind: DocKind,
    /// If part of a series, relative path to the series directory.
    pub series_dir: Option<String>,
}

// ----------------------------
// Stable identities
// ----------------------------

/// Document identity: stable across machines when derived from canonical inputs.
/// Use this in caches and build plans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocId(pub blake3::Hash);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SeriesId(pub blake3::Hash);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskId(pub blake3::Hash);

// ----------------------------
// Site content model (assembled)
// ----------------------------

#[derive(Debug, Clone)]
pub struct Page {
    pub id: DocId,
    pub source_path: String,
    pub header: Header,
    pub body_markdown: String,

    /// Derived/normalized output path within the site (e.g. "posts/hello-world/").
    /// Keep as a logical path, not an OS path.
    pub url_path: String,

    /// Hash of the canonicalized content inputs for caching/planning.
    /// Often computed as blake3(header_normalized + body + relevant config bits).
    pub content_hash: blake3::Hash,
}

#[derive(Debug, Clone)]
pub struct SeriesPart {
    pub part_no: i32,
    pub page: Page,
}

#[derive(Debug, Clone)]
pub struct Series {
    pub id: SeriesId,
    /// Series directory relative to project root.
    pub dir_path: String,
    pub index: Page,
    pub parts: Vec<SeriesPart>,
}

#[derive(Debug, Clone, Default)]
pub struct SiteContent {
    pub pages: Vec<Page>,
    pub series: Vec<Series>,
    pub diagnostics: Vec<Diagnostic>,
    pub write_back: WriteBackPlan,
}

#[derive(Debug, Clone, Default)]
pub struct WriteBackPlan {
    pub edits: Vec<WriteBackEdit>,
}

#[derive(Debug, Clone)]
pub struct WriteBackEdit {
    pub path: String,
    pub new_header_text: Option<String>,
    pub new_body: Option<String>,
}

// ----------------------------
// Diagnostics
// ----------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub source_path: Option<String>,
    pub message: String,
}

// ----------------------------
// Project config (YAML)
// ----------------------------

#[derive(Debug, Clone, Serialize)]
pub struct SiteConfig {
    pub site: SiteMeta,
    pub banner: Option<BannerConfig>,
    pub menu: Vec<MenuItem>,
    pub people: Option<PeopleConfig>,
    pub blog: Option<BlogConfig>,
    pub system: Option<SystemConfig>,
    pub publish: Option<PublishConfig>,
    pub rss: Option<RssConfig>,
    pub seo: Option<SeoConfig>,
    pub comments: Option<serde_yaml::Value>,
    pub chroma: Option<serde_yaml::Value>,
    pub plyr: Option<serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SiteMeta {
    /// Stable ID (used for cache folder key).
    pub id: String,
    pub title: String,
    pub abstract_text: Option<String>,
    pub base_url: String,
    pub language: String,
    pub timezone: Option<String>,
    pub url_style: UrlStyle,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlogConfig {
    pub abstract_cfg: BlogAbstractConfig,
    pub pagination: BlogPaginationConfig,
    pub series: BlogSeriesConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlogAbstractConfig {
    pub enabled: bool,
    pub max_chars: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlogPaginationConfig {
    pub enabled: bool,
    pub page_size: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BlogSeriesConfig {
    pub latest_parts: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BannerConfig {
    pub widths: Vec<u32>,
    pub quality: u32,
    pub align: i32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MenuItem {
    pub title: String,
    pub href: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PeopleConfig {
    pub default: String,
    pub entries: BTreeMap<String, PersonEntry>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PersonEntry {
    pub name: String,
    pub email: Option<String>,
    #[serde(default)]
    pub links: Vec<PersonLink>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PersonLink {
    pub id: String,
    pub name: String,
    pub url: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SystemConfig {
    pub date: Option<DateConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DateConfig {
    pub format: String,
    pub roundup_seconds: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PublishConfig {
    pub command: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RssConfig {
    pub enabled: bool,
    pub max_items: Option<usize>,
    pub ttl_days: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SeoConfig {
    pub sitemap: Option<SeoSitemapConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SeoSitemapConfig {
    pub priority: Option<SeoPriorityConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SeoPriorityConfig {
    pub frontpage: i32,
    pub article: i32,
    pub series: i32,
    pub tag: i32,
    pub tags: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UrlStyle {
    Html,
    Pretty,
    PrettyWithFallback,
}

impl Default for UrlStyle {
    fn default() -> Self {
        UrlStyle::Html
    }
}

// ----------------------------
// Project root model
// ----------------------------

#[derive(Debug, Clone)]
pub struct Project {
    /// Absolute root on this machine.
    pub root: PathBuf,
    pub config: SiteConfig,
    pub content: SiteContent,
}

// ----------------------------
// Build planning models (DAG-friendly)
// ----------------------------

#[derive(Debug, Clone)]
pub enum TaskKind {
    RenderPage { page: DocId },
    RenderBlogIndex { source_page: DocId, page_no: u32 },
    RenderSeries { series: SeriesId },
    RenderTagIndex { tag: String },
    RenderTagsIndex,
    RenderFrontPage,
    GenerateRss,
    GenerateSitemap,
    CopyAsset { rel_path: PathBuf },
}

#[derive(Debug, Clone)]
pub enum ContentId {
    Doc(DocId),
    Series(SeriesId),
    Tag(String),
    Asset(PathBuf),
}

#[derive(Debug, Clone)]
pub struct OutputArtifact {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct BuildTask {
    pub id: TaskId,
    pub kind: TaskKind,

    /// Logical input identifiers (docs, assets, etc.).
    pub inputs: Vec<ContentId>,
    pub outputs: Vec<OutputArtifact>,
}

#[derive(Debug, Clone, Default)]
pub struct BuildPlan {
    pub tasks: Vec<BuildTask>,
    pub edges: Vec<(TaskId, TaskId)>,
}
