"""Tests for transcript assembly in stt/transcript.py."""
from __future__ import annotations

from pathlib import Path

import pytest

from stt.transcript import (
    assign_speakers,
    fmt_timestamp,
    merge_and_write,
    render_transcript,
)


# ── fmt_timestamp ────────────────────────────────────────────────────────────

@pytest.mark.parametrize("secs, expected", [
    (0.0,    "00:00:00"),
    (1.9,    "00:00:01"),
    (59.0,   "00:00:59"),
    (60.0,   "00:01:00"),
    (3599.0, "00:59:59"),
    (3661.5, "01:01:01"),
])
def test_fmt_timestamp(secs, expected):
    assert fmt_timestamp(secs) == expected


# ── assign_speakers ──────────────────────────────────────────────────────────

def test_assign_speakers_picks_max_overlap():
    segments = [
        {"start": 1.0, "end": 5.0, "text": "hello"},
    ]
    diarization = [
        {"start": 0.0, "end": 2.0, "speaker": "raw_A"},  # overlap = 1.0
        {"start": 2.0, "end": 5.0, "speaker": "raw_B"},  # overlap = 3.0  ← wins
    ]
    out = assign_speakers(segments, diarization)
    # raw_A's friendly label is assigned first (it's seen first), so it gets
    # "Speaker 1"; raw_B then becomes "Speaker 2" and wins by overlap.
    assert out[0]["speaker"] == "Speaker 2"


def test_assign_speakers_renumbers_in_first_seen_order():
    segments = [
        {"start": 0.0, "end": 1.0, "text": "a"},
        {"start": 2.0, "end": 3.0, "text": "b"},
        {"start": 4.0, "end": 5.0, "text": "c"},
    ]
    diarization = [
        {"start": 0.0, "end": 1.0, "speaker": "spk_zzz"},
        {"start": 2.0, "end": 3.0, "speaker": "spk_aaa"},
        {"start": 4.0, "end": 5.0, "speaker": "spk_zzz"},
    ]
    out = assign_speakers(segments, diarization)
    assert [s["speaker"] for s in out] == ["Speaker 1", "Speaker 2", "Speaker 1"]


def test_assign_speakers_falls_back_to_remote_when_no_overlap():
    segments = [{"start": 10.0, "end": 11.0, "text": "lonely"}]
    diarization = [{"start": 0.0, "end": 1.0, "speaker": "spk_X"}]
    out = assign_speakers(segments, diarization)
    assert out[0]["speaker"] == "Remote"


def test_assign_speakers_preserves_original_fields():
    segments = [{"start": 0.0, "end": 1.0, "text": "hi", "lang": "en"}]
    diarization = [{"start": 0.0, "end": 1.0, "speaker": "spk_X"}]
    out = assign_speakers(segments, diarization)
    assert out[0]["text"] == "hi"
    assert out[0]["lang"] == "en"
    assert out[0]["speaker"] == "Speaker 1"


def test_assign_speakers_empty_diarization_returns_remote():
    segments = [{"start": 0.0, "end": 1.0, "text": "hi"}]
    out = assign_speakers(segments, [])
    assert out[0]["speaker"] == "Remote"


# ── render_transcript ────────────────────────────────────────────────────────

def test_render_transcript_sorts_by_start_time():
    mic = [{"start": 5.0, "end": 6.0, "speaker": "You", "text": "second"}]
    sys = [{"start": 1.0, "end": 2.0, "speaker": "Speaker 1", "text": "first"}]
    text = render_transcript(mic, sys)
    lines = text.strip().split("\n")
    assert lines[0] == "[00:00:01 → 00:00:02] [Speaker 1] first"
    assert lines[1] == "[00:00:05 → 00:00:06] [You] second"


def test_render_transcript_formats_single_line_exactly():
    seg = [{"start": 3.0, "end": 5.0, "speaker": "Speaker 1", "text": "hello world"}]
    assert render_transcript(seg, []) == "[00:00:03 → 00:00:05] [Speaker 1] hello world\n"


def test_render_transcript_empty_input_returns_blank():
    assert render_transcript([], []) == "\n"


# ── merge_and_write ──────────────────────────────────────────────────────────

def test_merge_and_write_writes_utf8_file(tmp_path: Path, capsys):
    mic = [{"start": 0.0, "end": 1.0, "speaker": "You", "text": "café"}]
    out = tmp_path / "transcript.md"
    merge_and_write(mic, [], out)
    assert out.read_text(encoding="utf-8") == "[00:00:00 → 00:00:01] [You] café\n"
    captured = capsys.readouterr()
    assert "transcript →" in captured.out
