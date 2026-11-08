# SPDX-License-Identifier: Apache-2.0
"""numpy.linalg.lstsq against `larmorx_transform::numpy_linalg::lstsq`, bit for bit.

Run with the Python environment to check (the shared `.venv`: numpy 2.3.5 with OpenBLAS
0.3.30's LAPACK 3.12.0; `oracles/python-newest`: numpy 2.5.4 with OpenBLAS 0.3.34's LAPACK
3.12.1), on Linux x86-64 with ``OPENBLAS_CORETYPE=Haswell``:

    python -P check_lstsq.py --checker <lstsq_check> --work <empty dir> [--real <dump dir> ...]

It writes seeded cases (inputs and numpy's outputs at one BLAS thread) into ``--work`` and runs
the Rust program ``lstsq_check`` (``cargo build --release -p larmorx-transform --example
lstsq_check``) on them, which solves each at one and at several threads:

- ``tedana``: tedana's log-linear design (``[1, -TE]`` repeated per volume, 2 to 5 echoes, 1 to
  1366 volumes: 2 to 4098 rows, both sides of OpenBLAS's 2048-row blocks) and its right-hand
  sides (``log(|float32 signal| + 1)``), 1 to 2001 columns (every count modulo 4, so every kernel
  class of the last columns);
- ``random``: Gaussian designs and right-hand sides, 2 to 4099 rows around the kernels' block
  sizes (4, 16, 2048), 1 to 9 columns;
- ``edge``: trailing and inner zero columns (``ILADLC``), zero rows at the end of the design
  (the reflectors' ``LASTV`` scan), collinear and nearly collinear designs (rank 1), an all-zero
  design, designs and right-hand sides scaled below ``SMLNUM`` or above ``BIGNUM`` (DLASCL's
  steps), NaN, infinities and signed zeros in the right-hand side, a non-finite design (numpy
  2.5.4 raises LinAlgError, 2.3.5 does not), ``rcond`` inside and outside (0, 1), no column;
- ``real``: the calls tedana's ``t2smap`` itself made (``--real``: folders written by
  ``dump_lstsq_calls.py``), with tedana's own results.

The output is one line per group (cases, identical) and every differing case.
"""

from __future__ import annotations

import argparse
import struct
import subprocess
import sys
from pathlib import Path

import numpy as np
from threadpoolctl import threadpool_limits


def release() -> str:
    major, minor = (int(v) for v in np.__version__.split(".")[:2])
    return "3.12.1" if (major, minor) >= (2, 5) else "3.12.0"


class Cases:
    def __init__(self, work: Path) -> None:
        self.work = work
        self.lines: list[str] = []
        self.n = 0

    def _save(self, name: str, arr: np.ndarray) -> str:
        np.ascontiguousarray(arr, dtype="<f8").tofile(self.work / name)
        return name

    def add(self, group: str, a: np.ndarray, b: np.ndarray, rcond=None, expected=None) -> None:
        """``b`` is (m, nrhs); ``expected`` is numpy's (x, residuals, rank, s) if already known."""
        self.n += 1
        k = self.n
        a = np.asarray(a, dtype=np.float64)
        m, nrhs = b.shape
        if expected is None:
            try:
                with threadpool_limits(limits=1, user_api="blas"):
                    x, res, rank, s = np.linalg.lstsq(a, b, rcond=rcond)
                expected = (x, res, rank, s)
            except np.linalg.LinAlgError:
                expected = None
        fa = self._save(f"c{k}_a.bin", a)
        # Column-major m × nrhs (LAPACK's copy of numpy's B), float64 (exact for float32 input).
        fb = self._save(f"c{k}_b.bin", np.asarray(b, dtype=np.float64).T)
        rc = "none" if rcond is None else str(struct.unpack("<Q", struct.pack("<d", rcond))[0])
        if expected is None:
            rank_s, fx, fr, fs = "err", "-", "-", "-"
        else:
            x, res, rank, s = expected
            rank_s = str(int(rank))
            fx = self._save(f"c{k}_x.bin", np.asarray(x, dtype=np.float64).reshape(2, nrhs))
            fr = self._save(f"c{k}_r.bin", np.asarray(res, dtype=np.float64))
            fs = self._save(f"c{k}_s.bin", np.asarray(s, dtype=np.float64))
        self.lines.append(
            "\t".join([f"{group}/{k}", group, str(m), str(nrhs), rc, release(), rank_s,
                       fa, fb, fx, fr, fs])
        )

    def write(self) -> None:
        (self.work / "manifest.tsv").write_text("\n".join(self.lines) + "\n", encoding="utf-8")


def design(tes, vols: int) -> np.ndarray:
    x = np.column_stack([np.ones(len(tes)), [-te for te in tes]])
    return np.repeat(x, vols, axis=0)


def tedana_rhs(rng, tes, vols: int, nrhs: int) -> np.ndarray:
    s0 = rng.gamma(4, 300, nrhs)
    t2 = rng.uniform(15, 80, nrhs)
    te_rows = np.repeat(np.asarray(tes, dtype=float), vols)
    y = s0[None, :] * np.exp(-te_rows[:, None] / t2[None, :])
    y = y * (1 + 0.02 * rng.standard_normal((len(te_rows), nrhs)))
    return np.log(np.abs(y.astype(np.float32)) + 1)


TES = {
    2: [12.0, 28.0],
    3: [14.0, 29.96, 45.92],
    4: [12.0, 28.0, 44.0, 60.0],
    5: [10.0, 20.0, 30.0, 40.0, 50.0],
}


def tedana_cases(c: Cases, rng) -> None:
    for ne, tes in TES.items():
        for vols in (1, 2, 3, 7, 50, 200):
            for nrhs in (1, 2, 3, 4, 5, 6, 7, 8, 9, 13, 1000):
                c.add("tedana", design(tes, vols), tedana_rhs(rng, tes, vols, nrhs))
    for ne, vols, nrhs in [
        (5, 600, 1), (5, 600, 3), (5, 600, 6), (5, 600, 2001),
        (3, 683, 5), (3, 683, 7), (3, 1366, 9), (2, 1024, 6), (4, 1024, 3), (2, 1025, 4),
    ]:
        c.add("tedana", design(TES[ne], vols), tedana_rhs(rng, TES[ne], vols, nrhs))


def random_cases(c: Cases, rng) -> None:
    ms = [2, 3, 4, 5, 6, 7, 8, 9, 15, 16, 17, 18, 19, 20, 31, 32, 33, 34, 35, 63, 64, 65,
          2047, 2048, 2049, 2050, 2051, 2052, 4095, 4096, 4099]
    for m in ms:
        for nrhs in (1, 2, 3, 5, 6, 7, 9):
            a = rng.standard_normal((m, 2)) * rng.uniform(0.1, 30, 2)
            b = rng.standard_normal((m, nrhs)) * 4 + 2
            c.add("random", a, b)


def edge_cases(c: Cases, rng) -> None:
    tes = TES[3]
    a = design(tes, 200)
    base = tedana_rhs(rng, tes, 200, 11)
    for k in (1, 2, 3, 5):  # trailing zero columns
        b = base.copy()
        b[:, -k:] = 0
        c.add("edge", a, b)
    b = base.copy()
    b[:, 4] = 0  # an inner zero column
    c.add("edge", a, b)
    b = base.copy()
    b[:, -1] = 0
    b[0, -1] = 1.5  # the last column nonzero only in its first row
    c.add("edge", a, b)
    b = base.copy()
    b[:, -1] = 0
    b[300, -1] = 2.5  # ... only in a middle row
    c.add("edge", a, b)
    # Zero rows at the end of the design (LASTV), and of its first column only.
    for m, z in ((40, 3), (40, 17), (9, 2), (5, 2)):
        am = rng.standard_normal((m, 2))
        am[-z:, :] = 0
        c.add("edge", am, rng.standard_normal((m, 5)))
        am = rng.standard_normal((m, 2))
        am[-z:, 0] = 0
        c.add("edge", am, rng.standard_normal((m, 5)))
    # Collinear and nearly collinear designs.
    for t, vols in (([30.0, 30.0], 4), ([30.0, 30.0, 30.0], 10), ([30.0, 30.0 + 1e-12], 6),
                    ([30.0, 30.0 + 1e-9, 30.0], 50)):
        c.add("edge", design(t, vols), tedana_rhs(rng, t, vols, 7))
    c.add("edge", np.zeros((6, 2)), rng.standard_normal((6, 3)))  # all-zero design
    for scale in (1e-300, 1e-295, 1e300, 1e295):  # DLASCL of A
        c.add("edge", design(tes, 5) * scale, tedana_rhs(rng, tes, 5, 6))
    rhs = tedana_rhs(rng, tes, 20, 9).astype(np.float64)
    a20 = design(tes, 20)
    b = rhs.copy()
    b[7, 3] = np.nan
    c.add("edge", a20, b)
    b = rhs.copy()
    b[7, 3] = np.inf
    c.add("edge", a20, b)
    b = rhs.copy()
    b[:, 5] = -0.0
    b[3, 2] = -0.0
    c.add("edge", a20, b)
    c.add("edge", a20, rhs * 1e-300)  # DLASCL of B (below SMLNUM)
    b = rhs.copy()
    b[2, 2] = 1e300  # above BIGNUM
    c.add("edge", a20, b)
    c.add("edge", a20, np.zeros_like(rhs))
    c.add("edge", a20, rhs * 1e150)  # magnitudes beyond the x87 emulation's range (rows only)
    for rcond in (0.5, -1.0, 1e-3, 2.0, 0.0, 0.999):
        c.add("edge", a20, rhs, rcond=rcond)
    for bad in (np.nan, np.inf, -np.inf):
        am = a20.copy()
        am[4, 1] = bad
        c.add("edge", am, rhs)
    c.add("edge", a20, np.zeros((60, 0)))  # no column
    for nrhs in (1, 2, 3, 4, 5, 1001):  # two rows: no QR (m < MNTHR)
        c.add("edge", design(TES[2], 1), tedana_rhs(rng, TES[2], 1, nrhs))
        c.add("edge", rng.standard_normal((2, 2)), rng.standard_normal((2, nrhs)))


def real_cases(c: Cases, dump: Path) -> None:
    """The calls ``dump_lstsq_calls.py`` recorded inside tedana (inputs and tedana's results)."""
    for meta in sorted(dump.glob("call*_meta.txt")):
        stem = meta.name[: -len("_meta.txt")]
        m, nrhs = (int(v) for v in meta.read_text(encoding="utf-8").split()[:2])
        a = np.fromfile(dump / f"{stem}_a.bin", dtype="<f8").reshape(m, 2)
        b = np.fromfile(dump / f"{stem}_b.bin", dtype="<f4").reshape(nrhs, m).T
        x = np.fromfile(dump / f"{stem}_x.bin", dtype="<f8").reshape(2, nrhs)
        res = np.fromfile(dump / f"{stem}_r.bin", dtype="<f8")
        s = np.fromfile(dump / f"{stem}_s.bin", dtype="<f8")
        rank = int(meta.read_text(encoding="utf-8").split()[2])
        c.add(f"real-{dump.name}", a, b, expected=(x, res, rank, s))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--checker", type=Path, required=True)
    ap.add_argument("--work", type=Path, required=True)
    ap.add_argument("--real", type=Path, action="append", default=[])
    ap.add_argument("--threads", type=int, default=4)
    ap.add_argument("--seed", type=int, default=20261108)
    args = ap.parse_args()
    work = args.work
    work.mkdir(parents=True, exist_ok=True)
    if any(work.iterdir()):
        sys.exit(f"{work} is not empty")
    print(f"numpy {np.__version__} (LAPACK {release()}), {sys.executable}", flush=True)
    rng = np.random.default_rng(args.seed)
    c = Cases(work)
    tedana_cases(c, rng)
    random_cases(c, rng)
    edge_cases(c, rng)
    for dump in args.real:
        real_cases(c, dump)
    c.write()
    print(f"{c.n} cases written", flush=True)
    return subprocess.run([str(args.checker), str(work), str(args.threads)], check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
