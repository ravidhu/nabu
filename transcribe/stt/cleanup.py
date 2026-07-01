"""Transcript clean-up heuristics — pure, stdlib only.

Two passes that compensate for the failure modes of transcribing the mic and
system tracks separately: phantom text on silence (hallucination filter) and the
remote party bleeding into the mic without headphones (cross-track echo dedup).
"""
from __future__ import annotations

import re
from difflib import SequenceMatcher


# ── hallucination filter ──────────────────────────────────────────────────────
#
# Whisper occasionally emits phantom text on near-silent or noisy audio (e.g.
# "Thank you.", "Subtitles by …", or a phrase repeated to death). These
# thresholds mirror openai-whisper's own decoding defaults; a segment is dropped
# when it looks like silence (high no-speech probability paired with low decoder
# confidence) or like repetitive gibberish (high gzip compression ratio).

NO_SPEECH_THRESHOLD = 0.6
LOGPROB_THRESHOLD = -1.0
COMPRESSION_RATIO_THRESHOLD = 2.4


def is_hallucination(seg: dict) -> bool:
    """True when a Whisper segment looks like a hallucination, not real speech.

    Uses the decoder's own confidence signals when present. Missing fields are
    treated as "not a hallucination", so segments without metrics are kept.
    """
    no_speech = seg.get("no_speech_prob")
    logprob = seg.get("avg_logprob")
    if (
        no_speech is not None
        and logprob is not None
        and no_speech >= NO_SPEECH_THRESHOLD
        and logprob <= LOGPROB_THRESHOLD
    ):
        return True
    ratio = seg.get("compression_ratio")
    if ratio is not None and ratio >= COMPRESSION_RATIO_THRESHOLD:
        return True
    return False


def filter_hallucinations(segments: list[dict]) -> list[dict]:
    """Drop segments that look like Whisper hallucinations. Order-preserving."""
    return [s for s in segments if not is_hallucination(s)]


# ── cross-track echo dedup ─────────────────────────────────────────────────────
#
# When the user isn't on headphones, the mic re-captures the remote party coming
# out of the speakers, so their speech is transcribed on *both* tracks. The
# system track is the clean digital source, so we keep it and drop the mic copy
# whenever a mic segment overlaps a system segment in time and their text is
# nearly identical.

ECHO_OVERLAP_RATIO = 0.5     # time overlap relative to the shorter segment
ECHO_TEXT_SIMILARITY = 0.80  # normalized text similarity, 0..1

_PUNCT_RE = re.compile(r"[^\w\s]", re.UNICODE)
_WS_RE = re.compile(r"\s+")


def _normalize(text: str) -> str:
    return _WS_RE.sub(" ", _PUNCT_RE.sub(" ", text.lower())).strip()


def _text_similarity(a: str, b: str) -> float:
    na, nb = _normalize(a), _normalize(b)
    if not na or not nb:
        return 0.0
    return SequenceMatcher(None, na, nb).ratio()


def _time_overlap_ratio(a: dict, b: dict) -> float:
    overlap = min(a["end"], b["end"]) - max(a["start"], b["start"])
    if overlap <= 0:
        return 0.0
    shorter = min(a["end"] - a["start"], b["end"] - b["start"])
    return overlap / shorter if shorter > 0 else 0.0


def dedup_cross_track(mic_segs: list[dict], sys_segs: list[dict]) -> list[dict]:
    """Return mic segments with acoustic echo of the system audio removed.

    A mic segment is treated as echo (and dropped) when it overlaps a system
    segment in time by at least ``ECHO_OVERLAP_RATIO`` of the shorter segment and
    their normalized text is at least ``ECHO_TEXT_SIMILARITY`` similar. The
    system segments are returned to the caller unchanged.
    """
    return [
        m
        for m in mic_segs
        if not any(
            _time_overlap_ratio(m, s) >= ECHO_OVERLAP_RATIO
            and _text_similarity(m["text"], s["text"]) >= ECHO_TEXT_SIMILARITY
            for s in sys_segs
        )
    ]
