"""Entry-point shim — the Rust bootstrap invokes ``python transcribe.py``.

All logic lives in the ``stt`` package; this file only exists to keep
the historical entry-point filename stable so build.rs / bootstrap.rs need no
changes. Running this script puts its directory on sys.path, so the sibling
``stt`` package imports without any install step.
"""
from stt.cli import main

if __name__ == "__main__":
    raise SystemExit(main())
