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
use stbl_core::feeds::{render_rss, render_sitemap};
use stbl_core::model::{BuildPlan, BuildTask, DocId, Page, Project, Series, TaskKind};
use stbl_core::render::{RenderOptions, render_markdown_to_html_with_media};
use stbl_core::theme::{ResolvedThemeVars, resolve_theme_vars};
use stbl_core::templates::{
    BlogIndexItem, BlogIndexPart, SeriesIndexPart, SeriesNavLink, SeriesNavView, TagLink,
    TagListingPage, format_timestamp_long_date, format_timestamp_ymd, page_title_or_filename,
    render_banner_html, render_blog_index, render_markdown_page, render_page,
    render_page_with_series_nav, render_redirect_page, render_series_index, render_tag_index,
};
use stbl_core::url::{UrlMapper, logical_key_from_source_path};
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

pub fn execute_plan(
    project: &Project,
    plan: &BuildPlan,
    out_dir: &PathBuf,
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
        if let TaskKind::CopyAsset {
            source, out_rel, ..
        } = &task.kind
        {
            copy_asset_to_out(out_dir, out_rel, source, asset_lookup)?;
            report.executed += output_count(task);
            report.executed_ids.push(task.id.0.clone());
            cache_put(&mut cache, task);
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
    Ok(report)
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
        ":root {{\n  --layout-max-width: {};\n  --bp-desktop-min: {};\n  --bp-wide-min: {};\n  --header-title-size: {};\n  --header-tagline-size: {};\n  --c-bg: {};\n  --c-fg: {};\n  --c-heading: {};\n  --c-muted: {};\n  --c-surface: {};\n  --c-border: {};\n  --c-link: {};\n  --c-link-hover: {};\n  --c-accent: {};\n  --c-nav-bg: {};\n  --c-nav-fg: {};\n  --c-nav-border: {};\n  --c-code-bg: {};\n  --c-code-fg: {};\n  --c-quote-bg: {};\n  --c-quote-border: {};\n  --c-wide-bg: {};\n  --wide-bg-image: {};\n  --wide-bg-repeat: {};\n  --wide-bg-size: {};\n  --wide-bg-position: {};\n  --wide-bg-opacity: {};\n}}\n",
        vars.max_body_width,
        vars.desktop_min,
        vars.wide_min,
        vars.header_title_size,
        vars.header_tagline_size,
        vars.c_bg,
        vars.c_fg,
        vars.c_heading,
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
    let current_href = current_href_for_task(project, mapper, kind)?;
    match kind {
        TaskKind::RenderPage { page } => render_page_by_id(
            project,
            *page,
            asset_manifest,
            &current_href,
            build_date_ymd,
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
        ),
        TaskKind::RenderSeries { series } => render_series(
            project,
            *series,
            asset_manifest,
            &current_href,
            build_date_ymd,
        ),
        TaskKind::RenderTagIndex { tag } => {
            render_tag_index_page(project, tag, asset_manifest, &current_href, build_date_ymd)
        }
        TaskKind::RenderTagsIndex => render_markdown_page(
            project,
            "Tags",
            "*Not implemented.*\n",
            asset_manifest,
            &current_href,
            build_date_ymd,
            None,
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
        ),
    }
}

fn render_page_by_id(
    project: &Project,
    page_id: DocId,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
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
        )
    } else {
        render_page(project, page, asset_manifest, current_href, build_date_ymd)
    }
}

fn render_series(
    project: &Project,
    series_id: stbl_core::model::SeriesId,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
) -> Result<String> {
    let series = find_series(project, series_id)
        .ok_or_else(|| anyhow!("series not found for render task"))?;
    let mapper = UrlMapper::new(&project.config);
    let parts = series
        .parts
        .iter()
        .map(|part| SeriesIndexPart {
            title: part
                .page
                .header
                .title
                .clone()
                .unwrap_or_else(|| "Untitled".to_string()),
            href: mapper
                .map(&logical_key_from_source_path(&part.page.source_path))
                .href,
            published_display: format_timestamp_ymd(part.page.header.published),
        })
        .collect::<Vec<_>>();
    render_series_index(
        project,
        &series.index,
        parts,
        asset_manifest,
        current_href,
        build_date_ymd,
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
        for (idx, part) in series.parts.iter().enumerate() {
            if part.page.id == page_id {
                let prev = if idx > 0 {
                    Some(nav_link_for_page(&series.parts[idx - 1].page, mapper))
                } else {
                    None
                };
                let next = if idx + 1 < series.parts.len() {
                    Some(nav_link_for_page(&series.parts[idx + 1].page, mapper))
                } else {
                    None
                };
                let index_title = series
                    .index
                    .header
                    .title
                    .clone()
                    .unwrap_or_else(|| "Series".to_string());
                let index_href = mapper
                    .map(&logical_key_from_source_path(&series.dir_path))
                    .href;
                let index = SeriesNavLink {
                    title: index_title,
                    href: index_href,
                };
                return Some(SeriesNavView { prev, index, next });
            }
        }
    }
    None
}

fn nav_link_for_page(page: &Page, mapper: &UrlMapper) -> SeriesNavLink {
    SeriesNavLink {
        title: page
            .header
            .title
            .clone()
            .unwrap_or_else(|| "Untitled".to_string()),
        href: mapper
            .map(&logical_key_from_source_path(&page.source_path))
            .href,
    }
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
    Ok(Some(mapper.map(&logical_key)))
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
        .map(|item| map_feed_item(item, &mapper))
        .collect::<Vec<_>>();

    let rel = rel_prefix_for_href(current_href);
    let intro_html = if page_no == 1 && !source_page.body_markdown.trim().is_empty() {
        let options = RenderOptions {
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

    render_blog_index(
        project,
        title,
        intro_html,
        banner_html,
        items,
        prev_href,
        next_href,
        page_range.page_no,
        page_range.total_pages,
        asset_manifest,
        current_href,
        build_date_ymd,
    )
}

fn render_tag_index_page(
    project: &Project,
    tag: &str,
    asset_manifest: &AssetManifest,
    current_href: &str,
    build_date_ymd: &str,
) -> Result<String> {
    let mapper = UrlMapper::new(&project.config);
    let feed_items = collect_tag_feed(project, tag);
    let items = feed_items
        .iter()
        .map(|item| map_feed_item(item, &mapper))
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

fn rel_prefix_for_href(href: &str) -> String {
    let trimmed = href.trim_start_matches('/');
    if trimmed.is_empty() {
        return String::new();
    }
    let depth = if trimmed.ends_with('/') {
        let stripped = trimmed.trim_end_matches('/');
        if stripped.is_empty() {
            0
        } else {
            stripped.split('/').count()
        }
    } else if let Some((parent, _)) = trimmed.rsplit_once('/') {
        if parent.is_empty() {
            0
        } else {
            parent.split('/').count()
        }
    } else {
        0
    };
    "../".repeat(depth)
}

fn map_feed_item(item: &FeedItem, mapper: &UrlMapper) -> BlogIndexItem {
    match item {
        FeedItem::Post(post) => BlogIndexItem {
            title: post.title.clone(),
            href: mapper.map(&post.logical_key).href,
            published_display: format_timestamp_ymd(post.published),
            updated_display: format_timestamp_ymd(post.updated),
            kind_label: None,
            abstract_text: post.abstract_text.clone(),
            tags: post
                .tags
                .iter()
                .map(|tag| TagLink {
                    label: tag.clone(),
                    href: mapper.map(&format!("tags/{}", tag)).href,
                })
                .collect(),
            latest_parts: Vec::new(),
        },
        FeedItem::Series(series) => BlogIndexItem {
            title: series.title.clone(),
            href: mapper.map(&series.logical_key).href,
            published_display: format_timestamp_ymd(series.published),
            updated_display: format_timestamp_ymd(series.updated),
            kind_label: Some("Series".to_string()),
            abstract_text: series.abstract_text.clone(),
            tags: series
                .tags
                .iter()
                .map(|tag| TagLink {
                    label: tag.clone(),
                    href: mapper.map(&format!("tags/{}", tag)).href,
                })
                .collect(),
            latest_parts: series
                .latest_parts
                .iter()
                .map(|part| BlogIndexPart {
                    title: part.title.clone(),
                    href: mapper.map(&part.logical_key).href,
                    published_display: format_timestamp_ymd(part.published),
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
        assert!(index_html.contains("page1.html"));
        assert!(index_html.contains("page2.html"));
        assert!(!index_html.contains("info.html"));
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
        assert!(index_html.contains("series.html"));
        assert!(index_html.contains("series&#x2f;part5.html"));
        assert!(index_html.contains("Part 5"));
        assert!(index_html.contains("Part 4"));
        assert!(index_html.contains("Part 3"));
        assert!(!index_html.contains("Part 2"));
        assert!(index_html.contains("Series abstract override."));
        assert!(index_html.contains("2024-01-15"));
        assert!(!index_html.contains("T10:00:00"));
        assert!(!index_html.contains("<span class=\"meta\"></span>"));

        let page2_html = fs::read_to_string(out_dir.join("page/2.html")).expect("read page2");
        assert!(page2_html.contains("page&#x2f;3.html"));
        assert!(page2_html.contains("index.html"));
        assert!(!page2_html.contains("series.html"));
        assert!(!page2_html.contains("<span class=\"meta\"></span>"));

        let page4_html = fs::read_to_string(out_dir.join("page/4.html")).expect("read page4");
        assert!(page4_html.contains("Custom abstract for page 1"));
        assert!(page4_html.contains("First paragraph for auto-abstract."));
        assert!(!page4_html.contains("Series"));

        let rust_tag_html = fs::read_to_string(out_dir.join("tags/rust.html")).expect("rust tag");
        assert!(rust_tag_html.contains("Custom abstract for page 1"));
        assert!(rust_tag_html.contains("First paragraph for auto-abstract."));
        assert!(rust_tag_html.contains("Series abstract override."));
        assert!(rust_tag_html.contains("2024-01-04"));
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

        assert_eq!(count_h1(&index_html), 1);
        assert_eq!(count_h1(&page_html), 1);

        assert!(index_html.contains("<title>Home</title>"));
        assert!(page_html.contains("<title>Page One</title>"));

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
        let marker = "Generated by stbl on ";
        let pos = contents.find(marker).expect("footer stamp");
        let date = &contents[pos + marker.len()..];
        let date = date.get(0..10).expect("date");
        assert!(is_ymd(date), "footer date format");
    }

    fn is_ymd(value: &str) -> bool {
        if value.len() != 10 {
            return false;
        }
        let bytes = value.as_bytes();
        bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes.iter().enumerate().all(|(idx, byte)| match idx {
                4 | 7 => *byte == b'-',
                _ => byte.is_ascii_digit(),
            })
    }
}
