"""Tests for the transcript clean-up heuristics in stt/cleanup.py."""
from __future__ import annotations

from stt.cleanup import (
    dedup_cross_track,
    filter_hallucinations,
    is_hallucination,
)


# ── filter_hallucinations ────────────────────────────────────────────────────

def test_hallucination_dropped_on_silence_signals():
    # High no-speech probability + low decoder confidence → silence hallucination.
    seg = {"start": 0.0, "end": 2.0, "text": "Thank you.",
           "no_speech_prob": 0.9, "avg_logprob": -1.5}
    assert is_hallucination(seg) is True


def test_hallucination_dropped_on_high_compression_ratio():
    # Repetitive/looping text compresses very well → gibberish.
    seg = {"start": 0.0, "end": 2.0, "text": "la la la la la",
           "compression_ratio": 3.0}
    assert is_hallucination(seg) is True


def test_real_speech_is_kept():
    seg = {"start": 0.0, "end": 2.0, "text": "let's start the meeting",
           "no_speech_prob": 0.05, "avg_logprob": -0.3, "compression_ratio": 1.4}
    assert is_hallucination(seg) is False


def test_missing_metrics_are_kept():
    # Segments without decoder metrics (e.g. from tests) must never be dropped.
    assert is_hallucination({"start": 0.0, "end": 1.0, "text": "hi"}) is False


def test_high_no_speech_alone_is_kept():
    # High no-speech probability but confident text is not enough to drop.
    seg = {"start": 0.0, "end": 1.0, "text": "yes",
           "no_speech_prob": 0.95, "avg_logprob": -0.2}
    assert is_hallucination(seg) is False


def test_filter_hallucinations_preserves_order_and_good_segments():
    segs = [
        {"start": 0.0, "end": 1.0, "text": "real one", "no_speech_prob": 0.1, "avg_logprob": -0.2},
        {"start": 1.0, "end": 2.0, "text": "ghost", "no_speech_prob": 0.9, "avg_logprob": -2.0},
        {"start": 2.0, "end": 3.0, "text": "real two"},
    ]
    out = filter_hallucinations(segs)
    assert [s["text"] for s in out] == ["real one", "real two"]


# ── dedup_cross_track ────────────────────────────────────────────────────────

def test_dedup_drops_echoed_mic_segment():
    mic = [{"start": 1.0, "end": 3.0, "speaker": "You", "text": "Can everyone hear me?"}]
    sys = [{"start": 1.0, "end": 3.0, "speaker": "Speaker 1", "text": "can everyone hear me"}]
    assert dedup_cross_track(mic, sys) == []


def test_dedup_keeps_mic_when_no_time_overlap():
    mic = [{"start": 10.0, "end": 12.0, "speaker": "You", "text": "hello there"}]
    sys = [{"start": 0.0, "end": 2.0, "speaker": "Speaker 1", "text": "hello there"}]
    assert dedup_cross_track(mic, sys) == mic


def test_dedup_keeps_mic_when_text_differs():
    mic = [{"start": 1.0, "end": 3.0, "speaker": "You", "text": "I disagree completely"}]
    sys = [{"start": 1.0, "end": 3.0, "speaker": "Speaker 1", "text": "let's move on"}]
    assert dedup_cross_track(mic, sys) == mic


def test_dedup_keeps_mic_on_small_overlap():
    # Overlap is only 0.2s of a 2s mic segment (ratio well below threshold).
    mic = [{"start": 1.0, "end": 3.0, "speaker": "You", "text": "same words here"}]
    sys = [{"start": 2.8, "end": 5.0, "speaker": "Speaker 1", "text": "same words here"}]
    assert dedup_cross_track(mic, sys) == mic


def test_dedup_no_system_segments_is_noop():
    mic = [{"start": 1.0, "end": 3.0, "speaker": "You", "text": "solo"}]
    assert dedup_cross_track(mic, []) == mic
