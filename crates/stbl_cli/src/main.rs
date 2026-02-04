mod assets;
mod config_loader;
mod exec;
mod init;
mod media;
mod preview;
mod upgrade;
mod walk;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use stbl_cache::{CacheStore, SqliteCacheStore};
use stbl_core::assemble::assemble_site;
use stbl_core::config::load_site_config;
use stbl_core::header::UnknownKeyPolicy;
use stbl_core::model::DiagnosticLevel;
use std::process::Command as ProcessCommand;

#[derive(Debug, Parser)]
#[command(name = "stbl_cli")]
struct Cli {
    #[arg(long = "source-dir", short = 's', global = true)]
    source_dir: Option<PathBuf>,
    #[arg(long)]
    include_unpublished: bool,
    #[arg(long, value_enum, default_value = "error")]
    unknown_header_keys: UnknownHeaderKeys,
    #[arg(long)]
    no_writeback: bool,
    #[arg(long)]
    commit_writeback: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum WriteBackMode {
    DryRun,
    Apply,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum UnknownHeaderKeys {
    Error,
    Warn,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InitKindArg {
    Blog,
    #[value(name = "landing-page")]
    LandingPage,
}

#[derive(Debug, Subcommand)]
enum Command {
    Scan {
        #[arg(default_value = "articles")]
        articles_dir: PathBuf,
    },
    Plan {
        #[arg(default_value = "articles")]
        articles_dir: PathBuf,
        #[arg(long, value_name = "PATH", num_args = 0..=1, default_missing_value = "stbl.dot")]
        dot: Option<PathBuf>,
    },
    #[command(about = "Build site from stbl.yaml.")]
    Build {
        #[arg(default_value = "articles")]
        articles_dir: PathBuf,
        #[arg(long, value_name = "PATH", default_value = "out")]
        out: PathBuf,
        #[arg(long)]
        no_cache: bool,
        #[arg(long, value_name = "PATH")]
        cache_path: Option<PathBuf>,
        #[arg(long)]
        preview: bool,
        #[arg(long, default_value = "127.0.0.1", requires = "preview")]
        preview_host: String,
        #[arg(long, default_value_t = 8080, requires = "preview")]
        preview_port: u16,
        #[arg(long, requires = "preview")]
        preview_no_open: bool,
        #[arg(long, conflicts_with = "preview_no_open", requires = "preview")]
        preview_open: bool,
        #[arg(long, default_value = "index.html", requires = "preview")]
        preview_index: String,
    },
    #[command(about = "Generate stbl.yaml from legacy stbl.conf.")]
    Upgrade {
        #[arg(long)]
        force: bool,
    },
    #[command(about = "Initialize a new site.")]
    Init {
        #[arg(long)]
        title: String,
        #[arg(long, default_value = "http://localhost:8080/")]
        url: String,
        #[arg(long, default_value = "en")]
        language: String,
        #[arg(long, value_enum, default_value = "blog")]
        kind: InitKindArg,
        #[arg(long)]
        copy_all: bool,
        target_dir: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    validate_flags(&cli)?;
    match &cli.command {
        Command::Scan { articles_dir } => run_scan(&cli, articles_dir),
        Command::Plan { articles_dir, dot } => run_plan(&cli, articles_dir, dot.as_ref()),
        Command::Build {
            articles_dir,
            out,
            no_cache,
            cache_path,
            preview,
            preview_host,
            preview_port,
            preview_no_open,
            preview_open,
            preview_index,
        } => run_build(
            &cli,
            articles_dir,
            out,
            *no_cache,
            cache_path.as_ref(),
            *preview,
            preview_host,
            *preview_port,
            *preview_no_open,
            *preview_open,
            preview_index,
        ),
        Command::Upgrade { force } => run_upgrade(&cli, *force),
        Command::Init {
            title,
            url,
            language,
            kind,
            copy_all,
            target_dir,
        } => run_init(title, url, language, *kind, *copy_all, target_dir.as_ref()),
    }
}

fn run_scan(cli: &Cli, articles_dir: &PathBuf) -> Result<()> {
    let root = root_dir(cli)?;
    let config_path = root.join("stbl.yaml");
    let _config = load_site_config(&config_path)
        .with_context(|| format!("failed to load {}", config_path.display()))?;
    let docs = walk::walk_content(&root, articles_dir, cli.unknown_header_keys.into())?;
    match assemble_site(docs) {
        Ok(site) => {
            println!("pages: {}", site.pages.len());
            println!("series: {}", site.series.len());
            let summary = handle_writeback(&root, cli, &site, WriteBackMode::DryRun)?;
            println!("{summary}");
            Ok(())
        }
        Err(diagnostics) => {
            for diag in diagnostics {
                let label = match diag.level {
                    DiagnosticLevel::Warning => "warning",
                    DiagnosticLevel::Error => "error",
                };
                if let Some(path) = diag.source_path {
                    eprintln!("{label}: {path}: {}", diag.message);
                } else {
                    eprintln!("{label}: {}", diag.message);
                }
            }
            std::process::exit(1);
        }
    }
}

fn run_plan(cli: &Cli, articles_dir: &PathBuf, dot: Option<&PathBuf>) -> Result<()> {
    let root = root_dir(cli)?;
    let config_path = root.join("stbl.yaml");
    let config = load_site_config(&config_path)
        .with_context(|| format!("failed to load {}", config_path.display()))?;
    let docs = walk::walk_content(&root, articles_dir, cli.unknown_header_keys.into())?;
    let content = match assemble_site(docs) {
        Ok(site) => site,
        Err(diagnostics) => {
            for diag in diagnostics {
                let label = match diag.level {
                    DiagnosticLevel::Warning => "warning",
                    DiagnosticLevel::Error => "error",
                };
                if let Some(path) = diag.source_path {
                    eprintln!("{label}: {path}: {}", diag.message);
                } else {
                    eprintln!("{label}: {}", diag.message);
                }
            }
            std::process::exit(1);
        }
    };
    let project = stbl_core::model::Project {
        root: root.clone(),
        config,
        content,
    };
    let site_assets_root = root.join("assets");
    let (asset_index, _asset_lookup) = assets::discover_assets(&site_assets_root)
        .with_context(|| format!("failed to discover assets under {}", site_assets_root.display()))?;
    let (image_plan, _image_lookup) =
        media::discover_images(&project).with_context(|| "failed to discover images")?;
    let (video_plan, _video_lookup) =
        media::discover_videos(&project).with_context(|| "failed to discover videos")?;
    let plan = stbl_core::plan::build_plan(&project, &asset_index, &image_plan, &video_plan);

    if let Some(dot_path) = dot {
        let dot_contents = render_dot(&plan);
        let output_path = if dot_path.is_absolute() {
            dot_path.clone()
        } else {
            root.join(dot_path)
        };
        std::fs::write(&output_path, dot_contents)
            .with_context(|| format!("failed to write {}", output_path.display()))?;
        println!("wrote {}", output_path.display());
    } else {
        println!("tasks: {}", plan.tasks.len());
        println!("edges: {}", plan.edges.len());
        for task in &plan.tasks {
            println!("task: {} {}", kind_label(&task.kind), task.id.0);
        }
        for (from, to) in &plan.edges {
            println!("edge: {} -> {}", from.0, to.0);
        }
    }
    let summary = handle_writeback(&root, cli, &project.content, WriteBackMode::DryRun)?;
    println!("{summary}");
    Ok(())
}

fn run_build(
    cli: &Cli,
    articles_dir: &PathBuf,
    out: &PathBuf,
    no_cache: bool,
    cache_path_override: Option<&PathBuf>,
    preview: bool,
    preview_host: &str,
    preview_port: u16,
    preview_no_open: bool,
    preview_open: bool,
    preview_index: &str,
) -> Result<()> {
    let root = root_dir(cli)?;
    let config = crate::config_loader::load_config_for_build(&root)
        .with_context(|| "failed to load stbl.yaml")?;
    let docs = walk::walk_content(&root, articles_dir, cli.unknown_header_keys.into())?;
    let content = match assemble_site(docs) {
        Ok(site) => site,
        Err(diagnostics) => {
            for diag in diagnostics {
                let label = match diag.level {
                    DiagnosticLevel::Warning => "warning",
                    DiagnosticLevel::Error => "error",
                };
                if let Some(path) = diag.source_path {
                    eprintln!("{label}: {path}: {}", diag.message);
                } else {
                    eprintln!("{label}: {}", diag.message);
                }
            }
            std::process::exit(1);
        }
    };
    let project = stbl_core::model::Project {
        root: root.clone(),
        config,
        content,
    };
    let site_assets_root = root.join("assets");
    let (asset_index, asset_lookup) = assets::discover_assets(&site_assets_root)
        .with_context(|| format!("failed to discover assets under {}", site_assets_root.display()))?;
    let (image_plan, image_lookup) =
        media::discover_images(&project).with_context(|| "failed to discover images")?;
    let (video_plan, video_lookup) =
        media::discover_videos(&project).with_context(|| "failed to discover videos")?;
    let asset_manifest =
        stbl_core::assets::build_asset_manifest(&asset_index, project.config.assets.cache_busting);
    let plan = stbl_core::plan::build_plan(&project, &asset_index, &image_plan, &video_plan);

    let out_dir = if out.is_absolute() {
        out.clone()
    } else {
        root.join(out)
    };

    let output_count: usize = plan.tasks.iter().map(|task| task.outputs.len()).sum();

    let (mut cache, cache_state, cache_path) = open_cache_store(&root, no_cache, cache_path_override);
    let report = exec::execute_plan(
        &project,
        &plan,
        &out_dir,
        &asset_lookup,
        &image_lookup,
        &video_lookup,
        &asset_manifest,
        cache.as_mut().map(|store| store as &mut dyn CacheStore),
    )?;
    println!("tasks: {}", plan.tasks.len());
    println!("edges: {}", plan.edges.len());
    println!("outputs: {}", output_count);
    println!("out: {}", out_dir.display());
    println!("executed: {}", report.executed);
    println!("skipped: {}", report.skipped);
    println!("cache: {}", cache_state);
    if let Some(path) = cache_path {
        println!("cache_path: {}", path.display());
    }

    let summary = handle_writeback(&root, cli, &project.content, WriteBackMode::DryRun)?;
    println!("{summary}");
    if preview {
        let no_open = if preview_open { false } else { preview_no_open };
        preview::run_preview(preview::PreviewOpts {
            site_dir: None,
            out_dir: Some(out_dir),
            host: preview_host.to_string(),
            port: preview_port,
            no_open,
            index: preview_index.to_string(),
        })?;
    }
    Ok(())
}

fn run_upgrade(cli: &Cli, force: bool) -> Result<()> {
    if cli.source_dir.is_none() {
        anyhow::bail!("upgrade requires --source-dir");
    }
    let root = root_dir(cli)?;
    let result = crate::upgrade::upgrade_site(&root, force)?;
    for warning in result.warnings {
        eprintln!("warning: {warning}");
    }
    println!("wrote {}", root.join("stbl.yaml").display());
    Ok(())
}

fn run_init(
    title: &str,
    url: &str,
    language: &str,
    kind: InitKindArg,
    copy_all: bool,
    target_dir: Option<&PathBuf>,
) -> Result<()> {
    let target_dir = match target_dir {
        Some(path) => path.clone(),
        None => std::env::current_dir().context("failed to resolve current directory")?,
    };
    let kind = match kind {
        InitKindArg::Blog => crate::init::InitKind::Blog,
        InitKindArg::LandingPage => crate::init::InitKind::LandingPage,
    };
    crate::init::init_site(crate::init::InitOptions {
        title: title.to_string(),
        base_url: url.to_string(),
        language: language.to_string(),
        kind,
        copy_all,
        target_dir,
    })
}

fn root_dir(cli: &Cli) -> Result<PathBuf> {
    match &cli.source_dir {
        Some(path) => {
            if path.is_absolute() {
                Ok(path.clone())
            } else {
                let cwd = std::env::current_dir().context("failed to read current directory")?;
                Ok(cwd.join(path))
            }
        }
        None => std::env::current_dir().context("failed to read current directory"),
    }
}

fn handle_writeback(
    root: &PathBuf,
    cli: &Cli,
    site: &stbl_core::model::SiteContent,
    mode: WriteBackMode,
) -> Result<String> {
    let ready_edits: Vec<_> = site
        .write_back
        .edits
        .iter()
        .filter(|edit| edit.new_header_text.is_some() && edit.new_body.is_some())
        .collect();
    let doc_count = ready_edits.len();
    if matches!(mode, WriteBackMode::DryRun) {
        return Ok(format!("would modify {} documents", doc_count));
    }
    if cli.no_writeback {
        return Ok(format!("would modify {} documents", doc_count));
    }
    if doc_count == 0 {
        return Ok("modified 0 documents".to_string());
    }
    for edit in &ready_edits {
        let path = root.join(&edit.path);
        let contents = format!(
            "{}{}",
            edit.new_header_text.as_ref().unwrap(),
            edit.new_body.as_ref().unwrap()
        );
        std::fs::write(&path, contents)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    if cli.commit_writeback {
        let touched = ready_edits
            .iter()
            .map(|edit| edit.path.as_str())
            .filter(|path| path.ends_with(".md"))
            .collect::<Vec<_>>();
        commit_writeback(root, &touched)?;
        return Ok(format!("modified and committed {} documents", doc_count));
    }
    Ok(format!("modified {} documents", doc_count))
}

fn validate_flags(cli: &Cli) -> Result<()> {
    let is_preview = match &cli.command {
        Command::Build { preview, .. } => *preview,
        _ => false,
    };
    if cli.include_unpublished && !is_preview {
        anyhow::bail!("--include-unpublished requires --preview");
    }
    if cli.no_writeback && !is_preview {
        anyhow::bail!("--no-writeback requires --preview");
    }
    if cli.commit_writeback && cli.no_writeback {
        anyhow::bail!("--commit-writeback cannot be used with --no-writeback");
    }
    Ok(())
}

fn commit_writeback(root: &PathBuf, touched: &[&str]) -> Result<()> {
    let repo_check = ProcessCommand::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .current_dir(root)
        .output();
    let repo_check = match repo_check {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("git is not available on PATH")
        }
        Err(err) => return Err(err.into()),
    };
    if !repo_check.status.success() || String::from_utf8_lossy(&repo_check.stdout).trim() != "true"
    {
        anyhow::bail!("not a git repository: {}", root.display());
    }

    let status = ProcessCommand::new("git")
        .arg("status")
        .arg("--porcelain")
        .current_dir(root)
        .output()
        .context("failed to run git status")?;
    if !status.status.success() {
        anyhow::bail!(
            "git status failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    if !touched.is_empty() {
        let add_status = ProcessCommand::new("git")
            .arg("add")
            .args(touched)
            .current_dir(root)
            .output()
            .context("failed to run git add")?;
        if !add_status.status.success() {
            anyhow::bail!(
                "git add failed: {}",
                String::from_utf8_lossy(&add_status.stderr)
            );
        }
    }

    let commit_status = ProcessCommand::new("git")
        .arg("commit")
        .arg("-m")
        .arg("stbl: write back header metadata")
        .current_dir(root)
        .output()
        .context("failed to run git commit")?;
    if !commit_status.status.success() {
        anyhow::bail!(
            "git commit failed: {}",
            String::from_utf8_lossy(&commit_status.stderr)
        );
    }
    Ok(())
}

fn open_cache_store(
    root: &PathBuf,
    no_cache: bool,
    cache_path_override: Option<&PathBuf>,
) -> (Option<SqliteCacheStore>, &'static str, Option<PathBuf>) {
    if no_cache {
        return (None, "off", None);
    }
    let cache_path = cache_path_override
        .map(|path| if path.is_absolute() { path.clone() } else { root.join(path) })
        .unwrap_or_else(|| root.join(".stbl").join("cache.sqlite"));
    if let Some(parent) = cache_path.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            eprintln!(
                "warning: failed to create cache directory {}: {}",
                parent.display(),
                err
            );
            return (None, "off", Some(cache_path));
        }
    }
    match SqliteCacheStore::open(&cache_path) {
        Ok(store) => (Some(store), "on", Some(cache_path)),
        Err(err) => {
            eprintln!(
                "warning: failed to open cache at {}: {}",
                cache_path.display(),
                err
            );
            (None, "off", Some(cache_path))
        }
    }
}

fn kind_label(kind: &stbl_core::model::TaskKind) -> &'static str {
    match kind {
        stbl_core::model::TaskKind::RenderPage { .. } => "RenderPage",
        stbl_core::model::TaskKind::RenderBlogIndex { .. } => "RenderBlogIndex",
        stbl_core::model::TaskKind::RenderSeries { .. } => "RenderSeries",
        stbl_core::model::TaskKind::RenderTagIndex { .. } => "RenderTagIndex",
        stbl_core::model::TaskKind::RenderTagsIndex => "RenderTagsIndex",
        stbl_core::model::TaskKind::RenderFrontPage => "RenderFrontPage",
        stbl_core::model::TaskKind::GenerateVarsCss { .. } => "GenerateVarsCss",
        stbl_core::model::TaskKind::CopyImageOriginal { .. } => "CopyImageOriginal",
        stbl_core::model::TaskKind::ResizeImage { .. } => "ResizeImage",
        stbl_core::model::TaskKind::CopyVideoOriginal { .. } => "CopyVideoOriginal",
        stbl_core::model::TaskKind::TranscodeVideoMp4 { .. } => "TranscodeVideoMp4",
        stbl_core::model::TaskKind::ExtractVideoPoster { .. } => "ExtractVideoPoster",
        stbl_core::model::TaskKind::GenerateRss => "GenerateRss",
        stbl_core::model::TaskKind::GenerateSitemap => "GenerateSitemap",
        stbl_core::model::TaskKind::CopyAsset { .. } => "CopyAsset",
    }
}

fn render_dot(plan: &stbl_core::model::BuildPlan) -> String {
    let mut output = String::from("digraph stbl {\n");
    for task in &plan.tasks {
        output.push_str(&format!(
            "  \"{}\" [label=\"{}\"];\n",
            task.id.0,
            kind_label(&task.kind)
        ));
    }
    for (from, to) in &plan.edges {
        output.push_str(&format!(
            "  \"{}\" -> \"{}\";\n",
            from.0,
            to.0
        ));
    }
    output.push_str("}\n");
    output
}

impl From<UnknownHeaderKeys> for UnknownKeyPolicy {
    fn from(value: UnknownHeaderKeys) -> Self {
        match value {
            UnknownHeaderKeys::Error => UnknownKeyPolicy::Error,
            UnknownHeaderKeys::Warn => UnknownKeyPolicy::Warn,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    fn base_cli() -> Cli {
        Cli {
            include_unpublished: false,
            unknown_header_keys: UnknownHeaderKeys::Error,
            no_writeback: false,
            commit_writeback: false,
            source_dir: None,
            command: Command::Scan {
                articles_dir: PathBuf::from("articles"),
            },
        }
    }

    #[test]
    fn include_unpublished_requires_preview() {
        let mut cli = base_cli();
        cli.include_unpublished = true;
        let err = validate_flags(&cli).expect_err("expected error");
        assert!(err.to_string().contains("include-unpublished"));
    }

    #[test]
    fn no_writeback_requires_preview() {
        let mut cli = base_cli();
        cli.no_writeback = true;
        let err = validate_flags(&cli).expect_err("expected error");
        assert!(err.to_string().contains("no-writeback"));
    }

    #[test]
    fn commit_writeback_conflicts_with_no_writeback() {
        let mut cli = base_cli();
        cli.command = Command::Build {
            articles_dir: PathBuf::from("articles"),
            out: PathBuf::from("out"),
            no_cache: false,
            cache_path: None,
            preview: true,
            preview_host: "127.0.0.1".to_string(),
            preview_port: 8080,
            preview_no_open: true,
            preview_open: false,
            preview_index: "index.html".to_string(),
        };
        cli.no_writeback = true;
        cli.commit_writeback = true;
        let err = validate_flags(&cli).expect_err("expected error");
        assert!(err.to_string().contains("commit-writeback"));
    }

    #[test]
    fn source_dir_resolves_relative_path() {
        let mut cli = base_cli();
        cli.source_dir = Some(PathBuf::from("examples/default"));
        let root = root_dir(&cli).expect("root dir");
        let expected = std::env::current_dir()
            .expect("cwd")
            .join("examples/default");
        assert_eq!(root, expected);
    }

    #[test]
    fn scan_dry_run_does_not_modify_files() {
        let temp = TempDir::new().expect("tempdir");
        write_fixture(temp.path());
        let before = read_fixture(temp.path());

        let mut cli = base_cli();
        cli.source_dir = Some(temp.path().to_path_buf());
        run_scan(&cli, &PathBuf::from("articles")).expect("scan");

        let after = read_fixture(temp.path());
        assert_eq!(before, after);
    }

    #[test]
    fn plan_dry_run_does_not_modify_files() {
        let temp = TempDir::new().expect("tempdir");
        write_fixture(temp.path());
        let before = read_fixture(temp.path());

        let mut cli = base_cli();
        cli.source_dir = Some(temp.path().to_path_buf());
        run_plan(&cli, &PathBuf::from("articles"), None).expect("plan");

        let after = read_fixture(temp.path());
        assert_eq!(before, after);
    }

    fn write_fixture(root: &Path) {
        fs::create_dir_all(root.join("articles/series")).expect("create dirs");
        fs::write(
            root.join("stbl.yaml"),
            "site:\n  id: \"fixture\"\n  title: \"Fixture\"\n  base_url: \"https://example.com/\"\n  language: \"en\"\n",
        )
        .expect("write config");
        fs::write(root.join("articles/page1.md"), "title: Page One\n\nBody\n")
            .expect("write page1");
        fs::write(
            root.join("articles/series/index.md"),
            "title: Series Index\npublished: 2024-01-01 10:00\n\nSeries\n",
        )
        .expect("write series index");
        fs::write(
            root.join("articles/series/part1.md"),
            "title: Series Part\npublished: 2024-01-02 10:00\n\nPart\n",
        )
        .expect("write series part");
    }

    fn read_fixture(root: &Path) -> Vec<(PathBuf, String)> {
        let mut files = Vec::new();
        let paths = [
            root.join("articles/page1.md"),
            root.join("articles/series/index.md"),
            root.join("articles/series/part1.md"),
        ];
        for path in paths {
            let contents = fs::read_to_string(&path).expect("read file");
            files.push((path, contents));
        }
        files
    }
}
