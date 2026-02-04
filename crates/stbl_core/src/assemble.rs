//! Site tree assembly and validation

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::SystemTime;

use crate::header::{Header, TemplateId};
use crate::model::{
    Diagnostic, DiagnosticLevel, DiscoveredDoc, DocId, DocKind, Page, Series, SeriesId, SeriesPart,
    SiteContent, WriteBackEdit, WriteBackPlan,
};
use chrono::Local;

struct SeriesPartCandidate {
    page: Page,
    part_no: Option<i32>,
    mtime: SystemTime,
    source_path: String,
    raw: String,
    header_present: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum TemplatePolicy {
    Warn,
    Error,
}

pub fn assemble_site(docs: Vec<DiscoveredDoc>) -> Result<SiteContent, Vec<Diagnostic>> {
    assemble_site_with_template_policy(docs, TemplatePolicy::Warn)
}

pub fn assemble_site_with_template_policy(
    docs: Vec<DiscoveredDoc>,
    template_policy: TemplatePolicy,
) -> Result<SiteContent, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut pages = Vec::new();
    let mut series_index_by_dir: HashMap<String, Page> = HashMap::new();
    let mut series_parts_by_dir: BTreeMap<String, Vec<SeriesPartCandidate>> = BTreeMap::new();
    let mut series_part_numbers: HashMap<String, HashMap<i32, String>> = HashMap::new();
    let mut write_back = WriteBackPlan::default();

    for doc in docs {
        if doc.parsed.header.published_needs_writeback && doc.parsed.header_present {
            if let Some(edit) = build_writeback_published_edit(&doc) {
                write_back.edits.push(edit);
            }
        }
        let doc_id_hash = blake3::hash(doc.parsed.src.source_path.as_bytes());
        let content_hash = blake3::hash(doc.parsed.src.raw.as_bytes());
        let mut header = doc.parsed.header.clone();
        normalize_template(
            &mut header,
            template_policy,
            &mut diagnostics,
            &doc.parsed.src.source_path,
        );
        let media_refs = crate::media::collect_media_refs(&doc.parsed.body_markdown);
        let banner_name = header.banner.clone();
        let page_doc = Page {
            id: DocId(doc_id_hash),
            source_path: doc.parsed.src.source_path.clone(),
            header,
            body_markdown: doc.parsed.body_markdown.clone(),
            banner_name,
            media_refs,
            url_path: String::new(),
            content_hash,
        };
        match doc.kind {
            DocKind::Page => pages.push(page_doc),
            DocKind::SeriesIndex => {
                if doc.parsed.src.file_name == "index.md" {
                    if let Some(series_dir) = doc.series_dir.clone() {
                        series_index_by_dir.insert(series_dir, page_doc);
                    } else {
                        pages.push(page_doc);
                    }
                } else {
                    pages.push(page_doc);
                }
            }
            DocKind::SeriesPart => {
                let series_dir = match doc.series_dir.clone() {
                    Some(dir) => dir,
                    None => {
                        pages.push(page_doc);
                        continue;
                    }
                };
                let part_text = doc.parsed.header.part.clone().unwrap_or_default();
                let part_no = if part_text.trim().is_empty() {
                    None
                } else {
                    match part_text.trim().parse::<i32>() {
                        Ok(value) if value >= 1 => Some(value),
                        Ok(_) => {
                            diagnostics.push(Diagnostic {
                                level: DiagnosticLevel::Error,
                                source_path: Some(doc.parsed.src.source_path.clone()),
                                message: "series part has invalid part number".to_string(),
                            });
                            continue;
                        }
                        Err(_) => {
                            diagnostics.push(Diagnostic {
                                level: DiagnosticLevel::Error,
                                source_path: Some(doc.parsed.src.source_path.clone()),
                                message: "series part has invalid part number".to_string(),
                            });
                            continue;
                        }
                    }
                };
                if let Some(value) = part_no {
                    let seen = series_part_numbers.entry(series_dir.clone()).or_default();
                    if let Some(existing) = seen.insert(value, doc.parsed.src.source_path.clone()) {
                        diagnostics.push(Diagnostic {
                            level: DiagnosticLevel::Error,
                            source_path: Some(doc.parsed.src.source_path.clone()),
                            message: format!(
                                "duplicate series part number {} (already used by {})",
                                value, existing
                            ),
                        });
                        continue;
                    }
                }
                series_parts_by_dir
                    .entry(series_dir)
                    .or_default()
                    .push(SeriesPartCandidate {
                        page: page_doc,
                        part_no,
                        mtime: doc.parsed.mtime,
                        source_path: doc.parsed.src.source_path.clone(),
                        raw: doc.parsed.src.raw.clone(),
                        header_present: doc.parsed.header_present,
                    });
            }
        }
    }

    let mut series = Vec::new();
    for (dir_path, candidates) in series_parts_by_dir {
        if let Some(index) = series_index_by_dir.remove(&dir_path) {
            let mut used_numbers = HashSet::new();
            let mut parts = Vec::new();
            let mut missing = Vec::new();

            for candidate in candidates {
                if let Some(value) = candidate.part_no {
                    used_numbers.insert(value);
                    parts.push(SeriesPart {
                        part_no: value,
                        page: candidate.page,
                    });
                } else {
                    missing.push(candidate);
                }
            }

            missing.sort_by_key(|candidate| candidate.mtime);
            let mut next_number = 1;
            for candidate in missing {
                while used_numbers.contains(&next_number) {
                    next_number += 1;
                }
                let assigned = next_number;
                next_number += 1;
                used_numbers.insert(assigned);
                if candidate.header_present {
                    if let Some(edit) =
                        build_writeback_part_edit(&candidate.raw, &candidate.source_path, assigned)
                    {
                        write_back.edits.push(edit);
                    }
                }
                parts.push(SeriesPart {
                    part_no: assigned,
                    page: candidate.page,
                });
            }

            let mut missing_numbers = Vec::new();
            if let Some(max) = used_numbers.iter().max().copied() {
                for value in 1..=max {
                    if !used_numbers.contains(&value) {
                        missing_numbers.push(value);
                    }
                }
            }
            if !missing_numbers.is_empty() {
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Warning,
                    source_path: None,
                    message: format!(
                        "series {} has missing part numbers: {}",
                        dir_path,
                        missing_numbers
                            .iter()
                            .map(|value| value.to_string())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                });
            }

            parts.sort_by_key(|part| part.part_no);
            series.push(Series {
                id: SeriesId(blake3::hash(dir_path.as_bytes())),
                dir_path,
                index,
                parts,
            });
        }
    }

    let site = SiteContent {
        pages,
        series,
        diagnostics: diagnostics.clone(),
        write_back,
    };

    if diagnostics
        .iter()
        .any(|diag| diag.level == DiagnosticLevel::Error)
    {
        Err(diagnostics)
    } else {
        Ok(site)
    }
}

fn normalize_template(
    header: &mut Header,
    policy: TemplatePolicy,
    diagnostics: &mut Vec<Diagnostic>,
    source_path: &str,
) {
    let Some(raw) = header.template_raw.take() else {
        return;
    };
    let value = raw.trim();
    if value.is_empty() {
        return;
    }
    if value.contains('/') {
        diagnostics.push(Diagnostic {
            level: template_level(policy),
            source_path: Some(source_path.to_string()),
            message: format!("template must be a filename without '/': {value}"),
        });
        return;
    }
    let normalized = match value {
        "landingpage.html" => TemplateId::Landing,
        "frontpage.html" => TemplateId::BlogIndex,
        "info.html" => TemplateId::Info,
        "landing" => TemplateId::Landing,
        "blog_index" => TemplateId::BlogIndex,
        "page" => TemplateId::Page,
        "info" => TemplateId::Info,
        _ => {
            diagnostics.push(Diagnostic {
                level: template_level(policy),
                source_path: Some(source_path.to_string()),
                message: format!("unknown template: {value}"),
            });
            return;
        }
    };
    header.template = Some(normalized);
}

fn template_level(policy: TemplatePolicy) -> DiagnosticLevel {
    match policy {
        TemplatePolicy::Warn => DiagnosticLevel::Warning,
        TemplatePolicy::Error => DiagnosticLevel::Error,
    }
}

fn build_writeback_published_edit(doc: &DiscoveredDoc) -> Option<WriteBackEdit> {
    let published_text = Local::now().format("%Y-%m-%d %H:%M").to_string();
    let (new_header_text, new_body) =
        update_header_value(&doc.parsed.src.raw, "published", &published_text)?;
    Some(WriteBackEdit {
        path: doc.parsed.src.source_path.clone(),
        new_header_text: Some(new_header_text),
        new_body: Some(new_body),
    })
}

fn build_writeback_part_edit(raw: &str, source_path: &str, part_no: i32) -> Option<WriteBackEdit> {
    let (new_header_text, new_body) = update_header_value(raw, "part", &part_no.to_string())?;
    Some(WriteBackEdit {
        path: source_path.to_string(),
        new_header_text: Some(new_header_text),
        new_body: Some(new_body),
    })
}

fn update_header_value(raw: &str, key: &str, value: &str) -> Option<(String, String)> {
    let header_end = header_end_index(raw)?;
    let (header_text, body) = raw.split_at(header_end);
    let mut lines: Vec<String> = header_text.lines().map(|line| line.to_string()).collect();
    let mut replaced = false;

    for line in &mut lines {
        if line.trim().is_empty() {
            continue;
        }
        let stripped = strip_inline_comment(line);
        let (found_key, _) = match stripped.split_once(':') {
            Some(value) => value,
            None => continue,
        };
        if found_key.trim() == key {
            *line = format!("{key}: {value}");
            replaced = true;
        }
    }

    if !replaced {
        let insert_at = lines
            .iter()
            .position(|line| line.trim().is_empty())
            .unwrap_or(lines.len());
        lines.insert(insert_at, format!("{key}: {value}"));
    }

    let mut new_header_text = lines.join("\n");
    if !new_header_text.ends_with('\n') {
        new_header_text.push('\n');
    }
    if !new_header_text.ends_with("\n\n") {
        new_header_text.push('\n');
    }

    Some((new_header_text, body.to_string()))
}

fn header_end_index(raw: &str) -> Option<usize> {
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
            return None;
        }
        saw_header_line = true;
        offset += line.len();
        header_end = offset;
    }
    if !saw_header_line {
        return None;
    }
    if header_end == 0 {
        header_end = raw.len();
    }
    Some(header_end)
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
    !key.is_empty()
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
}

fn strip_inline_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    for (idx, byte) in bytes.iter().enumerate() {
        if *byte == b'#' && idx > 0 && bytes[idx - 1].is_ascii_whitespace() {
            return line[..idx].trim_end();
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::Header;
    use crate::model::{DiscoveredDoc, DocKind, ParsedDoc, SourceDoc};
    use std::time::SystemTime;
    use uuid::Uuid;

    fn source(path: &str, dir: &str, file: &str) -> SourceDoc {
        SourceDoc {
            source_path: path.to_string(),
            dir_path: dir.to_string(),
            file_name: file.to_string(),
            raw: String::new(),
        }
    }

    fn parsed(path: &str, dir: &str, file: &str, header: Header) -> ParsedDoc {
        ParsedDoc {
            src: source(path, dir, file),
            header,
            body_markdown: "body".to_string(),
            header_present: true,
            mtime: SystemTime::UNIX_EPOCH,
        }
    }

    fn header_with_part(part: Option<&str>) -> Header {
        let mut header = Header::default();
        header.uuid = Some(Uuid::new_v4());
        header.part = part.map(|value| value.to_string());
        header
    }

    fn header_with_template(template: &str) -> Header {
        let mut header = Header::default();
        header.uuid = Some(Uuid::new_v4());
        header.template_raw = Some(template.to_string());
        header
    }

    #[test]
    fn series_with_index_and_parts_sorts() {
        let index = DiscoveredDoc {
            parsed: parsed("series/index.md", "series", "index.md", Header::default()),
            kind: DocKind::SeriesIndex,
            series_dir: Some("series".to_string()),
        };
        let part2 = DiscoveredDoc {
            parsed: parsed(
                "series/part2.md",
                "series",
                "part2.md",
                header_with_part(Some("2")),
            ),
            kind: DocKind::SeriesPart,
            series_dir: Some("series".to_string()),
        };
        let part1 = DiscoveredDoc {
            parsed: parsed(
                "series/part1.md",
                "series",
                "part1.md",
                header_with_part(Some("1")),
            ),
            kind: DocKind::SeriesPart,
            series_dir: Some("series".to_string()),
        };
        let site = assemble_site(vec![index, part2, part1]).expect("should assemble");
        assert_eq!(site.series.len(), 1);
        let series = &site.series[0];
        assert_eq!(series.parts[0].part_no, 1);
        assert_eq!(series.parts[1].part_no, 2);
    }

    #[test]
    fn series_part_missing_part_produces_error() {
        let part = DiscoveredDoc {
            parsed: parsed(
                "series/part1.md",
                "series",
                "part1.md",
                header_with_part(None),
            ),
            kind: DocKind::SeriesPart,
            series_dir: Some("series".to_string()),
        };
        let site = assemble_site(vec![part]).expect("should assemble");
        assert_eq!(site.series.len(), 0);
    }

    #[test]
    fn duplicate_part_numbers_produce_error() {
        let index = DiscoveredDoc {
            parsed: parsed("series/index.md", "series", "index.md", Header::default()),
            kind: DocKind::SeriesIndex,
            series_dir: Some("series".to_string()),
        };
        let part1a = DiscoveredDoc {
            parsed: parsed(
                "series/part1a.md",
                "series",
                "part1a.md",
                header_with_part(Some("1")),
            ),
            kind: DocKind::SeriesPart,
            series_dir: Some("series".to_string()),
        };
        let part1b = DiscoveredDoc {
            parsed: parsed(
                "series/part1b.md",
                "series",
                "part1b.md",
                header_with_part(Some("1")),
            ),
            kind: DocKind::SeriesPart,
            series_dir: Some("series".to_string()),
        };
        let err = assemble_site(vec![index, part1a, part1b]).expect_err("expected error");
        assert!(err.iter().any(|diag| diag.level == DiagnosticLevel::Error));
    }

    #[test]
    fn template_frontpage_html_normalizes() {
        let doc = DiscoveredDoc {
            parsed: parsed(
                "articles/page.md",
                "articles",
                "page.md",
                header_with_template("frontpage.html"),
            ),
            kind: DocKind::Page,
            series_dir: None,
        };
        let site =
            assemble_site_with_template_policy(vec![doc], TemplatePolicy::Warn).expect("assemble");
        assert_eq!(site.pages.len(), 1);
        assert_eq!(site.pages[0].header.template, Some(TemplateId::BlogIndex));
        assert!(site.diagnostics.is_empty());
    }

    #[test]
    fn template_normalized_id_passes_through() {
        let docs = vec![
            DiscoveredDoc {
                parsed: parsed(
                    "articles/page.md",
                    "articles",
                    "page.md",
                    header_with_template("blog_index"),
                ),
                kind: DocKind::Page,
                series_dir: None,
            },
            DiscoveredDoc {
                parsed: parsed(
                    "articles/info.md",
                    "articles",
                    "info.md",
                    header_with_template("info"),
                ),
                kind: DocKind::Page,
                series_dir: None,
            },
        ];
        let site =
            assemble_site_with_template_policy(docs, TemplatePolicy::Warn).expect("assemble");
        assert_eq!(site.pages.len(), 2);
        assert_eq!(site.pages[0].header.template, Some(TemplateId::BlogIndex));
        assert_eq!(site.pages[1].header.template, Some(TemplateId::Info));
        assert!(site.diagnostics.is_empty());
    }

    #[test]
    fn template_info_html_normalizes() {
        let doc = DiscoveredDoc {
            parsed: parsed(
                "articles/info.md",
                "articles",
                "info.md",
                header_with_template("info.html"),
            ),
            kind: DocKind::Page,
            series_dir: None,
        };
        let site =
            assemble_site_with_template_policy(vec![doc], TemplatePolicy::Warn).expect("assemble");
        assert_eq!(site.pages.len(), 1);
        assert_eq!(site.pages[0].header.template, Some(TemplateId::Info));
        assert!(site.diagnostics.is_empty());
    }

    #[test]
    fn template_with_path_emits_warning() {
        let doc = DiscoveredDoc {
            parsed: parsed(
                "articles/page.md",
                "articles",
                "page.md",
                header_with_template("templates/frontpage.html"),
            ),
            kind: DocKind::Page,
            series_dir: None,
        };
        let site =
            assemble_site_with_template_policy(vec![doc], TemplatePolicy::Warn).expect("assemble");
        assert_eq!(site.pages.len(), 1);
        assert_eq!(site.pages[0].header.template, None);
        assert_eq!(site.diagnostics.len(), 1);
        assert_eq!(site.diagnostics[0].level, DiagnosticLevel::Warning);
    }

    #[test]
    fn template_with_path_errors_in_strict_mode() {
        let doc = DiscoveredDoc {
            parsed: parsed(
                "articles/page.md",
                "articles",
                "page.md",
                header_with_template("templates/frontpage.html"),
            ),
            kind: DocKind::Page,
            series_dir: None,
        };
        let err =
            assemble_site_with_template_policy(vec![doc], TemplatePolicy::Error).expect_err("err");
        assert_eq!(err.len(), 1);
        assert_eq!(err[0].level, DiagnosticLevel::Error);
    }

    #[test]
    fn standalone_pages_collected() {
        let page = DiscoveredDoc {
            parsed: parsed("pages/a.md", "pages", "a.md", Header::default()),
            kind: DocKind::Page,
            series_dir: None,
        };
        let site = assemble_site(vec![page]).expect("should assemble");
        assert_eq!(site.pages.len(), 1);
        assert_eq!(site.series.len(), 0);
    }
}
