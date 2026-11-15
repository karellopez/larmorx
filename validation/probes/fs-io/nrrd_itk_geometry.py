# SPDX-License-Identifier: Apache-2.0
"""The geometry of NRRD volumes: FreeSurfer's reading against ITK's (FS-736 to FS-738).

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; the programs run as
black boxes).

Generates seeded NRRD files (the spaces RAS, LAS, LPS, scanner, right- and left-handed; diagonal,
flipped and oblique space directions, an axis without one now and then; with and without a space
origin), reads each with ITK 5.4.5's NRRD reader (ANTs 2.6.5's ``PrintHeader``, through the
oracle-output cache) and with larmorx-freesurfer-io's ``mriNrrdRead`` (the probe example
``fs_io_probe``, a ``nnh`` case) in both modes, and compares the voxel-to-RAS matrices: ITK's
(LPS, its origin, spacing and directions) turned to RAS, against the volume's (its voxel sizes,
direction cosines and centre). ``fix`` must agree with ITK on every file (to ITK's printed
precision); ``reproduce`` is FreeSurfer's reading, counted.

    python -P validation/probes/fs-io/nrrd_itk_geometry.py [--seed 1] [--n 200]

Prints the counts and ``PASS`` when ``fix`` agrees with ITK on every file.
"""

from __future__ import annotations

import argparse
import random
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))
sys.path.insert(0, str(HERE))

import check  # noqa: E402
from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

SPACES = [
    "right-anterior-superior",
    "left-anterior-superior",
    "left-posterior-superior",
    "scanner-xyz",
    "3D-right-handed",
    "3D-left-handed",
]


def nrrd(rng: random.Random) -> tuple[bytes, str, list[int]]:
    """A uchar NRRD file, its space and its axes without a space direction."""
    size = (rng.randint(1, 4), rng.randint(1, 4), rng.randint(1, 4))
    space = rng.choice(SPACES)
    oblique = rng.random() < 0.5
    dirs = []
    for i in range(3):
        if rng.random() < 0.08:
            dirs.append("none")
            continue
        v = [0.0, 0.0, 0.0]
        v[i] = rng.choice([1.0, 2.0, 0.5, -1.0, 1.5, -3.25])
        if oblique:
            for k in range(3):
                if k != i:
                    v[k] = rng.randint(-40, 40) / 64.0
        dirs.append("(" + ",".join(repr(c) for c in v) + ")")
    lines = [
        "NRRD0004",
        "type: uchar",
        "dimension: 3",
        f"space: {space}",
        "sizes: " + " ".join(str(s) for s in size),
        "space directions: " + " ".join(dirs),
    ]
    if rng.random() < 0.85:
        o = [rng.randint(-4000, 4000) / 16.0 for _ in range(3)]
        lines.append("space origin: (" + ",".join(repr(c) for c in o) + ")")
    lines.append("encoding: raw")
    n = size[0] * size[1] * size[2]
    data = bytes(rng.randrange(256) for _ in range(n))
    none = [i for i, x in enumerate(dirs) if x == "none"]
    return ("\n".join(lines) + "\n\n").encode() + data, space, none


def itk_vox2ras(header: str, space: str, none: list[int]) -> list[list[float]]:
    """ITK's voxel-to-world matrix (from PrintHeader's text), turned from LPS to RAS. ITK 5.4.5
    turns a ``left-anterior-superior`` file's coordinates to LPS by negating the first
    coordinate (its comment says "R -> L") where the second (A to P) is meant: its LPS matrix is
    then the file's geometry in RAS already, and is taken as it is (the finding in
    ``docs/findings/freesurfer-io.md``, Part 26), but for the axes without a direction, whose
    unit columns are ITK's own LPS ones."""
    rows = header.splitlines()

    def vec(label: str) -> list[float]:
        line = next(r for r in rows if r.strip().startswith(label))
        inside = line.split("[", 1)[1].rsplit("]", 1)[0]
        return [float(x) for x in inside.split(",")]

    origin, spacing = vec("Origin"), vec("Voxel Spacing")
    at = next(i for i, r in enumerate(rows) if "Direction Cos Mtx" in r)
    d = [[float(x) for x in rows[at + 1 + k].split()] for k in range(3)]
    m = [[d[r][c] * spacing[c] for c in range(3)] + [origin[r]] for r in range(3)]
    for r in (0, 1):
        for c in range(4):
            if space != "left-anterior-superior" or c in none:
                m[r][c] = -m[r][c]
    return m


def f32(h: str) -> float:
    return struct.unpack(">f", bytes.fromhex(h))[0]


def port_vox2ras(line: str) -> list[list[float]] | None:
    """The volume's voxel-to-RAS matrix from the probe's printed fields (piece 6's order)."""
    t = line.split(" | ")[0].split()
    if t[0] == "N":
        return None
    w, h, d = int(t[0]), int(t[1]), int(t[2])
    v = [f32(x) for x in t[7:22]]
    size, cols, c = v[0:3], [v[3:6], v[6:9], v[9:12]], v[12:15]
    m = [[cols[i][r] * size[i] for i in range(3)] for r in range(3)]
    half = (w / 2.0, h / 2.0, d / 2.0)
    p0 = [c[r] - sum(m[r][i] * half[i] for i in range(3)) for r in range(3)]
    return [m[r] + [p0[r]] for r in range(3)]


def agree(a: list[list[float]], b: list[list[float]]) -> bool:
    for ra, rb in zip(a, b, strict=True):
        for x, y in zip(ra, rb, strict=True):
            if abs(x - y) > 1e-4 * max(1.0, abs(x), abs(y)):
                return False
    return True


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--n", type=int, default=200)
    a = ap.parse_args()
    rng = random.Random(a.seed)
    ws = workspace()
    ph = ws / "oracles" / "ants-2.6.5" / "bin" / "PrintHeader"
    port = check.port_binary(8)
    inputs = ws / "oracles" / "inputs" / "fs-io-nrrd-itk"
    inputs.mkdir(parents=True, exist_ok=True)
    cases, files, itk = [], [], []
    for i in range(a.n):
        b, space, none = nrrd(rng)
        f = inputs / f"s{a.seed}-{i}.nrrd"
        f.write_bytes(b)
        files.append(f)
        env = {"PATH": "/usr/bin:/bin", "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}
        p = oracle_cache.run(
            [ph, f],
            env=env,
            policy=oracle_cache.Policy(threads=1, family="itk"),
            timeout=60,
            runner="fs-io-nrrd-itk",
        )
        itk.append(itk_vox2ras(p.stdout, space, none))
        name = b"w/v.nrrd".hex()
        cases.append(f"nnh 0 1 0 26 x{name} F{name}:{len(b)}+0:{b.hex()}")
    with tempfile.TemporaryDirectory() as td:
        cf = Path(td) / "cases.txt"
        cf.write_text("\n".join(cases) + "\n")
        res = {}
        for mode in ("fix", "reproduce"):
            out = subprocess.run(
                [str(port), "run", str(cf), mode], check=True, capture_output=True, text=True
            ).stdout.splitlines()
            ok = sum(
                1
                for line, m in zip(out, itk, strict=True)
                if (v := port_vox2ras(line)) is not None and agree(v, m)
            )
            res[mode] = ok
    print(
        f"files {a.n}: fix agrees with ITK on {res['fix']}, FreeSurfer's reading (reproduce) on {res['reproduce']}"
    )
    print("PASS" if res["fix"] == a.n else "FAIL")
    return 0 if res["fix"] == a.n else 1


if __name__ == "__main__":
    raise SystemExit(main())
