use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};

use ccb_release_checker::{
    github::{check_dev_branch_workflows_wrapper, check_github},
    local::{
        check_active_skill_sync, check_dev_change_set, check_git_tag, check_local_files,
        check_local_git_state, infer_repo_cli, normalize_version_cli, repo_root_cli,
    },
    markdown::{
        has_substantive_release_text, install_section, markdown_section, readme_release_block,
        release_note_versions,
    },
    Report,
};

#[derive(Parser)]
#[command(
    name = "ccb-release-checker",
    about = "Check CCB release-facing local and GitHub state."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[allow(clippy::enum_variant_names)]
enum Commands {
    /// Orchestrate the full release-state check (matches check_release_state.py).
    CheckReleaseState {
        #[arg(long)]
        repo_root: Option<Utf8PathBuf>,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        version: Option<String>,
        #[arg(long, value_parser = ["dev", "prepare", "published"], default_value = "prepare")]
        phase: String,
        #[arg(long, default_value_t = 0)]
        wait_seconds: u64,
        #[arg(long, default_value_t = 30)]
        poll_interval: u64,
    },
    /// Check GitHub release assets and SHA256SUMS (matches release_checker_assets.py).
    CheckAssets {
        #[arg(long)]
        repo_root: Option<Utf8PathBuf>,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        version: String,
    },
    /// Check GitHub-side release and homepage state (matches release_checker_github.py).
    CheckGithub {
        #[arg(long)]
        repo_root: Option<Utf8PathBuf>,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        version: String,
        #[arg(long, default_value_t = 0)]
        wait_seconds: u64,
        #[arg(long, default_value_t = 30)]
        poll_interval: u64,
    },
    /// Check local git state and release files (matches release_checker_local.py).
    CheckLocal {
        #[arg(long)]
        repo_root: Option<Utf8PathBuf>,
        #[arg(long)]
        repo: Option<String>,
        #[arg(long)]
        version: Option<String>,
        #[arg(long, value_parser = ["dev", "prepare", "published"], default_value = "prepare")]
        phase: String,
    },
    /// Check markdown/release text helpers (matches release_checker_markdown.py).
    CheckMarkdown {
        #[arg(long)]
        repo_root: Option<Utf8PathBuf>,
        #[arg(long)]
        version: String,
        #[arg(long)]
        file: Option<Utf8PathBuf>,
    },
    /// Check GitHub Actions workflow state (matches release_checker_workflows.py).
    CheckWorkflows {
        #[arg(long)]
        repo_root: Option<Utf8PathBuf>,
        #[arg(long)]
        repo: String,
        #[arg(long, default_value_t = 0)]
        wait_seconds: u64,
        #[arg(long, default_value_t = 30)]
        poll_interval: u64,
    },
}

fn resolve_root(repo_root: Option<Utf8PathBuf>) -> Utf8PathBuf {
    let start = repo_root.unwrap_or_else(|| {
        Utf8PathBuf::from_path_buf(std::env::current_dir().unwrap_or_default()).unwrap_or_default()
    });
    repo_root_cli(&start)
}

fn print_report(
    report: &Report,
    version: &str,
    phase: &str,
    root: &camino::Utf8Path,
    repo: &str,
) -> i32 {
    println!("CCB release check: {version} ({phase})");
    println!("repo root: {root}");
    println!("github repo: {repo}");

    if !report.warnings.is_empty() {
        println!("\nWarnings:");
        for item in &report.warnings {
            println!("- {item}");
        }
    }

    if !report.issues.is_empty() {
        println!("\nIssues:");
        for item in &report.issues {
            println!("- {item}");
        }
        return 1;
    }

    println!("\nOK: no blocking release-surface drift found.");
    0
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Commands::CheckReleaseState {
            repo_root,
            repo,
            version,
            phase,
            wait_seconds,
            poll_interval,
        } => cmd_check_release_state(repo_root, repo, version, phase, wait_seconds, poll_interval),
        Commands::CheckAssets {
            repo_root,
            repo,
            version,
        } => cmd_check_assets(repo_root, repo, version),
        Commands::CheckGithub {
            repo_root,
            repo,
            version,
            wait_seconds,
            poll_interval,
        } => cmd_check_github(repo_root, repo, version, wait_seconds, poll_interval),
        Commands::CheckLocal {
            repo_root,
            repo,
            version,
            phase,
        } => cmd_check_local(repo_root, repo, version, phase),
        Commands::CheckMarkdown {
            repo_root,
            version,
            file,
        } => cmd_check_markdown(repo_root, version, file),
        Commands::CheckWorkflows {
            repo_root,
            repo,
            wait_seconds,
            poll_interval,
        } => cmd_check_workflows(repo_root, repo, wait_seconds, poll_interval),
    };
    std::process::exit(code);
}

fn cmd_check_release_state(
    repo_root: Option<Utf8PathBuf>,
    repo: Option<String>,
    version: Option<String>,
    phase: String,
    wait_seconds: u64,
    poll_interval: u64,
) -> i32 {
    let root = resolve_root(repo_root);
    let repo = repo.unwrap_or_else(|| infer_repo_cli(&root));
    let raw_version = version.unwrap_or_else(|| {
        ccb_release_checker::read(&root.join("VERSION"))
            .trim()
            .to_string()
    });
    let version = normalize_version_cli(&raw_version);
    let mut report = Report::default();

    check_active_skill_sync(&root, &mut report);
    check_local_git_state(&root, &phase, &mut report);
    if phase == "dev" {
        check_dev_change_set(&root, &mut report);
        check_dev_branch_workflows_wrapper(&root, &repo, wait_seconds, poll_interval, &mut report);
    } else {
        check_local_files(&root, &version, &repo, &mut report);
        check_git_tag(&root, &version, &phase, &mut report);
    }

    if phase == "published" {
        check_github(
            &root,
            &version,
            &repo,
            &mut report,
            wait_seconds,
            poll_interval,
        );
    }

    print_report(&report, &version, &phase, &root, &repo)
}

fn cmd_check_assets(repo_root: Option<Utf8PathBuf>, repo: String, version: String) -> i32 {
    let root = resolve_root(repo_root);
    let version = normalize_version_cli(&version);
    let mut report = Report::default();
    ccb_release_checker::assets::check_sha256sums(&root, &version, &repo, &mut report);
    print_report(&report, &version, "assets", &root, &repo)
}

fn cmd_check_github(
    repo_root: Option<Utf8PathBuf>,
    repo: String,
    version: String,
    wait_seconds: u64,
    poll_interval: u64,
) -> i32 {
    let root = resolve_root(repo_root);
    let version = normalize_version_cli(&version);
    let mut report = Report::default();
    check_github(
        &root,
        &version,
        &repo,
        &mut report,
        wait_seconds,
        poll_interval,
    );
    print_report(&report, &version, "published", &root, &repo)
}

fn cmd_check_local(
    repo_root: Option<Utf8PathBuf>,
    repo: Option<String>,
    version: Option<String>,
    phase: String,
) -> i32 {
    let root = resolve_root(repo_root);
    let repo = repo.unwrap_or_else(|| infer_repo_cli(&root));
    let version = version
        .map(|v| normalize_version_cli(&v))
        .unwrap_or_else(|| {
            normalize_version_cli(ccb_release_checker::read(&root.join("VERSION")).trim())
        });
    let mut report = Report::default();
    check_active_skill_sync(&root, &mut report);
    check_local_git_state(&root, &phase, &mut report);
    if phase == "dev" {
        check_dev_change_set(&root, &mut report);
    } else {
        check_local_files(&root, &version, &repo, &mut report);
        check_git_tag(&root, &version, &phase, &mut report);
    }
    print_report(&report, &version, &phase, &root, &repo)
}

fn cmd_check_markdown(
    repo_root: Option<Utf8PathBuf>,
    version: String,
    file: Option<Utf8PathBuf>,
) -> i32 {
    let root = resolve_root(repo_root);
    let version = normalize_version_cli(&version);
    let mut report = Report::default();
    let target = file
        .as_ref()
        .map(|f| root.join(f))
        .unwrap_or_else(|| root.join("README.md"));
    let body = ccb_release_checker::read(&target);
    let versions = release_note_versions(&body);
    if versions.is_empty() {
        report.fail(
            format!("No release note versions found in {}", target),
            None,
        );
    } else if versions[0] != version {
        report.fail(
            format!(
                "First release notes entry is {}, expected {version}",
                versions[0]
            ),
            None,
        );
    }
    if !has_substantive_release_text(readme_release_block(&body, &version).as_deref()) {
        report.fail(format!("Release notes entry for {version} is empty"), None);
    }
    let section = markdown_section(&body, &version);
    if section.is_none() {
        report.fail(format!("No markdown section for {version}"), None);
    }
    let install = install_section(&body, "How to Install");
    if install.is_empty() {
        report.warn("No How to Install section found");
    }
    print_report(&report, &version, "markdown", &root, "")
}

fn cmd_check_workflows(
    repo_root: Option<Utf8PathBuf>,
    repo: String,
    wait_seconds: u64,
    poll_interval: u64,
) -> i32 {
    let root = resolve_root(repo_root);
    let mut report = Report::default();
    check_dev_branch_workflows_wrapper(&root, &repo, wait_seconds, poll_interval, &mut report);
    print_report(&report, "", "workflows", &root, &repo)
}
