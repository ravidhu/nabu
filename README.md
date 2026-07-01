# nabu

> Babylonian god of writing, scribes, and wisdom.

A macOS CLI that records **mic + system audio** simultaneously, saves separate WAV files, then transcribes with **mlx-whisper** (Metal GPU) and labels speakers — no account required.

**Requirements:** macOS 13+, Apple Silicon (M1/M2/M3/M4).

> **New here?** See the [step-by-step setup guide](docs/setup-guide.md) — no technical background needed.
> **Developer?** See the [architecture doc](docs/architecture.md) for how the code fits together.

---

## Quick start

### One-line install (Apple Silicon)

**Prerequisites:** macOS 13+, Apple Silicon (M1/M2/M3/M4), internet connection, `sudo` access. No Python, Rust, or other tools required — the binary is self-contained.

Paste this in Terminal — it installs the binary, downloads AI models, and creates a **nabu.app** launcher you can put in your Dock:

```bash
curl -fsSL https://raw.githubusercontent.com/ravidhu/nabu/main/install.sh | bash
```

After install, double-click **nabu.app** in `~/Applications` to start recording. Press **Ctrl-C** to stop — transcript appears in `~/nabu_data/`.

### Manual install

```bash
# 1. Download and install
sudo curl -Lo /usr/local/bin/nabu \
  https://github.com/ravidhu/nabu/releases/latest/download/nabu-aarch64-apple-darwin
sudo chmod +x /usr/local/bin/nabu

# 2. Remove macOS quarantine flag (set automatically on internet downloads)
sudo xattr -d com.apple.quarantine /usr/local/bin/nabu

# 3. Download AI models (one-time)
nabu --setup

# 4. Grant macOS permissions when prompted, then record
nabu
# Press Ctrl-C to stop — transcript appears in ~/nabu_data/
```

Or clone the repo, build, and copy locally — no quarantine flag needed:
```bash
make build
sudo cp bin/nabu-aarch64-apple-darwin /usr/local/bin/nabu
```

### Build from source

**Prerequisites:** macOS 13+, Apple Silicon, [`rustup`](https://rustup.rs) (≥ 1.75), [`uv`](https://docs.astral.sh/uv/getting-started/installation/).

```bash
make dev-setup   # install Rust target, sync Python deps, download AI models
make build       # compile release binary → bin/nabu-aarch64-apple-darwin
```

---

## Setup

### 1. Get the binary

**Option A — pre-built** (Apple Silicon only):

Download `nabu-aarch64-apple-darwin` from the [latest release](https://github.com/ravidhu/nabu/releases/latest), place it in your PATH, then remove the macOS quarantine flag:

```bash
curl -fLo nabu-aarch64-apple-darwin \
  https://github.com/ravidhu/nabu/releases/latest/download/nabu-aarch64-apple-darwin
chmod +x nabu-aarch64-apple-darwin
xattr -d com.apple.quarantine nabu-aarch64-apple-darwin
sudo mv nabu-aarch64-apple-darwin /usr/local/bin/nabu
```

**Option B — build from source:**

```bash
make dev-setup   # first time: install Rust target, sync Python deps, download AI models
make build       # compile → bin/nabu-aarch64-apple-darwin
```

Requires: Rust toolchain (≥ 1.75), `uv`, macOS 13+.

The binary is self-contained — it embeds the Python runtime manager (`uv`) and all transcription scripts. No separate Python installation needed.

### 2. Download AI models

**Default — transcription + speaker labels:**
```bash
nabu --setup
```

Downloads the Whisper `large-v3` transcription model (~3 GB) and the wespeaker diarization model (~26 MB). Everything runs fully offline after this point.

**Advanced — higher-accuracy speaker identification (free HuggingFace account):**

Accept model terms once at:
- [pyannote/speaker-diarization-community-1](https://huggingface.co/pyannote/speaker-diarization-community-1) → Agree and access repository
- [pyannote/segmentation-3.0](https://huggingface.co/pyannote/segmentation-3.0) → Agree and access repository

Then run:
```bash
nabu --setup --hf-token hf_your_token
```

After setup, all runs are fully offline. The token is only needed for first-time model download.

### 3. macOS permissions

- **Microphone** — macOS prompts automatically on first run. Click Allow.
- **Screen Recording** — add your terminal app manually:
  > System Settings → Privacy & Security → Screen Recording → add your terminal

Despite the name, nabu records audio only — it captures no screen content.

### 4. Make it available system-wide (optional)

```bash
sudo cp target/release/nabu /usr/local/bin/nabu
```

### Updating

Re-run the one-line installer — it fetches the latest release binary and replaces the one in place:

```bash
curl -fsSL https://raw.githubusercontent.com/ravidhu/nabu/main/install.sh | bash
```

Or update manually: download the latest `nabu-aarch64-apple-darwin`, then `chmod +x`, remove the quarantine flag, and move it over `/usr/local/bin/nabu` (same steps as [Manual install](#manual-install)).

The binary is self-contained, so updating is just the binary. On the first run after an update it re-extracts its bundled Python (you'll see `extracting Python scripts …`); your `~/.nabu/.venv` and downloaded models are **preserved**, so there's nothing to re-download and no need to re-run `nabu --setup`. Your recordings in `~/nabu_data/` are never touched.

If anything looks stale after an update, force a clean reinstall of the internals (recordings are left alone):

```bash
rm -rf ~/.nabu && nabu --setup
```

### Uninstall

```bash
curl -fsSL https://raw.githubusercontent.com/ravidhu/nabu/main/uninstall.sh | bash
# or, from a checkout:
make uninstall
```

Removes the binary, `nabu.app`, `~/.nabu/`, and `~/.wespeaker/`. Asks separately before deleting your recordings in `~/nabu_data/` or the shared HuggingFace model cache. macOS Privacy permissions stay — remove those manually in System Settings.

---

## Output

Each recording creates a timestamped session folder:

```
~/nabu_data/
└── 2026_05_27_14_32/
    ├── mic.wav         ← 16 kHz mono — your microphone
    ├── system.wav      ← 16 kHz mono — system audio (calls, speakers)
    ├── merged.wav      ← 16 kHz stereo — mic=L, system=R
    └── transcript.md   ← timestamped, speaker-labelled transcript
```

Files are written to `~/nabu_data/.tmp/<session>/` during recording and atomically moved to the final path on clean exit (Ctrl-C). A folder remaining in `.tmp/` means the process was killed mid-session — audio files are still intact there.

### Transcript format

With wespeaker diarization (default):
```
[00:00:03 → 00:00:04] [You]       Hey, can you hear me?
[00:00:05 → 00:00:07] [Speaker 1] Yes loud and clear.
[00:00:09 → 00:00:11] [You]       Great, let's get started.
[00:00:14 → 00:00:17] [Speaker 2] Joining now, sorry for the delay.
```

Without diarization (`--no-stt` or diarization disabled):
```
[00:00:03 → 00:00:04] [You]     Hey, can you hear me?
[00:00:05 → 00:00:07] [Remote]  Yes loud and clear.
```

### Transcript quality

Your microphone and the system audio are transcribed **separately** (never the
mixed `merged.wav`), so each pass sees one clean source — that's what gives exact
`You` vs `Speaker N` attribution. Two automatic clean-up passes handle the two
things that separation can't:

- **Hallucination filter** — Whisper sometimes invents phantom text on silence
  or noise (a stray "Thank you.", subtitle credits, a phrase repeated over and
  over). Those segments are detected from the model's own confidence signals and
  dropped before they reach the transcript.
- **Echo dedup** — if you record **without headphones**, your mic re-captures the
  remote party coming out of the speakers, so their words would otherwise appear
  twice (once as `You`, once as `Speaker N`). nabu detects these overlapping
  near-identical segments and keeps only the clean system-audio copy.

> **Tip:** wearing headphones removes speaker bleed at the source and gives the
> cleanest transcripts — the echo dedup is there for when you can't.

---

## CLI reference

```
nabu [OPTIONS]
```

| Flag | Type | Default | Description |
|---|---|---|---|
| `--setup` | flag | off | Download AI models and exit. No recording started. |
| `--out <DIR>` | path | `~/nabu_data/YYYY_MM_DD_HH_mm/` | Override session output directory. |
| `--model <NAME>` | string | `large-v3` | Whisper model for transcription. |
| `--diarizer <NAME>` | string | `wespeaker` | Speaker diarization backend: `wespeaker` or `pyannote`. |
| `--hf-token <TOKEN>` | string | `$HF_TOKEN` | HuggingFace token — only needed for `--diarizer pyannote`. |
| `--no-stt` | flag | off | Skip transcription entirely. Save WAV files only. |
| `-y`, `--yes` | flag | off | Transcribe immediately without the "Transcribe now?" prompt (automation). |
| `--transcribe <DIR>` | path | — | Transcribe an existing saved session directory and exit. No recording started. |
| `--max-duration <DUR>` | string | `4h` | Auto-stop after this duration. Accepts `h`/`m`/`s` (e.g. `90m`, `2h15m`). |
| `--extend-by <DUR>` | string | `1h` | Amount added each time you press `[e]` near the deadline. |
| `--list-models` | flag | — | List Whisper models + diarizers, marking which are cached. |
| `--where` | flag | — | Print on-disk paths nabu uses (binary, caches, sessions). |
| `--doctor` | flag | — | Run permission, model, and disk-space checks. |
| `-h`, `--help` | flag | — | Print help. |
| `-V`, `--version` | flag | — | Print version. |

### Long recordings

By default a recording stops after **4 hours** so a forgotten Ctrl-C cannot fill your disk overnight. In the last 10 minutes the timer asks:

```
⚠  Recording will auto-stop in 10:00 — press [e] to extend +1h, Ctrl-C to stop now
```

Press `e` to keep going for another hour. Use `--max-duration 8h` (or any `h`/`m`/`s` value) to change the default; use `--extend-by 30m` to change the extension increment.

### Transcribing later

When a recording ends interactively, nabu asks:

```
Transcribe now? [Y/n]
```

Press Enter (or `y`) to transcribe immediately; press `n` to save the audio
now and transcribe later. Either way the session (mic.wav + system.wav) is
saved. To run transcription afterwards:

```bash
nabu --transcribe ~/nabu_data/2026_07_01_14_30
```

Non-interactive runs — a piped stdin, or an unattended `--max-duration`
auto-stop with no terminal attached — **skip transcription by default** and
print the `--transcribe` command to run later. Use `-y`/`--yes` to force
transcription without prompting (useful in scripts).

### Whisper models

| Name | Size | Notes |
|---|---|---|
| `tiny.en` | ~75 MB | Fastest, lowest accuracy. English only. |
| `base.en` | ~145 MB | Good balance. English only. |
| `small.en` | ~480 MB | Better accuracy. English only. |
| `medium.en` | ~1.5 GB | High accuracy. English only. |
| `large-v3` | ~3 GB | **Default.** Best quality, multilingual (EN + FR + more). |
| `distil-large-v3` | ~1.5 GB | Fast + high quality. Multilingual (EN + FR + more). |

Use `large-v3` or `distil-large-v3` for French or other languages. All models run on Metal GPU via Apple MLX.

Models other than `base.en` are downloaded automatically on first use — no separate setup step needed. To pre-download for offline use:

```bash
nabu --setup --model distil-large-v3
```

### Diarizer comparison

| | `wespeaker` (default) | `pyannote` |
|---|---|---|
| Account needed | No | Free HuggingFace account |
| Setup | `nabu --setup` | `nabu --setup --hf-token TOKEN` |
| Model size | ~26 MB | ~300 MB |
| Languages | EN + FR | EN + FR |
| Accuracy | Good | Higher |

---

## Usage examples

**Record and transcribe (default):**
```bash
nabu
```

**Use the higher-accuracy pyannote diarizer (requires prior setup with token):**
```bash
nabu --diarizer pyannote
```

**Better transcription accuracy:**
```bash
nabu --model small.en
```

**French or multilingual recording:**
```bash
nabu --model large-v3
```

**Audio only, skip transcription:**
```bash
nabu --no-stt
```

**Skip the prompt and always transcribe (automation):**
```bash
nabu -y
```

**Transcribe a saved session later:**
```bash
nabu --transcribe ~/nabu_data/2026_07_01_14_30
```

**Custom output directory:**
```bash
nabu --out ~/Desktop/meeting-2026-05-27
```

**Verbose logs:**
```bash
RUST_LOG=info nabu
```

---

## Transcription pipeline

```
mic.wav  ──► mlx-whisper (Metal) ──► [You] segments
                                          ↓
                                   merge by timestamp ──► transcript.md
                                          ↑
sys.wav  ──► mlx-whisper (Metal) ──► raw segments
         ──► wespeaker or pyannote ─► speaker segments
                                   └─► assign → [Speaker 1], [Speaker 2]…
```

| Step | Library | Acceleration |
|---|---|---|
| Transcription | mlx-whisper | Metal (MLX + Neural Engine) |
| Diarization (default) | wespeaker | CPU / ONNX |
| Diarization (advanced) | pyannote community-1 | MPS (Metal via PyTorch) |

---

## Project structure

```
nabu/
├── src/
│   ├── main.rs           ← CLI args, thread wiring, Ctrl-C, post-processing
│   ├── bootstrap.rs      ← self-contained binary: uv extraction + Python invocation
│   ├── session.rs        ← session path resolution (tmp → final rename)
│   ├── display.rs        ← live recording timer + per-stream input-level bars
│   ├── meter.rs          ← lock-free peak meter shared worker → display
│   ├── resample.rs       ← rubato FFT resampler → 16 kHz mono
│   ├── writer.rs         ← hound WAV writer + stereo merge
│   ├── permissions.rs    ← macOS screen recording permission check
│   └── capture/
│       ├── mic.rs        ← cpal microphone capture
│       └── system.rs     ← ScreenCaptureKit system audio capture
├── transcribe/
│   ├── transcribe.py     ← entry shim → stt.cli:main
│   ├── stt/              ← package: models · asr · diarize · cleanup · transcript · download · cli
│   ├── pyproject.toml
│   └── uv.lock
├── build.rs              ← embeds uv binary + transcribe/ into the Rust binary
├── Makefile              ← dev-setup / build / run / clean targets
├── docs/
│   └── setup-guide.md    ← non-technical walkthrough
└── Cargo.toml
```

---

## Inspecting output

```bash
# Play audio
afplay ~/nabu_data/2026_05_27_14_32/mic.wav
afplay ~/nabu_data/2026_05_27_14_32/system.wav

# Read transcript
cat ~/nabu_data/2026_05_27_14_32/transcript.md
```

---

## Developer notes

### Makefile

| Target | Description |
|---|---|
| `make dev-setup` | Install the `aarch64-apple-darwin` Rust target, sync the Python venv (`transcribe/`), and download AI models. Run once after cloning. |
| `make build` | Compile a release binary and copy it to `bin/nabu-aarch64-apple-darwin`. |
| `make run` | Run nabu directly via `cargo run` (debug build, no cross-compile). Fast iteration during development. |
| `make clean` | Wipe the Cargo build cache. |

### Dev mode (skip binary embedding)

Set `NABU_TRANSCRIBE_DIR` to the source `transcribe/` directory to use the source tree directly without rebuilding:

```bash
export NABU_TRANSCRIBE_DIR=$(pwd)/transcribe
cargo run -- --setup
cargo run
```

This skips uv/script extraction and uses the system `uv` from PATH.

### Logging

```bash
RUST_LOG=info nabu        # info-level logs
RUST_LOG=debug nabu       # verbose
RUST_LOG=nabu=trace nabu  # nabu only, trace level
```

---

## Privacy

nabu processes everything on your device. Nothing is uploaded anywhere.

- **Recordings** are saved to `~/nabu_data/` and never leave your Mac.
- **AI models** are cached in `~/.nabu/` after `nabu --setup`. All inference runs locally on Metal — no API calls.
- **Network access** happens only during `nabu --setup`, when models are downloaded from HuggingFace. After that, nabu works fully offline.
- **HuggingFace token** (if you use the pyannote option) is passed on the command line or via `HF_TOKEN`. nabu does not store it anywhere.
- **No telemetry, no crash reporting, no analytics** of any kind.

---

## Known limitations

- macOS only — system audio capture uses ScreenCaptureKit (macOS 13+).
- Apple Silicon only — there are no plans to support Intel Macs.
- pyannote diarization may struggle with 5+ simultaneous speakers or heavy background noise.
- English-only Whisper models (`*.en`) will not transcribe other languages accurately — use `large-v3` instead.

---

## License

[MIT](LICENSE) © 2026 Ravidhu Dissanayake

---

Questions? → [GitHub Discussions](https://github.com/ravidhu/nabu/discussions) ·
Bugs? → [GitHub Issues](https://github.com/ravidhu/nabu/issues) ·
Security? → see [SECURITY.md](SECURITY.md)
