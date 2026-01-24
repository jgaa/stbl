//! Content tree walker for stbl documents

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use stbl_core::header::parse_header;
use stbl_core::model::{DiscoveredDoc, DocKind, ParsedDoc, SourceDoc};
use walkdir::WalkDir;

pub fn walk_content(root: &Path, articles_dir: &Path) -> Result<Vec<DiscoveredDoc>> {
    let articles_dir = if articles_dir.is_absolute() {
        articles_dir.to_path_buf()
    } else {
        root.join(articles_dir)
    };
    let mut markdown_files = Vec::new();
    let mut series_dirs = HashSet::new();

    for entry in WalkDir::new(&articles_dir).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            if name.starts_with('_') {
                continue;
            }
        }
        if let Some(file_name) = path.file_name().and_then(|name| name.to_str()) {
            if file_name == "index.md" {
                if let Some(parent) = path.parent() {
                    if parent != articles_dir {
                        series_dirs.insert(parent.to_path_buf());
                    }
                }
            }
        }
        markdown_files.push(path.to_path_buf());
    }

    let mut docs = Vec::new();
    for path in markdown_files {
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let (header_opt, body_slice) = extract_header_body(&raw);
        let body_markdown = body_slice.to_string();
        let header = match header_opt {
            Some(text) => parse_header(text)
                .with_context(|| format!("failed to parse header in {}", path.display()))?,
            None => stbl_core::header::Header::default(),
        };

        let source_path = to_relative_path(root, &path);
        let dir_path = to_relative_path(
            root,
            path.parent().unwrap_or_else(|| Path::new("")),
        );
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();

        let parsed = ParsedDoc {
            src: SourceDoc {
                source_path: source_path.clone(),
                dir_path: dir_path.clone(),
                file_name: file_name.clone(),
                raw,
            },
            header,
            body_markdown,
        };

        let (kind, series_dir) = classify_doc(root, &path, &articles_dir, &series_dirs);
        docs.push(DiscoveredDoc {
            parsed,
            kind,
            series_dir,
        });
    }

    Ok(docs)
}

fn classify_doc(
    root: &Path,
    path: &Path,
    articles_dir: &Path,
    series_dirs: &HashSet<PathBuf>,
) -> (DocKind, Option<String>) {
    let dir = path.parent().unwrap_or(articles_dir);
    if series_dirs.contains(dir) {
        let dir_path = to_relative_path(root, dir);
        if path.file_name().and_then(|name| name.to_str()) == Some("index.md") {
            return (DocKind::SeriesIndex, Some(dir_path));
        }
        return (DocKind::SeriesPart, Some(dir_path));
    }

    if path.parent() == Some(articles_dir) {
        return (DocKind::Page, None);
    }

    (DocKind::Page, None)
}

fn extract_header_body(raw: &str) -> (Option<&str>, &str) {
    let mut offset = 0;
    let mut saw_header_line = false;
    let mut header_end = 0;
    for line in raw.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            header_end = offset + line.len();
            break;
        }
        if is_full_line_comment(line) {
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

fn is_full_line_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

fn looks_like_header_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    let (key, _) = match trimmed.split_once(':') {
        Some(value) => value,
        None => return false,
    };
    let key = key.trim_end();
    !key.is_empty() && key.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn to_relative_path(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use stbl_core::model::DocKind;
    use tempfile::TempDir;

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn walk_discovers_and_classifies_docs() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let articles = root.join("articles");

        write_file(&articles.join("standalone.md"), "title: Standalone\n\nBody");
        write_file(&articles.join("series/index.md"), "title: Series\n\nIntro");
        write_file(&articles.join("series/part1.md"), "title: Part 1\n\nBody");
        write_file(&articles.join("series/part2.md"), "title: Part 2\n\nBody");
        write_file(&articles.join("_grouped/index.md"), "title: Grouped\n\nBody");
        write_file(&articles.join("_grouped/part.md"), "title: Part\n\nBody");
        write_file(&articles.join("_ignored.md"), "title: Ignore\n\nBody");

        let docs = walk_content(root, &articles).expect("walk should succeed");
        assert_eq!(docs.len(), 6);

        let mut by_path = docs
            .into_iter()
            .map(|doc| (doc.parsed.src.source_path.clone(), doc))
            .collect::<std::collections::HashMap<_, _>>();

        let standalone = by_path.remove("articles/standalone.md").unwrap();
        assert!(matches!(standalone.kind, DocKind::Page));
        assert_eq!(standalone.series_dir, None);

        let series_index = by_path.remove("articles/series/index.md").unwrap();
        assert!(matches!(series_index.kind, DocKind::SeriesIndex));
        assert_eq!(series_index.series_dir.as_deref(), Some("articles/series"));

        let series_part = by_path.remove("articles/series/part1.md").unwrap();
        assert!(matches!(series_part.kind, DocKind::SeriesPart));
        assert_eq!(series_part.series_dir.as_deref(), Some("articles/series"));

        let grouped_index = by_path.remove("articles/_grouped/index.md").unwrap();
        assert!(matches!(grouped_index.kind, DocKind::SeriesIndex));
        assert_eq!(grouped_index.series_dir.as_deref(), Some("articles/_grouped"));
    }
}
