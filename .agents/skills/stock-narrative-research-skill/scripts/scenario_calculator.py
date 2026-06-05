#!/usr/bin/env python3
"""Deterministic scenario calculator.

This is the preferred name for the former projection helper. The agent owns the
scenario assumptions; this script only turns those assumptions into consistent
derived math.
"""

from __future__ import annotations

import sys
from pathlib import Path


SCRIPT_DIR = Path(__file__).resolve().parent
if str(SCRIPT_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIR))

from projection_model import (  # noqa: E402
    PROJECTION_NOTE,
    build_projection,
    main,
    sample_assumptions,
)


__all__ = ["PROJECTION_NOTE", "build_projection", "sample_assumptions"]


if __name__ == "__main__":
    sys.exit(main())
