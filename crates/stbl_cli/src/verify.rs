use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;
use pulldown_cmark::{Event, Parser, Tag};
use serde_yaml::Value;
use stbl_core::header::{Header, HeaderError, HeaderWarning, TemplateId, UnknownKeyPolicy, parse_header};
use stbl_core::model::DocKind;
use stbl_core::url::{UrlMapper, logical_key_from_source_path};

use crate::media::resolve_banner_name;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IssueLevel {
    Warning,
    Error,
}

#[derive(Debug, Clone)]
struct Issue {
    level: IssueLevel,
    source_path: Option<String>,
    message: String,
}

#[derive(Debug, Default)]
struct Report {
    issues: Vec<Issue>,
}

impl Report {
    fn add(&mut self, level: IssueLevel, source_path: Option<String>, message: String) {
        let label = match level {
            IssueLevel::Warning => "warning",
            IssueLevel::Error => "error",
        };
        if let Some(path) = source_path.as_deref() {
            eprintln!("{label}: {path}: {message}");
        } else {
            eprintln!("{label}: {message}");
        }
        self.issues.push(Issue {
            level,
            source_path,
            message,
        });
    }

    fn error(&mut self, source_path: Option<String>, message: impl Into<String>) {
        self.add(IssueLevel::Error, source_path, message.into());
    }

    fn warn(&mut self, source_path: Option<String>, message: impl Into<String>) {
        self.add(IssueLevel::Warning, source_path, message.into());
    }

    fn counts(&self) -> (usize, usize) {
        let mut errors = 0;
        let mut warnings = 0;
        for issue in &self.issues {
            match issue.level {
                IssueLevel::Error => errors += 1,
                IssueLevel::Warning => warnings += 1,
            }
        }
        (errors, warnings)
    }

    fn grouped(&self) -> BTreeMap<String, Vec<&Issue>> {
        let mut grouped: BTreeMap<String, Vec<&Issue>> = BTreeMap::new();
        for issue in &self.issues {
            let key = issue
                .source_path
                .clone()
                .unwrap_or_else(|| "<global>".to_string());
            grouped.entry(key).or_default().push(issue);
        }
        grouped
    }
}

#[derive(Debug)]
struct DocInfo {
    source_path: String,
    header: Header,
    header_present: bool,
    body_markdown: String,
    mtime: SystemTime,
    kind: DocKind,
    series_dir: Option<String>,
}

#[derive(Debug)]
struct DocListing {
    path: String,
    mtime: SystemTime,
}

pub fn run_verify(root: &Path, articles_dir: &Path, strict: bool, verbose: bool) -> Result<i32> {
    let mut report = Report::default();

    let (config, config_ok) = verify_config(root, &mut report);

    let docs = match collect_docs(root, articles_dir, verbose, &mut report) {
        Ok(docs) => docs,
        Err(err) => {
            report.error(
                Some(articles_dir.display().to_string()),
                format!("failed to scan documents: {err}"),
            );
            Vec::new()
        }
    };

    let mut withheld = Vec::new();
    let mut new_articles = Vec::new();

    for doc in &docs {
        if doc.header_present && !doc.header.is_published {
            withheld.push(DocListing {
                path: doc.source_path.clone(),
                mtime: doc.mtime,
            });
        }
        if doc.header_present && doc.header.published_needs_writeback || !doc.header_present {
            new_articles.push(DocListing {
                path: doc.source_path.clone(),
                mtime: doc.mtime,
            });
        }
        verify_banner(root, doc, &mut report);
        verify_markdown_links(root, doc, &mut report);
    }

    verify_duplicate_uuids(&docs, &mut report);
    verify_tag_case_mismatches(&docs, &mut report);
    verify_series_headers(&docs, &mut report);
    if config_ok {
        if let Some(config) = &config {
            verify_site_logo(root, config, &mut report);
            verify_url_collisions(config, &docs, &mut report);
        }
    }

    withheld.sort_by_key(|item| item.mtime);
    new_articles.retain(|item| !is_index_markdown(&item.path));
    new_articles.sort_by_key(|item| item.mtime);

    print_report(&report, &withheld, &new_articles);

    let (errors, warnings) = report.counts();
    if errors > 0 {
        return Ok(1);
    }
    if strict && warnings > 0 {
        return Ok(1);
    }
    Ok(0)
}

fn verify_config(root: &Path, report: &mut Report) -> (Option<stbl_core::model::SiteConfig>, bool) {
    let config_path = root.join("stbl.yaml");
    if !config_path.exists() {
        report.error(
            Some("stbl.yaml".to_string()),
            format!("missing config file: {}", config_path.display()),
        );
        return (None, false);
    }
    let raw = match fs::read_to_string(&config_path) {
        Ok(contents) => contents,
        Err(err) => {
            report.error(
                Some("stbl.yaml".to_string()),
                format!("failed to read config: {err}"),
            );
            return (None, false);
        }
    };
    match serde_yaml::from_str::<Value>(&raw) {
        Ok(value) => {
            if let Some(schema) = load_schema_from_docs(root) {
                warn_unknown_config_entries_with_schema(&value, &schema, report, "stbl.yaml");
            } else {
                warn_unknown_config_entries(&value, report, "stbl.yaml");
            }
        }
        Err(err) => {
            report.error(
                Some("stbl.yaml".to_string()),
                format!("failed to parse YAML: {err}"),
            );
            return (None, false);
        }
    }

    match stbl_core::config::load_site_config(&config_path) {
        Ok(config) => (Some(config), true),
        Err(err) => {
            report.error(
                Some("stbl.yaml".to_string()),
                format!("invalid config: {err}"),
            );
            (None, false)
        }
    }
}

fn collect_docs(
    root: &Path,
    articles_dir: &Path,
    verbose: bool,
    report: &mut Report,
) -> Result<Vec<DocInfo>> {
    let articles_dir = if articles_dir.is_absolute() {
        articles_dir.to_path_buf()
    } else {
        root.join(articles_dir)
    };
    if !articles_dir.exists() {
        report.error(
            Some(articles_dir.display().to_string()),
            "articles directory not found".to_string(),
        );
        return Ok(Vec::new());
    }

    let mut markdown_files = Vec::new();
    let mut series_dirs = HashSet::new();

    for entry in walkdir::WalkDir::new(&articles_dir)
        .into_iter()
        .filter_map(Result::ok)
    {
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
        let raw = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) => {
                report.error(
                    Some(to_relative_path(root, &path)),
                    format!("failed to read: {err}"),
                );
                continue;
            }
        };
        let mtime = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        let (header_opt, body_slice) = extract_header_body(&raw);
        let header_present = header_opt.is_some();
        let header_text = header_opt.map(str::to_string);
        let body_markdown = body_slice.to_string();
        let header = match header_text.as_deref() {
            Some(text) => match parse_header(text, UnknownKeyPolicy::Warn) {
                Ok(parsed) => {
                    emit_header_warnings(report, &path, &parsed.warnings);
                    emit_header_notices(&path, &parsed.notices, verbose);
                    parsed.header
                }
                Err(err) => {
                    report.error(
                        Some(to_relative_path(root, &path)),
                        header_error_message(err),
                    );
                    Header::default()
                }
            },
            None => Header::default(),
        };

        let source_path = to_relative_path(root, &path);
        let (kind, series_dir) = classify_doc(root, &path, &articles_dir, &series_dirs);
        docs.push(DocInfo {
            source_path,
            header,
            header_present,
            body_markdown,
            mtime,
            kind,
            series_dir,
        });
    }

    Ok(docs)
}

fn emit_header_warnings(report: &mut Report, path: &Path, warnings: &[HeaderWarning]) {
    if warnings.is_empty() {
        return;
    }
    let source_path = Some(path.to_string_lossy().replace('\\', "/"));
    for warning in warnings {
        report.warn(source_path.clone(), warning.message());
    }
}

fn emit_header_notices(path: &Path, notices: &[stbl_core::header::HeaderNotice], verbose: bool) {
    if !verbose {
        return;
    }
    for notice in notices {
        eprintln!("debug: {}: {}", path.display(), notice.message());
    }
}

fn header_error_message(err: HeaderError) -> String {
    format!("invalid header: {err}")
}

fn verify_banner(root: &Path, doc: &DocInfo, report: &mut Report) {
    let Some(banner) = doc.header.banner.as_deref() else {
        return;
    };
    if banner.trim().is_empty() {
        report.error(
            Some(doc.source_path.clone()),
            "banner must not be empty".to_string(),
        );
        return;
    }
    if banner.contains('/') || banner.contains('\\') {
        report.error(
            Some(doc.source_path.clone()),
            format!("banner must be an image name, not a path: {banner}"),
        );
        return;
    }
    if let Err(err) = resolve_banner_name(root, banner) {
        report.error(
            Some(doc.source_path.clone()),
            format!("banner image not found: {err}"),
        );
    }
}

fn verify_site_logo(root: &Path, config: &stbl_core::model::SiteConfig, report: &mut Report) {
    let Some(raw) = config.site.logo.as_deref() else {
        return;
    };
    if let Err(err) = crate::assets::resolve_site_logo(root, raw) {
        report.warn(Some("stbl.yaml".to_string()), format!("{err}"));
    }
}

fn verify_markdown_links(root: &Path, doc: &DocInfo, report: &mut Report) {
    let parser = Parser::new(&doc.body_markdown);
    for event in parser {
        match event {
            Event::Start(Tag::Image { dest_url, .. }) => {
                validate_media_link(root, doc, &dest_url, report);
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                validate_external_link(doc, &dest_url, report);
            }
            _ => {}
        }
    }
}

fn validate_media_link(root: &Path, doc: &DocInfo, dest_url: &str, report: &mut Report) {
    let base = dest_url.split(';').next().unwrap_or("").trim();
    if base.is_empty() {
        report.error(
            Some(doc.source_path.clone()),
            "media link is empty".to_string(),
        );
        return;
    }
    if base.starts_with("//") {
        report.error(
            Some(doc.source_path.clone()),
            format!("media link must be relative: {base}"),
        );
        return;
    }
    if let Some(scheme) = parse_scheme(base) {
        report.error(
            Some(doc.source_path.clone()),
            format!("media link must be relative (found scheme '{scheme}'): {base}"),
        );
        return;
    }
    if base.starts_with('/') {
        report.error(
            Some(doc.source_path.clone()),
            format!("media link must be relative: {base}"),
        );
        return;
    }
    if base.starts_with("images/") || base.starts_with("video/") {
        let abs = root.join(base);
        if !abs.is_file() {
            report.error(
                Some(doc.source_path.clone()),
                format!("media file not found: {base}"),
            );
        }
    }
}

fn validate_external_link(doc: &DocInfo, dest_url: &str, report: &mut Report) {
    let trimmed = dest_url.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return;
    }
    if trimmed.starts_with("//") {
        report.error(
            Some(doc.source_path.clone()),
            format!("external link missing scheme: {trimmed}"),
        );
        return;
    }
    let Some(scheme) = parse_scheme(trimmed) else {
        return;
    };
    let allowed = ["http", "https", "mailto", "tel"];
    if !allowed.contains(&scheme) {
        report.error(
            Some(doc.source_path.clone()),
            format!("external link uses unsupported scheme '{scheme}': {trimmed}"),
        );
        return;
    }
    if let Err(err) = validate_url_syntax(trimmed, scheme) {
        report.error(
            Some(doc.source_path.clone()),
            format!("external link invalid: {err}"),
        );
    }
}

fn validate_url_syntax(url: &str, scheme: &str) -> Result<(), String> {
    if url.chars().any(|ch| ch.is_whitespace()) {
        return Err(format!("contains whitespace: {url}"));
    }
    match scheme {
        "http" | "https" => {
            let rest = url.strip_prefix(&format!("{scheme}://")).unwrap_or("");
            let host = rest
                .split(['/', '?', '#'])
                .next()
                .unwrap_or("")
                .trim();
            if host.is_empty() {
                return Err(format!("missing host in {url}"));
            }
        }
        "mailto" => {
            let rest = url.strip_prefix("mailto:").unwrap_or("");
            if rest.trim().is_empty() || !rest.contains('@') {
                return Err(format!("invalid mailto address: {url}"));
            }
        }
        "tel" => {
            let rest = url.strip_prefix("tel:").unwrap_or("");
            if rest.trim().is_empty() {
                return Err(format!("invalid tel address: {url}"));
            }
        }
        _ => {}
    }
    Ok(())
}

fn parse_scheme(value: &str) -> Option<&str> {
    let mut chars = value.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let mut end = 1;
    for ch in chars {
        if ch == ':' {
            return Some(&value[..end]);
        }
        if !(ch.is_ascii_alphanumeric() || ch == '+' || ch == '-' || ch == '.') {
            return None;
        }
        end += ch.len_utf8();
    }
    None
}

fn verify_duplicate_uuids(docs: &[DocInfo], report: &mut Report) {
    let mut seen: HashMap<String, String> = HashMap::new();
    let mut flagged: HashSet<String> = HashSet::new();
    for doc in docs {
        let Some(uuid) = doc.header.uuid.as_ref() else {
            continue;
        };
        let uuid_text = uuid.to_string();
        if let Some(existing) = seen.get(&uuid_text) {
            report.error(
                Some(doc.source_path.clone()),
                format!("duplicate uuid {uuid_text} (already used by {existing})"),
            );
            if flagged.insert(uuid_text.clone()) {
                report.error(
                    Some(existing.clone()),
                    format!("duplicate uuid {uuid_text} (also used by {})", doc.source_path),
                );
            }
        } else {
            seen.insert(uuid_text, doc.source_path.clone());
        }
    }
}

fn verify_tag_case_mismatches(docs: &[DocInfo], report: &mut Report) {
    let mut tag_map: BTreeMap<String, BTreeMap<String, BTreeSet<String>>> = BTreeMap::new();
    for doc in docs {
        for tag in &doc.header.tags {
            let key = tag.to_lowercase();
            let variants = tag_map.entry(key).or_default();
            variants
                .entry(tag.clone())
                .or_default()
                .insert(doc.source_path.clone());
        }
    }
    for (key, variants) in tag_map {
        if variants.len() <= 1 {
            continue;
        }
        let mut message = format!("tag case mismatch for '{key}':");
        for (variant, paths) in variants.into_iter() {
            let list = paths.into_iter().collect::<Vec<_>>().join(", ");
            message.push_str("\n  ");
            message.push_str(&variant);
            message.push_str(": ");
            message.push_str(&list);
        }
        report.warn(None, message);
    }
}

fn verify_series_headers(docs: &[DocInfo], report: &mut Report) {
    for doc in docs {
        let source = Some(doc.source_path.clone());
        if doc.series_dir.is_none()
            && doc.header.part.as_deref().unwrap_or("").trim().len() > 0
        {
            report.warn(
                source.clone(),
                "series part header is set but page is not part of a series".to_string(),
            );
        }
        if doc.series_dir.is_some() {
            let is_info_template = matches!(doc.header.template, Some(TemplateId::Info));
            let is_info_type = doc
                .header
                .content_type
                .as_deref()
                .map(|value| value.eq_ignore_ascii_case("info"))
                .unwrap_or(false);
            if is_info_template || is_info_type {
                report.warn(
                    source,
                    "series pages must not use info template or content type".to_string(),
                );
            }
        }
    }
}

fn verify_url_collisions(
    config: &stbl_core::model::SiteConfig,
    docs: &[DocInfo],
    report: &mut Report,
) {
    let mapper = UrlMapper::new(config);
    let mut outputs: HashMap<String, String> = HashMap::new();
    let mut flagged: HashSet<String> = HashSet::new();
    for doc in docs {
        let logical_key = match doc.kind {
            DocKind::SeriesIndex => match doc.series_dir.as_deref() {
                Some(dir) => logical_key_from_source_path(dir),
                None => logical_key_from_source_path(&doc.source_path),
            },
            _ => logical_key_from_source_path(&doc.source_path),
        };
        let mapping = mapper.map(&logical_key);
        let primary = mapping.primary_output.to_string_lossy().replace('\\', "/");
        record_collision(
            doc,
            &primary,
            &mut outputs,
            &mut flagged,
            report,
        );
        if let Some(fallback) = mapping.fallback {
            let fallback_path = fallback.from.to_string_lossy().replace('\\', "/");
            record_collision(
                doc,
                &fallback_path,
                &mut outputs,
                &mut flagged,
                report,
            );
        }
    }
}

fn record_collision(
    doc: &DocInfo,
    output_path: &str,
    outputs: &mut HashMap<String, String>,
    flagged: &mut HashSet<String>,
    report: &mut Report,
) {
    if let Some(existing) = outputs.get(output_path) {
        let key = format!("{output_path}|{existing}");
        report.error(
            Some(doc.source_path.clone()),
            format!("output collision at {output_path} (already used by {existing})"),
        );
        if flagged.insert(key) {
            report.error(
                Some(existing.clone()),
                format!("output collision at {output_path} (also used by {})", doc.source_path),
            );
        }
    } else {
        outputs.insert(output_path.to_string(), doc.source_path.clone());
    }
}

fn print_report(report: &Report, withheld: &[DocListing], new_articles: &[DocListing]) {
    let (errors, warnings) = report.counts();
    println!("errors: {errors} warnings: {warnings}");
    println!();
    println!("Diagnostics:");
    let grouped = report.grouped();
    if grouped.is_empty() {
        println!("none");
    } else {
        for (path, issues) in grouped {
            println!("{path}:");
            for issue in issues {
                let label = match issue.level {
                    IssueLevel::Warning => "warning",
                    IssueLevel::Error => "error",
                };
                println!("  {label}: {}", issue.message);
            }
        }
    }
    println!();
    println!("Withheld articles:");
    if withheld.is_empty() {
        println!("none");
    } else {
        for item in withheld {
            println!("- {}", item.path);
        }
    }
    println!();
    println!("New or missing published:");
    if new_articles.is_empty() {
        println!("none");
    } else {
        for item in new_articles {
            println!("- {}", item.path);
        }
    }
}

fn is_index_markdown(path: &str) -> bool {
    path == "index.md" || path.ends_with("/index.md")
}

fn load_schema_from_docs(root: &Path) -> Option<Value> {
    let mut candidates = Vec::new();
    candidates.push(root.join("doc/config-schema.md"));
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("doc/config-schema.md"));
    }

    for path in candidates {
        if !path.exists() {
            continue;
        }
        let contents = fs::read_to_string(&path).ok()?;
        let yaml = extract_schema_yaml(&contents)?;
        if let Ok(schema) = serde_yaml::from_str::<Value>(&yaml) {
            return Some(schema);
        }
    }
    None
}

fn extract_schema_yaml(contents: &str) -> Option<String> {
    let mut in_schema_section = false;
    let mut in_code = false;
    let mut buffer = Vec::new();

    for line in contents.lines() {
        if line.trim_start().starts_with("## Full schema") {
            in_schema_section = true;
        }
        if !in_code && in_schema_section && line.trim_start().starts_with("```yaml") {
            in_code = true;
            continue;
        }
        if in_code {
            if line.trim_start().starts_with("```") {
                break;
            }
            buffer.push(line);
        }
    }
    if buffer.is_empty() {
        return None;
    }
    Some(buffer.join("\n"))
}

fn warn_unknown_config_entries_with_schema(
    value: &Value,
    schema: &Value,
    report: &mut Report,
    path: &str,
) {
    if !matches!(value, Value::Mapping(_)) {
        report.warn(Some(path.to_string()), "config root must be a map".to_string());
        return;
    }
    warn_unknown_entries_against_schema(value, schema, report, path);
}

fn warn_unknown_entries_against_schema(
    value: &Value,
    schema: &Value,
    report: &mut Report,
    path: &str,
) {
    match (value, schema) {
        (Value::Mapping(map), Value::Mapping(schema_map)) => {
            let mut placeholder_schema: Option<&Value> = None;
            let mut allowed_keys = HashSet::new();
            for (key, schema_value) in schema_map {
                if let Some(key_str) = key.as_str() {
                    if key_str.starts_with('<') && key_str.ends_with('>') {
                        placeholder_schema = Some(schema_value);
                    } else {
                        allowed_keys.insert(key_str.to_string());
                    }
                }
            }
            for (key, value) in map {
                let key_str = key.as_str().unwrap_or("<non-string>");
                if allowed_keys.contains(key_str) {
                    if let Some(schema_value) = schema_map.get(key) {
                        let child_path = format!("{path}.{key_str}");
                        warn_unknown_entries_against_schema(value, schema_value, report, &child_path);
                    }
                    continue;
                }
                if let Some(schema_value) = placeholder_schema {
                    let child_path = format!("{path}.{key_str}");
                    warn_unknown_entries_against_schema(value, schema_value, report, &child_path);
                } else {
                    report.warn(
                        Some(normalize_config_path(path)),
                        format!("unknown config entry: {key_str}"),
                    );
                }
            }
        }
        (Value::Sequence(values), Value::Sequence(schema_values)) => {
            if let Some(schema_entry) = schema_values.first() {
                for (idx, value) in values.iter().enumerate() {
                    let child_path = format!("{path}[{idx}]");
                    warn_unknown_entries_against_schema(value, schema_entry, report, &child_path);
                }
            }
        }
        _ => {}
    }
}

fn warn_unknown_config_entries(value: &Value, report: &mut Report, path: &str) {
    let Some(map) = value.as_mapping() else {
        report.warn(Some(path.to_string()), "config root must be a map".to_string());
        return;
    };
    let allowed = [
        "site",
        "banner",
        "menu",
        "theme",
        "assets",
        "security",
        "media",
        "footer",
        "people",
        "blog",
        "system",
        "publish",
        "rss",
        "seo",
        "comments",
    ];
    for (key, value) in map {
        let key = key.as_str().unwrap_or("<non-string>");
        if !allowed.contains(&key) {
            report.warn(
                Some(path.to_string()),
                format!("unknown config entry: {key}"),
            );
            continue;
        }
        match key {
            "site" => warn_unknown_site_entries(value, report, path, "site"),
            "banner" => warn_unknown_entries(value, report, path, "banner", &["widths", "quality", "align"]),
            "menu" => warn_unknown_list_entries(value, report, path, "menu", &["title", "href"]),
            "theme" => warn_unknown_theme_entries(value, report, path),
            "assets" => warn_unknown_entries(value, report, path, "assets", &["cache_busting"]),
            "security" => warn_unknown_security_entries(value, report, path),
            "media" => warn_unknown_media_entries(value, report, path),
            "footer" => warn_unknown_entries(value, report, path, "footer", &["show_stbl"]),
            "people" => warn_unknown_people_entries(value, report, path),
            "blog" => warn_unknown_blog_entries(value, report, path),
            "system" => warn_unknown_system_entries(value, report, path),
            "publish" => warn_unknown_entries(value, report, path, "publish", &["command"]),
            "rss" => warn_unknown_entries(value, report, path, "rss", &["enabled", "max_items", "ttl_days"]),
            "seo" => warn_unknown_seo_entries(value, report, path),
            "comments" | "chroma" | "plyr" => {}
            _ => {}
        }
    }
}

fn warn_unknown_security_entries(value: &Value, report: &mut Report, path: &str) {
    warn_unknown_entries(value, report, path, "security", &["svg"]);
    if let Some(map) = value.as_mapping() {
        if let Some(svg) = map.get(&Value::String("svg".to_string())) {
            warn_unknown_entries(svg, report, path, "security.svg", &["mode"]);
        }
    }
}

fn warn_unknown_site_entries(value: &Value, report: &mut Report, path: &str, prefix: &str) {
    let allowed = [
        "id",
        "title",
        "abstract",
        "copyright",
        "base_url",
        "language",
        "timezone",
        "url_style",
        "nav",
    ];
    warn_unknown_entries(value, report, path, prefix, &allowed);
    if let Some(map) = value.as_mapping() {
        if let Some(nav) = map.get(&Value::String("nav".to_string())) {
            warn_unknown_list_entries(nav, report, path, "site.nav", &["label", "href"]);
        }
    }
}

fn warn_unknown_theme_entries(value: &Value, report: &mut Report, path: &str) {
    warn_unknown_entries(
        value,
        report,
        path,
        "theme",
        &[
            "variant",
            "max_body_width",
            "breakpoints",
            "colors",
            "nav",
            "wide_background",
        ],
    );
    if let Some(map) = value.as_mapping() {
        if let Some(breakpoints) = map.get(&Value::String("breakpoints".to_string())) {
            warn_unknown_entries(
                breakpoints,
                report,
                path,
                "theme.breakpoints",
                &["desktop_min", "wide_min"],
            );
        }
        if let Some(colors) = map.get(&Value::String("colors".to_string())) {
            warn_unknown_entries(
                colors,
                report,
                path,
                "theme.colors",
                &[
                    "bg",
                    "fg",
                    "heading",
                    "accent",
                    "link",
                    "muted",
                    "surface",
                    "border",
                    "link_hover",
                    "code_bg",
                    "code_fg",
                    "quote_bg",
                    "quote_border",
                    "wide_bg",
                ],
            );
        }
        if let Some(nav) = map.get(&Value::String("nav".to_string())) {
            warn_unknown_entries(nav, report, path, "theme.nav", &["bg", "fg", "border"]);
        }
        if let Some(wide) = map.get(&Value::String("wide_background".to_string())) {
            warn_unknown_entries(
                wide,
                report,
                path,
                "theme.wide_background",
                &["color", "image", "style", "position", "opacity"],
            );
        }
    }
}

fn warn_unknown_media_entries(value: &Value, report: &mut Report, path: &str) {
    warn_unknown_entries(value, report, path, "media", &["images", "video"]);
    if let Some(map) = value.as_mapping() {
        if let Some(images) = map.get(&Value::String("images".to_string())) {
            warn_unknown_entries(
                images,
                report,
                path,
                "media.images",
                &["widths", "quality"],
            );
        }
        if let Some(video) = map.get(&Value::String("video".to_string())) {
            warn_unknown_entries(
                video,
                report,
                path,
                "media.video",
                &["heights", "poster_time"],
            );
        }
    }
}

fn warn_unknown_people_entries(value: &Value, report: &mut Report, path: &str) {
    warn_unknown_entries(value, report, path, "people", &["default", "entries"]);
    let Some(map) = value.as_mapping() else {
        return;
    };
    let Some(entries) = map.get(&Value::String("entries".to_string())) else {
        return;
    };
    let Some(entries_map) = entries.as_mapping() else {
        return;
    };
    for (key, value) in entries_map {
        let entry_id = key.as_str().unwrap_or("<non-string>");
        let prefix = format!("people.entries.{entry_id}");
        warn_unknown_entries(value, report, path, &prefix, &["name", "email", "links"]);
        if let Some(entry_map) = value.as_mapping() {
            if let Some(links) = entry_map.get(&Value::String("links".to_string())) {
                warn_unknown_list_entries(links, report, path, &format!("{prefix}.links"), &["id", "name", "url", "icon"]);
            }
        }
    }
}

fn warn_unknown_blog_entries(value: &Value, report: &mut Report, path: &str) {
    warn_unknown_entries(
        value,
        report,
        path,
        "blog",
        &["abstract", "pagination", "page_size", "series"],
    );
    if let Some(map) = value.as_mapping() {
        if let Some(abstract_cfg) = map.get(&Value::String("abstract".to_string())) {
            warn_unknown_entries(
                abstract_cfg,
                report,
                path,
                "blog.abstract",
                &["enabled", "max_chars"],
            );
        }
        if let Some(pagination) = map.get(&Value::String("pagination".to_string())) {
            warn_unknown_entries(
                pagination,
                report,
                path,
                "blog.pagination",
                &["enabled", "page_size"],
            );
        }
        if let Some(series) = map.get(&Value::String("series".to_string())) {
            warn_unknown_entries(series, report, path, "blog.series", &["latest_parts"]);
        }
    }
}

fn warn_unknown_system_entries(value: &Value, report: &mut Report, path: &str) {
    warn_unknown_entries(value, report, path, "system", &["date"]);
    if let Some(map) = value.as_mapping() {
        if let Some(date_cfg) = map.get(&Value::String("date".to_string())) {
            warn_unknown_entries(
                date_cfg,
                report,
                path,
                "system.date",
                &["format", "roundup_seconds"],
            );
        }
    }
}

fn warn_unknown_seo_entries(value: &Value, report: &mut Report, path: &str) {
    warn_unknown_entries(value, report, path, "seo", &["sitemap"]);
    if let Some(map) = value.as_mapping() {
        if let Some(sitemap) = map.get(&Value::String("sitemap".to_string())) {
            warn_unknown_entries(sitemap, report, path, "seo.sitemap", &["priority"]);
            if let Some(site_map) = sitemap.as_mapping() {
                if let Some(priority) = site_map.get(&Value::String("priority".to_string())) {
                    warn_unknown_entries(
                        priority,
                        report,
                        path,
                        "seo.sitemap.priority",
                        &["frontpage", "article", "series", "tag", "tags"],
                    );
                }
            }
        }
    }
}

fn warn_unknown_entries(
    value: &Value,
    report: &mut Report,
    _path: &str,
    prefix: &str,
    allowed: &[&str],
) {
    let Some(map) = value.as_mapping() else {
        return;
    };
    for (key, _) in map {
        let key_str = key.as_str().unwrap_or("<non-string>");
        if !allowed.contains(&key_str) {
            report.warn(
                Some(normalize_config_path(prefix)),
                format!("unknown config entry: {key_str}"),
            );
        }
    }
}

fn warn_unknown_list_entries(
    value: &Value,
    report: &mut Report,
    _path: &str,
    prefix: &str,
    allowed: &[&str],
) {
    let Some(list) = value.as_sequence() else {
        return;
    };
    for (idx, item) in list.iter().enumerate() {
        let Some(map) = item.as_mapping() else {
            continue;
        };
        for (key, _) in map {
            let key_str = key.as_str().unwrap_or("<non-string>");
            if !allowed.contains(&key_str) {
                let entry_path = format!("{prefix}[{idx}]");
                report.warn(
                    Some(normalize_config_path(&entry_path)),
                    format!("unknown config entry: {key_str}"),
                );
            }
        }
    }
}

fn normalize_config_path(path: &str) -> String {
    let trimmed = path.strip_prefix("stbl.yaml.").unwrap_or(path);
    let trimmed = trimmed.strip_prefix("stbl.yaml").unwrap_or(trimmed);
    let trimmed = trimmed.trim_start_matches('.');
    if trimmed.is_empty() {
        "<root>".to_string()
    } else {
        trimmed.to_string()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scheme_recognizes_standard_schemes() {
        assert_eq!(parse_scheme("https://example.com"), Some("https"));
        assert_eq!(parse_scheme("mailto:foo@example.com"), Some("mailto"));
        assert_eq!(parse_scheme("tel:+123"), Some("tel"));
        assert_eq!(parse_scheme("/relative/path"), None);
    }

    #[test]
    fn validate_url_syntax_rejects_missing_host() {
        let err = validate_url_syntax("https://", "https").expect_err("expected error");
        assert!(err.contains("missing host"));
    }

    #[test]
    fn validate_url_syntax_accepts_basic_http() {
        validate_url_syntax("https://example.com/path", "https").expect("valid");
    }

    #[test]
    fn verify_tag_case_mismatch_reports_variants() {
        let mut report = Report::default();
        let mut header_a = Header::default();
        header_a.tags = vec!["grpc".to_string()];
        let mut header_b = Header::default();
        header_b.tags = vec!["gRPC".to_string()];
        let docs = vec![
            DocInfo {
                source_path: "articles/a.md".to_string(),
                header: header_a,
                header_present: true,
                body_markdown: String::new(),
                mtime: SystemTime::UNIX_EPOCH,
                kind: DocKind::Page,
                series_dir: None,
            },
            DocInfo {
                source_path: "articles/b.md".to_string(),
                header: header_b,
                header_present: true,
                body_markdown: String::new(),
                mtime: SystemTime::UNIX_EPOCH,
                kind: DocKind::Page,
                series_dir: None,
            },
        ];

        verify_tag_case_mismatches(&docs, &mut report);
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("tag case mismatch")));
    }

    #[test]
    fn index_markdown_is_ignored_for_new_articles() {
        assert!(is_index_markdown("index.md"));
        assert!(is_index_markdown("articles/index.md"));
        assert!(!is_index_markdown("articles/post.md"));
    }

    #[test]
    fn verify_series_headers_flags_part_outside_series_and_info_in_series() {
        let mut report = Report::default();
        let mut header_part = Header::default();
        header_part.part = Some("1".to_string());
        let mut header_info = Header::default();
        header_info.template = Some(TemplateId::Info);
        let docs = vec![
            DocInfo {
                source_path: "articles/standalone.md".to_string(),
                header: header_part,
                header_present: true,
                body_markdown: String::new(),
                mtime: SystemTime::UNIX_EPOCH,
                kind: DocKind::Page,
                series_dir: None,
            },
            DocInfo {
                source_path: "articles/series/part1.md".to_string(),
                header: header_info,
                header_present: true,
                body_markdown: String::new(),
                mtime: SystemTime::UNIX_EPOCH,
                kind: DocKind::SeriesPart,
                series_dir: Some("articles/series".to_string()),
            },
        ];

        verify_series_headers(&docs, &mut report);
        let messages = report
            .issues
            .iter()
            .map(|issue| issue.message.as_str())
            .collect::<Vec<_>>();
        assert!(messages
            .iter()
            .any(|message| message.contains("not part of a series")));
        assert!(messages
            .iter()
            .any(|message| message.contains("must not use info template")));
    }

    #[test]
    fn warn_unknown_entries_uses_schema_placeholders() {
        let schema = serde_yaml::from_str::<Value>(
            r#"
comments:
  <provider>:
    template: string
    <key>: string
"#,
        )
        .expect("schema");
        let value = serde_yaml::from_str::<Value>(
            r#"
comments:
  disqus:
    template: "x"
    shortname: "y"
unknown_root: true
"#,
        )
        .expect("value");
        let mut report = Report::default();
        warn_unknown_entries_against_schema(&value, &schema, &mut report, "stbl.yaml");
        assert!(report
            .issues
            .iter()
            .any(|issue| issue.message.contains("unknown config entry")));
    }
}
