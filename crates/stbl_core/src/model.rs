//! Core document and site models

use crate::header::Header;

#[derive(Debug, Clone)]
pub enum DocKind {
    Page,
    SeriesIndex,
    SeriesPart,
}

#[derive(Debug, Clone)]
pub struct SourceDoc {
    pub source_path: String,
    pub dir_path: String,
    pub file_name: String,
    pub raw: String,
}

#[derive(Debug, Clone)]
pub struct ParsedDoc {
    pub src: SourceDoc,
    pub header: Header,
    pub body_markdown: String,
}

#[derive(Debug, Clone)]
pub struct DiscoveredDoc {
    pub parsed: ParsedDoc,
    pub kind: DocKind,
    pub series_dir: Option<String>,
}

pub type DocId = blake3::Hash;
pub type SeriesId = blake3::Hash;

#[derive(Debug, Clone)]
pub struct PageDoc {
    pub id: DocId,
    pub source_path: String,
    pub header: Header,
    pub body_markdown: String,
}

#[derive(Debug, Clone)]
pub struct SeriesPart {
    pub part_no: i32,
    pub doc: PageDoc,
}

#[derive(Debug, Clone)]
pub struct Series {
    pub id: SeriesId,
    pub dir_path: String,
    pub index: PageDoc,
    pub parts: Vec<SeriesPart>,
}

#[derive(Debug, Clone, Default)]
pub struct SiteTree {
    pub pages: Vec<PageDoc>,
    pub series: Vec<Series>,
    pub diagnostics: Vec<Diagnostic>,
}

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

pub type TaskId = blake3::Hash;

#[derive(Debug, Clone)]
pub enum TaskKind {
    VideoTranscode,
    ImageResize,
    RenderPage,
    RenderIndex,
    RenderFeeds,
    CopyAsset,
}

#[derive(Debug, Clone)]
pub struct BuildTask {
    pub id: TaskId,
    pub kind: TaskKind,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub deps: Vec<TaskId>,
}

#[derive(Debug, Clone, Default)]
pub struct BuildPlan {
    pub tasks: Vec<BuildTask>,
}
