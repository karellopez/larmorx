# SPDX-License-Identifier: Apache-2.0
"""FreeSurfer's randomNumber(low, high) with infinities, NaNs and signed zeros, from the unit oracle.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions).

For every pair of eight bounds (0, -0, 1.5, -2, inf, -inf, nan, -nan) it calls the unit oracle's
``fs-random`` (``setRandomSeed(5)`` then three ``randomNumber(low, high)``; the oracle-output
cache, one thread) and prints the table that
``crates-freesurfer/larmorx-freesurfer-core/tests/random_nan.rs`` checks the port against (each
value as the oracle prints it: C's ``%a``, so a NaN shows its sign, not its payload).

    python -P validation/probes/fs-maths/random_number_nan.py
"""

from __future__ import annotations

import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import freesurfer_unit as fu  # noqa: E402

BOUNDS = ["0x0p+0", "-0x0p+0", "0x1.8p+0", "-0x1p+1", "inf", "-inf", "nan", "-nan"]


def main() -> int:
    for low in BOUNDS:
        for high in BOUNDS:
            res = fu.call("fs-random", "5", "3", low, high)
            vals = ", ".join(f'"{v["hex"]}"' for v in res.data["values"])
            print(f'    ("{low}", "{high}", [{vals}]),')
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
