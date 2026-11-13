# SPDX-License-Identifier: Apache-2.0
"""The corrected `.mat` readers (fs.io piece 5, `fix`) against files SciPy writes.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions).

Writes matrices with ``scipy.io.savemat`` (version 4; version 5 uncompressed and compressed;
doubles, singles, signed and unsigned 8-, 16- and 32-bit integers, complex; names of 1 to 20
characters; 1 x 1 to 5 x 4), reads each with the port's ``MatlabRead`` in ``fix`` mode (the probe
example ``fs_io_probe``, command ``mlr``) and compares the floats with SciPy's values converted to
float; then counts how many FreeSurfer's own reading (``reproduce``) gets right, per format. Prints
the counts; exit status 1 when the corrected reader differs.

    python -P validation/probes/fs-io/matfile_scipy.py
"""

from __future__ import annotations

import io
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
import scipy.io

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check  # noqa: E402


def entry(name: str, b: bytes) -> str:
    return f"F{name.encode().hex()}:{len(b)}+0:{b.hex() or '-'}"


def read_right(a: np.ndarray, line: str) -> bool:
    """The port's `mlr` line holds `a` (as floats, row by row; complex as pairs)."""
    t = line.split(" | ")[0].split()
    if t[0] in ("N", "EXIT") or len(t) < 3:
        return False
    rows, cols, ty = int(t[0]), int(t[1]), int(t[2])
    if (
        any(x.startswith("fnv") for x in t[3:4])
        or (rows, cols) != a.shape
        or ty != (2 if np.iscomplexobj(a) else 1)
    ):
        return False
    vals = [struct.unpack(">f", bytes.fromhex(x))[0] for x in t[3 : 3 + rows * cols * ty]]
    if np.iscomplexobj(a):
        exp = [
            v
            for z in a.astype(np.complex128).ravel()
            for v in (np.float32(z.real), np.float32(z.imag))
        ]
    else:
        exp = [np.float32(v) for v in a.astype(np.float64).ravel()]
    return len(vals) == len(exp) and all(np.float32(x) == y for x, y in zip(vals, exp, strict=True))


def main() -> int:
    rng = np.random.default_rng(15)
    port = check.port_binary(6)
    cases, want = [], []
    dtypes = [np.float64, np.float32, np.int8, np.uint8, np.int16, np.uint16, np.int32, np.uint32]
    for k in range(400):
        dt = dtypes[k % len(dtypes)]
        fmt = ["4", "5", "5z"][k // len(dtypes) % 3]
        shape = (int(rng.integers(1, 6)), int(rng.integers(1, 5)))
        if np.issubdtype(dt, np.integer):
            info = np.iinfo(dt)
            a = rng.integers(info.min, int(info.max) + 1, size=shape, dtype=np.int64).astype(dt)
        else:
            a = (rng.standard_normal(shape) * 1000).astype(dt)
        if k % 7 == 0 and dt in (np.float64, np.float32):
            a = a + 1j * (rng.standard_normal(shape) * 10).astype(dt)
        if fmt == "4" and dt not in (
            np.float64,
            np.float32,
            np.int32,
            np.int16,
            np.uint16,
            np.uint8,
        ):
            a = a.astype(np.float64)
        name = "v" + "abcdefghijklmnopqrs"[: int(rng.integers(0, 19))]
        buf = io.BytesIO()
        scipy.io.savemat(
            buf, {name: a}, format="4" if fmt == "4" else "5", do_compression=fmt == "5z"
        )
        cases.append(f"mlr 0 -1 x{b'w/m.mat'.hex()} {entry('w/m.mat', buf.getvalue())}")
        want.append(a)
    fmts = [["4", "5", "5z"][k // len(dtypes) % 3] for k in range(len(cases))]
    with tempfile.TemporaryDirectory() as d:
        cp = Path(d) / "cases.txt"
        cp.write_text("\n".join(cases) + "\n", encoding="utf-8")
        out = subprocess.run(
            [str(port), "run", str(cp), "fix"], check=True, capture_output=True, text=True
        ).stdout
        rep = subprocess.run(
            [str(port), "run", str(cp), "reproduce"], check=True, capture_output=True, text=True
        ).stdout
    right = {}
    for a, line, f in zip(want, rep.splitlines(), fmts, strict=True):
        right.setdefault(f, [0, 0])
        right[f][1] += 1
        right[f][0] += read_right(a, line)
    same = differ = 0
    for a, line in zip(want, out.splitlines(), strict=True):
        if read_right(a, line):
            same += 1
        else:
            differ += 1
            if differ <= 5:
                print("DIFFER", a.dtype, a.shape, line[:300])
    print(f"fix: {same} of {same + differ} matrices read as SciPy wrote them")
    for f, (r, n) in sorted(right.items()):
        print(f"reproduce (FreeSurfer), format {f}: {r} of {n} read right")
    return 1 if differ else 0


if __name__ == "__main__":
    raise SystemExit(main())
