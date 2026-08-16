mod bootstrap;
mod capture;
mod display;
mod doctor;
mod language;
mod meter;
mod models;
mod paths;
mod permissions;
mod resample;
mod session;
mod term;
mod writer;

use std::fs;
use std::io::{self, IsTerminal};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use crossbeam_channel::{bounded, unbounded};

use crate::capture::{mic, system, RawChunk};

const DEFAULT_MAX_DURATION: &str = "4h";
const DEFAULT_EXTEND_BY:    &str = "1h";

#[derive(Parser, Debug)]
#[command(version, about = "mic + system audio recorder with mlx-whisper transcription")]
struct Args {
    /// Session output directory. Defaults to ~/nabu_data/YYYY_MM_DD_HH_mm/.
    #[arg(long)]
    out: Option<std::path::PathBuf>,

    /// Whisper model name (e.g. tiny.en, base.en, small.en, distil-large-v3).
    #[arg(long, default_value = "large-v3")]
    model: String,

    /// HuggingFace token — only needed for --diarizer pyannote.
    /// Can also be set via the HF_TOKEN environment variable.
    #[arg(long, env = "HF_TOKEN")]
    hf_token: Option<String>,

    /// Speaker diarization backend.
    /// 'wespeaker' (default) needs no account — models download automatically.
    /// 'pyannote' gives higher accuracy but requires: nabu --setup --hf-token TOKEN
    #[arg(long, default_value = "wespeaker")]
    diarizer: String,

    /// Transcribe an existing audio file and exit. No recording is started.
    #[arg(long)]
    file: Option<std::path::PathBuf>,

    /// Transcribe an existing saved session directory and exit. No recording is
    /// started. The directory must contain mic.wav + system.wav (as saved by a
    /// prior run); transcript.md is written into it.
    #[arg(long)]
    transcribe: Option<std::path::PathBuf>,

    /// Language code to force during transcription (e.g. en, fr, ja).
    /// Omit to pick one interactively (or auto-detect on a non-TTY run).
    /// Only useful with multilingual models like large-v3.
    #[arg(short = 'l', long)]
    language: Option<String>,

    /// Skip transcription after recording (save WAV files only).
    #[arg(long, default_value_t = false)]
    no_stt: bool,

    /// Transcribe immediately without prompting (for automation / scripts).
    /// Skips the interactive "Transcribe now?" and language questions, always
    /// runs STT, and leaves the language auto-detected unless -l is given.
    #[arg(short = 'y', long, default_value_t = false)]
    yes: bool,

    /// Pre-download all models then exit. No recording is started.
    /// Add --hf-token to also cache the pyannote diarization model.
    #[arg(long, default_value_t = false)]
    setup: bool,

    /// List available Whisper models and diarizers, marking which are cached locally.
    #[arg(long, default_value_t = false)]
    list_models: bool,

    /// Print the on-disk locations nabu uses (binary, ~/.nabu/, model caches, sessions).
    #[arg(long, default_value_t = false)]
    r#where: bool,

    /// Run a diagnostic of macOS version, permissions, models, and disk space.
    #[arg(long, default_value_t = false)]
    doctor: bool,

    /// Auto-stop the recording after this duration. Accepts h/m/s units
    /// (e.g. 4h, 90m, 1h30m). Press [e] in the last 10 minutes to extend.
    #[arg(long, default_value = DEFAULT_MAX_DURATION)]
    max_duration: String,

    /// Amount to add each time the user presses [e] near the deadline.
    #[arg(long, default_value = DEFAULT_EXTEND_BY)]
    extend_by: String,
}

/// Poll the shared stop flag from an async context. Cheap (one atomic load
/// per 250 ms) and only used inside a `tokio::select!` against `ctrl_c()`.
async fn wait_for_stop(stop: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    loop {
        if stop.load(Ordering::Relaxed) { return; }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Ask on the TTY whether to transcribe now. Enter / empty input defaults to
/// YES; `y`/`yes` is yes; anything else is no. Only called when both stdin and
/// stdout are terminals (raw mode is already off — the display thread joined).
fn prompt_transcribe() -> bool {
    use std::io::Write;
    print!("Transcribe now? [Y/n] ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() { return false; }
    let ans = line.trim().to_ascii_lowercase();
    ans.is_empty() || ans == "y" || ans == "yes"
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    // Diagnostic commands — no bootstrap needed, no recording, no permission prompt.
    if args.list_models {
        models::print_list();
        return Ok(());
    }
    if args.r#where {
        return paths::print();
    }
    if args.doctor {
        // doctor::run is a diagnostic that returns its own exit code; this is
        // the one place where a non-zero exit is the result, not an error to
        // propagate via anyhow.
        #[allow(clippy::disallowed_methods)]
        std::process::exit(doctor::run(&args.model, &args.diarizer));
    }

    let (uv, t_dir) = bootstrap::resolve_env()?;

    if args.setup {
        return bootstrap::run_setup(&uv, &t_dir, &args.model, args.hf_token.as_deref());
    }

    if let Some(ref audio) = args.file {
        bootstrap::Bootstrap::check_ready(&t_dir)?;
        let lang = language::resolve(args.language.as_deref(), &args.model, args.yes);
        return bootstrap::run_transcription_file(
            &uv, &t_dir, audio,
            &args.model, &args.diarizer,
            args.hf_token.as_deref(),
            lang.as_deref(),
        );
    }

    if let Some(ref dir) = args.transcribe {
        bootstrap::Bootstrap::check_ready(&t_dir)?;
        // Resolve to an absolute path and confirm it exists / is a directory.
        let dir = fs::canonicalize(dir)
            .with_context(|| format!("--transcribe: cannot open session dir {}", dir.display()))?;
        if !dir.is_dir() {
            return Err(anyhow!("--transcribe: {} is not a directory", dir.display()));
        }
        let lang = language::resolve(args.language.as_deref(), &args.model, args.yes);
        // session_dir == final_dir: transcribe.py reads mic.wav + system.wav from
        // the folder and writes transcript.md back into it.
        bootstrap::run_transcription(
            &uv, &t_dir, &dir, &dir,
            &args.model, &args.diarizer, args.hf_token.as_deref(),
            lang.as_deref(),
        )?;
        let _ = std::process::Command::new("open").arg(&dir).spawn();
        return Ok(());
    }

    if !args.no_stt {
        bootstrap::Bootstrap::check_ready(&t_dir)?;
    }

    permissions::check_screen_recording().context("screen recording permission")?;

    let session = session::resolve(args.out)?;

    // ── Audio capture ─────────────────────────────────────────────────────────

    let (raw_mic_tx, raw_mic_rx) = unbounded::<RawChunk>();
    let (raw_sys_tx, raw_sys_rx) = unbounded::<RawChunk>();
    let (mic_wav_tx, mic_wav_rx) = bounded::<Vec<f32>>(1024);
    let (sys_wav_tx, sys_wav_rx) = bounded::<Vec<f32>>(1024);

    let mic_stream = mic::start(raw_mic_tx).map_err(|e| {
        // Open the exact Microphone pane so the user doesn't have to navigate.
        permissions::open_microphone_settings();
        anyhow!(
            "{e}\n\nMicrophone access denied or unavailable.\n  \
             → System Settings has been opened to Privacy & Security → Microphone.\n  \
             Toggle your terminal app ON, then restart nabu."
        )
    })?;
    let sys_stream = system::start(raw_sys_tx).context("start system audio")?;

    // One level meter per stream — updated on the resample worker thread (never
    // in the real-time capture callbacks) and read live by the display thread.
    let mic_meter = meter::Meter::new();
    let sys_meter = meter::Meter::new();

    resample::spawn_worker(raw_mic_rx, mic_stream.format.sample_rate, mic_stream.format.channels,
        mic_wav_tx, mic_meter.clone());
    resample::spawn_worker(raw_sys_rx, sys_stream.format.sample_rate, sys_stream.format.channels,
        sys_wav_tx, sys_meter.clone());

    // ── WAV writers ───────────────────────────────────────────────────────────

    let mic_path = session.mic_write();
    let mic_writer = thread::spawn(move || {
        if let Err(e) = writer::run(&mic_path, mic_wav_rx) {
            tracing::error!(error = ?e, "mic wav writer failed");
        }
    });

    let sys_path = session.sys_write();
    let sys_writer = thread::spawn(move || {
        if let Err(e) = writer::run(&sys_path, sys_wav_rx) {
            tracing::error!(error = ?e, "system wav writer failed");
        }
    });

    // ── Record until Ctrl-C or max-duration ───────────────────────────────────

    let max_duration = display::parse_duration(&args.max_duration)
        .map_err(|e| anyhow!("--max-duration: {e}"))?;
    let extend_by = display::parse_duration(&args.extend_by)
        .map_err(|e| anyhow!("--extend-by: {e}"))?;

    // Recording-consent reminder. Recording others without their knowledge is
    // illegal in many places — surface this before the live region starts.
    println!("  ⚠  Recording mic + system audio — make sure everyone involved knows they're being recorded.");
    println!();

    let handle = display::spawn(Instant::now(), max_duration, extend_by, mic_meter, sys_meter);
    let stop_for_ticker = handle.stop.clone();

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            stop_for_ticker.store(true, Ordering::Relaxed);
        }
        _ = wait_for_stop(stop_for_ticker.clone()) => {
            // Display thread tripped the flag (deadline hit, or Ctrl-C in raw mode).
        }
    }
    let _ = handle.thread.join();

    drop(mic_stream);
    drop(sys_stream);
    let _ = mic_writer.join();
    let _ = sys_writer.join();

    // ── Post-processing ───────────────────────────────────────────────────────

    if let Err(e) = writer::merge(&session.mic_write(), &session.sys_write(), &session.merged_write()) {
        tracing::warn!(error = ?e, "could not create merged.wav");
    }

    // Decide whether to run STT now. Non-interactive runs (piped stdin, or an
    // unattended max-duration auto-stop with no TTY) skip it so a saved session
    // isn't blocked on a heavy model run nobody is waiting for. --transcribe
    // <dir> can run it later.
    let want_stt = if args.no_stt {
        false
    } else if args.yes {
        true
    } else if io::stdin().is_terminal() && io::stdout().is_terminal() {
        prompt_transcribe()
    } else {
        false
    };

    if want_stt {
        let lang = language::resolve(args.language.as_deref(), &args.model, args.yes);
        tracing::info!("running transcription …");
        if let Err(e) = bootstrap::run_transcription(
            &uv, &t_dir, &session.write_dir, &session.final_dir,
            &args.model, &args.diarizer, args.hf_token.as_deref(),
            lang.as_deref(),
        ) {
            tracing::error!(error = ?e, "transcription failed — audio files are intact");
        }
    }

    // The original per-stream WAVs (mic.wav, system.wav) are always kept alongside
    // merged.wav and transcript.md — nothing deletes the sources anymore.

    if session.using_tmp() {
        fs::rename(&session.write_dir, &session.final_dir)
            .context("move session out of .tmp")?;
    }

    tracing::info!(session = %session.final_dir.display(), "audio saved");

    // When STT was skipped, tell the user how to run it later — mic.wav + system.wav
    // are still in the session, so the deferred command has its inputs.
    if !want_stt {
        println!("\nSession saved without a transcript.\nTo transcribe later:  nabu --transcribe {}",
            session.final_dir.display());
    }

    // Open the session folder regardless of whether STT ran.
    let _ = std::process::Command::new("open").arg(&session.final_dir).spawn();

    Ok(())
}
