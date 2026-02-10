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

use crate::assets::{AssetRelPath, AssetSourceId};
use crate::media::{ImageVariantIndex, MediaRef, VideoVariantIndex};
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn new(kind: &str, parts: &[&str]) -> Self {
        let mut out = String::with_capacity(32);
        out.push_str(kind);
        for part in parts {
            out.push(':');
            out.push_str(&encode_task_id_component(part));
        }
        TaskId(out)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputFingerprint(pub [u8; 32]);

impl InputFingerprint {
    pub fn to_hex(self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            use std::fmt::Write;
            let _ = write!(out, "{:02x}", byte);
        }
        out
    }
}

fn encode_task_id_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for &byte in value.as_bytes() {
        let ch = byte as char;
        let is_safe = ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' || ch == '/';
        if is_safe {
            out.push(ch);
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{:02X}", byte);
        }
    }
    out
}

// ----------------------------
// Site content model (assembled)
// ----------------------------

#[derive(Debug, Clone)]
pub struct Page {
    pub id: DocId,
    pub source_path: String,
    pub header: Header,
    pub body_markdown: String,
    pub banner_name: Option<String>,
    pub media_refs: Vec<MediaRef>,

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
    pub nav: Vec<NavItem>,
    pub theme: ThemeConfig,
    pub syntax: SyntaxConfig,
    pub assets: AssetsConfig,
    pub security: SecurityConfig,
    pub media: MediaConfig,
    pub footer: FooterConfig,
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
    pub tagline: Option<String>,
    pub logo: Option<String>,
    pub copyright: Option<String>,
    pub base_url: String,
    pub language: String,
    pub timezone: Option<String>,
    pub url_style: UrlStyle,
    pub macros: MacrosConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MacrosConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetsConfig {
    pub cache_busting: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    pub svg: SvgSecurityConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SvgSecurityConfig {
    pub mode: SvgSecurityMode,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SvgSecurityMode {
    Off,
    Warn,
    Fail,
    Sanitize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SyntaxConfig {
    pub highlight: bool,
    pub theme: String,
    pub line_numbers: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MediaConfig {
    pub images: ImageConfig,
    pub video: VideoConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageConfig {
    pub widths: Vec<u32>,
    pub quality: u8,
    #[serde(default)]
    pub format_mode: ImageFormatMode,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VideoConfig {
    pub heights: Vec<u32>,
    pub poster_time_sec: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ImageFormatMode {
    Normal,
    Fast,
}

impl Default for ImageFormatMode {
    fn default() -> Self {
        ImageFormatMode::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ImageOutputFormat {
    Avif,
    Webp,
    Jpeg,
    Png,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeConfig {
    pub variant: String,
    pub max_body_width: String,
    pub breakpoints: ThemeBreakpoints,
    pub colors: ThemeColorOverrides,
    pub nav: ThemeNavOverrides,
    pub header: ThemeHeaderConfig,
    pub wide_background: ThemeWideBackgroundOverrides,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeBreakpoints {
    pub desktop_min: String,
    pub wide_min: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ThemeColorOverrides {
    pub bg: Option<String>,
    pub fg: Option<String>,
    pub heading: Option<String>,
    pub accent: Option<String>,
    pub link: Option<String>,
    pub muted: Option<String>,
    pub surface: Option<String>,
    pub border: Option<String>,
    pub link_hover: Option<String>,
    pub code_bg: Option<String>,
    pub code_fg: Option<String>,
    pub quote_bg: Option<String>,
    pub quote_border: Option<String>,
    pub wide_bg: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ThemeNavOverrides {
    pub bg: Option<String>,
    pub fg: Option<String>,
    pub border: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MenuAlign {
    Left,
    Center,
    Right,
}

impl Default for MenuAlign {
    fn default() -> Self {
        MenuAlign::Right
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeHeaderLayout {
    Inline,
    Stacked,
}

impl Default for ThemeHeaderLayout {
    fn default() -> Self {
        ThemeHeaderLayout::Stacked
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ThemeHeaderConfig {
    pub layout: ThemeHeaderLayout,
    pub menu_align: MenuAlign,
    pub title_size: String,
    pub tagline_size: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum WideBackgroundStyle {
    Cover,
    Tile,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ThemeWideBackgroundOverrides {
    pub color: Option<String>,
    pub image: Option<String>,
    pub style: Option<WideBackgroundStyle>,
    pub position: Option<String>,
    pub opacity: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeVars {
    pub max_body_width: String,
    pub desktop_min: String,
    pub wide_min: String,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NavItem {
    pub label: String,
    pub href: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FooterConfig {
    pub show_stbl: bool,
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
    pub ttl_channel: Option<u32>,
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
    pub image_alpha: BTreeMap<String, bool>,
    pub image_variants: ImageVariantIndex,
    pub video_variants: VideoVariantIndex,
}

// ----------------------------
// Build planning models (DAG-friendly)
// ----------------------------

#[derive(Debug, Clone)]
pub enum TaskKind {
    RenderPage {
        page: DocId,
    },
    RenderBlogIndex {
        source_page: DocId,
        page_no: u32,
    },
    RenderSeries {
        series: SeriesId,
    },
    RenderTagIndex {
        tag: String,
    },
    RenderTagsIndex,
    RenderFrontPage,
    GenerateVarsCss {
        vars: ThemeVars,
        out_rel: String,
    },
    CopyImageOriginal {
        source: AssetSourceId,
        out_rel: String,
    },
    ResizeImage {
        source: AssetSourceId,
        width: u32,
        quality: u8,
        format: ImageOutputFormat,
        out_rel: String,
    },
    CopyVideoOriginal {
        source: AssetSourceId,
        out_rel: String,
    },
    TranscodeVideoMp4 {
        source: AssetSourceId,
        height: u32,
        out_rel: String,
    },
    ExtractVideoPoster {
        source: AssetSourceId,
        poster_time_sec: u32,
        out_rel: String,
    },
    GenerateRss,
    GenerateSitemap,
    CopyAsset {
        rel: AssetRelPath,
        source: AssetSourceId,
        out_rel: String,
    },
}

#[derive(Debug, Clone)]
pub enum ContentId {
    Doc(DocId),
    Series(SeriesId),
    Tag(String),
    Asset(AssetRelPath),
    Image(String),
    Video(String),
}

#[derive(Debug, Clone)]
pub struct OutputArtifact {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct BuildTask {
    pub id: TaskId,
    pub kind: TaskKind,
    pub inputs_fingerprint: InputFingerprint,

    /// Logical input identifiers (docs, assets, etc.).
    pub inputs: Vec<ContentId>,
    pub outputs: Vec<OutputArtifact>,
}

#[derive(Debug, Clone, Default)]
pub struct BuildPlan {
    pub tasks: Vec<BuildTask>,
    pub edges: Vec<(TaskId, TaskId)>,
}
