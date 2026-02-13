use std::fs;
use std::path::{Path, PathBuf};

use crate::assets::{AssetSourceLookup, copy_asset_to_out};
use crate::media::{ImageSourceLookup, VideoSourceLookup};
use anyhow::{Context, Result, anyhow, bail};
use image::codecs::avif::AvifEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, GenericImageView, ImageEncoder};
use stbl_cache::CacheStore;
use stbl_core::assets::AssetManifest;
use stbl_core::blog_index::{
    FeedItem, blog_index_page_logical_key, blog_pagination_settings, collect_blog_feed,
    collect_tag_feed, paginate_blog_index,
};
use stbl_core::comments::CommentTemplateProvider;
use stbl_core::feeds::{render_rss, render_sitemap};
use stbl_core::macros::{IncludeProvider, IncludeRequest, IncludeResponse};
use stbl_core::model::{BuildPlan, BuildTask, DocId, Page, Project, Series, TaskKind};
use stbl_core::render::{RenderOptions, render_markdown_to_html_with_media};
use stbl_core::theme::{ResolvedThemeVars, resolve_theme_vars};
use stbl_core::templates::{
    BlogIndexItem, BlogIndexPart, SeriesIndexPart, SeriesNavEntry, SeriesNavLink, SeriesNavView,
    TagLink,
    TagListingPage, format_timestamp_display, format_timestamp_long_date, format_timestamp_rfc3339,
    normalize_timestamp,
    page_title_or_filename,
    render_banner_html, render_blog_index, render_markdown_page, render_page,
    render_page_with_series_nav, render_redirect_page, render_series_index, render_tag_index,
};
use stbl_core::url::{UrlMapper, logical_key_from_source_path, map_series_index};
use stbl_core::visibility::is_published_page;
use stbl_embedded_assets as embedded;
use std::process::Command;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct ExecSummary {
    pub executed: usize,
    pub skipped: usize,
    pub executed_ids: Vec<String>,
    pub skipped_ids: Vec<String>,
}

struct FsIncludeProvider {
    site_root: PathBuf,
    canonical_root: PathBuf,
}

impl FsIncludeProvider {
    fn new(site_root: &Path) -> Result<Self> {
        let canonical_root = fs::canonicalize(site_root)
            .with_context(|| format!("failed to canonicalize site root {}", site_root.display()))?;
        Ok(Self {
            site_root: site_root.to_path_buf(),
            canonical_root,
        })
    }
}

impl IncludeProvider for FsIncludeProvider {
    fn include(&self, request: &IncludeRequest<'_>) -> Result<IncludeResponse> {
        let raw_path = Path::new(request.path);
        if raw_path.is_absolute() {
            bail!("include path must be relative");
        }

        let base_dir = match request.current_source_path.and_then(|path| path.parent()) {
            Some(parent) => self.site_root.join(parent),
            None => self.site_root.clone(),
        };
        let target = base_dir.join(raw_path);
        let canonical = fs::canonicalize(&target)
            .with_context(|| format!("failed to resolve include {}", target.display()))?;
        if !canonical.starts_with(&self.canonical_root) {
            bail!("include path escapes site root");
        }

        let content = fs::read_to_string(&canonical)
            .with_context(|| format!("failed to read include {}", canonical.display()))?;

        Ok(IncludeResponse {
            content,
            resolved_id: canonical.to_string_lossy().to_string(),
        })
    }
}

struct FsCommentTemplateProvider {
    site_root: PathBuf,
    canonical_root: PathBuf,
}

impl FsCommentTemplateProvider {
    fn new(site_root: &Path) -> Result<Self> {
        let canonical_root = fs::canonicalize(site_root)
            .with_context(|| format!("failed to canonicalize site root {}", site_root.display()))?;
        Ok(Self {
            site_root: site_root.to_path_buf(),
            canonical_root,
        })
    }
}

impl CommentTemplateProvider for FsCommentTemplateProvider {
    fn load_template(&self, template: &str) -> Result<Option<String>> {
        let raw_path = Path::new(template);
        if raw_path.is_absolute() {
            bail!("comments template path must be relative");
        }

        let mut candidates = Vec::new();
        candidates.push(self.site_root.join(raw_path));
        if !template.contains('/') && !template.contains('\\') {
            candidates.push(self.site_root.join("templates").join(raw_path));
        }

        for candidate in candidates {
            if !candidate.is_file() {
                continue;
            }
            let canonical = fs::canonicalize(&candidate)
                .with_context(|| format!("failed to resolve template {}", candidate.display()))?;
            if !canonical.starts_with(&self.canonical_root) {
                bail!("comments template path escapes site root");
            }
            let contents = fs::read_to_string(&canonical).with_context(|| {
                format!("failed to read comments template {}", canonical.display())
            })?;
            return Ok(Some(contents));
        }

        if let Some(contents) = load_embedded_comment_template(template)? {
            return Ok(Some(contents));
        }

        Ok(None)
    }
}

fn load_embedded_comment_template(template: &str) -> Result<Option<String>> {
    let embedded_template = match embedded::template("default") {
        Some(template) => template,
        None => return Ok(None),
    };
    let candidates = embedded_template_candidates(template);
    for candidate in candidates {
        if let Some(entry) = embedded_template
            .assets
            .iter()
            .find(|entry| entry.path == candidate)
        {
            let bytes = embedded::decompress_to_vec(&entry.hash)
                .ok_or_else(|| anyhow!("failed to decompress embedded template {}", candidate))?;
            let contents = String::from_utf8(bytes)
                .map_err(|err| anyhow!("embedded template {} is not utf-8: {}", candidate, err))?;
            return Ok(Some(contents));
        }
    }
    Ok(None)
}

fn embedded_template_candidates(template: &str) -> Vec<String> {
    let trimmed = template.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    if trimmed.contains('/') || trimmed.contains('\\') {
        candidates.push(trimmed.replace('\\', "/"));
    } else {
        candidates.push(trimmed.to_string());
        candidates.push(format!("templates/{trimmed}"));
    }
    candidates
}

pub fn execute_plan(
    project: &Project,
    plan: &BuildPlan,
    out_dir: &PathBuf,
    asset_index: &stbl_core::assets::AssetIndex,
    asset_lookup: &AssetSourceLookup,
    image_lookup: &ImageSourceLookup,
    video_lookup: &VideoSourceLookup,
    asset_manifest: &AssetManifest,
    mut cache: Option<&mut dyn CacheStore>,
    jobs: Option<usize>,
    regenerate_content: bool,
) -> Result<ExecSummary> {
    let mut report = ExecSummary::default();
    let mapper = UrlMapper::new(&project.config);
    let build_date_ymd = build_date_ymd_now();
    let mut image_jobs: Vec<BuildTask> = Vec::new();
    let mut video_jobs: Vec<BuildTask> = Vec::new();
    let mut copy_asset_jobs: Vec<BuildTask> = Vec::new();
    for task in &plan.tasks {
        if let TaskKind::GenerateVarsCss { vars: _, out_rel } = &task.kind {
            if should_skip_task(&mut cache, task, out_dir, regenerate_content)? {
                report.skipped += output_count(task);
                report.skipped_ids.push(task.id.0.clone());
                continue;
            }
            let out_path = out_dir.join(out_rel);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            let defaults = stbl_embedded_assets::template_colors_yaml(&project.config.theme.variant)
                .map_err(|err| anyhow!("theme defaults for {}: {}", project.config.theme.variant, err))?;
            let resolved = resolve_theme_vars(defaults, &project.config)?;
            let contents = render_vars_css(&resolved);
            fs::write(&out_path, contents)
                .with_context(|| format!("failed to write {}", out_path.display()))?;
            report.executed += output_count(task);
            report.executed_ids.push(task.id.0.clone());
            cache_put(&mut cache, task);
        }
    }
    for task in &plan.tasks {
        if matches!(task.kind, TaskKind::GenerateRss)
            && !project.config.rss.as_ref().is_some_and(|rss| rss.enabled)
        {
            continue;
        }
        if matches!(task.kind, TaskKind::GenerateVarsCss { .. }) {
            continue;
        }
        if should_skip_task(&mut cache, task, out_dir, regenerate_content)? {
            report.skipped += output_count(task);
            report.skipped_ids.push(task.id.0.clone());
            continue;
        }
        if matches!(task.kind, TaskKind::CopyAsset { .. }) {
            copy_asset_jobs.push(task.clone());
            continue;
        }
        match &task.kind {
            TaskKind::CopyImageOriginal { source, out_rel } => {
                copy_image_original(out_dir, out_rel, source, image_lookup)?;
                report.executed += output_count(task);
                report.executed_ids.push(task.id.0.clone());
                cache_put(&mut cache, task);
                continue;
            }
            TaskKind::ResizeImage { .. } => {
                image_jobs.push(task.clone());
                continue;
            }
            TaskKind::CopyVideoOriginal { source, out_rel } => {
                copy_video_original(out_dir, out_rel, source, video_lookup)?;
                report.executed += output_count(task);
                report.executed_ids.push(task.id.0.clone());
                cache_put(&mut cache, task);
                continue;
            }
            TaskKind::TranscodeVideoMp4 { .. } => {
                video_jobs.push(task.clone());
                continue;
            }
            TaskKind::ExtractVideoPoster { .. } => {
                video_jobs.push(task.clone());
                continue;
            }
            _ => {}
        }
        for output in &task.outputs {
            let out_path = out_dir.join(&output.path);
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
            let contents = render_output(
                project,
                &mapper,
                &task.kind,
                &output.path,
                asset_manifest,
                &build_date_ymd,
            )
            .with_context(|| format!("failed to render {}", out_path.display()))?;
            fs::write(&out_path, contents)
                .with_context(|| format!("failed to write {}", out_path.display()))?;
        }
        report.executed += output_count(task);
        report.executed_ids.push(task.id.0.clone());
        cache_put(&mut cache, task);
    }
    if !image_jobs.is_empty() {
        let max_threads = jobs.unwrap_or_else(max_parallelism);
        let results = run_parallel_image_jobs(
            image_jobs,
            max_threads,
            out_dir,
            image_lookup,
        )?;
        for (task, result) in results {
            result?;
            report.executed += output_count(&task);
            report.executed_ids.push(task.id.0.clone());
            cache_put(&mut cache, &task);
        }
    }
    if !video_jobs.is_empty() {
        let max_threads = jobs.unwrap_or_else(max_parallelism);
        let video_threads = std::cmp::max(1, max_threads / 4);
        let results = run_parallel_video_jobs(
            video_jobs,
            video_threads,
            out_dir,
            video_lookup,
        )?;
        for (task, result) in results {
            result?;
            report.executed += output_count(&task);
            report.executed_ids.push(task.id.0.clone());
            cache_put(&mut cache, &task);
        }
    }
    if !copy_asset_jobs.is_empty() {
        let used_icons = collect_used_icons(out_dir, asset_index, asset_lookup, asset_manifest)?;
        for task in copy_asset_jobs {
            let TaskKind::CopyAsset { rel, source, out_rel } = &task.kind else {
                continue;
            };
            let is_icon = is_icon_asset(&rel.0);
            if is_icon && !used_icons.contains(&rel.0) {
                let out_path = out_dir.join(out_rel);
                if out_path.exists() {
                    let _ = fs::remove_file(&out_path);
                }
                report.skipped += output_count(&task);
                report.skipped_ids.push(task.id.0.clone());
                continue;
            }
            if should_skip_task(&mut cache, &task, out_dir, regenerate_content)? {
                report.skipped += output_count(&task);
                report.skipped_ids.push(task.id.0.clone());
                continue;
            }
            copy_asset_to_out(out_dir, out_rel, source, asset_lookup, &project.config.security)?;
            report.executed += output_count(&task);
            report.executed_ids.push(task.id.0.clone());
            cache_put(&mut cache, &task);
        }
    }
    Ok(report)
}

fn is_icon_asset(rel: &str) -> bool {
    (rel.starts_with("icons/") || rel.starts_with("feather/"))
        && rel.to_ascii_lowercase().ends_with(".svg")
}

fn collect_used_icons(
    out_dir: &Path,
    asset_index: &stbl_core::assets::AssetIndex,
    asset_lookup: &AssetSourceLookup,
    asset_manifest: &AssetManifest,
) -> Result<std::collections::HashSet<String>> {
    use std::collections::{HashMap, HashSet};
    let mut icons = HashSet::new();
    for asset in &asset_index.assets {
        if is_icon_asset(&asset.rel.0) {
            icons.insert(asset.rel.0.clone());
        }
    }
    if icons.is_empty() {
        return Ok(HashSet::new());
    }
    let mut out_to_rel = HashMap::new();
    for (rel, out) in &asset_manifest.entries {
        if is_icon_asset(rel) {
            out_to_rel.insert(out.clone(), rel.clone());
        }
    }

    let mut used = HashSet::new();
    scan_output_for_icons(out_dir, &icons, &out_to_rel, &mut used)?;
    scan_css_assets_for_icons(asset_index, asset_lookup, &icons, &out_to_rel, &mut used)?;
    Ok(used)
}

fn scan_output_for_icons(
    out_dir: &Path,
    icons: &std::collections::HashSet<String>,
    out_to_rel: &std::collections::HashMap<String, String>,
    used: &mut std::collections::HashSet<String>,
) -> Result<()> {
    for entry in walkdir::WalkDir::new(out_dir).follow_links(false) {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let ext = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if !matches!(ext, "html" | "css" | "xml") {
            continue;
        }
        let bytes = fs::read(entry.path())?;
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        scan_text_for_icons(text, icons, out_to_rel, used);
    }
    Ok(())
}

fn scan_css_assets_for_icons(
    asset_index: &stbl_core::assets::AssetIndex,
    asset_lookup: &AssetSourceLookup,
    icons: &std::collections::HashSet<String>,
    out_to_rel: &std::collections::HashMap<String, String>,
    used: &mut std::collections::HashSet<String>,
) -> Result<()> {
    for asset in &asset_index.assets {
        if !asset.rel.0.ends_with(".css") {
            continue;
        }
        let source = asset_lookup
            .resolve(&asset.source)
            .ok_or_else(|| anyhow!("unknown asset source {}", asset.source.0))?;
        let bytes = match source {
            crate::assets::AssetSource::File(path) => fs::read(path)?,
            crate::assets::AssetSource::Embedded(bytes) => bytes.clone(),
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        scan_text_for_icons(text, icons, out_to_rel, used);
    }
    Ok(())
}

fn scan_text_for_icons(
    text: &str,
    icons: &std::collections::HashSet<String>,
    out_to_rel: &std::collections::HashMap<String, String>,
    used: &mut std::collections::HashSet<String>,
) {
    scan_with_prefix(text, "icons/", |candidate| {
        if icons.contains(candidate) {
            used.insert(candidate.to_string());
        }
    });
    scan_with_prefix(text, "feather/", |candidate| {
        if icons.contains(candidate) {
            used.insert(candidate.to_string());
        }
    });
    scan_with_prefix(text, "artifacts/", |candidate| {
        if let Some(rel) = out_to_rel.get(candidate) {
            used.insert(rel.clone());
        }
    });
    scan_with_prefix(text, "/artifacts/", |candidate| {
        let trimmed = candidate.trim_start_matches('/');
        if let Some(rel) = out_to_rel.get(trimmed) {
            used.insert(rel.clone());
        }
    });
}

fn scan_with_prefix<F: FnMut(&str)>(text: &str, prefix: &str, mut on_match: F) {
    let mut idx = 0;
    let bytes = text.as_bytes();
    while let Some(pos) = text[idx..].find(prefix) {
        let start = idx + pos;
        let mut end = start + prefix.len();
        while end < bytes.len() {
            let ch = bytes[end];
            if ch.is_ascii_alphanumeric()
                || matches!(ch, b'.' | b'/' | b'_' | b'-' | b'~' | b'+')
            {
                end += 1;
                continue;
            }
            break;
        }
        let candidate = &text[start..end];
        if candidate.ends_with(".svg") {
            on_match(candidate);
        }
        idx = end;
    }
}

fn run_parallel_image_jobs(
    jobs: Vec<BuildTask>,
    concurrency: usize,
    out_dir: &PathBuf,
    lookup: &ImageSourceLookup,
) -> Result<Vec<(BuildTask, Result<()>)>> {
    let out_dir = Arc::new(out_dir.clone());
    let lookup = Arc::new(lookup.clone());
    run_parallel_jobs(jobs, concurrency, move |task| match &task.kind {
        TaskKind::ResizeImage {
            source,
            width,
            quality,
            format,
            out_rel,
        } => resize_image(
            &out_dir,
            out_rel,
            source,
            *width,
            *quality,
            *format,
            &lookup,
        ),
        _ => Ok(()),
    })
}

fn run_parallel_video_jobs(
    jobs: Vec<BuildTask>,
    concurrency: usize,
    out_dir: &PathBuf,
    lookup: &VideoSourceLookup,
) -> Result<Vec<(BuildTask, Result<()>)>> {
    let out_dir = Arc::new(out_dir.clone());
    let lookup = Arc::new(lookup.clone());
    run_parallel_jobs(jobs, concurrency, move |task| match &task.kind {
        TaskKind::TranscodeVideoMp4 {
            source,
            height,
            out_rel,
        } => transcode_video_mp4(&out_dir, out_rel, source, *height, &lookup),
        TaskKind::ExtractVideoPoster {
            source,
            poster_time_sec,
            out_rel,
        } => extract_video_poster(&out_dir, out_rel, source, *poster_time_sec, &lookup),
        _ => Ok(()),
    })
}

fn run_parallel_jobs<F>(
    jobs: Vec<BuildTask>,
    concurrency: usize,
    worker: F,
) -> Result<Vec<(BuildTask, Result<()>)>>
where
    F: Fn(&BuildTask) -> Result<()> + Send + Sync + 'static,
{
    let job_count = jobs.len();
    if jobs.is_empty() {
        return Ok(Vec::new());
    }
    let concurrency = std::cmp::max(1, std::cmp::min(concurrency, job_count));
    let worker = Arc::new(worker);
    let (tx, rx) = mpsc::channel::<BuildTask>();
    let rx = Arc::new(Mutex::new(rx));
    let (result_tx, result_rx) = mpsc::channel::<(BuildTask, Result<()>)>();

    let mut handles = Vec::with_capacity(concurrency);
    for _ in 0..concurrency {
        let rx = Arc::clone(&rx);
        let result_tx = result_tx.clone();
        let worker = Arc::clone(&worker);
        handles.push(thread::spawn(move || loop {
            let task = {
                let rx = rx.lock().expect("lock receiver");
                rx.recv()
            };
            match task {
                Ok(task) => {
                    let result = (worker)(&task);
                    let _ = result_tx.send((task, result));
                }
                Err(_) => break,
            }
        }));
    }
    for task in jobs {
        tx.send(task)?;
    }
    drop(tx);
    drop(result_tx);

    let mut results = Vec::new();
    for _ in 0..job_count {
        if let Ok(value) = result_rx.recv() {
            results.push(value);
        }
    }
    for handle in handles {
        let _ = handle.join();
    }
    Ok(results)
}

fn max_parallelism() -> usize {
    thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(1)
}

fn output_count(task: &BuildTask) -> usize {
    if task.outputs.is_empty() {
        1
    } else {
        task.outputs.len()
    }
}

fn outputs_exist(outputs: &[String], out_dir: &Path) -> bool {
    outputs.iter().all(|output| {
        let path = out_dir.join(output);
        fs::metadata(&path).map(|meta| meta.is_file()).unwrap_or(false)
    })
}

fn should_skip_task(
    cache: &mut Option<&mut dyn CacheStore>,
    task: &BuildTask,
    out_dir: &Path,
    regenerate_content: bool,
) -> Result<bool> {
    if regenerate_content && !is_media_task(&task.kind) {
        return Ok(false);
    }
    let Some(cache) = cache.as_mut() else {
        return Ok(false);
    };
    let cache: &mut dyn CacheStore = &mut **cache;
    if task.outputs.is_empty() {
        return Ok(false);
    }
    match cache.get(task.id.0.as_str()) {
        Ok(Some(cached)) => {
            if cached.inputs_fingerprint == task.inputs_fingerprint.0
                && outputs_exist(&cached.outputs, out_dir)
            {
                return Ok(true);
            }
        }
        Ok(None) => {}
        Err(err) => {
            eprintln!("warning: cache get failed for {}: {}", task.id.0, err);
        }
    }
    Ok(false)
}

fn is_media_task(kind: &TaskKind) -> bool {
    matches!(
        kind,
        TaskKind::CopyImageOriginal { .. }
            | TaskKind::ResizeImage { .. }
            | TaskKind::CopyVideoOriginal { .. }
            | TaskKind::TranscodeVideoMp4 { .. }
            | TaskKind::ExtractVideoPoster { .. }
    )
}

fn cache_put_inner(cache: &mut dyn CacheStore, task: &BuildTask) {
    if task.outputs.is_empty() {
        return;
    }
    let outputs = task
        .outputs
        .iter()
        .map(|output| output.path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    if let Err(err) = cache.put(task.id.0.as_str(), task.inputs_fingerprint.0, &outputs) {
        eprintln!("warning: cache put failed for {}: {}", task.id.0, err);
    }
}

fn cache_put(cache: &mut Option<&mut dyn CacheStore>, task: &BuildTask) {
    let Some(cache) = cache.as_mut() else {
        return;
    };
    let cache: &mut dyn CacheStore = &mut **cache;
    cache_put_inner(cache, task);
}

fn render_vars_css(vars: &ResolvedThemeVars) -> String {
    format!(
        ":root {{\n  --layout-max-width: {};\n  --bp-desktop-min: {};\n  --bp-wide-min: {};\n  --header-title-size: {};\n  --header-tagline-size: {};\n  --c-bg: {};\n  --c-fg: {};\n  --c-heading: {};\n  --c-title-fg: {};\n  --c-muted: {};\n  --c-surface: {};\n  --c-border: {};\n  --c-link: {};\n  --c-link-hover: {};\n  --c-accent: {};\n  --c-nav-bg: {};\n  --c-nav-fg: {};\n  --c-nav-border: {};\n  --c-code-bg: {};\n  --c-code-fg: {};\n  --c-quote-bg: {};\n  --c-quote-border: {};\n  --c-wide-bg: {};\n  --wide-bg-image: {};\n  --wide-bg-repeat: {};\n  --wide-bg-size: {};\n  --wide-bg-position: {};\n  --wide-bg-opacity: {};\n}}\n",
        vars.max_body_width,
        vars.desktop_min,
        vars.wide_min,
        vars.header_title_size,
        vars.header_tagline_size,
        vars.c_bg,
        vars.c_fg,
        vars.c_heading,
        vars.c_title_fg,
        vars.c_muted,
        vars.c_surface,
        vars.c_border,
        vars.c_link,
        vars.c_link_hover,
        vars.c_accent,
        vars.c_nav_bg,
        vars.c_nav_fg,
        vars.c_nav_border,
        vars.c_code_bg,
        vars.c_code_fg,
        vars.c_quote_bg,
        vars.c_quote_border,
        vars.c_wide_bg,
        vars.wide_bg_image,
        vars.wide_bg_repeat,
        vars.wide_bg_size,
        vars.wide_bg_position,
        vars.wide_bg_opacity
    )
}

fn copy_image_original(
    out_dir: &PathBuf,
    out_rel: &str,
    source: &stbl_core::assets::AssetSourceId,
    lookup: &ImageSourceLookup,
) -> Result<()> {
    let src_path = lookup
        .resolve(source)
        .ok_or_else(|| anyhow!("unknown image source {}", source.0))?;
    let out_path = out_dir.join(out_rel);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(src_path, &out_path).with_context(|| {
        format!(
            "failed to copy {} to {}",
            src_path.display(),
            out_path.display()
        )
    })?;
    Ok(())
}

fn resize_image(
    out_dir: &PathBuf,
    out_rel: &str,
    source: &stbl_core::assets::AssetSourceId,
    width: u32,
    _quality: u8,
    format: stbl_core::model::ImageOutputFormat,
    lookup: &ImageSourceLookup,
) -> Result<()> {
    let src_path = lookup
        .resolve(source)
        .ok_or_else(|| anyhow!("unknown image source {}", source.0))?;
    if is_svg(src_path) {
        return copy_image_original(out_dir, out_rel, source, lookup);
    }
    let reader = image::ImageReader::open(src_path)
        .with_context(|| format!("failed to open {}", src_path.display()))?
        .with_guessed_format()
        .with_context(|| format!("failed to guess format for {}", src_path.display()))?;
    let image = reader
        .decode()
        .with_context(|| format!("failed to decode {}", src_path.display()))?;
    let (src_w, src_h) = image.dimensions();
    if src_w <= width {
        return write_resized(out_dir, out_rel, &image, format);
    }
    let height = ((src_h as f64) * (width as f64) / (src_w as f64)).round() as u32;
    let resized = image.resize_exact(width, height, FilterType::Lanczos3);
    write_resized(out_dir, out_rel, &resized, format)
}

fn write_resized(
    out_dir: &PathBuf,
    out_rel: &str,
    image: &DynamicImage,
    format: stbl_core::model::ImageOutputFormat,
) -> Result<()> {
    let out_path = out_dir.join(out_rel);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let mut file = fs::File::create(&out_path)
        .with_context(|| format!("failed to create {}", out_path.display()))?;
    let (width, height) = image.dimensions();
    match format {
        stbl_core::model::ImageOutputFormat::Jpeg => {
            let rgb = image.to_rgb8();
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut file, 84);
            encoder
                .write_image(&rgb, width, height, ExtendedColorType::Rgb8)
                .with_context(|| format!("failed to encode {}", out_path.display()))?;
        }
        stbl_core::model::ImageOutputFormat::Png => {
            let rgba = image.to_rgba8();
            let encoder = image::codecs::png::PngEncoder::new(&mut file);
            encoder
                .write_image(&rgba, width, height, ExtendedColorType::Rgba8)
                .with_context(|| format!("failed to encode {}", out_path.display()))?;
        }
        stbl_core::model::ImageOutputFormat::Webp => {
            let rgba = image.to_rgba8();
            let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut file);
            encoder
                .write_image(&rgba, width, height, ExtendedColorType::Rgba8)
                .with_context(|| format!("failed to encode {}", out_path.display()))?;
        }
        stbl_core::model::ImageOutputFormat::Avif => {
            let rgba = image.to_rgba8();
            let encoder = AvifEncoder::new_with_speed_quality(&mut file, 4, 50);
            encoder
                .write_image(&rgba, width, height, ExtendedColorType::Rgba8)
                .with_context(|| format!("failed to encode {}", out_path.display()))?;
        }
    }
    Ok(())
}

fn is_svg(path: &PathBuf) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("svg"))
        .unwrap_or(false)
}

fn copy_video_original(
    out_dir: &PathBuf,
    out_rel: &str,
    source: &stbl_core::assets::AssetSourceId,
    lookup: &VideoSourceLookup,
) -> Result<()> {
    let src_path = lookup
        .resolve(source)
        .ok_or_else(|| anyhow!("unknown video source {}", source.0))?;
    let out_path = out_dir.join(out_rel);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::copy(src_path, &out_path).with_context(|| {
        format!(
            "failed to copy {} to {}",
            src_path.display(),
            out_path.display()
        )
    })?;
    Ok(())
}

fn transcode_video_mp4(
    out_dir: &PathBuf,
    out_rel: &str,
    source: &stbl_core::assets::AssetSourceId,
    height: u32,
    lookup: &VideoSourceLookup,
) -> Result<()> {
    let src_path = lookup
        .resolve(source)
        .ok_or_else(|| anyhow!("unknown video source {}", source.0))?;
    let out_path = out_dir.join(out_rel);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let scale_arg = format!("scale=-2:min({height}\\,ih)");
    let status = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-threads")
        .arg("4")
        .arg("-y")
        .arg("-i")
        .arg(src_path)
        .arg("-vf")
        .arg(scale_arg)
        .arg("-c:v")
        .arg("libx264")
        .arg("-preset")
        .arg("veryfast")
        .arg("-crf")
        .arg("23")
        .arg("-c:a")
        .arg("aac")
        .arg("-b:a")
        .arg("128k")
        .arg("-movflags")
        .arg("+faststart")
        .arg(out_path.as_os_str())
        .status()
        .with_context(|| "failed to run ffmpeg")?;
    if !status.success() {
        bail!("ffmpeg failed to transcode {}", src_path.display());
    }
    Ok(())
}

fn extract_video_poster(
    out_dir: &PathBuf,
    out_rel: &str,
    source: &stbl_core::assets::AssetSourceId,
    poster_time_sec: u32,
    lookup: &VideoSourceLookup,
) -> Result<()> {
    let src_path = lookup
        .resolve(source)
        .ok_or_else(|| anyhow!("unknown video source {}", source.0))?;
    let out_path = out_dir.join(out_rel);
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let status = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-threads")
        .arg("4")
        .arg("-y")
        .arg("-ss")
        .arg(poster_time_sec.to_string())
        .arg("-i")
        .arg(src_path)
        .arg("-frames:v")
        .arg("1")
        .arg("-update")
        .arg("1")
        .arg("-q:v")
        .arg("2")
        .arg(out_path.as_os_str())
        .status()
        .with_context(|| "failed to run ffmpeg")?;
    if !status.success() {
        bail!("ffmpeg failed to extract poster for {}", src_path.display());
    }
    Ok(())
}

fn render_output(
    project: &Project,
    mapper: &UrlMapper,
    kind: &TaskKind,
    output_path: &PathBuf,
    asset_manifest: &AssetManifest,
    build_date_ymd: &str,
) -> Result<String> {
    match output_path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => render_html_output(
            project,
            mapper,
            kind,
            output_path,
            asset_manifest,
            build_date_ymd,
        ),
        Some("xml") => render_xml_output(project, mapper, kind),
        _ => Ok(String::new()),
    }
}

fn render_html_output(
    project: &Project,
    mapper: &UrlMapper,
    kind: &TaskKind,
    output_path: &PathBuf,
    asset_manifest: &AssetManifest,
    build_date_ymd: &str,
) -> Result<String> {
    if let Some(mapping) = mapping_for_task(project, mapper, kind)? {
        if output_path == &mapping.primary_output {
            return render_primary_html(project, mapper, kind, asset_manifest, build_date_ymd);
        }
        if mapping
            .fallback
            .as_ref()
            .is_some_and(|redirect| output_path == &redirect.from)
        {
            return render_redirect_stub(project, &mapping.href, asset_manifest, build_date_ymd);
        }
    }

    render_primary_html(project, mapper, kind, asset_manifest, build_date_ymd)
}

fn render_xml_output(project: &Project, mapper: &UrlMapper, kind: &TaskKind) -> Result<String> {
    match kind {
        TaskKind::GenerateRss => Ok(render_rss(project, mapper)),
        TaskKind::GenerateSitemap => Ok(render_sitemap(project, mapper)),
        _ => Ok(String::new()),
    }
}

fn render_primary_html(
    project: &Project,
    mapper: &UrlMapper,
    kind: &TaskKind,
    asset_manifest: &AssetManifest,
    build_date_ymd: &str,
) -> Result<String> {
    let include_provider = FsIncludeProvider::new(&project.root)?;
    let comment_template_provider = FsCommentTemplateProvider::new(&project.root)?;
    let current_href = current_href_for_task(project, mapper, kind)?;
    match kind {
        TaskKind::RenderPage { page } => render_page_by_id(
            project,
            *page,
            asset_manifest,
            &current_href,
            build_date_ymd,
            &include_provider,
            &comment_template_provider,
        ),
        TaskKind::RenderBlogIndex {
            source_page,
            page_no,
        } => render_blog_index_page(
            project,
            source_page,
            *page_no,
            asset_manifest,
            &current_href,
            build_date_ymd,
            &include_provider,
        ),
        TaskKind::RenderSeries { series } => render_series(
            project,
            *series,
            asset_manifest,
            &current_href,
            build_date_ymd,
            &include_provider,
        ),
        TaskKind::RenderTagIndex { tag } => {
            render_tag_index_page(
                project,
                tag,
                asset_manifest,
                &current_href,
                build_date_ymd,
                &include_provider,
            )
        }
        TaskKind::RenderTagsIndex => render_markdown_page(
            project,
            "Tags",
            "*Not implemented.*\n",
            asset_manifest,
            &current_href,
            build_date_ymd,
            None,
            true,
            Some(&include_provider),
            Some(&comment_template_provider),
        ),
        TaskKind::RenderFrontPage => {
            let title = project.config.site.title.clone();
            render_markdown_page(
                project,
                &title,
                "*Not implemented.*\n",
                asset_manifest,
                &current_href,
                build_date_ymd,
                None,
                false,
                Some(&include_provider),
                Some(&comment_template_provider),
            )
        }
        _ => render_markdown_page(
            project,
            "Not implemented",
            "*Not implemented.*\n",
            asset_manifest,
            &current_href,
            build_date_ymd,
            None,
            true,
            Some(&include_provider),
            Some(&comment_template_provider),
        ),
    }
}

fn render_page_by_id(
    project: &Project,
    page_id: DocId,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    include_provider: &FsIncludeProvider,
    comment_template_provider: &FsCommentTemplateProvider,
) -> Result<String> {
    let page =
        find_page(project, page_id).ok_or_else(|| anyhow!("page not found for render task"))?;
    let mapper = UrlMapper::new(&project.config);
    let series_nav = series_nav_for_page(project, page_id, &mapper);
    if series_nav.is_some() {
        render_page_with_series_nav(
            project,
            page,
            asset_manifest,
            series_nav,
            current_href,
            build_date_ymd,
            Some(include_provider),
            Some(comment_template_provider),
        )
    } else {
        render_page(
            project,
            page,
            asset_manifest,
            current_href,
            build_date_ymd,
            Some(include_provider),
            Some(comment_template_provider),
        )
    }
}

fn render_series(
    project: &Project,
    series_id: stbl_core::model::SeriesId,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    include_provider: &FsIncludeProvider,
) -> Result<String> {
    let series = find_series(project, series_id)
        .ok_or_else(|| anyhow!("series not found for render task"))?;
    let mapper = UrlMapper::new(&project.config);
    let rel = root_prefix_for_base_url(&project.config.site.base_url);
    let parts = series
        .parts
        .iter()
        .filter(|part| is_published_page(&part.page))
        .map(|part| SeriesIndexPart {
            title: part
                .page
                .header
                .title
                .clone()
                .unwrap_or_else(|| "Untitled".to_string()),
            href: resolve_root_href(
                &mapper
                .map(&logical_key_from_source_path(&part.page.source_path))
                .href,
                &rel,
            ),
            published_display: format_timestamp_display(
                normalize_timestamp(part.page.header.published, project.config.system.as_ref()),
                project.config.system.as_ref(),
                project.config.site.timezone.as_deref(),
            ),
            published_raw: {
                let ts = normalize_timestamp(part.page.header.published, project.config.system.as_ref());
                format_timestamp_rfc3339(ts)
            },
        })
        .collect::<Vec<_>>();
    render_series_index(
        project,
        &series.index,
        parts,
        asset_manifest,
        current_href,
        build_date_ymd,
        Some(include_provider),
    )
}

fn find_page(project: &Project, page_id: DocId) -> Option<&Page> {
    if let Some(page) = project.content.pages.iter().find(|page| page.id == page_id) {
        return Some(page);
    }
    for series in &project.content.series {
        if series.index.id == page_id {
            return Some(&series.index);
        }
        if let Some(page) = series
            .parts
            .iter()
            .find(|part| part.page.id == page_id)
            .map(|part| &part.page)
        {
            return Some(page);
        }
    }
    None
}

fn find_series(project: &Project, series_id: stbl_core::model::SeriesId) -> Option<&Series> {
    project
        .content
        .series
        .iter()
        .find(|series| series.id == series_id)
}

fn series_nav_for_page(
    project: &Project,
    page_id: DocId,
    mapper: &UrlMapper,
) -> Option<SeriesNavView> {
    for series in &project.content.series {
        for part in &series.parts {
            if part.page.id == page_id {
                let rel = root_prefix_for_base_url(&project.config.site.base_url);
                let index_title = series
                    .index
                    .header
                    .title
                    .clone()
                    .unwrap_or_else(|| "Series".to_string());
                let index_href = resolve_root_href(
                    &map_series_index(&logical_key_from_source_path(&series.dir_path)).href,
                    &rel,
                );
                let index = SeriesNavLink {
                    title: index_title,
                    href: index_href,
                };
                let parts = series
                    .parts
                    .iter()
                    .filter(|part| is_published_page(&part.page))
                    .map(|part| SeriesNavEntry {
                        title: format!(
                            "Part {} {}",
                            part.part_no,
                            part.page
                                .header
                                .title
                                .clone()
                                .unwrap_or_else(|| "Untitled".to_string())
                        ),
                        href: resolve_root_href(
                            &mapper
                                .map(&logical_key_from_source_path(&part.page.source_path))
                                .href,
                            &rel,
                        ),
                        is_current: part.page.id == page_id,
                    })
                    .collect();
                return Some(SeriesNavView { index, parts });
            }
        }
    }
    None
}

fn mapping_for_task(
    project: &Project,
    mapper: &UrlMapper,
    kind: &TaskKind,
) -> Result<Option<stbl_core::url::UrlMapping>> {
    let logical_key = match kind {
        TaskKind::RenderPage { page } => {
            let page = find_page(project, *page)
                .ok_or_else(|| anyhow!("page not found for render task"))?;
            logical_key_from_source_path(&page.source_path)
        }
        TaskKind::RenderBlogIndex {
            source_page,
            page_no,
        } => {
            let page = find_page(project, *source_page)
                .ok_or_else(|| anyhow!("blog index page not found"))?;
            let base_key = logical_key_from_source_path(&page.source_path);
            blog_index_page_logical_key(&base_key, *page_no)
        }
        TaskKind::RenderSeries { series } => {
            let series = find_series(project, *series)
                .ok_or_else(|| anyhow!("series not found for render task"))?;
            logical_key_from_source_path(&series.dir_path)
        }
        TaskKind::RenderTagIndex { tag } => format!("tags/{tag}"),
        TaskKind::RenderTagsIndex => "tags".to_string(),
        TaskKind::RenderFrontPage => "index".to_string(),
        _ => return Ok(None),
    };
    let mapping = match kind {
        TaskKind::RenderSeries { .. } => map_series_index(&logical_key),
        _ => mapper.map(&logical_key),
    };
    Ok(Some(mapping))
}

fn render_redirect_stub(
    project: &Project,
    href: &str,
    asset_manifest: &AssetManifest,
    build_date_ymd: &str,
) -> Result<String> {
    let target = format!("/{}", href.trim_start_matches('/'));
    render_redirect_page(project, &target, asset_manifest, &target, build_date_ymd)
}

fn render_blog_index_page(
    project: &Project,
    source_page_id: &DocId,
    page_no: u32,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    include_provider: &FsIncludeProvider,
) -> Result<String> {
    let mapper = UrlMapper::new(&project.config);
    let source_page =
        find_page(project, *source_page_id).ok_or_else(|| anyhow!("blog index page not found"))?;

    let feed_items = collect_blog_feed(project, source_page.id);
    let base_key = logical_key_from_source_path(&source_page.source_path);
    let pagination = blog_pagination_settings(project);
    let page_ranges = paginate_blog_index(pagination, &base_key, feed_items.len());
    let page_range = page_ranges
        .iter()
        .find(|page| page.page_no == page_no)
        .ok_or_else(|| anyhow!("blog index page out of range"))?;
    let (start, end) = (page_range.start, page_range.end);
    let items = feed_items[start..end]
        .iter()
        .map(|item| map_feed_item(item, &mapper, project))
        .collect::<Vec<_>>();

    let rel = root_prefix_for_base_url(&project.config.site.base_url);
    let intro_html = if page_no == 1 && !source_page.body_markdown.trim().is_empty() {
        let options = RenderOptions {
            macro_project: Some(project),
            macro_page: Some(source_page),
            macros_enabled: project.config.site.macros.enabled,
            include_provider: Some(include_provider),
            rel_prefix: &rel,
            video_heights: &project.config.media.video.heights,
            image_widths: &project.config.media.images.widths,
            max_body_width: &project.config.theme.max_body_width,
            desktop_min: &project.config.theme.breakpoints.desktop_min,
            wide_min: &project.config.theme.breakpoints.wide_min,
            image_format_mode: project.config.media.images.format_mode,
            image_alpha: Some(&project.image_alpha),
            image_variants: Some(&project.image_variants),
            video_variants: Some(&project.video_variants),
            syntax_highlight: project.config.syntax.highlight,
            syntax_theme: &project.config.syntax.theme,
            syntax_line_numbers: project.config.syntax.line_numbers,
        };
        Some(render_markdown_to_html_with_media(
            &source_page.body_markdown,
            &options,
        ))
    } else {
        None
    };

    let title = page_title_or_filename(project, source_page);
    let banner_html = render_banner_html(project, source_page, &rel);
    let prev_href = page_range.prev_key.as_ref().map(|key| mapper.map(key).href);
    let next_href = page_range.next_key.as_ref().map(|key| mapper.map(key).href);
    let first_href = if page_range.total_pages > 2 && page_range.page_no > 1 {
        Some(mapper.map(&base_key).href)
    } else {
        None
    };
    let last_href = if page_range.total_pages > 2 && page_range.page_no < page_range.total_pages {
        Some(mapper.map(&blog_index_page_logical_key(&base_key, page_range.total_pages)).href)
    } else {
        None
    };

    render_blog_index(
        project,
        title,
        intro_html,
        banner_html,
        items,
        prev_href,
        next_href,
        first_href,
        last_href,
        page_range.page_no,
        page_range.total_pages,
        asset_manifest,
        current_href,
        build_date_ymd,
        false,
    )
}

fn render_tag_index_page(
    project: &Project,
    tag: &str,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
    _include_provider: &FsIncludeProvider,
) -> Result<String> {
    let mapper = UrlMapper::new(&project.config);
    let feed_items = collect_tag_feed(project, tag);
    let items = feed_items
        .iter()
        .map(|item| map_feed_item(item, &mapper, project))
        .collect::<Vec<_>>();
    let listing = TagListingPage {
        tag: tag.to_string(),
        items,
    };
    render_tag_index(
        project,
        listing,
        asset_manifest,
        current_href,
        build_date_ymd,
    )
}

fn current_href_for_task(project: &Project, mapper: &UrlMapper, kind: &TaskKind) -> Result<String> {
    let mapping = mapping_for_task(project, mapper, kind)?;
    Ok(mapping.map(|value| value.href).unwrap_or_default())
}

fn build_date_ymd_now() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    format_timestamp_long_date(Some(timestamp))
        .unwrap_or_else(|| "January 1, 1970".to_string())
}

fn root_prefix_for_base_url(base_url: &str) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }
    let without_scheme = trimmed
        .split("://")
        .nth(1)
        .unwrap_or(trimmed);
    let path = match without_scheme.find('/') {
        Some(idx) => &without_scheme[idx..],
        None => "",
    };
    let path = path.trim();
    if path.is_empty() || path == "/" {
        return "/".to_string();
    }
    let mut normalized = path.to_string();
    if !normalized.starts_with('/') {
        normalized.insert(0, '/');
    }
    if !normalized.ends_with('/') {
        normalized.push('/');
    }
    normalized
}

fn resolve_root_href(href: &str, rel: &str) -> String {
    if rel.is_empty() || is_external_href(href) || is_absolute_or_fragment_href(href) {
        return href.to_string();
    }
    format!("{rel}{}", href.trim_start_matches('/'))
}

fn is_external_href(href: &str) -> bool {
    let href = href.trim();
    href.starts_with("http://")
        || href.starts_with("https://")
        || href.starts_with("mailto:")
        || href.starts_with("tel:")
}

fn is_absolute_or_fragment_href(href: &str) -> bool {
    let href = href.trim();
    href.starts_with('/') || href.starts_with('#') || href.starts_with('?')
}

fn map_feed_item(item: &FeedItem, mapper: &UrlMapper, project: &Project) -> BlogIndexItem {
    let rel = root_prefix_for_base_url(&project.config.site.base_url);
    match item {
        FeedItem::Post(post) => BlogIndexItem {
            title: post.title.clone(),
            href: resolve_root_href(&mapper.map(&post.logical_key).href, &rel),
            published_display: {
                let ts = normalize_timestamp(post.published, project.config.system.as_ref());
                format_timestamp_display(
                    ts,
                    project.config.system.as_ref(),
                    project.config.site.timezone.as_deref(),
                )
            },
            updated_display: {
                let published_ts =
                    normalize_timestamp(post.published, project.config.system.as_ref());
                let updated_ts = normalize_timestamp(post.updated, project.config.system.as_ref());
                if updated_ts.is_some() && updated_ts == published_ts {
                    None
                } else {
                    format_timestamp_display(
                        updated_ts,
                        project.config.system.as_ref(),
                        project.config.site.timezone.as_deref(),
                    )
                }
            },
            published_raw: {
                let ts = normalize_timestamp(post.published, project.config.system.as_ref());
                format_timestamp_rfc3339(ts)
            },
            updated_raw: {
                let published_ts =
                    normalize_timestamp(post.published, project.config.system.as_ref());
                let updated_ts = normalize_timestamp(post.updated, project.config.system.as_ref());
                if updated_ts.is_some() && updated_ts == published_ts {
                    None
                } else {
                    format_timestamp_rfc3339(updated_ts)
                }
            },
            kind_label: None,
            abstract_text: post.abstract_text.clone(),
            tags: post
                .tags
                .iter()
                .map(|tag| TagLink {
                    label: tag.clone(),
                    href: resolve_root_href(&mapper.map(&format!("tags/{}", tag)).href, &rel),
                })
                .collect(),
            latest_parts: Vec::new(),
        },
        FeedItem::Series(series) => BlogIndexItem {
            title: series.title.clone(),
            href: resolve_root_href(&map_series_index(&series.logical_key).href, &rel),
            published_display: {
                let ts = normalize_timestamp(series.published, project.config.system.as_ref());
                format_timestamp_display(
                    ts,
                    project.config.system.as_ref(),
                    project.config.site.timezone.as_deref(),
                )
            },
            updated_display: {
                let published_ts =
                    normalize_timestamp(series.published, project.config.system.as_ref());
                let updated_ts =
                    normalize_timestamp(series.updated, project.config.system.as_ref());
                if updated_ts.is_some() && updated_ts == published_ts {
                    None
                } else {
                    format_timestamp_display(
                        updated_ts,
                        project.config.system.as_ref(),
                        project.config.site.timezone.as_deref(),
                    )
                }
            },
            published_raw: {
                let ts = normalize_timestamp(series.published, project.config.system.as_ref());
                format_timestamp_rfc3339(ts)
            },
            updated_raw: {
                let published_ts =
                    normalize_timestamp(series.published, project.config.system.as_ref());
                let updated_ts =
                    normalize_timestamp(series.updated, project.config.system.as_ref());
                if updated_ts.is_some() && updated_ts == published_ts {
                    None
                } else {
                    format_timestamp_rfc3339(updated_ts)
                }
            },
            kind_label: Some("Series".to_string()),
            abstract_text: series.abstract_text.clone(),
            tags: series
                .tags
                .iter()
                .map(|tag| TagLink {
                    label: tag.clone(),
                    href: resolve_root_href(&mapper.map(&format!("tags/{}", tag)).href, &rel),
                })
                .collect(),
            latest_parts: series
                .latest_parts
                .iter()
                .map(|part| BlogIndexPart {
                    part_no: part.part_no,
                    title: part.title.clone(),
                    href: resolve_root_href(&mapper.map(&part.logical_key).href, &rel),
                    published_display: format_timestamp_display(
                        normalize_timestamp(part.published, project.config.system.as_ref()),
                        project.config.system.as_ref(),
                        project.config.site.timezone.as_deref(),
                    ),
                    published_raw: {
                        let ts =
                            normalize_timestamp(part.published, project.config.system.as_ref());
                        format_timestamp_rfc3339(ts)
                    },
                    abstract_text: part.abstract_text.clone(),
                })
                .collect(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stbl_core::assemble::assemble_site;
    use stbl_core::config::load_site_config;
    use stbl_core::header::UnknownKeyPolicy;
    use stbl_core::model::{Project, UrlStyle};
    use tempfile::TempDir;

    fn fixture_root(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("stbl_core")
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    fn build_project_at(root: PathBuf, url_style: UrlStyle) -> Project {
        let config_path = root.join("stbl.yaml");
        let mut config = load_site_config(&config_path).expect("load config");
        config.site.url_style = url_style;
        let docs =
            crate::walk::walk_content(&root, &root.join("articles"), UnknownKeyPolicy::Error, false)
                .expect("walk content");
        let content = assemble_site(docs).expect("assemble site");
        Project {
            root,
            config,
            content,
            image_alpha: std::collections::BTreeMap::new(),
            image_variants: Default::default(),
            video_variants: Default::default(),
        }
    }

    fn build_project(url_style: UrlStyle) -> Project {
        build_project_at(fixture_root("site1"), url_style)
    }

    fn build_brand_project(url_style: UrlStyle) -> Project {
        build_project_at(fixture_root("site-brand"), url_style)
    }

    fn build_header_layout_project(url_style: UrlStyle) -> Project {
        build_project_at(fixture_root("site-header-layout"), url_style)
    }

    fn build_into_temp(url_style: UrlStyle) -> (TempDir, PathBuf) {
        let project = build_project(url_style);
        build_project_into_temp(project)
    }

    fn build_project_into_temp(project: Project) -> (TempDir, PathBuf) {
        let mut project = project;
        let site_assets_root = project.root.join("assets");
        let (mut asset_index, mut asset_lookup) =
            crate::assets::discover_assets(&site_assets_root).expect("discover assets");
        crate::assets::include_site_logo(&project.root, &project.config, &mut asset_index, &mut asset_lookup)
            .expect("resolve site.logo");
        let (image_plan, image_lookup) =
            crate::media::discover_images(&project).expect("discover images");
        project.image_alpha = image_plan.alpha.clone();
        project.image_variants = stbl_core::media::build_image_variant_index(
            &image_plan,
            &project.config.media.images.widths,
            project.config.media.images.format_mode,
        );
        let (video_plan, video_lookup) =
            crate::media::discover_videos(&project).expect("discover videos");
        project.video_variants = stbl_core::media::build_video_variant_index(
            &video_plan,
            &project.config.media.video.heights,
        );
        let asset_manifest = stbl_core::assets::build_asset_manifest(
            &asset_index,
            project.config.assets.cache_busting,
        );
        let plan = stbl_core::plan::build_plan(&project, &asset_index, &image_plan, &video_plan);
        let temp = TempDir::new().expect("tempdir");
        let out_dir = temp.path().join("out");
        execute_plan(
            &project,
            &plan,
            &out_dir,
            &asset_index,
            &asset_lookup,
            &image_lookup,
            &video_lookup,
            &asset_manifest,
            None,
            None,
            false,
        )
        .expect("execute plan");
        (temp, out_dir)
    }

    #[test]
    fn html_style_writes_flat_html() {
        let (_temp, out_dir) = build_into_temp(UrlStyle::Html);
        assert!(out_dir.join("page1.html").exists());
        assert!(!out_dir.join("page1").join("index.html").exists());
    }

    #[test]
    fn blog_index_lists_pages() {
        let (_temp, out_dir) = build_into_temp(UrlStyle::Html);
        let index_html = fs::read_to_string(out_dir.join("index.html")).expect("read index");
        assert!(index_html.contains("&#x2f;page1.html"));
        assert!(index_html.contains("&#x2f;page2.html"));
        assert!(!index_html.contains("&#x2f;info.html"));
    }

    #[test]
    fn pretty_style_writes_index_html() {
        let (_temp, out_dir) = build_into_temp(UrlStyle::Pretty);
        assert!(out_dir.join("page1").join("index.html").exists());
        assert!(!out_dir.join("page1.html").exists());
    }

    #[test]
    fn pretty_with_fallback_writes_redirect_stub() {
        let (_temp, out_dir) = build_into_temp(UrlStyle::PrettyWithFallback);
        let index_path = out_dir.join("page1").join("index.html");
        let fallback_path = out_dir.join("page1.html");
        assert!(index_path.exists());
        assert!(fallback_path.exists());
        let contents = fs::read_to_string(fallback_path).expect("read fallback");
        assert!(contents.contains("http-equiv=\"refresh\""));
        assert!(contents.contains("href=\"/page1/\""));
    }

    #[test]
    fn vars_css_is_generated_with_defaults() {
        let (_temp, out_dir) = build_into_temp(UrlStyle::Html);
        let vars_path = out_dir.join("artifacts/css/vars.css");
        let contents = fs::read_to_string(vars_path).expect("read vars css");
        let required = [
            "--layout-max-width:",
            "--bp-desktop-min:",
            "--bp-wide-min:",
            "--header-title-size:",
            "--header-tagline-size:",
            "--c-bg:",
            "--c-fg:",
            "--c-heading:",
            "--c-muted:",
            "--c-surface:",
            "--c-border:",
            "--c-link:",
            "--c-link-hover:",
            "--c-accent:",
            "--c-nav-bg:",
            "--c-nav-fg:",
            "--c-nav-border:",
            "--c-code-bg:",
            "--c-code-fg:",
            "--c-quote-bg:",
            "--c-quote-border:",
            "--c-wide-bg:",
            "--wide-bg-image:",
            "--wide-bg-repeat:",
            "--wide-bg-size:",
            "--wide-bg-position:",
            "--wide-bg-opacity:",
        ];
        for key in required {
            assert!(contents.contains(key), "missing {}", key);
        }
        assert!(contents.contains("--c-bg: #ffffff;"));
        assert!(contents.contains("--c-fg: #000000;"));
        assert!(contents.contains("--c-heading: #0b1f3a;"));
        assert!(contents.contains("--c-nav-bg: #000000;"));
        assert!(contents.contains("--c-nav-fg: #ffffff;"));
    }

    #[test]
    fn vars_css_overrides_heading_and_nav() {
        let mut project = build_project(UrlStyle::Html);
        project.config.theme.colors.heading = Some("#112233".to_string());
        project.config.theme.nav.bg = Some("#abcdef".to_string());
        let (_temp, out_dir) = build_project_into_temp(project);
        let vars_path = out_dir.join("artifacts/css/vars.css");
        let contents = fs::read_to_string(vars_path).expect("read vars css");
        assert!(contents.contains("--c-heading: #112233;"));
        assert!(contents.contains("--c-nav-bg: #abcdef;"));
    }

    #[test]
    fn vars_css_wide_background_tile_sets_repeat_auto() {
        let mut project = build_project(UrlStyle::Html);
        project.config.theme.wide_background.style = Some(
            stbl_core::model::WideBackgroundStyle::Tile,
        );
        let (_temp, out_dir) = build_project_into_temp(project);
        let vars_path = out_dir.join("artifacts/css/vars.css");
        let contents = fs::read_to_string(vars_path).expect("read vars css");
        assert!(contents.contains("--wide-bg-repeat: repeat;"));
        assert!(contents.contains("--wide-bg-size: auto;"));
    }

    #[test]
    fn pagination_fixture_generates_multiple_blog_pages() {
        let mut project = build_project_at(fixture_root("site-pagination"), UrlStyle::Html);
        let site_assets_root = project.root.join("assets");
        let (asset_index, asset_lookup) =
            crate::assets::discover_assets(&site_assets_root).expect("discover assets");
        let (image_plan, image_lookup) =
            crate::media::discover_images(&project).expect("discover images");
        let (video_plan, video_lookup) =
            crate::media::discover_videos(&project).expect("discover videos");
        project.video_variants = stbl_core::media::build_video_variant_index(
            &video_plan,
            &project.config.media.video.heights,
        );
        let asset_manifest = stbl_core::assets::build_asset_manifest(
            &asset_index,
            project.config.assets.cache_busting,
        );
        let plan = stbl_core::plan::build_plan(&project, &asset_index, &image_plan, &video_plan);
        let temp = TempDir::new().expect("tempdir");
        let out_dir = temp.path().join("out");
        execute_plan(
            &project,
            &plan,
            &out_dir,
            &asset_index,
            &asset_lookup,
            &image_lookup,
            &video_lookup,
            &asset_manifest,
            None,
            None,
            false,
        )
        .expect("execute plan");

        assert!(out_dir.join("index.html").exists());
        assert!(out_dir.join("page/2.html").exists());
        assert!(out_dir.join("page/3.html").exists());
        assert!(out_dir.join("page/4.html").exists());

        let index_html = fs::read_to_string(out_dir.join("index.html")).expect("read index");
        assert!(index_html.contains("&#x2f;series&#x2f;"));
        assert!(index_html.contains("&#x2f;series&#x2f;part5.html"));
        assert!(index_html.contains("Part 5"));
        assert!(index_html.contains("Part 4"));
        assert!(index_html.contains("Part 3"));
        assert!(!index_html.contains("Part 2"));
        assert!(index_html.contains("Series abstract override."));
        assert!(index_html.contains("January 15, 2024"));
        assert!(index_html.contains("datetime=\"2024-01-15T10:00:00+00:00\""));
        assert!(!index_html.contains("<span class=\"meta\"></span>"));

        let page2_html = fs::read_to_string(out_dir.join("page/2.html")).expect("read page2");
        assert!(page2_html.contains("&#x2f;page&#x2f;3.html"));
        assert!(page2_html.contains("&#x2f;index.html"));
        assert!(!page2_html.contains("&#x2f;series&#x2f;"));
        assert!(!page2_html.contains("<span class=\"meta\"></span>"));

        let page4_html = fs::read_to_string(out_dir.join("page/4.html")).expect("read page4");
        assert!(page4_html.contains("Custom abstract for page 1"));
        assert!(page4_html.contains("First paragraph for auto-abstract."));
        assert!(!page4_html.contains("Series"));

        let rust_tag_html = fs::read_to_string(out_dir.join("tags/rust.html")).expect("rust tag");
        assert!(rust_tag_html.contains("Custom abstract for page 1"));
        assert!(rust_tag_html.contains("First paragraph for auto-abstract."));
        assert!(rust_tag_html.contains("Series abstract override."));
        assert!(rust_tag_html.contains("January 4, 2024"));
        assert!(!rust_tag_html.contains("<span class=\"meta\"></span>"));
        assert!(rust_tag_html.contains("Pagination Series"));

        let series_tag_html =
            fs::read_to_string(out_dir.join("tags/series-only.html")).expect("series tag");
        assert!(series_tag_html.contains("Pagination Series"));
        assert!(!series_tag_html.contains("Page 1"));
    }

    #[test]
    fn base_layout_contract_is_applied() {
        let (_temp, out_dir) = build_into_temp(UrlStyle::Html);
        let index_html = fs::read_to_string(out_dir.join("index.html")).expect("read index");
        let page_html = fs::read_to_string(out_dir.join("page1.html")).expect("read page1");

        assert!(index_html.contains("<header"));
        assert!(index_html.contains("<main>"));
        assert!(index_html.contains("<footer>"));
        assert!(page_html.contains("<header"));
        assert!(page_html.contains("<main>"));
        assert!(page_html.contains("<footer>"));
        assert!(page_html.contains("class=\"brand\" href=\"&#x2f;index.html\""));

        assert_eq!(count_h1(&index_html), 0);
        assert_eq!(count_h1(&page_html), 1);

        assert!(index_html.contains("<title>Home · Site One</title>"));
        assert!(page_html.contains("<title>Page One · Site One</title>"));

        assert_footer_stamp(&index_html);
        assert_footer_stamp(&page_html);
    }

    #[test]
    fn brand_block_and_menu_alignment_render() {
        let project = build_brand_project(UrlStyle::Html);
        let (_temp, out_dir) = build_project_into_temp(project);
        let index_html = fs::read_to_string(out_dir.join("index.html")).expect("read index");

        assert!(index_html.contains("class=\"brand\""));
        assert!(index_html.contains("brand-title"));
        assert!(index_html.contains("brand-tagline"));
        assert!(index_html.contains("menu-align-center"));
        assert!(index_html.contains("<img class=\"brand-logo\""));
    }

    #[test]
    fn header_layout_classes_render() {
        let project = build_header_layout_project(UrlStyle::Html);
        let (_temp, out_dir) = build_project_into_temp(project);
        let index_html = fs::read_to_string(out_dir.join("index.html")).expect("read index");

        assert!(index_html.contains("header-stacked"));
        assert!(index_html.contains("menu-align-right"));
    }

    fn count_h1(contents: &str) -> usize {
        contents.match_indices("<h1").count()
    }

    fn assert_footer_stamp(contents: &str) {
        let marker = "Generated by <a href=\"https://github.com/jgaa/stbl\">stbl</a> on ";
        let pos = contents.find(marker).expect("footer stamp");
        let rest = &contents[pos + marker.len()..];
        let end = rest.find('<').unwrap_or(rest.len());
        let date = rest[..end].trim();
        assert!(is_long_date(date), "footer date format");
    }

    fn is_long_date(value: &str) -> bool {
        let months = [
            "January", "February", "March", "April", "May", "June", "July", "August", "September",
            "October", "November", "December",
        ];
        let (month, rest) = match value.split_once(' ') {
            Some(parts) => parts,
            None => return false,
        };
        if !months.contains(&month) {
            return false;
        }
        let (day_part, year_part) = match rest.split_once(", ") {
            Some(parts) => parts,
            None => return false,
        };
        if day_part.is_empty()
            || day_part.len() > 2
            || !day_part.chars().all(|ch| ch.is_ascii_digit())
        {
            return false;
        }
        year_part.len() == 4 && year_part.chars().all(|ch| ch.is_ascii_digit())
    }
}
