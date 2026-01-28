use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use stbl_core::assemble::assemble_site;
use stbl_core::config::load_site_config;
use stbl_core::header::{Header, UnknownKeyPolicy, parse_header};
use stbl_core::model::{DiscoveredDoc, DocKind, ParsedDoc, Project, SourceDoc, TaskKind};
use stbl_core::plan::build_plan;

#[test]
fn build_plan_is_deterministic_and_complete() {
    let root = fixture_root();
    let config = load_site_config(&root.join("stbl.yaml")).expect("load config");
    let docs = scan_fixture(&root).expect("scan fixture");
    let content = assemble_site(docs).expect("assemble site");
    let project = Project {
        root: root.clone(),
        config,
        content,
    };

    let plan = build_plan(&project);
    let plan_second = build_plan(&project);

    let task_ids: Vec<_> = plan.tasks.iter().map(|task| task.id).collect();
    let task_ids_second: Vec<_> = plan_second.tasks.iter().map(|task| task.id).collect();
    assert_eq!(task_ids, task_ids_second);
    assert_eq!(plan.edges, plan_second.edges);
    assert!(is_sorted(&plan.tasks));

    let mut kind_counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for task in &plan.tasks {
        *kind_counts.entry(kind_label(&task.kind)).or_default() += 1;
    }
    assert_eq!(kind_counts.get("RenderPage"), Some(&4));
    assert_eq!(kind_counts.get("RenderBlogIndex"), Some(&1));
    assert_eq!(kind_counts.get("RenderSeries"), Some(&1));
    assert_eq!(kind_counts.get("RenderTagIndex"), Some(&2));
    assert_eq!(kind_counts.get("RenderTagsIndex"), Some(&1));
    assert_eq!(kind_counts.get("RenderFrontPage"), None);
    let rss_enabled = project.config.rss.as_ref().is_some_and(|rss| rss.enabled);
    if rss_enabled {
        assert_eq!(kind_counts.get("GenerateRss"), Some(&1));
    } else {
        assert_eq!(kind_counts.get("GenerateRss"), None);
    }
    assert_eq!(kind_counts.get("GenerateSitemap"), Some(&1));

    let rss_id = find_task_id(&plan.tasks, "GenerateRss");
    if let Some(rss_id) = rss_id {
        let page_ids = plan
            .tasks
            .iter()
            .filter(|task| matches!(task.kind, TaskKind::RenderPage { .. }))
            .map(|task| task.id)
            .collect::<Vec<_>>();
        for page_id in page_ids {
            assert!(
                plan.edges.contains(&(page_id, rss_id)),
                "rss should depend on page render"
            );
        }
    }
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("site1")
}

fn scan_fixture(root: &Path) -> anyhow::Result<Vec<DiscoveredDoc>> {
    let mut docs = Vec::new();
    let articles = root.join("articles");
    let entries = [
        articles.join("index.md"),
        articles.join("excluded.md"),
        articles.join("info.md"),
        articles.join("page1.md"),
        articles.join("page2.md"),
        articles.join("series/index.md"),
        articles.join("series/part1.md"),
        articles.join("series/part2.md"),
        articles.join("series/part3.md"),
    ];

    for path in entries {
        let raw = fs::read_to_string(&path)?;
        let mtime = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let (header_opt, body_slice) = extract_header_body(&raw);
        let header_present = header_opt.is_some();
        let header_text = header_opt.map(str::to_string);
        let body_markdown = body_slice.to_string();
        let header = match header_text.as_deref() {
            Some(text) => parse_header(text, UnknownKeyPolicy::Error)?.header,
            None => Header::default(),
        };

        let rel_path = to_relative_path(root, &path);
        let dir_path = to_relative_path(root, path.parent().unwrap_or_else(|| Path::new("")));
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("")
            .to_string();

        let parsed = ParsedDoc {
            src: SourceDoc {
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
        docs.push(DiscoveredDoc {
            parsed,
            kind,
            series_dir,
        });
    }

    Ok(docs)
}

fn classify_doc(root: &Path, path: &Path, articles_dir: &Path) -> (DocKind, Option<String>) {
    let file_name = path.file_name().and_then(|name| name.to_str());
    let parent = path.parent().unwrap_or(articles_dir);
    if parent != articles_dir {
        let dir_path = to_relative_path(root, parent);
        if file_name == Some("index.md") {
            return (DocKind::SeriesIndex, Some(dir_path));
        }
        return (DocKind::SeriesPart, Some(dir_path));
    }
    (DocKind::Page, None)
}

fn extract_header_body(raw: &str) -> (Option<&str>, &str) {
    if let Some((header, body)) = extract_frontmatter(raw) {
        return (Some(header), body);
    }
    extract_plain_header(raw)
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

fn to_relative_path(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.to_string_lossy().replace('\\', "/")
}

fn kind_label(kind: &TaskKind) -> &'static str {
    match kind {
        TaskKind::RenderPage { .. } => "RenderPage",
        TaskKind::RenderBlogIndex { .. } => "RenderBlogIndex",
        TaskKind::RenderSeries { .. } => "RenderSeries",
        TaskKind::RenderTagIndex { .. } => "RenderTagIndex",
        TaskKind::RenderTagsIndex => "RenderTagsIndex",
        TaskKind::RenderFrontPage => "RenderFrontPage",
        TaskKind::GenerateRss => "GenerateRss",
        TaskKind::GenerateSitemap => "GenerateSitemap",
        TaskKind::CopyAsset { .. } => "CopyAsset",
    }
}

fn find_task_id(
    tasks: &[stbl_core::model::BuildTask],
    label: &'static str,
) -> Option<stbl_core::model::TaskId> {
    tasks
        .iter()
        .find(|task| kind_label(&task.kind) == label)
        .map(|task| task.id)
}

fn is_sorted(tasks: &[stbl_core::model::BuildTask]) -> bool {
    tasks
        .windows(2)
        .all(|pair| pair[0].id.0.as_bytes() <= pair[1].id.0.as_bytes())
}
