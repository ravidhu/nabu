"""Model registry — Whisper shorthand → mlx-community repos, plus backend model IDs.

Pure: stdlib only, safe to import without the heavy ML dependencies.
"""
from __future__ import annotations

PYANNOTE_MODEL = "pyannote/speaker-diarization-community-1"

# GitHub Releases mirror for the wespeaker English model — avoids the slow
# ModelScope CDN (modelscope.cn). wespeaker checks ~/.wespeaker/english/ first,
# so pre-populating it bypasses the ModelScope download entirely.
WESPEAKER_MIRROR_URL = (
    "https://github.com/ravidhu/nabu/releases/latest/download/"
    "voxceleb_resnet221_LM.tar.gz"
)

# Maps Whisper shorthand names → mlx-community HuggingFace repos
MLX_REPOS: dict[str, str] = {
    "tiny.en":         "mlx-community/whisper-tiny.en-mlx",
    "tiny":            "mlx-community/whisper-tiny-mlx",
    "base.en":         "mlx-community/whisper-base.en-mlx",
    "base":            "mlx-community/whisper-base-mlx",
    "small.en":        "mlx-community/whisper-small.en-mlx",
    "small":           "mlx-community/whisper-small-mlx",
    "medium.en":       "mlx-community/whisper-medium.en-mlx",
    "medium":          "mlx-community/whisper-medium-mlx",
    "large-v3":        "mlx-community/whisper-large-v3-mlx",
    "large":           "mlx-community/whisper-large-v3-mlx",
    "distil-large-v3": "mlx-community/distil-whisper-large-v3",
    "distil-small.en": "mlx-community/distil-whisper-small.en-mlx",
}


def mlx_repo(model_name: str) -> str:
    """Resolve a Whisper shorthand to its mlx-community repo (with a sane fallback)."""
    return MLX_REPOS.get(model_name, f"mlx-community/whisper-{model_name}-mlx")
