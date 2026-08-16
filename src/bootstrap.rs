use std::{
    fs,
    path::{Path, PathBuf},
    process::Command as StdCommand,
};

use anyhow::{bail, Context, Result};

// Embedded at compile time by build.rs.
static UV_BINARY: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/uv"));
static TRANSCRIBE_ARCHIVE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/transcribe.tar.gz"));

// Version tag — changes whenever Cargo.toml version bumps, triggering a re-extract.
const NABU_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct Bootstrap {
    pub uv: PathBuf,
    pub transcribe_dir: PathBuf,
}

impl Bootstrap {
    /// Ensure ~/.nabu/ is set up and return paths for uv + transcribe/.
    ///
    /// Returns None when NABU_TRANSCRIBE_DIR is set — dev mode uses the
    /// source tree directly and assumes system uv is in PATH.
    pub fn ensure() -> Result<Option<Self>> {
        if std::env::var("NABU_TRANSCRIBE_DIR").is_ok() {
            return Ok(None);
        }

        let nabu_home = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("cannot determine home directory"))?
            .join(".nabu");

        let uv = nabu_home.join("bin").join("uv");
        let transcribe_dir = nabu_home.join("transcribe");

        extract_uv(&uv)?;
        extract_transcribe(&nabu_home, &transcribe_dir)?;

        Ok(Some(Bootstrap { uv, transcribe_dir }))
    }

    /// Return an error with a friendly message if the Python venv isn't ready.
    /// Call this before recording when transcription will be needed.
    pub fn check_ready(transcribe_dir: &Path) -> Result<()> {
        if !transcribe_dir.join(".venv").exists() {
            bail!(
                "nabu is not set up yet — run 'nabu --setup' first.\n\
                 This installs Python dependencies and downloads the speech models.\n\
                 Use 'nabu --setup --hf-token <TOKEN>' to also enable speaker diarization."
            );
        }
        Ok(())
    }
}

/// Resolve the uv executable and transcribe/ directory.
///
/// In release mode (no `NABU_TRANSCRIBE_DIR` env var), uses the bootstrapped
/// `~/.nabu/` paths. In dev mode (env var set), falls back to the source tree
/// with system `uv`.
pub fn resolve_env() -> Result<(PathBuf, PathBuf)> {
    match Bootstrap::ensure()? {
        Some(bootstrap) => Ok((bootstrap.uv, bootstrap.transcribe_dir)),
        None => {
            let t_dir = std::env::var("NABU_TRANSCRIBE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(env!("NABU_TRANSCRIBE_DIR")));
            Ok((PathBuf::from("uv"), t_dir))
        }
    }
}

/// Run `nabu --setup` via the Python transcription script.
pub fn run_setup(uv: &Path, t_dir: &Path, model: &str, hf_token: Option<&str>) -> Result<()> {
    let mut cmd = StdCommand::new(uv);
    cmd.arg("run")
        .arg("--directory")
        .arg(t_dir)
        .arg("python")
        .arg("transcribe.py")
        .arg("--setup")
        .arg("--model")
        .arg(model);
    if let Some(token) = hf_token {
        cmd.arg("--hf-token").arg(token);
    }
    let status = cmd.status().context("failed to launch uv")?;
    if !status.success() {
        anyhow::bail!("setup script exited with status {status}");
    }
    Ok(())
}

/// Run transcription on a recorded session via the Python script.
pub fn run_transcription(
    uv: &Path,
    t_dir: &Path,
    session_dir: &Path,
    final_dir: &Path,
    model: &str,
    diarizer: &str,
    hf_token: Option<&str>,
    language: Option<&str>,
) -> Result<()> {
    let mut cmd = StdCommand::new(uv);
    cmd.arg("run")
        .arg("--directory")
        .arg(t_dir)
        .arg("python")
        .arg("transcribe.py")
        .arg(session_dir)
        .arg("--final-dir")
        .arg(final_dir)
        .arg("--model")
        .arg(model)
        .arg("--diarizer")
        .arg(diarizer);
    if let Some(token) = hf_token {
        cmd.arg("--hf-token").arg(token);
    }
    if let Some(lang) = language {
        cmd.arg("--language").arg(lang);
    }
    let status = cmd.status().context("failed to launch uv")?;
    if !status.success() {
        anyhow::bail!("transcription script exited with status {status}");
    }
    Ok(())
}

/// Transcribe a single existing audio file via the Python script.
pub fn run_transcription_file(
    uv: &Path,
    t_dir: &Path,
    audio: &Path,
    model: &str,
    diarizer: &str,
    hf_token: Option<&str>,
    language: Option<&str>,
) -> Result<()> {
    let mut cmd = StdCommand::new(uv);
    cmd.arg("run")
        .arg("--directory")
        .arg(t_dir)
        .arg("python")
        .arg("transcribe.py")
        .arg("--file")
        .arg(audio)
        .arg("--model")
        .arg(model)
        .arg("--diarizer")
        .arg(diarizer);
    if let Some(token) = hf_token {
        cmd.arg("--hf-token").arg(token);
    }
    if let Some(lang) = language {
        cmd.arg("--language").arg(lang);
    }
    let status = cmd.status().context("failed to launch uv")?;
    if !status.success() {
        anyhow::bail!("transcription script exited with status {status}");
    }
    Ok(())
}

fn extract_uv(uv: &PathBuf) -> Result<()> {
    if uv.exists() {
        return Ok(());
    }
    fs::create_dir_all(uv.parent().unwrap()).context("create ~/.nabu/bin")?;
    eprintln!("[nabu] first-run setup: extracting uv …");
    fs::write(uv, UV_BINARY).context("write uv binary")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(uv, fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

fn extract_transcribe(nabu_home: &PathBuf, transcribe_dir: &PathBuf) -> Result<()> {
    let version_file = nabu_home.join(".version");
    let installed = fs::read_to_string(&version_file).unwrap_or_default();

    if installed.trim() == NABU_VERSION && transcribe_dir.exists() {
        return Ok(());
    }

    eprintln!("[nabu] first-run setup: extracting Python scripts …");
    fs::create_dir_all(transcribe_dir).context("create transcribe dir")?;

    // Remove old scripts but preserve .venv to avoid a full re-sync on minor updates.
    if let Ok(entries) = fs::read_dir(transcribe_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().map_or(true, |name| name != ".venv") {
                let _ = fs::remove_file(&path);
                let _ = fs::remove_dir_all(&path);
            }
        }
    }

    let tmp = nabu_home.join("transcribe.tar.gz");
    fs::write(&tmp, TRANSCRIBE_ARCHIVE).context("write transcribe archive")?;
    let ok = StdCommand::new("tar")
        .args([
            "-xzf",
            tmp.to_str().unwrap(),
            "-C",
            transcribe_dir.to_str().unwrap(),
        ])
        .status()
        .context("tar not found")?
        .success();
    fs::remove_file(&tmp).ok();

    if !ok {
        bail!("failed to extract Python scripts");
    }
    fs::write(&version_file, NABU_VERSION)?;
    Ok(())
}
