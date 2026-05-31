use std::path::PathBuf;

/// Catalog entry for a Whisper model exposed via `--list-models`.
pub struct WhisperEntry {
    pub name: &'static str,
    pub size: &'static str,
    pub hf_repo: &'static str,
    pub notes: &'static str,
}

pub const WHISPER_MODELS: &[WhisperEntry] = &[
    WhisperEntry { name: "tiny.en",         size: "~75 MB",  hf_repo: "mlx-community/whisper-tiny.en-mlx",        notes: "Fastest, lowest accuracy. English only." },
    WhisperEntry { name: "base.en",         size: "~145 MB", hf_repo: "mlx-community/whisper-base.en-mlx",        notes: "Good balance. English only." },
    WhisperEntry { name: "small.en",        size: "~480 MB", hf_repo: "mlx-community/whisper-small.en-mlx",       notes: "Better accuracy. English only." },
    WhisperEntry { name: "medium.en",       size: "~1.5 GB", hf_repo: "mlx-community/whisper-medium.en-mlx",      notes: "High accuracy. English only." },
    WhisperEntry { name: "large-v3",        size: "~3 GB",   hf_repo: "mlx-community/whisper-large-v3-mlx",       notes: "Default. Best quality, multilingual." },
    WhisperEntry { name: "distil-large-v3", size: "~1.5 GB", hf_repo: "mlx-community/distil-whisper-large-v3",    notes: "Fast + high quality. Multilingual." },
];

pub struct DiarizerEntry {
    pub name: &'static str,
    pub size: &'static str,
    pub needs_token: bool,
    pub notes: &'static str,
}

pub const DIARIZERS: &[DiarizerEntry] = &[
    DiarizerEntry { name: "wespeaker", size: "~26 MB",  needs_token: false, notes: "Default. No account needed." },
    DiarizerEntry { name: "pyannote",  size: "~300 MB", needs_token: true,  notes: "Higher accuracy. Free HuggingFace token required." },
];

/// Map a model shorthand to its mlx-community HuggingFace repo, mirroring the
/// table in `transcribe/transcribe.py`.
pub fn mlx_repo(model: &str) -> String {
    WHISPER_MODELS
        .iter()
        .find(|m| m.name == model)
        .map(|m| m.hf_repo.to_string())
        .unwrap_or_else(|| format!("mlx-community/whisper-{model}-mlx"))
}

/// HuggingFace hub turns `org/repo` into `models--org--repo` on disk.
pub fn hf_cache_dir(repo: &str) -> Option<PathBuf> {
    let folder = format!("models--{}", repo.replace('/', "--"));
    dirs::home_dir().map(|h| h.join(".cache/huggingface/hub").join(folder))
}

pub fn whisper_cached(model: &str) -> bool {
    match hf_cache_dir(&mlx_repo(model)) {
        Some(p) => p.exists(),
        None => false,
    }
}

pub fn wespeaker_cached() -> bool {
    dirs::home_dir()
        .map(|h| h.join(".wespeaker/english/avg_model.pt").exists())
        .unwrap_or(false)
}

pub fn pyannote_cached() -> bool {
    let repos = [
        "pyannote/speaker-diarization-community-1",
        "pyannote/segmentation-3.0",
    ];
    repos.iter().all(|r| hf_cache_dir(r).map(|p| p.exists()).unwrap_or(false))
}

pub fn print_list() {
    println!("Whisper models");
    println!("  {:<18} {:<8}  {}", "name", "size", "status");
    println!("  {:-<18} {:-<8}  {:-<40}", "", "", "");
    for m in WHISPER_MODELS {
        let cached = if whisper_cached(m.name) { "cached" } else { "not cached" };
        println!("  {:<18} {:<8}  [{}] — {}", m.name, m.size, cached, m.notes);
    }

    println!();
    println!("Diarizers");
    println!("  {:<10} {:<8}  {}", "name", "size", "status");
    println!("  {:-<10} {:-<8}  {:-<40}", "", "", "");
    for d in DIARIZERS {
        let cached = match d.name {
            "wespeaker" => wespeaker_cached(),
            "pyannote" => pyannote_cached(),
            _ => false,
        };
        let token = if d.needs_token { " (needs HF_TOKEN)" } else { "" };
        let status = if cached { "cached" } else { "not cached" };
        println!("  {:<10} {:<8}  [{}] — {}{}", d.name, d.size, status, d.notes, token);
    }

    println!();
    println!("Run 'nabu --setup --model <NAME>' to pre-download a Whisper model.");
    println!("Run 'nabu --setup --hf-token <TOKEN>' to also cache the pyannote diarizer.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_models_map_to_explicit_repos() {
        assert_eq!(mlx_repo("large-v3"),        "mlx-community/whisper-large-v3-mlx");
        assert_eq!(mlx_repo("distil-large-v3"), "mlx-community/distil-whisper-large-v3");
        assert_eq!(mlx_repo("tiny.en"),         "mlx-community/whisper-tiny.en-mlx");
    }

    #[test]
    fn unknown_models_fall_back_to_pattern() {
        assert_eq!(mlx_repo("turbo"), "mlx-community/whisper-turbo-mlx");
    }

    #[test]
    fn hf_cache_dir_escapes_slash() {
        let p = hf_cache_dir("pyannote/segmentation-3.0").unwrap();
        assert!(p.ends_with("models--pyannote--segmentation-3.0"));
    }
}
