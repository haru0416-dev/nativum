//! nativum CLI.

#![forbid(unsafe_code)]

mod host;
mod preview;
mod scaffold;

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use nativum_engine::{load_app_dir, Session, Step};
use nativum_markup::check_document;

#[derive(Parser)]
#[command(
    name = "nativum",
    version,
    about = "Native desktop UI toolkit in Rust. No browser, no WebView."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scaffold a three-file app (view, core, manifest).
    Init {
        /// Directory to create.
        name: String,
    },
    /// Validate markup against the core's model and messages.
    Check {
        /// App directory (contains app.json).
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Render the current view to a PNG (headless).
    Render {
        /// App directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output PNG path.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Dark tokens.
        #[arg(long)]
        dark: bool,
    },
    /// Print an accessibility snapshot as JSON.
    Snapshot {
        /// App directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Replay a journal of press/click/type/key steps, then render.
    Replay {
        /// App directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Journal JSON (array of steps, or `{ "steps": [...] }`).
        journal: PathBuf,
        /// Optional PNG output after replay.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Run the app's `tests/*.json` journals.
    Test {
        /// App directory.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Open a real OS window and present the software-rendered frames.
    Run {
        /// App directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Dark tokens.
        #[arg(long)]
        dark: bool,
        /// Present this many frames then exit (smoke / CI). Omit to run until closed.
        #[arg(long)]
        frames: Option<u32>,
    },
    /// Serve a clickable software-rendered preview (no GPU, no display server).
    Preview {
        /// App directory.
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Bind address.
        #[arg(long, default_value = "127.0.0.1:8787")]
        bind: SocketAddr,
        /// Rebuild when view/core files change.
        #[arg(long)]
        watch: bool,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("nativum: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Init { name } => scaffold::init(&name),
        Commands::Check { path } => cmd_check(&path),
        Commands::Render { path, out, dark } => cmd_render(&path, out.as_deref(), dark),
        Commands::Snapshot { path } => cmd_snapshot(&path),
        Commands::Replay { path, journal, out } => cmd_replay(&path, &journal, out.as_deref()),
        Commands::Test { path } => cmd_test(&path),
        Commands::Run { path, dark, frames } => host::run(&path, dark, frames),
        Commands::Preview { path, bind, watch } => preview::serve(&path, bind, watch),
    }
}

fn open_session(path: &Path) -> Result<Session> {
    let loaded = load_app_dir(path).map_err(|e| anyhow::anyhow!("{e}"))?;
    Session::from_loaded(loaded).map_err(|e| anyhow::anyhow!("{e}"))
}

fn cmd_check(path: &Path) -> Result<()> {
    let session = open_session(path)?;
    let keys: BTreeSet<String> = session.core().binding_names().into_iter().collect();
    let tags: BTreeSet<String> = session.core().message_tags().into_iter().collect();
    let report = check_document(session.document(), &keys, &tags);
    for e in &report.errors {
        eprintln!("error: {e}");
    }
    for w in &report.warnings {
        eprintln!("warning: {w}");
    }
    if !report.ok() {
        bail!("{} error(s)", report.errors.len());
    }
    println!(
        "ok  {}  {} bindings  {} messages",
        path.display(),
        keys.len(),
        tags.len()
    );
    Ok(())
}

fn cmd_render(path: &Path, out: Option<&Path>, dark: bool) -> Result<()> {
    let mut session = open_session(path)?;
    if dark {
        session
            .set_appearance(nativum_core::Appearance::Dark)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    let png = session.png().map_err(|e| anyhow::anyhow!("{e}"))?;
    let out = out
        .map(PathBuf::from)
        .unwrap_or_else(|| path.join("preview.png"));
    std::fs::write(&out, png)?;
    println!("wrote {}", out.display());
    Ok(())
}

fn cmd_snapshot(path: &Path) -> Result<()> {
    let session = open_session(path)?;
    let snap = session.snapshot().context("no snapshot")?;
    println!("{}", serde_json::to_string_pretty(&snap)?);
    Ok(())
}

fn read_journal(path: &Path) -> Result<Vec<Step>> {
    let text = std::fs::read_to_string(path)?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    if let Some(steps) = v.get("steps") {
        Ok(serde_json::from_value(steps.clone())?)
    } else {
        Ok(serde_json::from_value(v)?)
    }
}

fn cmd_replay(path: &Path, journal: &Path, out: Option<&Path>) -> Result<()> {
    let mut session = open_session(path)?;
    let steps = read_journal(journal)?;
    session.replay(&steps).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!(
        "{}",
        serde_json::to_string_pretty(&session.model().to_json())?
    );
    if let Some(out) = out {
        std::fs::write(out, session.png().map_err(|e| anyhow::anyhow!("{e}"))?)?;
        println!("wrote {}", out.display());
    }
    Ok(())
}

fn cmd_test(path: &Path) -> Result<()> {
    let tests_dir = path.join("tests");
    if !tests_dir.is_dir() {
        bail!("no tests/ in {}", path.display());
    }
    let mut failed = 0usize;
    let mut ran = 0usize;
    let mut entries: Vec<_> = std::fs::read_dir(&tests_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    entries.sort();
    for file in entries {
        ran += 1;
        match run_one_test(path, &file) {
            Ok(()) => println!("ok    {}", file.file_name().unwrap().to_string_lossy()),
            Err(e) => {
                failed += 1;
                println!(
                    "FAIL  {}  {e:#}",
                    file.file_name().unwrap().to_string_lossy()
                );
            }
        }
    }
    if failed > 0 {
        bail!("{failed}/{ran} test(s) failed");
    }
    println!("{ran} test(s) passed");
    Ok(())
}

fn run_one_test(app: &Path, file: &Path) -> Result<()> {
    #[derive(serde::Deserialize)]
    struct Spec {
        #[serde(default)]
        steps: Vec<Step>,
        #[serde(default)]
        assert: Option<serde_json::Value>,
    }
    let spec: Spec = serde_json::from_str(&std::fs::read_to_string(file)?)?;
    let mut session = open_session(app)?;
    session
        .replay(&spec.steps)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    if let Some(expected) = spec.assert {
        let actual = session.model().to_json();
        check_assert(&actual, &expected)
            .with_context(|| format!("assert in {}", file.display()))?;
    }
    Ok(())
}

fn check_assert(actual: &serde_json::Value, expected: &serde_json::Value) -> Result<()> {
    match expected {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let got = pointer(actual, k).with_context(|| format!("missing '{k}'"))?;
                if !json_match(got, v) {
                    bail!("{k}: expected {v}, got {got}");
                }
            }
            Ok(())
        }
        other => {
            if !json_match(actual, other) {
                bail!("expected {other}, got {actual}");
            }
            Ok(())
        }
    }
}

fn json_match(got: &serde_json::Value, expected: &serde_json::Value) -> bool {
    match (got, expected) {
        (serde_json::Value::Number(a), serde_json::Value::Number(b)) => a.as_f64() == b.as_f64(),
        _ => got == expected,
    }
}

fn pointer<'a>(v: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cur = v;
    for seg in path.split('.') {
        cur = match cur {
            serde_json::Value::Object(m) => m.get(seg)?,
            serde_json::Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur)
}

/// Watch helper used by preview --watch. Polls mtimes; no inotify dependency.
pub fn files_changed(paths: &[PathBuf], last: &mut Vec<Option<Duration>>) -> bool {
    let mut changed = false;
    if last.len() != paths.len() {
        last.clear();
        last.resize(paths.len(), None);
        changed = true;
    }
    for (i, p) in paths.iter().enumerate() {
        let mtime = std::fs::metadata(p)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok());
        if last[i] != mtime {
            last[i] = mtime;
            changed = true;
        }
    }
    changed
}
