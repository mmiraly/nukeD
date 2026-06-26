mod age;
mod cache;
mod cleanup;
mod display;
mod fuzzy;
mod profiles;
mod report;
mod scanner;
mod tui;

use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;

use crate::age::AgeThreshold;
use crate::cache::scan_caches;
use crate::cleanup::TrashCleaner;
use crate::profiles::Profiles;
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

  nuked --profile work
      Load roots and default age from the saved work profile.

  nuked --root ~/Documents/Repos --json
      Print a machine-readable JSON scan report.

  nuked --cache --dry-run
      Inspect package-manager caches without scanning project roots.

AGE accepts values like 7d, 2w, 30d, 3m, or 1y."
)]
struct Args {
    /// Root directory to scan. Can be passed multiple times.
    #[arg(short, long, value_name = "PATH")]
    root: Vec<PathBuf>,

    /// Only select dependency folders whose project has been untouched for this age.
    #[arg(short, long, value_name = "AGE")]
    older_than: Option<AgeThreshold>,

    /// Load roots and default age from a saved profile.
    #[arg(long, value_name = "NAME")]
    profile: Option<String>,

    /// Print a report and do not launch the interactive UI or delete anything.
    #[arg(long)]
    dry_run: bool,

    /// Print a machine-readable JSON report to stdout.
    #[arg(long)]
    json: bool,

    /// Write a machine-readable JSON report to disk.
    #[arg(long, value_name = "PATH")]
    report: Option<PathBuf>,

    /// Inspect package-manager caches instead of project dependency folders.
    #[arg(long)]
    cache: bool,

    /// Fuzzy-filter results by path, kind, size, or age text.
    #[arg(short, long, value_name = "QUERY")]
    filter: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let profiles = Profiles::load()?;
    let profile = args
        .profile
        .as_deref()
        .map(|name| {
            profiles
                .get(name)
                .cloned()
                .with_context(|| format!("profile {name:?} not found"))
        })
        .transpose()?;
    let roots = if !args.root.is_empty() {
        args.root
    } else if let Some(profile) = &profile {
        profile.roots.clone()
    } else {
        vec![std::env::current_dir().context("failed to read current directory")?]
    };
    let older_than = args
        .older_than
        .or_else(|| profile.as_ref().and_then(|profile| profile.older_than))
        .unwrap_or_else(|| AgeThreshold::days(30));

    if args.cache {
        let summary = scan_caches();
        let json_report = report::cache_report(&summary);
        if args.json {
            report::print_json(&json_report)?;
        }
        if let Some(path) = &args.report {
            report::write_json(path, &json_report)?;
        }
        if args.dry_run && !args.json {
            display::print_report(&summary.to_scan_summary(), AgeThreshold::days(1), None);
        }
        if args.dry_run || args.json || args.report.is_some() {
            return Ok(());
        }

        let cleaner = TrashCleaner;
        return tui::run(
            summary.to_scan_summary(),
            AgeThreshold::days(1),
            String::new(),
            &cleaner,
            profiles,
            args.profile,
            true,
        );
    }

    let scan = scan_roots(&roots, ScanOptions::default()).context("scan failed")?;

    let json_report = report::project_report(&scan, older_than, args.filter.as_deref());
    if args.json {
        report::print_json(&json_report)?;
    }
    if let Some(path) = &args.report {
        report::write_json(path, &json_report)?;
    }
    if args.json || args.report.is_some() {
        return Ok(());
    }

    if args.dry_run {
        display::print_report(&scan, older_than, args.filter.as_deref());
        return Ok(());
    }

    let cleaner = TrashCleaner;
    tui::run(
        scan,
        older_than,
        args.filter.unwrap_or_default(),
        &cleaner,
        profiles,
        args.profile,
        false,
    )
}
