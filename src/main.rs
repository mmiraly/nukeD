mod age;
mod cleanup;
mod display;
mod fuzzy;
mod scanner;
mod tui;

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;

use crate::age::AgeThreshold;
use crate::cleanup::TrashCleaner;
use crate::scanner::{ScanOptions, scan_roots};

#[derive(Debug, Parser)]
#[command(
    author,
    version,
    about,
    after_help = "Examples:
  nuked
      Launch the interactive TUI in the current directory.

  nuked --root ~/Documents/Repos
      Launch the TUI and scan a specific repo folder.

  nuked --root ~/Documents/Repos --root ~/Code --dry-run
      Scan multiple roots and print a report without opening the TUI.

  nuked --root ~/Documents/Repos --dry-run --older-than 7d
      Print folders whose projects have been untouched for at least 7 days.

  nuked --root ~/Documents/Repos --dry-run --filter api
      Fuzzy-filter the report by path, ecosystem, size, or age.

AGE accepts values like 7d, 2w, 30d, 3m, or 1y."
)]
struct Args {
    /// Root directory to scan. Can be passed multiple times.
    #[arg(short, long, value_name = "PATH")]
    root: Vec<PathBuf>,

    /// Only select dependency folders whose project has been untouched for this age.
    #[arg(short, long, default_value = "30d", value_name = "AGE")]
    older_than: AgeThreshold,

    /// Print a report and do not launch the interactive UI or delete anything.
    #[arg(long)]
    dry_run: bool,

    /// Fuzzy-filter results by path, kind, size, or age text.
    #[arg(short, long, value_name = "QUERY")]
    filter: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let roots = if args.root.is_empty() {
        vec![std::env::current_dir().context("failed to read current directory")?]
    } else {
        args.root
    };

    let scan = scan_roots(&roots, ScanOptions::default()).context("scan failed")?;

    if args.dry_run {
        display::print_report(&scan, args.older_than, args.filter.as_deref());
        return Ok(());
    }

    let cleaner = TrashCleaner;
    tui::run(
        scan,
        args.older_than,
        args.filter.unwrap_or_default(),
        &cleaner,
    )
}
