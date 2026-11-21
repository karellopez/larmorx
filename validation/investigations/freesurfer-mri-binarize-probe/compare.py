# SPDX-License-Identifier: Apache-2.0
"""mri_binarize as a black box: FreeSurfer 8.2.0's official program against a candidate, case by case.

An open probe (``README.md``): it writes seeded synthetic inputs (MGZ volumes: a segmentation of a
head-like pattern with FreeSurfer's labels, intensity volumes in uchar, short and float, a
three-frame float volume, a mask, a merge volume, volumes of other sizes, shapes whose surface has
a handle or none, a colour table, a transform, a closed surface and a measure on its vertices)
into ``<workspace>/oracles/inputs/fs-binarize-prog/<seed>/``, runs each case's command line through
the official program (``$FREESURFER_HOME/bin/mri_binarize``, found on the ``PATH`` by its name as
the scripts run it) by the oracle-output cache at one thread, and through the candidate
(``--candidate``, a command line whose ``{mode}`` becomes ``reproduce`` and then ``fix``), each in a
folder of its own (outputs are written there). Compared: the exit status, standard output and
error (the ``cwd`` line of the report and the candidate's ``larmorx:`` notices aside) and every file
written (FreeSurfer's provenance aside: ``larmorx_validation.freesurfer.files``). ``reproduce`` must
give FreeSurfer's on every case but those whose reproduce run notes a defect of ``NOT_COMPARABLE``,
which are counted; ``fix`` may differ only with a notice.

    python -P validation/investigations/freesurfer-mri-binarize-probe/compare.py --seed 1 \
        --candidate '<program> --upstream-defects {mode} freesurfer mri_binarize'
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import subprocess
import sys
from collections import Counter
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer import files as fsfiles  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

#: The defects (``larmorx_core::defects``) whose cases are counted, not compared: FreeSurfer
#: reads memory it never wrote or does not own, writes past its lists, crashes there, or runs
#: out of the address space it never releases (FS-53, FS-58, FS-1020, FS-1021, FS-1023, FS-1025,
#: FS-1026, FS-1027).
NOT_COMPARABLE = frozenset(
    {"FS-53", "FS-58", "FS-1020", "FS-1021", "FS-1023", "FS-1025", "FS-1026", "FS-1027"}
)
#: FreeSurfer's runs compared with their entries.
POLICY = oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer")
#: The report's working folder, which differs between the sides.
CWD = re.compile(r"(?m)^cwd .*$")


def head(rng: np.random.Generator, shape: tuple[int, int, int]) -> np.ndarray:
    """A head-like intensity pattern: white matter near 110, grey matter, CSF, a dark rim."""
    idx = np.indices(shape).astype(np.float64)
    c = [(s - 1) / 2 for s in shape]
    r = np.sqrt(sum(((idx[i] - c[i]) / (shape[i] / 2)) ** 2 for i in range(3)))
    v = np.where(r < 0.45, 110.0, np.where(r < 0.6, 75.0, np.where(r < 0.75, 35.0, 4.0)))
    return np.clip(v + rng.normal(0.0, 5.0, shape), 0, None)


def segmentation(rng: np.random.Generator, shape: tuple[int, int, int]) -> np.ndarray:
    """FreeSurfer's labels in a head-like pattern: white matter 2 and 41, cortex 3 and 42,
    ventricles 4 and 43, hippocampi 17 and 53, the corpus callosum 251, cerebellar white matter
    7 and 46, CSF 24, scattered 77, 1000-range cortical labels, background 0."""
    idx = np.indices(shape).astype(np.float64)
    c = [(s - 1) / 2 for s in shape]
    r = np.sqrt(sum(((idx[i] - c[i]) / (shape[i] / 2)) ** 2 for i in range(3)))
    left = idx[0] < c[0]
    seg = np.zeros(shape, np.int32)
    seg[r < 0.75] = 24
    seg[r < 0.62] = np.where(left, 3, 42)[r < 0.62]
    seg[r < 0.5] = np.where(left, 2, 41)[r < 0.5]
    seg[r < 0.2] = np.where(left, 4, 43)[r < 0.2]
    hip = (np.abs(idx[1] - c[1]) < 2) & (np.abs(idx[2] - c[2] + 4) < 2) & (r < 0.45)
    seg[hip] = np.where(left, 17, 53)[hip]
    seg[(np.abs(idx[0] - c[0]) < 1) & (r < 0.3)] = 251
    seg[(idx[2] < c[2] * 0.6) & (r < 0.5)] = np.where(left, 7, 46)[
        (idx[2] < c[2] * 0.6) & (r < 0.5)
    ]
    ctx = (seg == 3) & (rng.random(shape) < 0.3)
    seg[ctx] = 1000 + rng.integers(1, 35, shape)[ctx]
    seg[(seg == 2) & (rng.random(shape) < 0.03)] = 77
    return seg


def save(path: Path, data: np.ndarray, affine: np.ndarray) -> Path:
    nib.save(nib.MGHImage(data, affine), str(path))
    return path


def inputs(ws: Path, seed: int) -> dict[str, Path]:
    d = ws / "oracles" / "inputs" / "fs-binarize-prog" / str(seed)
    d.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(seed)
    aff = np.diag([1.0, 1.0, 1.0, 1.0])
    aff[:3, 3] = [-16.0, -16.0, -16.0]
    f: dict[str, Path] = {}
    seg = segmentation(rng, (32, 32, 32))
    f["seg"] = save(d / "seg_32.mgz", seg, aff)
    f["segu8"] = save(d / "seg_u8_32.mgz", np.where(seg > 255, 3, seg).astype(np.uint8), aff)
    t = head(rng, (32, 32, 32))
    f["u8"] = save(d / "t1_u8_32.mgz", np.clip(np.rint(t), 0, 255).astype(np.uint8), aff)
    f["f32"] = save(d / "t1_f32_32.mgz", (t - 40.0).astype(np.float32), aff)
    f["s16"] = save(d / "t1_s16_32.mgz", np.rint(t * 4 - 100).astype(np.int16), aff)
    nx = rng.normal(0.0, 1.0, (32, 32, 32, 3)).astype(np.float32)
    f["f3"] = save(d / "nxyz_32x3.mgz", nx, aff)
    f["u83"] = save(
        d / "frames_u8_32x3.mgz", (rng.random((32, 32, 32, 3)) < 0.6).astype(np.uint8), aff
    )
    p = np.abs(rng.normal(0.0, 1.5, (32, 32, 32))) * np.where(seg == 2, 3.0, 1.0)
    f["sig"] = save(
        d / "sig_f32_32.mgz", (p * np.sign(rng.normal(0, 1, p.shape))).astype(np.float32), aff
    )
    f["mask"] = save(d / "mask_f32_32.mgz", rng.random((32, 32, 32)).astype(np.float32), aff)
    f["merge"] = save(d / "merge_32.mgz", rng.integers(0, 9, (32, 32, 32)).astype(np.int32), aff)
    aff2 = np.diag([1.2, 0.9, 1.1, 1.0])
    aff2[:3, 3] = [-20.0, -15.0, -14.0]
    f["segb"] = save(d / "seg_40x36x28.mgz", segmentation(rng, (40, 36, 28)), aff2)
    f["small"] = save(d / "small_16.mgz", rng.integers(0, 5, (16, 16, 16)).astype(np.int32), aff)
    a = np.zeros((20, 20, 20), np.int32)
    a[4:14, 5:15, 3:16] = 7
    a[8:10, 9:11, :] = 0
    f["torus"] = save(d / "torus_20.mgz", a, aff)
    b = np.zeros((16, 16, 16), np.int32)
    b[4:10, 5:11, 3:12] = 7
    f["cube"] = save(d / "cube_16.mgz", b, aff)
    # a colour table: a few of FreeSurfer's labels
    lut = [(0, "Unknown"), (2, "Left-Cerebral-White-Matter"), (3, "Left-Cerebral-Cortex"),
           (17, "Left-Hippocampus"), (41, "Right-Cerebral-White-Matter"),
           (42, "Right-Cerebral-Cortex"), (53, "Right-Hippocampus")]  # fmt: skip
    (d / "lut.txt").write_text(
        "".join(f"{k:4d}  {n:<32s} {(k * 37) % 256:3d} {(k * 91) % 256:3d} {(k * 53) % 256:3d}   0\n"
                for k, n in lut),
        encoding="utf-8",
    )  # fmt: skip
    f["lut"] = d / "lut.txt"
    # a closed surface (an octahedron, tkregister coordinates) and a measure on its vertices
    v = np.array([[6, 0, 0], [-6, 0, 0], [0, 6, 0], [0, -6, 0], [0, 0, 6], [0, 0, -6]], float)
    fa = np.array(
        [[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]]
    )
    nib.freesurfer.write_geometry(str(d / "lh.white"), v, fa)
    f["surf"] = d / "lh.white"
    f["meas"] = save(
        d / "measure_6.mgz", np.arange(1, 7, dtype=np.float32).reshape(6, 1, 1), np.eye(4)
    )
    # a linear transform (vox2vox identity, LTA text)
    (d / "shift.lta").write_text(
        "type      = 1 # LINEAR_RAS_TO_RAS\nnxforms   = 1\nmean      = 0.0000 0.0000 0.0000\n"
        "sigma     = 1.0000\n1 4 4\n1 0 0 2\n0 1 0 -1\n0 0 1 0.5\n0 0 0 1\n",
        encoding="utf-8",
    )
    f["lta"] = d / "shift.lta"
    return f


def cases(f: dict[str, Path], fs_home: Path) -> list[list[str]]:
    """The command lines (``@x`` an input's absolute name; outputs are relative)."""
    o = ["--o", "out.mgz"]
    lut_all = str(fs_home / "FreeSurferColorLUT.txt")
    c: list[list[str]] = [
        [],
        ["--help"],
        ["--version"],
        ["--all-info"],
        ["--bogus"],
        ["-i", "@seg"],
        ["--i"],
        ["--i", "@seg"],
        ["--i", "@seg", *o],
        ["--i", "@seg", "--min", "1", "--match", "2", *o],
        ["--i", "@seg", "--min", "1", "--rmin", "0.5", *o],
        ["--i", "@seg", "--max", "1", "--rmax", "0.5", *o],
        ["--i", "@seg", "--min", "5", "--max", "1", *o],
        ["--i", "@seg", "--min", "1", "--checkopts", *o],
        ["--i", "@seg", "--min", "1", "--checkopts", "--nocheckopts", *o],
        ["--i", "@u8", "--min", "100", *o],
        ["--i", "@u8", "--min", "50", "--max", "100", *o, "--count", "count.txt"],
        ["--i", "@f32", "--max", "0", *o],
        ["--i", "@s16", "--min", "-1", *o],
        ["--i", "@s16", "--min", "-1", "--uchar", *o],
        ["--i", "@u8", "--min", "nan", *o],
        ["--i", "@u8", "--min", "abc", *o],
        ["--i", "@u8", "--rmin", "0.5", *o],
        ["--i", "@f32", "--rmin", "0.5", "--rmax", "1.5", *o],
        ["--i", "@u8", "--pct", "10", *o],
        ["--i", "@u8", "--pct", "25", "--mask", "@mask", *o],
        ["--i", "@u8", "--min", "60", "--mask", "@mask", "--mask-thresh", "0.3", *o],
        ["--i", "@u8", "--min", "60", "--mask", "@segb", *o],
        ["--i", "@seg", "--match", "2", "41", *o],
        ["--i", "@seg", "--match", "3", "42", "--inv", *o],
        ["--i", "@seg", "--match", "abc", "2", *o],
        ["--i", "@seg", "--match", "--o", "out.mgz"],
        ["--i", "@seg", "--ctx-wm", *o],
        ["--i", "@seg", "--wm", "--count", "c.txt"],
        ["--i", "@seg", "--all-wm", *o],
        ["--i", "@seg", "--ventricles", *o],
        ["--i", "@seg", "--wm+vcsf", *o],
        ["--i", "@seg", "--gm", *o],
        ["--i", "@seg", "--subcort-gm", *o],
        ["--i", "@seg", "--scm-lh", *o],
        ["--i", "@seg", "--scm-rh", *o],
        ["--i", "@seg", "--match-ctab", "@lut", *o],
        ["--i", "@seg", "--match-ctab", "@lut", "2", "41", *o],
        ["--i", "@seg", "--match-ctab", "/nonexistent/lut.txt", *o],
        ["--i", "@seg", "--match-ctab", lut_all, *o],
        ["--i", "@seg", "--match", "2", "--binval", "5", "--binvalnot", "-2", *o],
        ["--i", "@seg", "--match", "2", "--merge", "@merge", *o],
        ["--i", "@seg", "--match", "2", "--merge", "@segb", *o],
        ["--i", "@seg", "--match", "2", "--copy", "@merge", *o],
        ["--i", "@seg", "--match", "2", "--copy", "@small", *o],
        ["--i", "@seg", "--match", "2", "--zero-edges", "--binvalnot", "3", *o],
        ["--i", "@seg", "--match", "2", "--zero-slice-edges", "--merge", "@merge", *o],
        ["--i", "@seg", "--match", "2", "41", "--dilate", "2", *o],
        ["--i", "@seg", "--match", "2", "41", "--erode", "1", *o],
        ["--i", "@seg", "--match", "2", "41", "--erode-face", "2", *o],
        ["--i", "@seg", "--match", "2", "41", "--erode-edge", "1", "--erode-edges", *o],
        ["--i", "@seg", "--match", "2", "41", "--erode-corner", "1", *o],
        ["--i", "@seg", "--match", "2", "41", "--erode2d", "1", *o],
        ["--i", "@seg", "--match", "2", "41", "--bincol", *o],
        ["--i", "@seg", "--match", "17", "--bb", "2", *o],
        ["--i", "@seg", "--match", "17", "--crop", "0", *o],
        ["--i", "@seg", "--match", "2", "--remove-islands", *o],
        ["--i", "@seg", "--match", "2", "--fill-holes", *o],
        [
            "--i",
            "@seg",
            "--match",
            "2",
            "--remove-islands",
            "--no-remove-islands",
            "--fill-holes",
            "--no-fill-holes",
            *o,
        ],
        ["--i", "@seg", "--match", "2", "--inv", "--remove-islands", *o],
        ["--i", "@seg", "--match", "2", "--copy-ctab", *o],
        ["--i", "@seg", "--match", "2", "--ctab", "@lut", *o],
        ["--i", "@seg", "--match", "2", "--ctab", "/nonexistent/lut.txt", *o],
        ["--i", "@seg", "--replace", "2", "5", *o],
        ["--i", "@seg", "--replace", "2", "41", "--replace", "41", "3", *o],
        ["--i", "@seg", "--replaceonly", "2", "41", "--replaceonly", "41", "3", *o],
        [
            "--i",
            "@seg",
            "--replaceonly",
            "2",
            "41",
            "--replaceonly",
            "41",
            "3",
            "--no-transitive-replace",
            *o,
        ],
        ["--i", "@seg", "--replace", "2", "5", "--mask", "@mask", *o],
        ["--i", "@seg", "--min", "1", "--replace", "2", "5", *o],
        ["--i", "@seg", "--replace-nn", "77", "3", *o],
        ["--i", "@seg", "--replaceonly-nn", "77", "3", "--replaceonly-nn", "251", "5", *o],
        ["--i", "@seg", "--match", "2", "--replace-nn", "77", "3", *o],
        ["--i", "@seg", "--replaceonly", "2", "abc", *o],
        ["--i", "@f3", "--min", "0.5", *o],
        ["--i", "@f3", "--frame", "1", "--max", "-0.5", *o],
        ["--i", "@f3", "--frame", "2", "--mask", "@mask", "--max", "-1", *o],
        ["--i", "@f3", "--frame", "3", "--min", "0", *o],
        ["--i", "@f3", "--frame", "-1", "--min", "0", *o],
        ["--i", "@f3", "--abs", "--min", "1", *o],
        ["--i", "@f3", "--neg", "--min", "1", *o],
        ["--i", "@u83", "--frame-sum", "--min", "2", *o],
        ["--i", "@u83", "--frame-and", *o],
        ["--i", "@f3", "--min", "0", "--merge", "@seg", *o],
        ["--i", "@sig", "--fdr", "0.05", *o],
        ["--i", "@sig", "--fdr", "0.05", "--fdr-pos", *o],
        ["--i", "@sig", "--fdr", "0.05", "--fdr-neg", "--mask", "@mask", *o],
        ["--i", "@sig", "--fdr", "0.05", "--fdr-abs", *o],
        ["--i", "@seg", "--fdr", "0.05", *o],
        ["--i", "@seg", "--match", "2", "41", "--noverbose", *o],
        ["--i", "@seg", "--match", "2", "--no-count", *o, "--count", "c.txt"],
        ["--i", "@seg", "--match", "2", *o, "--count", "c.txt", "--no-count"],
        ["--i", "@seg", "--match", "2", "--count", "/nonexistent/dir/c.txt"],
        ["--i", "@seg", "--match", "2", "--debug", "--threads", "1", *o],
        ["--i", "@seg", "--match", "2", "--nthreads", "abc", *o],
        ["--i", "@segb", "--match", "2", "41", *o],
        ["--i", "@segu8", "--match", "2", "41", "--uchar", *o],
        ["--i", "@seg", "--match", "2", "--surf", "lh.out"],
        ["--i", "@seg", "--match", "17", "--surf", "lh.out", "--surf-smooth", "3"],
        ["--i", "@seg", "--match", "17", "--surf", "lh.out", "--reverse", *o],
        ["--i", "@torus", "--match", "7", "--surf", "lh.out", *o],
        ["--i", "@torus", "--match", "7", "--fix-vol-topo", "--surf", "lh.out", *o],
        ["--i", "@torus", "--match", "7", "--fix-vol-topo", "--no-fix-vol-topo", *o],
        ["--i", "@cube", "--match", "7", "--fix-vol-topo", *o],
        ["--i", "@small", "--match", "9", "--fix-vol-topo", *o],
        ["--ohe", "@small", "ohe.mgz", "1", "2", "4"],
        ["--ohe", "@small", "ohe.mgz"],
        ["--ohe", "/nonexistent/s.mgz", "ohe.mgz", "1"],
        ["--crop-around-ras", "crop.mgz", "@u8", "nolta", "0", "0", "0", "10", "12", "14"],
        [
            "--crop-around-ras",
            "crop.mgz",
            "@seg",
            "nolta",
            "2",
            "-3",
            "1",
            "16",
            "16",
            "16",
            "2",
            "41",
        ],
        ["--crop-around-ras", "crop.mgz", "@u8", "@lta", "0", "0", "0", "10", "10", "10"],
        ["--crop-around-ras", "crop.mgz", "@u8", "nolta", "cras", "@segb", "0", "8", "8", "8"],
        ["--crop-around-ras", "crop.mgz", "@u8", "nolta", "0", "0"],
        ["--crop-around-ras", "crop.mgz", "@u8", "nolta", "x", "0", "0", "10", "10", "10"],
        ["--dilate-vertex", "0", "@surf", "5", "dv.mgz"],
        ["--dilate-vertex", "0", "@surf", "5"],
        ["--dilate-vertex-sum", "2", "@surf", "area", "100", "dv.mgz"],
        ["--dilate-vertex-sum", "2", "@surf", "@meas", "4", "dv.mgz"],
        ["--dilate-vertex-sum", "2", "@surf", "nofile", "1e9", "dv.mgz"],
        ["--dilate-vertex-sum", "2", "@surf", "area", "100"],
        ["--dilate-vertex", "9", "@surf", "5", "dv.mgz"],
        ["--i", "/nonexistent/in.mgz", "--min", "1", *o],
        ["--i", "@seg", "--min", "1", "--o", "/nonexistent/dir/out.mgz"],
        ["--i", "@seg", "--min", "1", "--mask", "/nonexistent/m.mgz", *o],
        ["--i", "@seg", "--min", "1", "--merge", "/nonexistent/m.mgz", *o],
        ["--i", "@seg", "--match", "2", "--min"],
    ]
    return [[str(f[a[1:]]) if a.startswith("@") else a for a in x] for x in c]


def mask(text: str, port: bool) -> str:
    text = CWD.sub("cwd <cwd>", text)
    if port:
        text = "".join(x for x in text.splitlines(True) if not x.startswith("larmorx: "))
    return text


def notices(text: str) -> list[str]:
    return sorted(
        {m.group(1) for m in re.finditer(r"^larmorx: upstream defect (\S+) ", text, re.M)}
    )


def written(folder: Path) -> dict[str, Path]:
    return {p.name: p for p in sorted(folder.iterdir()) if p.is_file()}


def official() -> Path:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    return ws / FS_HOME / "bin" / "mri_binarize"


def run(
    seed: int,
    candidate: list[str],
    *,
    modes: tuple[str, ...] = ("reproduce", "fix"),
    only: str | None = None,
    show: int = 6,
    skip: frozenset[str] = NOT_COMPARABLE,
    tag: str = "candidate",
) -> dict:
    """Every case of ``seed``: the official program against ``candidate`` in each of ``modes``
    (the first is the one that must give FreeSurfer's results); the counts and each case's row."""
    ws = workspace()
    exe = official()
    f = inputs(ws, seed)
    all_cases = cases(f, exe.parent.parent)
    keep = {int(x) for x in only.split(",")} if only else set(range(len(all_cases)))
    work = ws / "oracles" / "inputs" / "fs-binarize-prog" / f"work-{tag}-{seed}"
    # argv[0] as the scripts give it (the program's messages print it): the name, on the PATH
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(work), "USER": "probe"}  # fmt: skip
    env.update(freesurfer_env())
    ctx = fsfiles.Context(any_directory=True)
    rows = []
    verdicts: Counter = Counter()
    shown = 0
    for i, args in enumerate(all_cases):
        if i not in keep:
            continue
        sides = {}
        for side in ("official", *modes):
            d = work / f"{i:03d}" / side
            if d.exists():
                shutil.rmtree(d)
            d.mkdir(parents=True)
            if side == "official":
                done = oracle_cache.run(
                    ["mri_binarize", *args],
                    env=env,
                    cwd=d,
                    outputs=[d],
                    policy=POLICY,
                    timeout=1800,
                    runner="fs-binarize-prog",
                )
                sides[side] = (done.returncode, done.stdout, done.stderr, d)
            else:
                p = subprocess.run(
                    [*(w.replace("{mode}", side) for w in candidate), *args],
                    env=env,
                    cwd=d,
                    capture_output=True,
                    text=True,
                    timeout=1800,
                )
                sides[side] = (p.returncode, p.stdout, p.stderr, d)
        oc, oo, oe, od = sides["official"]
        row = {"case": i, "args": args, "official_exit": oc}
        for side in modes:
            pc, po, pe, pd = sides[side]
            diffs = []
            if pc != oc:
                diffs.append(f"exit {pc} != {oc}")
            if mask(po, True) != mask(oo, False):
                diffs.append("stdout")
            if mask(pe, True) != mask(oe, False):
                diffs.append("stderr")
            fo, fp = written(od), written(pd)
            if sorted(fo) != sorted(fp):
                diffs.append(f"files {sorted(fo)} != {sorted(fp)}")
            for name in sorted(set(fo) & set(fp)):
                c = fsfiles.compare(fo[name], fp[name], ctx, name=name)
                if not c.same:
                    diffs.append(f"{name}: {c.verdict} {c.detail[:200]}")
            ids = notices(pe)
            row[side] = {"exit": pc, "differ": diffs, "notices": ids}
            rep_ids = notices(sides[modes[0]][2])
            if any(x in skip for x in rep_ids):
                row[side]["skip"] = True
                verdicts[f"{side}-skip"] += 1
                continue
            if side == modes[0]:
                verdict = "identical" if not diffs else "different"
            else:
                verdict = (
                    "fix-identical"
                    if not diffs
                    else ("fix-explained" if ids else "fix-unexplained")
                )
            verdicts[verdict] += 1
            if diffs and shown < show and (side == modes[0] or not ids):
                shown += 1
                print(f"case {i} {side}: {args}\n  {diffs}\n  official stdout tail: "
                      f"{oo[-300:]!r}\n  candidate stdout tail: {po[-300:]!r}\n  official "
                      f"stderr: {oe[-300:]!r}\n  candidate stderr: {pe[-400:]!r}",
                      flush=True)  # fmt: skip
        rows.append(row)
        print(f"{i:3d} exit {oc} " + " ".join(
            f"{m} {'ok' if not row[m]['differ'] else 'DIFF ' + ','.join(row[m]['notices'])}"
            for m in modes), flush=True)  # fmt: skip
    return {"seed": seed, "cases": len(rows), "verdicts": dict(verdicts), "rows": rows}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--candidate", required=True, help="the command line; {mode} is replaced")
    ap.add_argument("--only", help="comma-separated case numbers")
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", type=int, default=6)
    a = ap.parse_args(argv)
    summary = run(a.seed, a.candidate.split(), only=a.only, show=a.show)
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    v = summary["verdicts"]
    return 1 if v.get("different", 0) + v.get("fix-unexplained", 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
