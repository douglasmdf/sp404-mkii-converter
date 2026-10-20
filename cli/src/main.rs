use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

mod analyze;
mod eval;

use clap::{Parser, Subcommand};
use sp404_core::analyze::AnalysisCache;
use sp404_core::config::{BpmRange, SaveMode, Settings};
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
    /// Convert one or more pack folders and/or loose audio files
    Convert {
        /// Pack folders and/or audio files
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Output folder (otherwise the saved settings decide; required with loose files)
        #[arg(long)]
        out: Option<PathBuf>,
        /// Show what would happen without writing anything
        #[arg(long)]
        dry_run: bool,
        /// Write conversion_results.json into each output pack
        #[arg(long)]
        json_log: bool,
        /// Add the detected key/note/chord and BPM to the output names (otherwise the saved
        /// settings decide)
        #[arg(long, conflicts_with = "no_detect_key_bpm")]
        detect_key_bpm: bool,
        /// Do not add key/BPM to the names, even if the saved settings turn it on
        #[arg(long)]
        no_detect_key_bpm: bool,
        /// BPM range detected tempos are folded into: 58-115, 68-135, 78-166, 88-175 or 98-195
        /// (default: the saved range, 78-166 when nothing is saved)
        #[arg(long, value_parser = parse_range)]
        bpm_range: Option<BpmRange>,
    },
    /// Detect the type, key and BPM of audio files (and score the result against an answer key)
    Analyze {
        /// Pack folders and/or audio files
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// BPM range detected tempos are folded into
        #[arg(long, value_parser = parse_range, default_value = "78-166")]
        bpm_range: BpmRange,
        /// Analyse without using the folder and file names
        #[arg(long)]
        ignore_names: bool,
        /// CSV answer key (path,kind,label,bpm) to measure accuracy against
        #[arg(long)]
        labels: Option<PathBuf>,
        /// Write the accuracy tables to this Markdown file
        #[arg(long)]
        report: Option<PathBuf>,
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
        Command::Convert { inputs, out, dry_run, json_log, detect_key_bpm, no_detect_key_bpm, bpm_range } => {
            let detect = if detect_key_bpm {
                Some(true)
            } else if no_detect_key_bpm {
                Some(false)
            } else {
                None
            };
            convert(&inputs, out.as_deref(), dry_run, json_log, detect, bpm_range)
        }
        Command::Analyze { inputs, bpm_range, ignore_names, labels, report } => {
            analyze::run(&analyze::Options { inputs, range: bpm_range, ignore_names, labels, report }).map(|()| ExitCode::SUCCESS)
        }
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

fn parse_range(text: &str) -> Result<BpmRange, String> {
    text.parse()
}

fn convert(
    inputs: &[PathBuf],
    out: Option<&Path>,
    dry_run: bool,
    json_log: bool,
    detect_key_bpm: Option<bool>,
    bpm_range: Option<BpmRange>,
) -> Result<ExitCode, String> {
    if out.is_none() && inputs.iter().any(|p| p.is_file() && scan::classify(p) != JobKind::Copy) {
        return Err(String::from("loose audio files need an output folder: pass --out <folder>"));
    }
    let mut settings = Settings::load();
    if out.is_some() {
        settings.save_mode = SaveMode::Ask;
    }
    if json_log {
        settings.write_json_log = true;
    }
    if let Some(detect) = detect_key_bpm {
        settings.detect_key_bpm = detect;
    }
    if let Some(range) = bpm_range {
        settings.bpm_range = range;
    }
    let cancel = AtomicBool::new(false);
    let plan = scan::plan_with(inputs, out, &settings, &AnalysisCache::default(), &cancel, |_| {}).map_err(|e| e.to_string())?;
    if dry_run {
        print_plan(&plan);
        if settings.detect_key_bpm {
            println!("\n{}", uncertain_summary(&plan));
        }
        return Ok(ExitCode::SUCCESS);
    }
    space::check_free_space(&plan).map_err(|e| e.to_string())?;
    let manifest_file = undo::manifest_path();
    let (summary, _) = runner::run(&plan, &settings, &cancel, Some(&manifest_file), |event| {
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
    println!(
        "\n{} converted, {} copied, {} skipped, {} failed, {} with warnings",
        summary.converted, summary.copied, summary.skipped, summary.failed, summary.with_warnings
    );
    if settings.detect_key_bpm {
        println!("{}", uncertain_summary(&plan));
    }
    Ok(if summary.failed > 0 { ExitCode::from(1) } else { ExitCode::SUCCESS })
}

/// Same count as the app: files with any uncertain value (type, key/note/chord, BPM) or
/// that could not be analysed.
fn uncertain_summary(plan: &Plan) -> String {
    let n = plan
        .jobs()
        .filter_map(|j| j.analysis.as_ref())
        .filter(|a| a.error.is_some() || a.kind_uncertain || a.label_uncertain || a.bpm_uncertain)
        .count();
    format!("{n} files to check by ear (uncertain or not analyzed)")
}

fn analysis_note(job: &scan::Job) -> String {
    let Some(a) = &job.analysis else { return String::new() };
    if let Some(error) = &a.error {
        return format!("  (not analyzed: {error})");
    }
    let mut out = String::new();
    if a.label_uncertain {
        out.push_str("  (key uncertain)");
    }
    if a.bpm_uncertain {
        out.push_str("  (bpm uncertain)");
    }
    if a.kind_uncertain {
        out.push_str("  (type uncertain)");
    }
    out
}

fn print_plan(plan: &Plan) {
    for pack in &plan.packs {
        if pack.loose {
            println!("(loose files) -> {}", pack.dest_root.display());
        } else {
            println!("{} -> {}", pack.source_root.display(), pack.dest_root.display());
        }
        for job in &pack.jobs {
            let source = job.source.strip_prefix(&pack.source_root).unwrap_or(&job.source);
            let dest = job.dest.strip_prefix(&pack.dest_root).unwrap_or(&job.dest);
            let kind = match job.kind {
                JobKind::Convert => "convert",
                JobKind::Copy => "copy",
                JobKind::Unsupported => "unsupported",
            };
            let note = if job.already_exists { "  (exists, will skip)" } else { "" };
            println!("  {kind:<11} {} -> {}{note}{}", source.display(), dest.display(), analysis_note(job));
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
