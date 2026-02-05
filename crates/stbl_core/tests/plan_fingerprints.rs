use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use stbl_core::assemble::assemble_site;
use stbl_core::assets::AssetIndex;
use stbl_core::config::load_site_config;
use stbl_core::header::{Header, UnknownKeyPolicy, parse_header};
use stbl_core::media::{ImagePlanInput, VideoPlanInput};
use stbl_core::model::{
    BlogAbstractConfig, BlogConfig, BlogPaginationConfig, BlogSeriesConfig, DiscoveredDoc, DocKind,
    ParsedDoc, Project, SourceDoc, TaskKind,
};
use stbl_core::plan::build_plan;

#[test]
fn task_ids_are_stable() {
    let project = load_fixture_project();
    let plan = build_plan(
        &project,
        &AssetIndex::default(),
        &ImagePlanInput::default(),
        &VideoPlanInput::default(),
    );
    let plan_second = build_plan(
        &project,
        &AssetIndex::default(),
        &ImagePlanInput::default(),
        &VideoPlanInput::default(),
    );
    let ids = plan
        .tasks
        .iter()
        .map(|task| task.id.clone())
        .collect::<Vec<_>>();
    let ids_second = plan_second
        .tasks
        .iter()
        .map(|task| task.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(ids, ids_second);
}

#[test]
fn fingerprint_changes_when_doc_changes() {
    let mut project = load_fixture_project();
    let plan = build_plan(
        &project,
        &AssetIndex::default(),
        &ImagePlanInput::default(),
        &VideoPlanInput::default(),
    );
    let page_id = project
        .content
        .pages
        .iter()
        .find(|page| page.source_path.ends_with("page1.md"))
        .expect("page1 exists")
        .id;
    let before = find_render_page_fingerprint(&plan, page_id);

    let page_mut = project
        .content
        .pages
        .iter_mut()
        .find(|page| page.source_path.ends_with("page1.md"))
        .expect("page1 exists");
    page_mut.content_hash = blake3::hash(b"page1-changed");

    let plan_changed = build_plan(
        &project,
        &AssetIndex::default(),
        &ImagePlanInput::default(),
        &VideoPlanInput::default(),
    );
    let after = find_render_page_fingerprint(&plan_changed, page_id);
    assert_ne!(before, after);
}

#[test]
fn fingerprint_changes_when_config_changes() {
    let mut project = load_fixture_project();
    let plan = build_plan(
        &project,
        &AssetIndex::default(),
        &ImagePlanInput::default(),
        &VideoPlanInput::default(),
    );
    let before = find_first_blog_index_fingerprint(&plan);

    project.config.blog = Some(BlogConfig {
        abstract_cfg: BlogAbstractConfig {
            enabled: true,
            max_chars: 123,
        },
        pagination: BlogPaginationConfig {
            enabled: false,
            page_size: 10,
        },
        series: BlogSeriesConfig { latest_parts: 3 },
    });

    let plan_changed = build_plan(
        &project,
        &AssetIndex::default(),
        &ImagePlanInput::default(),
        &VideoPlanInput::default(),
    );
    let after = find_first_blog_index_fingerprint(&plan_changed);
    assert_ne!(before, after);
}

#[test]
fn derived_page_fingerprint_changes_when_feed_item_changes() {
    let mut project = load_fixture_project();
    let plan = build_plan(
        &project,
        &AssetIndex::default(),
        &ImagePlanInput::default(),
        &VideoPlanInput::default(),
    );
    let before = find_tag_index_fingerprint(&plan, "rust");

    let page_mut = project
        .content
        .pages
        .iter_mut()
        .find(|page| page.source_path.ends_with("page2.md"))
        .expect("page2 exists");
    page_mut.content_hash = blake3::hash(b"page2-changed");

    let plan_changed = build_plan(
        &project,
        &AssetIndex::default(),
        &ImagePlanInput::default(),
        &VideoPlanInput::default(),
    );
    let after = find_tag_index_fingerprint(&plan_changed, "rust");
    assert_ne!(before, after);
}

fn find_render_page_fingerprint(
    plan: &stbl_core::model::BuildPlan,
    page_id: stbl_core::model::DocId,
) -> stbl_core::model::InputFingerprint {
    plan.tasks
        .iter()
        .find(|task| matches!(task.kind, TaskKind::RenderPage { page } if page == page_id))
        .map(|task| task.inputs_fingerprint)
        .expect("render page task")
}

fn find_first_blog_index_fingerprint(
    plan: &stbl_core::model::BuildPlan,
) -> stbl_core::model::InputFingerprint {
    plan.tasks
        .iter()
        .find(|task| matches!(task.kind, TaskKind::RenderBlogIndex { .. }))
        .map(|task| task.inputs_fingerprint)
        .expect("blog index task")
}

fn find_tag_index_fingerprint(
    plan: &stbl_core::model::BuildPlan,
    tag: &str,
) -> stbl_core::model::InputFingerprint {
    plan.tasks
        .iter()
        .find(|task| matches!(&task.kind, TaskKind::RenderTagIndex { tag: t } if t == tag))
        .map(|task| task.inputs_fingerprint)
        .expect("tag index task")
}

fn load_fixture_project() -> Project {
    let root = fixture_root();
    let config = load_site_config(&root.join("stbl.yaml")).expect("load config");
    let docs = scan_fixture(&root).expect("scan fixture");
    let content = assemble_site(docs).expect("assemble site");
    Project {
        root,
        config,
        content,
        image_alpha: std::collections::BTreeMap::new(),
        image_variants: Default::default(),
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
    let header = &raw[..header_end];
    let body = &raw[header_end..];
    (Some(header), body)
}

fn to_relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
