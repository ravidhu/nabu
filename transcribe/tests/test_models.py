"""Tests for the model registry in stt/models.py."""
from __future__ import annotations

from stt.models import MLX_REPOS, mlx_repo


def test_known_shorthand_maps_to_mlx_repo():
    assert mlx_repo("large-v3") == "mlx-community/whisper-large-v3-mlx"
    assert mlx_repo("tiny.en") == "mlx-community/whisper-tiny.en-mlx"


def test_alias_large_maps_to_v3():
    assert mlx_repo("large") == MLX_REPOS["large-v3"]


def test_unknown_name_falls_back_to_convention():
    assert mlx_repo("turbo") == "mlx-community/whisper-turbo-mlx"
