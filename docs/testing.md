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

Tests live in `transcribe/tests/`. They cover the pure-Python helpers in `transcribe/postprocess.py`:

- `assign_speakers` — overlap-based labelling, first-seen renumbering, `Remote` fallback when no overlap.
- `fmt_timestamp` — formatting at the hour/minute/second boundaries.
- `render_transcript` / `merge_and_write` — chronological merge of mic and system segments, UTF-8 output, exact line format.

`postprocess.py` is intentionally split out from `transcribe.py` so the tests never have to import `mlx-whisper`, `torch`, or `pyannote`. They run in well under a second.

Dev dependency (transcribe): `pytest` (in the `dev` dependency group).

## Out of scope

There are no integration tests against real recordings, real models, or macOS permissions. Those need hardware, several gigabytes of downloads, and user interaction — running them in a script wouldn't validate anything the unit tests don't already cover.
