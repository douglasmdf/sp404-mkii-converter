use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use clap::{Parser, Subcommand};
use sp404_core::config::{SaveMode, Settings};
use sp404_core::runner::{self, Event, FileStatus};
use sp404_core::scan::{self, JobKind, Plan};
use sp404_core::{space, undo};

#[derive(Parser)]
#[command(name = "sp404", version, about = "Convert sample packs to Roland SP-404 MKII format (16-bit / 48 kHz WAV)")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Convert one or more pack folders
    Convert {
        #[arg(required = true)]
        packs: Vec<PathBuf>,
        /// Output folder (otherwise the saved settings decide)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
        /// Write conversion_results.json into each output pack
        #[arg(long)]
        json_log: bool,
    },
    /// Delete the files created by the last conversion
    Undo {
        /// Also remove folders left empty
        #[arg(long)]
        remove_empty_dirs: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let outcome = match cli.command {
        Command::Convert { packs, out, dry_run, json_log } => convert(&packs, out.as_deref(), dry_run, json_log),
        Command::Undo { remove_empty_dirs } => undo_last(remove_empty_dirs),
    };
    match outcome {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn convert(packs: &[PathBuf], out: Option<&Path>, dry_run: bool, json_log: bool) -> Result<ExitCode, String> {
    let mut settings = Settings::load();
    if out.is_some() {
        settings.save_mode = SaveMode::Ask;
    }
    if json_log {
        settings.write_json_log = true;
    }
    let plan = scan::build_plan(packs, out, &settings).map_err(|e| e.to_string())?;
    if dry_run {
        print_plan(&plan);
        return Ok(ExitCode::SUCCESS);
    }
    space::check_free_space(&plan).map_err(|e| e.to_string())?;
    let cancel = AtomicBool::new(false);
    let (summary, manifest) = runner::run(&plan, &settings, &cancel, |event| {
        if let Event::FileDone { done, total, result } = event {
            let label = match &result.status {
                FileStatus::Converted => String::from("converted"),
                FileStatus::Copied => String::from("copied"),
                FileStatus::Skipped => String::from("skipped (already exists)"),
                FileStatus::Failed { reason } => format!("FAILED: {reason}"),
                FileStatus::Cancelled => String::from("cancelled"),
            };
            println!("[{done}/{total}] {} — {label}", result.source.display());
        }
    })
    .map_err(|e| e.to_string())?;
    manifest.save_to(&undo::manifest_path()).map_err(|e| e.to_string())?;
    println!(
        "\n{} converted, {} copied, {} skipped, {} failed, {} with warnings",
        summary.converted, summary.copied, summary.skipped, summary.failed, summary.with_warnings
    );
    Ok(if summary.failed > 0 { ExitCode::from(1) } else { ExitCode::SUCCESS })
}

fn print_plan(plan: &Plan) {
    for pack in &plan.packs {
        println!("{} -> {}", pack.source_root.display(), pack.dest_root.display());
        for job in &pack.jobs {
            let source = job.source.strip_prefix(&pack.source_root).unwrap_or(&job.source);
            let dest = job.dest.strip_prefix(&pack.dest_root).unwrap_or(&job.dest);
            let kind = match job.kind {
                JobKind::Convert => "convert",
                JobKind::Copy => "copy",
                JobKind::Unsupported => "unsupported",
            };
            let note = if job.already_exists { "  (exists, will skip)" } else { "" };
            println!("  {kind:<11} {} -> {}{note}", source.display(), dest.display());
        }
    }
}

fn undo_last(remove_empty_dirs: bool) -> Result<ExitCode, String> {
    match undo::undo_last_session(&undo::manifest_path(), remove_empty_dirs).map_err(|e| e.to_string())? {
        None => println!("Nothing to undo."),
        Some(report) => {
            println!("Removed {} files and {} folders.", report.removed, report.removed_dirs);
            for path in &report.kept_modified {
                println!("Kept (changed since the conversion): {}", path.display());
            }
            for path in &report.failed {
                println!("Could not remove: {}", path.display());
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}
