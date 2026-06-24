use std::path::PathBuf;
use std::process;

use anyhow::Result;
use clap::{Parser, Subcommand};

use ccbr_release_builder::{BuildOptions, PackageOptions, VerifyOptions};

#[derive(Parser)]
#[command(name = "ccbr-release-builder")]
#[command(about = "Build CCBR release artifacts")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build a release artifact for the requested platform.
    Build {
        /// Target platform (linux or macos).
        #[arg(long, value_name = "PLATFORM")]
        platform: String,

        /// Output directory for the artifact and checksum.
        #[arg(long, value_name = "DIR", default_value = ccbr_release_builder::DEFAULT_OUTPUT_DIR)]
        output_dir: PathBuf,

        /// Build channel metadata override.
        #[arg(long, value_name = "CHANNEL")]
        channel: Option<String>,

        /// Git ref to archive when building from a clean git checkout.
        #[arg(long, value_name = "REF", default_value = "HEAD")]
        git_ref: String,

        /// Allow building from the current dirty worktree (local preview only).
        #[arg(long)]
        allow_dirty: bool,
    },

    /// Package an existing artifact directory into a tar.gz + SHA256SUMS.
    Package {
        /// Target platform (linux or macos).
        #[arg(long, value_name = "PLATFORM")]
        platform: String,

        /// Directory to package.
        #[arg(long, value_name = "DIR")]
        artifact_dir: PathBuf,

        /// Output directory for the artifact and checksum.
        #[arg(long, value_name = "DIR", default_value = ccbr_release_builder::DEFAULT_OUTPUT_DIR)]
        output_dir: PathBuf,
    },

    /// Verify a release artifact contains required metadata.
    Verify {
        /// Path to the release artifact.
        #[arg(long, value_name = "PATH")]
        artifact: PathBuf,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {:#}", err);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Build {
            platform,
            output_dir,
            channel,
            git_ref,
            allow_dirty,
        } => {
            let opts = BuildOptions {
                platform,
                output_dir,
                channel,
                git_ref,
                allow_dirty,
            };
            ccbr_release_builder::run_build(&opts)?;
        }
        Commands::Package {
            platform,
            artifact_dir,
            output_dir,
        } => {
            let opts = PackageOptions {
                platform,
                artifact_dir,
                output_dir,
            };
            ccbr_release_builder::run_package(&opts)?;
        }
        Commands::Verify { artifact } => {
            let opts = VerifyOptions {
                artifact_path: artifact,
            };
            ccbr_release_builder::run_verify(&opts)?;
        }
    }
    Ok(())
}
