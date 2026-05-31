# nabu — Architecture

This document explains how nabu works internally, how the pieces fit together, and where to look when changing something.

---

## What nabu does in one paragraph

nabu is a Rust binary that simultaneously captures microphone audio (via `cpal`) and system audio (via ScreenCaptureKit), resamples both streams to 16 kHz mono, and writes them to WAV files. On Ctrl-C it hands those files to an embedded Python script (`transcribe.py`) that runs mlx-whisper for speech-to-text and a speaker diarization model to label speakers. The final output is a markdown transcript.

---

## Files on disk

```
~/.nabu/
├── bin/uv                  ← uv runtime manager (extracted from binary on first run)
├── transcribe/             ← Python scripts (extracted from binary on first run)
│   ├── transcribe.py
│   ├── pyproject.toml
│   ├── uv.lock
│   └── .venv/              ← Python venv (created by nabu --setup)
└── .version                ← installed version tag; triggers re-extract on upgrade

~/nabu_data/
└── 2026_05_27_14_32/       ← one folder per recording session
    ├── mic.wav             ← 16 kHz mono PCM — microphone
    ├── system.wav          ← 16 kHz mono PCM — system audio
    ├── merged.wav          ← 16 kHz stereo PCM — mic=L, system=R
    └── transcript.md       ← timestamped, speaker-labelled transcript
```

During recording, files are written to `~/nabu_data/.tmp/<session>/`. After Ctrl-C, the full post-processing pipeline runs in `.tmp/`: merge, transcription, then deletion of the source WAVs. Only once all of that completes is the folder atomically renamed to its final path. A folder left in `.tmp/` means the process was killed mid-session — `mic.wav` and `system.wav` will still be there.

### Why two separate directories?

The two directories serve fundamentally different purposes and that separation is intentional.

`~/.nabu/` holds **application internals**: the `uv` runtime, Python scripts, AI models, and the Python venv. None of this is user data — it is all re-creatable from scratch. If something breaks, `rm -rf ~/.nabu && nabu --setup` performs a clean reinstall without touching any recordings. The dot prefix follows the Unix convention for directories that users do not interact with directly (like `~/.cargo` or `~/.npm`) — they stay hidden in Finder and are excluded from `ls` by default.

`~/nabu_data/` holds **your recordings and transcripts**. These are irreplaceable. Keeping them in a visible, undotted directory makes them easy to find in Finder, Time Machine, and `ls`. They are deliberately kept out of `~/.nabu/` so there is no risk of a clean reinstall accidentally deleting them.

The rule of thumb: `~/.nabu/` can always be deleted and regenerated; `~/nabu_data/` should be backed up.

---

## Execution flow

```
nabu (CLI)
│
├─ bootstrap::resolve_env()         extract uv + transcribe/ to ~/.nabu/ on first run
│
├─ [--setup flag]
│   └─ bootstrap::run_setup()       run transcribe.py --setup (download models)
│
├─ bootstrap::check_ready()         bail early if .venv missing
├─ permissions::check_screen_recording()  request permission if not yet granted
├─ session::resolve()               create ~/nabu_data/.tmp/<timestamp>/
│
├─ CAPTURE PHASE (concurrent threads)
│   ├─ mic::start()                 cpal → raw_mic_tx
│   ├─ system::start()              ScreenCaptureKit → raw_sys_tx
│   ├─ resample::spawn_worker()     raw_mic_rx → 16 kHz mono → mic_wav_tx
│   ├─ resample::spawn_worker()     raw_sys_rx → 16 kHz mono → sys_wav_tx
│   ├─ writer::run()  [thread]      mic_wav_rx → mic.wav
│   ├─ writer::run()  [thread]      sys_wav_rx → system.wav
│   └─ display::spawn()  [thread]   live timer on stdout
│
├─ tokio::signal::ctrl_c()          block until Ctrl-C
│
├─ POST-PROCESSING (sequential, all in .tmp/)
│   ├─ writer::merge()              mic.wav + system.wav → merged.wav
│   ├─ bootstrap::run_transcription()  transcribe.py → transcript.md
│   ├─ fs::remove_file(mic.wav)     } only if both merge and transcription succeeded;
│   ├─ fs::remove_file(system.wav)  } merged.wav + transcript.md make them redundant
│   └─ fs::rename()                 .tmp/<session>/ → nabu_data/<session>/
    └─ transcribe.py
        ├─ transcribe(mic.wav)      mlx-whisper → [{start, end, text}] labelled [You]
        ├─ transcribe(system.wav)   mlx-whisper → [{start, end, text}]
        ├─ diarize_wespeaker() or diarize_pyannote()  → [{start, end, speaker}]
        ├─ assign_speakers()        match Whisper segments to diarization turns
        └─ merge_and_write()        sort all segments by time → transcript.md
```

---

## Concurrency model

```
main thread (tokio async)
│
│  raw audio chunks (interleaved float32, native sample rate)
│
├── mic::start()       → [unbounded channel] → resample worker thread
│                                               └── [bounded channel] → writer thread → mic.wav
│
└── system::start()    → [unbounded channel] → resample worker thread
                                                └── [bounded channel] → writer thread → system.wav
```

**Why unbounded for raw → resample, bounded for resample → writer?**
The capture callbacks (cpal, ScreenCaptureKit) must never block — they run on real-time audio threads. Unbounded channels absorb bursts without stalling the callback. The bounded channel downstream applies backpressure: if the writer falls behind, the resampler slows down, which is fine because the resampler thread is not real-time.

The display timer and both WAV writers each run on their own `std::thread`. Transcription runs after all channels are closed and writers have joined, so it always sees complete WAV files.

---

## Module reference

### `src/main.rs`

Entry point. Parses CLI args with `clap`, wires all the channels and threads together, waits for Ctrl-C, then drives post-processing. Contains no audio logic itself — it is purely coordination.

### `src/bootstrap.rs`

Manages the self-contained binary trick. On first run, `ensure()` extracts the embedded `uv` binary and `transcribe.tar.gz` to `~/.nabu/`. A `.version` file tracks whether the extracted scripts match the current binary version; on mismatch the scripts are re-extracted (but `.venv` is preserved to avoid a full Python re-install on every update). `run_setup()` and `run_transcription()` invoke `uv run python transcribe.py` with the appropriate arguments.

### `src/capture/mic.rs`

Opens the default input device via `cpal`. Handles `F32`, `I16`, and `U16` sample formats, normalising everything to `f32` before sending chunks over the channel.

### `src/capture/system.rs`

Captures system audio using ScreenCaptureKit. The API delivers **non-interleaved** float32 at 48 kHz (one `AudioBuffer` per channel). `AudioDelegate::did_output_sample_buffer` manually interleaves the channels (L R L R …) before sending the chunk, so the downstream resampler sees the same format as mic audio.

### `src/resample.rs`

`MonoResampler` wraps rubato's `FftFixedIn` resampler. It downmixes multichannel interleaved audio to mono (simple average), then resamples to 16 kHz in 1024-sample blocks. `spawn_worker()` runs this in a dedicated thread, pulling `RawChunk`s from the capture channel and pushing `Vec<f32>` blocks to the writer channel.

### `src/writer.rs`

`run()` receives 16 kHz mono `f32` samples and writes them to a 16-bit PCM WAV via `hound`. `merge()` reads `mic.wav` and `system.wav` and interleaves them into a stereo WAV (mic=L, system=R), padding the shorter file with silence.

### `src/session.rs`

`resolve()` determines the session directory. With `--out`, it uses that path directly. Without, it creates a timestamped folder under `~/nabu_data/.tmp/` for writing, and records the final path for the post-recording rename.

### `src/display.rs`

Spawns a thread that prints a blinking recording timer to stdout every 600 ms using `\r` to overwrite in place. Stopped via an `AtomicBool`.

### `src/permissions.rs`

Calls the CoreGraphics private API (`CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess`) to check and request Screen Recording permission before capture starts. On non-macOS builds it compiles to a no-op.

### `build.rs`

Runs at compile time. Bundles `transcribe/` (excluding `.venv`, `__pycache__`, `*.pyc`) into `transcribe.tar.gz` using `tar`, then downloads the `uv` binary from GitHub releases and caches it in `OUT_DIR`. Both are baked into the binary via `include_bytes!` in `bootstrap.rs`. Triggers a rebuild only when Python sources, `pyproject.toml`, `uv.lock`, or `build.rs` itself change.

---

## Python side: `transcribe/transcribe.py`

The script has two modes, selected by the `--setup` flag:

**Setup mode** (`nabu --setup`): calls `huggingface_hub.snapshot_download` for the mlx-whisper model and `wespeaker.load_model` to trigger its model download. Optionally downloads the pyannote model if `--hf-token` is provided.

**Transcription mode** (`nabu` normal run): runs mlx-whisper on both `mic.wav` and `system.wav` in parallel (sequential calls but fast on Metal), then runs the chosen diarizer on `system.wav`. `assign_speakers()` labels each Whisper segment by finding the diarization turn with the maximum time overlap. Mic segments are always labelled `[You]`. System segments fall back to `[Remote]` if diarization fails. All segments are sorted by start time and written to `transcript.md`.

**HF_HUB_OFFLINE trick**: pyannote's `Pipeline.from_pretrained` ignores `local_files_only=True` due to an internal bug — the `_pipeline_from_cache` helper temporarily sets `HF_HUB_OFFLINE=1` in the environment instead, which forces fully offline loading.

---

## Dev mode

Set `NABU_TRANSCRIBE_DIR` to the source `transcribe/` directory. Bootstrap will skip extraction and use the source tree directly with system `uv` from PATH:

```bash
export NABU_TRANSCRIBE_DIR=$(pwd)/transcribe
cargo run -- --setup
cargo run
```

This avoids a full rebuild when iterating on the Python side.

---

## Adding a new diarizer

1. Add a `diarize_<name>(audio_path, ...)` function in `transcribe.py` returning `list[{"start": float, "end": float, "speaker": str}]`.
2. Add `<name>` to the `--diarizer` choices in the argparse block.
3. Wire it into the `if args.diarizer == ...` block in `main()`.
4. Update the `--diarizer` help text in `src/main.rs` and the diarizer table in `README.md`.
