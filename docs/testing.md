# Testing

nabu has two test suites — one per language. They are fast, hermetic, and require neither macOS permissions nor downloaded models.

## Run everything

```bash
make test
```

That target runs:

```
cargo test --bin nabu
uv run --directory transcribe pytest
```

## Rust — `cargo test --bin nabu`

Unit tests live in-module under `#[cfg(test)] mod tests` blocks. Coverage:

| File | What |
|---|---|
| `src/display.rs` | `format_hms`, `parse_duration` |
| `src/session.rs` | `resolve(Some(out))` skips `.tmp/` and creates the expected files |
| `src/writer.rs` | `merge()` interleaves stereo and pads the shorter channel with silence; `to_i16` clamps |
| `src/resample.rs` | 48 kHz → 16 kHz length ratio is within 1%; stereo → mono downmix sums to zero |
| `src/models.rs` | known shorthands map to the right `mlx-community/` repos; unknown names fall back |
| `src/paths.rs` | `dir_size` walks recursively; `human_size` thresholds |

Dev dependency: `tempfile` (only used in tests).

## Python — `uv run --directory transcribe pytest`

Tests live in `transcribe/tests/`, one file per pure package module:

`test_transcript.py` (`stt/transcript.py`):
- `assign_speakers` — overlap-based labelling, first-seen renumbering, `Remote` fallback when no overlap.
- `fmt_timestamp` — formatting at the hour/minute/second boundaries.
- `render_transcript` / `merge_and_write` — chronological merge of mic and system segments, UTF-8 output, exact line format.

`test_cleanup.py` (`stt/cleanup.py`):
- `is_hallucination` / `filter_hallucinations` — drops phantom Whisper text via decoder-confidence thresholds; keeps segments whose metrics are absent.
- `dedup_cross_track` — drops mic segments that echo the system audio (time overlap + near-identical text); keeps them when timing or text diverges.

`test_models.py` (`stt/models.py`):
- `mlx_repo` — known shorthands map to the right `mlx-community/` repos; unknown names fall back to the naming convention.

Only the **pure** modules (`transcript`, `cleanup`, `models`) are imported by the tests, so they never pull in `mlx-whisper`, `torch`, or `pyannote` — the heavy backends live in `asr.py` / `diarize.py` / `download.py` and import their deps lazily. The suite runs in well under a second.

Dev dependency (transcribe): `pytest` (in the `dev` dependency group).

## Out of scope

There are no integration tests against real recordings, real models, or macOS permissions. Those need hardware, several gigabytes of downloads, and user interaction — running them in a script wouldn't validate anything the unit tests don't already cover.
