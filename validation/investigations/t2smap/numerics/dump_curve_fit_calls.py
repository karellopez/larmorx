# SPDX-License-Identifier: Apache-2.0
"""Records every ``scipy.optimize.curve_fit`` call tedana's ``t2smap --fittype curvefit`` makes.

Runs tedana's own command line (``tedana.workflows.t2smap._main``) in this process with
``scipy.optimize.curve_fit`` wrapped (tedana's ``_fit_single_voxel`` looks it up at each call;
joblib runs ``--n-threads 1`` in this process): each call's echo times, signal (tedana's float32
column), start ``p0`` and bounds go into ``--out`` as ``check_curve_fit.py``'s records (kind 5
when the echo times change, then kind 6 per call), with SciPy's result as tedana got it
(``full_output=True`` adds the evaluation count and status; it changes nothing else).
``check_curve_fit.py --real <out>`` then checks larmorx on them. ``warnings.txt`` counts the
Python warnings the calls raised (class, message, file and line), which tedana's own run prints
once per place.

    NPY_DISABLE_CPU_FEATURES="..." OPENBLAS_CORETYPE=Haswell python -P dump_curve_fit_calls.py \\
        --out <dir> --workdir <scratch> -- <t2smap arguments, e.g. -d e1.nii.gz e2.nii.gz
        e3.nii.gz -e 14 29.96 45.92 --mask mask.nii.gz --fittype curvefit>
"""

from __future__ import annotations

import argparse
import os
import struct
import warnings
from pathlib import Path

import numpy as np
import scipy.optimize


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--workdir", type=Path, required=True)
    ap.add_argument("t2smap_args", nargs=argparse.REMAINDER)
    args = ap.parse_args()
    if os.environ.get("DUMP_CURVE_FIT_GUARD"):
        raise SystemExit("recursion guard")
    os.environ["DUMP_CURVE_FIT_GUARD"] = "1"
    out = args.out
    out.mkdir(parents=True, exist_ok=True)
    original = scipy.optimize.curve_fit
    with open(out / "records.bin", "wb") as records, open(out / "expected.bin", "wb") as expected:
        return run(args, out, original, records, expected)


def run(args, out: Path, original, records, expected) -> int:
    kinds: list[str] = []
    lengths: list[int] = []
    last_x: list[np.ndarray] = []
    counts = {"calls": 0, "failures": 0}
    seen_warnings: dict[str, int] = {}

    def put(kind: str, rec: bytes, exp: bytes) -> None:
        records.write(rec)
        expected.write(struct.pack("<I", len(exp)) + exp)
        kinds.append(kind)
        lengths.append(len(rec))

    def note(caught) -> None:
        for w in caught:
            key = f"{w.category.__name__}\t{w.message}\t{Path(w.filename).name}:{w.lineno}"
            seen_warnings[key] = seen_warnings.get(key, 0) + 1

    def recording_curve_fit(f, xdata, ydata, p0=None, bounds=None, **kwargs):
        x = np.asarray(xdata, dtype=np.float64)
        if not last_x or last_x[0].shape != x.shape or not np.array_equal(last_x[0], x):
            last_x[:] = [x.copy()]
            put("x", b"\x05" + struct.pack("<I", len(x)) + x.astype("<f8").tobytes(), b"")
        y = np.asarray(ydata)
        assert y.dtype == np.float32, y.dtype
        lb = np.asarray(bounds[0], dtype=np.float64)
        ub = np.asarray(bounds[1], dtype=np.float64)
        rec = (
            b"\x06"
            + y.astype("<f4").tobytes()
            + np.asarray(p0, dtype="<f8").tobytes()
            + lb.astype("<f8").tobytes()
            + ub.astype("<f8").tobytes()
        )
        counts["calls"] += 1
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            try:
                popt, pcov, info, _msg, ier = original(
                    f, xdata, ydata, p0=p0, bounds=bounds, full_output=True, **kwargs
                )
            except (RuntimeError, ValueError) as e:
                note(caught)
                counts["failures"] += 1
                put("fit", rec, b"\x00" + f"{type(e).__name__}: {e}".encode())
                raise
        warn = any(issubclass(w.category, scipy.optimize.OptimizeWarning) for w in caught)
        note(caught)
        exp = (
            b"\x01"
            + np.asarray(popt, dtype="<f8").tobytes()
            + np.asarray(pcov, dtype="<f8").reshape(4).tobytes()
            + bytes([warn])
            + struct.pack("<Ii", int(info["nfev"]), int(ier))
        )
        put("fit", rec, exp)
        return popt, pcov

    scipy.optimize.curve_fit = recording_curve_fit
    from tedana.workflows import t2smap

    argv = [a for a in args.t2smap_args if a != "--"]
    args.workdir.mkdir(parents=True, exist_ok=True)
    t2smap._main([*argv, "--out-dir", str(args.workdir), "--overwrite"])
    records.flush()
    expected.flush()
    (out / "kinds.txt").write_text(" ".join(kinds) + "\n", encoding="utf-8")
    (out / "lengths.txt").write_text(" ".join(map(str, lengths)) + "\n", encoding="utf-8")
    (out / "warnings.txt").write_text(
        "".join(f"{n}\t{k}\n" for k, n in sorted(seen_warnings.items())), encoding="utf-8"
    )
    print(f"recorded {counts['calls']} curve_fit calls, {counts['failures']} failures")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
