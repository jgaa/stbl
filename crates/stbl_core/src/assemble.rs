//! Site tree assembly and validation

use std::collections::{BTreeMap, HashMap};

use crate::model::{
    Diagnostic, DiagnosticLevel, DiscoveredDoc, DocKind, PageDoc, Series, SeriesPart, SiteTree,
};

pub fn assemble_site(docs: Vec<DiscoveredDoc>) -> Result<SiteTree, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut pages = Vec::new();
    let mut series_index_by_dir: HashMap<String, PageDoc> = HashMap::new();
    let mut series_parts_by_dir: BTreeMap<String, Vec<SeriesPart>> = BTreeMap::new();
    let mut series_part_numbers: HashMap<String, HashMap<i32, String>> = HashMap::new();

    for doc in docs {
        let page_doc = PageDoc {
            id: blake3::hash(doc.parsed.src.source_path.as_bytes()),
            source_path: doc.parsed.src.source_path.clone(),
            header: doc.parsed.header.clone(),
            body_markdown: doc.parsed.body_markdown.clone(),
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
                if part_text.trim().is_empty() {
                    diagnostics.push(Diagnostic {
                        level: DiagnosticLevel::Error,
                        source_path: Some(doc.parsed.src.source_path.clone()),
                        message: "series part missing required part number".to_string(),
                    });
                    continue;
                }
                let part_no = match part_text.trim().parse::<i32>() {
                    Ok(value) => value,
                    Err(_) => {
                        diagnostics.push(Diagnostic {
                            level: DiagnosticLevel::Error,
                            source_path: Some(doc.parsed.src.source_path.clone()),
                            message: "series part has invalid part number".to_string(),
                        });
                        continue;
                    }
                };
                let seen = series_part_numbers
                    .entry(series_dir.clone())
                    .or_default();
                if let Some(existing) = seen.insert(part_no, doc.parsed.src.source_path.clone()) {
                    diagnostics.push(Diagnostic {
                        level: DiagnosticLevel::Error,
                        source_path: Some(doc.parsed.src.source_path.clone()),
                        message: format!(
                            "duplicate series part number {} (already used by {})",
                            part_no, existing
                        ),
                    });
                    continue;
                }
                series_parts_by_dir
                    .entry(series_dir)
                    .or_default()
                    .push(SeriesPart { part_no, doc: page_doc });
            }
        }
    }

    let mut series = Vec::new();
    for (dir_path, mut parts) in series_parts_by_dir {
        if let Some(index) = series_index_by_dir.remove(&dir_path) {
            parts.sort_by_key(|part| part.part_no);
            series.push(Series {
                id: blake3::hash(dir_path.as_bytes()),
                dir_path,
                index,
                parts,
            });
        }
    }

    let site = SiteTree {
        pages,
        series,
        diagnostics: diagnostics.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::header::Header;
    use crate::model::{DiscoveredDoc, DocKind, ParsedDoc, SourceDoc};
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
        }
    }

    fn header_with_part(part: Option<&str>) -> Header {
        let mut header = Header::default();
        header.uuid = Uuid::new_v4();
        header.part = part.map(|value| value.to_string());
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
        let err = assemble_site(vec![part]).expect_err("expected error");
        assert!(err.iter().any(|diag| diag.level == DiagnosticLevel::Error));
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
