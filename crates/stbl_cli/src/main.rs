mod exec;
mod walk;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
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
    preview: bool,
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
    Build {
        #[arg(default_value = "articles")]
        articles_dir: PathBuf,
        #[arg(long, value_name = "PATH", default_value = "out")]
        out: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    validate_flags(&cli)?;
    match &cli.command {
        Command::Scan { articles_dir } => run_scan(&cli, articles_dir),
        Command::Plan { articles_dir, dot } => run_plan(&cli, articles_dir, dot.as_ref()),
        Command::Build { articles_dir, out } => run_build(&cli, articles_dir, out),
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
    let plan = stbl_core::plan::build_plan(&project);

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
            println!("task: {} {}", kind_label(&task.kind), task.id.0.to_hex());
        }
        for (from, to) in &plan.edges {
            println!("edge: {} -> {}", from.0.to_hex(), to.0.to_hex());
        }
    }
    let summary = handle_writeback(&root, cli, &project.content, WriteBackMode::DryRun)?;
    println!("{summary}");
    Ok(())
}

fn run_build(cli: &Cli, articles_dir: &PathBuf, out: &PathBuf) -> Result<()> {
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
    let plan = stbl_core::plan::build_plan(&project);

    let out_dir = if out.is_absolute() {
        out.clone()
    } else {
        root.join(out)
    };

    let output_count: usize = plan.tasks.iter().map(|task| task.outputs.len()).sum();
    exec::execute_plan(&project, &plan, &out_dir)?;
    println!("tasks: {}", plan.tasks.len());
    println!("edges: {}", plan.edges.len());
    println!("outputs: {}", output_count);
    println!("out: {}", out_dir.display());

    let summary = handle_writeback(&root, cli, &project.content, WriteBackMode::DryRun)?;
    println!("{summary}");
    Ok(())
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
    if cli.include_unpublished && !cli.preview {
        anyhow::bail!("--include-unpublished requires --preview");
    }
    if cli.no_writeback && !cli.preview {
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

fn kind_label(kind: &stbl_core::model::TaskKind) -> &'static str {
    match kind {
        stbl_core::model::TaskKind::RenderPage { .. } => "RenderPage",
        stbl_core::model::TaskKind::RenderBlogIndex { .. } => "RenderBlogIndex",
        stbl_core::model::TaskKind::RenderSeries { .. } => "RenderSeries",
        stbl_core::model::TaskKind::RenderTagIndex { .. } => "RenderTagIndex",
        stbl_core::model::TaskKind::RenderTagsIndex => "RenderTagsIndex",
        stbl_core::model::TaskKind::RenderFrontPage => "RenderFrontPage",
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
            task.id.0.to_hex(),
            kind_label(&task.kind)
        ));
    }
    for (from, to) in &plan.edges {
        output.push_str(&format!(
            "  \"{}\" -> \"{}\";\n",
            from.0.to_hex(),
            to.0.to_hex()
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
            preview: false,
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
        cli.preview = true;
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
