# SPDX-License-Identifier: Apache-2.0
"""mri_fill as a black box: FreeSurfer 8.2.0's official program against a candidate.

An open probe (``README.md``): it writes seeded synthetic volumes into
``<workspace>/oracles/inputs/fs-fill-prog/<seed>/s/``: 32^3 coronal volumes of 2 mm: a white matter
volume (``wm.mgz``, bytes: two hemispheres joined by a thin bridge above a brainstem, with the edit
values 255 and 1, the fill labels 200 to 240, values below 5 and random voxels added and removed),
a segmentation of it (``aseg.mgz``: left and right white matter and cortex, the bridge as corpus
callosum labels, the brainstem, cerebellum, lesions and white-matter hypointensities), an intensity
volume (``norm.mgz``), the white matter as ``float`` (NaNs and infinities by their bits), with two
frames, of 1 mm voxels, of 2 x 2 x 2 and of one slice, an MNI transform (``tal.xfm``, a translation)
and its text broken (``bad.xfm``), a colour table, control points, an edited copy for
``-auto-man``, and a morph file that is not one. It runs each case's command line through the
official program (``$FREESURFER_HOME/bin/mri_fill``, found on the ``PATH`` by its name as recon-all
runs it) by the oracle-output cache at one thread, and through the candidate (``--candidate``, a
command line whose ``{mode}`` becomes ``reproduce`` and then ``fix``), each in a folder of its own
holding a copy of the case's files. Compared: the exit status, standard output and error (the case's
folder, ``--all-info``'s time, the ``filling took`` line and the candidate's ``larmorx:`` notices
aside) and every file written (FreeSurfer's provenance aside: ``larmorx_validation.freesurfer.files``).
``reproduce`` must give FreeSurfer's on every case; ``fix`` may differ only with a notice.
``--heads`` adds cases on recorded heads with options recon-all does not give (the fill without a
segmentation, ``-topofix``, seeds, ``-lhonly``, ``-rhonly``, ``-fillonly``, ``-auto-man``, ...).

No case gives the official program a diagnostic setting: no ``DIAG*`` variable and no option
starting ``-diag`` or ``-w`` (the licence check prints the licence file from a diagnostic level of
1). The probe refuses such a case.

    python -P validation/investigations/freesurfer-mri-fill-probe/compare.py \\
        --seed 1 --candidate '<program> --upstream-defects {mode} freesurfer mri_fill'
"""

from __future__ import annotations

import argparse
import concurrent.futures
import json
import os
import re
import shutil
import stat
import subprocess
import sys
from collections import Counter
from dataclasses import dataclass, field
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

#: The defects (``larmorx_core::defects``) whose cases are counted, not compared: FreeSurfer reads
#: or writes memory it does not own there (voxels outside the volumes and their buffers).
NOT_COMPARABLE = frozenset({"FS-52", "FS-53", "FS-1300"})
#: FreeSurfer's runs compared with their entries: ``--all-info``'s report and the fill's time.
POLICY = oracle_cache.Policy(
    threads=1,
    family="freesurfer",
    formats="freesurfer",
    volatile=(r"TimeStamp: \S+", r"filling took \S+ minutes"),
)
STAMP = re.compile(r"TimeStamp: \S+")
TOOK = re.compile(r"filling took \S+ minutes")
#: Never a diagnostic setting on the official program (an option or the variables).
DIAG_ARG = re.compile(r"^(--?diag|-[wW])", re.I)
DIAG_ENV = re.compile(r"^(DIAG|diag)")
N = 32
#: The recorded heads (``oracles/freesurfer-8.2.0/runs/<run>/``) for ``--heads``.
HEAD_RUNS = ("ds000005-sub-01-classic", "ds004173-sub-603309-v8")


def lia(shape: tuple[int, ...], size: float = 1.0) -> np.ndarray:
    """A coronal (LIA) grid of ``shape`` voxels of ``size`` mm centred on the origin."""
    a, b, c = (n / 2 * size for n in shape[:3])
    return np.array([[-size, 0, 0, a], [0, 0, size, -c], [0, -size, 0, b], [0, 0, 0, 1.0]])


def save(path: Path, data: np.ndarray, affine: np.ndarray) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    nib.save(nib.MGHImage(data, affine), str(path))
    return path


def with_bits(data: np.ndarray, places: list[tuple[tuple[int, int, int], int]]) -> np.ndarray:
    """``float32`` data with the given voxels set from their bit patterns (no float conversion)."""
    bits = data.astype(np.float32).view(np.uint32).copy()
    for (x, y, z), b in places:
        bits[x, y, z] = b
    return bits.view(np.float32)


def brain(rng: np.random.Generator) -> tuple[np.ndarray, np.ndarray]:
    """The white matter and its segmentation (x from the right to the left, y from the top down,
    z from the back to the front): two hemispheres (x 2-13 and 18-29, y 2-14, z 4-27), a bridge
    between them (x 13-18, y 6-10, z 6-25), a brainstem below (x 11-20, y 17-30, z 12-21)."""
    shape = (N, N, N)
    wm = np.zeros(shape, np.uint8)
    seg = np.zeros(shape, np.int32)
    wm[2:14, 2:15, 4:28] = 110
    wm[18:30, 2:15, 4:28] = 110
    wm[13:19, 6:11, 6:26] = 110
    wm[11:21, 17:31, 12:22] = 110
    seg[2:14, 2:15, 4:28] = 41
    seg[18:30, 2:15, 4:28] = 2
    seg[1:15, 1:16, 3:29][(seg[1:15, 1:16, 3:29] == 0)] = 42
    seg[17:31, 1:16, 3:29][(seg[17:31, 1:16, 3:29] == 0)] = 3
    seg[13:16, 6:11, 6:26] = 41
    seg[16:19, 6:11, 6:26] = 2
    seg[14:18, 7:10, 8:24] = rng.choice([251, 252, 253, 254, 255], size=(4, 3, 16))
    seg[11:21, 17:31, 12:22] = 16
    seg[3:8, 10:14, 4:8] = 8
    seg[3:8, 12:14, 4:6] = 7
    seg[24:29, 10:14, 4:8] = 47
    seg[24:29, 12:14, 4:6] = 46
    # random voxels removed and added; edits, fill labels, lesions, hypointensities
    u = rng.uniform(size=shape)
    on = wm > 0
    wm[on & (u < 0.02)] = 0
    wm[on & (u > 0.99)] = 255
    wm[(~on) & (u > 0.995)] = 110
    wm[on & (u > 0.5) & (u < 0.505)] = 250
    for k, v in enumerate((200, 210, 220, 230, 240)):
        wm[on & (u > 0.6 + k * 0.004) & (u < 0.602 + k * 0.004)] = v
    wm[on & (u > 0.7) & (u < 0.705)] = 1
    wm[on & (u > 0.8) & (u < 0.803)] = rng.integers(2, 5, size=shape)[on & (u > 0.8) & (u < 0.803)]
    seg[on & (wm < 5) & (u > 0.8) & (u < 0.802)] = 25
    seg[on & (wm < 5) & (u > 0.7) & (u < 0.702)] = 77
    return wm, seg


def small(d: Path, rng: np.random.Generator) -> None:
    """The inputs in ``s``: see the module's text."""
    shape = (N, N, N)
    aff = lia(shape, 2.0)
    s = d / "s"
    wm, seg = brain(rng)
    save(s / "wm.mgz", wm, aff)
    save(s / "aseg.mgz", seg, aff)
    save(s / "aseg_uchar.mgz", seg.astype(np.uint8), aff)
    nocc = seg.copy()
    nocc[nocc >= 251] = 2
    save(s / "aseg_nocc.mgz", nocc, aff)
    onehemi = seg.copy()
    onehemi[onehemi == 41] = 2
    save(s / "aseg_onehemi.mgz", onehemi, aff)
    save(s / "empty.mgz", np.zeros(shape, np.uint8), aff)
    norm = (rng.integers(0, 12, size=shape) * 10 + (wm > 0) * 50).astype(np.uint8)
    save(s / "norm.mgz", norm, aff)
    wf = wm.astype(np.float32)
    wf[wm == 110] = 110.4
    nan = [((x, y, z), b) for (x, y, z, b) in
           ((5, 5, 10, 0x7FC00000), (6, 5, 10, 0xFFC00000), (20, 5, 10, 0x7F800000),
            (21, 5, 10, 0xFF800000), (22, 5, 10, 0x7FA00000))]  # fmt: skip
    save(s / "wm_float.mgz", with_bits(wf, nan), aff)
    save(s / "wm_2f.mgz", np.stack([wm, wm[::-1]], axis=-1), aff)
    save(s / "wm_1mm.mgz", wm, lia(shape, 1.0))
    save(s / "aseg_1mm.mgz", seg, lia(shape, 1.0))
    save(s / "wm_2.mgz", wm[14:16, 6:8, 10:12].copy(), lia((2, 2, 2), 2.0))
    save(s / "wm_1slice.mgz", wm[:, :, 16:17].copy(), lia((N, N, 1), 2.0))
    # an edited fill for -auto-man (filled later from the official output: a copy with edits)
    auto = np.zeros(shape, np.uint8)
    auto[2:14, 2:15, 4:28] = 127
    auto[18:30, 2:15, 4:28] = 255
    man = auto.copy()
    man[5:7, 5:7, 10:12] = 0
    man[22, 8, 15] = 1
    save(s / "auto.mgz", auto, aff)
    save(s / "man.mgz", man, aff)
    save(s / "man_small.mgz", man[:16, :16, :16].copy(), lia((16, 16, 16), 2.0))
    (s / "tal.xfm").write_text(
        "MNI Transform File\n% a translation to the average 305 centre\n\n"
        "Transform_Type = Linear;\nLinear_Transform =\n"
        " 1 0 0 -0.095\n 0 1 0 -16.51\n 0 0 1 9.75;\n",
        encoding="utf-8",
    )
    (s / "bad.xfm").write_text("not a transform\n", encoding="utf-8")
    ctab = fs_home_dir() / "SubCorticalMassLUT.txt"
    shutil.copy(ctab, s / "lut.txt")
    (s / "bad_lut.txt").write_text("", encoding="utf-8")
    (s / "points.dat").write_text(
        "-8 4 -2\n-8 4 6\n-4 2 2\ninfo\nnumpoints 3\nuseRealRAS 1\n", encoding="utf-8"
    )
    tr = s / "sub" / "mri" / "transforms"
    tr.mkdir(parents=True, exist_ok=True)
    (tr / "atl.m3d").write_bytes(b"\x00\x01\x02\x03 not a morph")


def fs_home_dir() -> Path:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    return ws / FS_HOME


def official() -> Path:
    return fs_home_dir() / "bin" / "mri_fill"


def base_env(home: Path) -> dict[str, str]:
    exe = official()
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(home), "USER": "probe",
           "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    return env


def inputs(ws: Path, seed: int) -> Path:
    """The inputs of ``seed`` (written once: a marker file says they are complete)."""
    d = ws / "oracles" / "inputs" / "fs-fill-prog" / str(seed)
    done = d / ".complete-v1"
    if done.exists():
        return d
    if d.exists():
        shutil.rmtree(d)
    small(d, np.random.default_rng(seed))
    done.write_text("complete\n", encoding="utf-8")
    return d


@dataclass
class Case:
    """A command line, the input folder whose files the case's folder holds, the variables it
    adds, whether that folder is made read-only."""

    args: list[str]
    folder: str = "s"
    env: dict[str, str] = field(default_factory=dict)
    readonly: bool = False


SEG = ["-segmentation", "aseg.mgz"]
IO = ["wm.mgz", "out.mgz"]


def cases() -> list[Case]:
    c: list[Case] = [
        # the command line
        Case([]),
        Case(["wm.mgz"]),
        Case([*IO, "extra.mgz"]),
        Case(["--help"]),
        Case(["--usage"]),
        Case(["-help"]),
        Case(["-u"]),
        Case(["-?"]),
        Case(["-h"]),
        Case(["--version"]),
        Case(["-version"]),
        Case(["--all-info"]),
        Case(["--version", *IO]),
        Case(["-zzz", *IO]),
        Case(["-", *IO]),
        Case(["missing.mgz", "out.mgz"]),
        Case([*SEG[:1], "missing.mgz", *IO]),
        Case(["-xform", "missing.xfm", *IO]),
        Case(["-xform", "bad.xfm", *IO]),
        Case(["-ctab", "missing.txt", *IO]),
        Case(["-ctab", "bad_lut.txt", *IO]),
        Case([*SEG, "wm.mgz", "nodir/out.mgz"]),
        Case([*SEG, *IO], readonly=True),
        Case(["-a", "nodir/log.txt", *IO]),
        Case(["-T", "x", *IO]),
        # options without their values (each as the last argument)
        *(
            Case([o])
            for o in (
                "-rval",
                "-lval",
                "-fillven",
                "-segmentation",
                "-topofix",
                "-xform",
                "-ctab",
                "-T",
                "-F",
                "-a",
                "-L",
                "-atlas",
            )
        ),
        Case(["-debug_voxel", "1", "2"]),
        Case(["-lh", "1", "2"]),
        Case(["-CV", "1"]),
        Case(["-P", "1", "2"]),
        Case(["-atlas", "atl"]),
        Case(["-auto-man", "auto.mgz", "man.mgz"]),
        Case(["-auto-man", "auto.mgz"]),
        Case(["-pointset", "wm.mgz"]),
        # the segmentation's path
        Case([*SEG, *IO]),
        Case(["-segmentation", "aseg_uchar.mgz", *IO]),
        Case(["-segmentation", "aseg_nocc.mgz", *IO]),
        Case(["-segmentation", "aseg_onehemi.mgz", *IO]),
        Case(["-segmentation", "empty.mgz", *IO]),
        Case([*SEG, "-ctab", "lut.txt", *IO]),
        Case([*SEG, "-a", "cut.log", "-L", "cc.txt", *IO]),
        Case([*SEG, "-xform", "tal.xfm", *IO]),
        Case([*SEG, "-xform", "tal.xfm", *IO], env={"NO_AVERAGE305": "1"}),
        Case([*SEG, "-lhonly", *IO]),
        Case([*SEG, "-rhonly", *IO]),
        Case([*SEG, "-lhonly", "-rhonly", *IO]),
        Case([*SEG, "-rval", "100", "-lval", "200", *IO]),
        Case([*SEG, "-rval", "300", *IO]),
        Case([*SEG, "-topofix", "norm.mgz", *IO]),
        Case([*SEG, "-topofix", "missing.mgz", *IO]),
        Case([*SEG, "-auto-man", "auto.mgz", "man.mgz", "edits.txt", *IO]),
        Case([*SEG, "-auto-man", "auto.mgz", "man.mgz", "nofile", *IO]),
        Case([*SEG, "-auto-man", "auto.mgz", "man.mgz", "edits.txt", "-no-auto-man", *IO]),
        Case([*SEG, "-auto-man", "auto.mgz", "man_small.mgz", "edits.txt", *IO]),
        Case([*SEG, "-auto-man", "auto.mgz", "man.mgz", "nodir/e.txt", *IO]),
        Case([*SEG, "-F", "3", *IO]),
        Case([*SEG, "-D", *IO]),
        Case([*SEG, "-ccmask", *IO]),
        Case([*SEG, "-debug_voxel", "5", "5", "5", *IO]),
        Case([*SEG, "-fillven", "1", *IO]),
        Case([*SEG, "-T", "3", *IO]),
        Case([*SEG, "-C", "0", "10", "6", *IO]),
        Case([*SEG, "-C", "0", "10", "600", *IO]),
        Case([*SEG, "-CV", "14", "8", "16", *IO]),
        Case([*SEG, "-CV", "14", "8", "40", *IO]),
        Case([*SEG, "-lh", "-18", "10", "6", "-rh", "18", "10", "6", *IO]),
        Case([*SEG, "-lhv", "23", "8", "16", "-rhv", "6", "8", "16", *IO]),
        Case([*SEG, "-lhv", "40", "8", "16", *IO]),
        Case([*SEG, "-atlas", "atl", "sub", *IO], env={"SUBJECTS_DIR": "."}),
        Case([*SEG, "-atlas", "missing", "sub", *IO], env={"SUBJECTS_DIR": "."}),
        Case(["-segmentation", "aseg_1mm.mgz", "wm_1mm.mgz", "out.mgz"]),
        Case([*SEG, "wm_float.mgz", "out.mgz"]),
        Case([*SEG, "wm_float.mgz", "out.mgz"], env={"FS_LEAVE_NANS": "1"}),
        Case([*SEG, "wm_2f.mgz", "out.mgz"]),
        Case([*SEG, "wm.mgz", "out.nii.gz"]),
        # without a segmentation
        Case(IO),
        Case(["-xform", "tal.xfm", *IO]),
        Case(["-ccmask", *IO]),
        Case(["-C", "0", "10", "6", "-P", "0", "40", "0", *IO]),
        Case(["-CV", "14", "8", "16", "-PV", "15", "20", "17", *IO]),
        Case(["-PV", "15", "60", "17", *IO]),
        Case(["-P", "0", "-500", "0", *IO]),
        Case(["-lhonly", *IO]),
        Case(["-rhonly", *IO]),
        Case(["-T", "0", *IO]),
        Case(["-D", "-a", "cut.log", "-L", "cc.txt", *IO]),
        Case(["-F", "5", *IO]),
        Case(["wm_1mm.mgz", "out.mgz"]),
        Case(["empty.mgz", "out.mgz"]),
        Case(["wm_2.mgz", "out.mgz"]),
        # -fillonly, -findrhv, -pointset
        Case(["-fillonly", *IO]),
        Case(["-fillonly", "-rhv", "6", "8", "16", *IO]),
        Case(["-fillonly", "-lhv", "23", "8", "16", "-lval", "300", *IO]),
        Case(["-fillonly", "-rhv", "6", "8", "16", "wm_1slice.mgz", "out.mgz"]),
        Case(["-findrhv", "-fillonly", *IO]),
        Case(["-findlhv", "-fillonly", "empty.mgz", "out.mgz"]),
        Case(["-pointset", "wm.mgz", "pts.mgz"]),
        Case(["-pointset", "wm.mgz", "pts.mgz", "points.dat"]),
        Case(["-pointset", "wm.mgz", "pts.mgz", "missing.dat"]),
        Case(["-pointset", "missing.mgz", "pts.mgz", "points.dat"]),
    ]
    return c


def heads(ws: Path) -> tuple[Path, list[Case]]:
    """The recorded heads: for each run a folder with its ``wm.mgz``, ``aseg.presurf.mgz``,
    ``norm.mgz``, ``filled.mgz`` (and an edited copy), ``transforms/talairach.lta`` and the colour
    table; options recon-all does not give."""
    root = ws / "oracles" / "inputs" / "fs-fill-prog" / "heads"
    out: list[Case] = []
    for run in HEAD_RUNS:
        subjects = ws / "oracles" / "freesurfer-8.2.0" / "runs" / run / "subjects"
        rec = next(p for p in sorted(subjects.iterdir()) if p.name != "fsaverage") / "mri"
        m = root / run
        done = m / ".complete-v1"
        if not done.exists():
            if m.exists():
                shutil.rmtree(m)
            (m / "transforms").mkdir(parents=True)
            for f in ("wm.mgz", "aseg.presurf.mgz", "norm.mgz", "filled.mgz"):
                shutil.copy(rec / f, m)
            shutil.copy(rec / "transforms" / "talairach.lta", m / "transforms")
            shutil.copy(fs_home_dir() / "SubCorticalMassLUT.txt", m / "lut.txt")
            img = nib.load(str(m / "filled.mgz"))
            data = np.asarray(img.dataobj).copy()
            data[120:124, 100:104, 120:124] = 0
            data[60, 110, 128] = 255
            nib.save(nib.MGHImage(data, img.affine, img.header), str(m / "filled_edit.mgz"))
            done.write_text("complete\n", encoding="utf-8")
        x = ["-xform", "transforms/talairach.lta"]
        sg = ["-segmentation", "aseg.presurf.mgz"]
        io = ["wm.mgz", "out.mgz"]
        out += [
            Case([*x, *io], folder=run),
            Case([*x, "-ccmask", "-T", "3", *io], folder=run),
            Case([*x, *sg, "-topofix", "norm.mgz", *io], folder=run),
            Case([*x, *sg, "-lhonly", *io], folder=run),
            Case([*x, *sg, "-rhonly", "-rval", "120", *io], folder=run),
            Case(
                [
                    *x,
                    "-C",
                    "0",
                    "-20",
                    "18",
                    "-P",
                    "-2",
                    "-15",
                    "-17",
                    "-a",
                    "cut.log",
                    "-L",
                    "cc.txt",
                    "-D",
                    *io,
                ],
                folder=run,
            ),
            Case([*x, "-CV", "128", "110", "128", *io], folder=run),
            Case([*x, "-fillonly", "-rhv", "100", "120", "128", *io], folder=run),
            Case(
                [
                    *x,
                    *sg,
                    "-auto-man",
                    "filled.mgz",
                    "filled_edit.mgz",
                    "edits.txt",
                    "-ctab",
                    "lut.txt",
                    *io,
                ],
                folder=run,
            ),
        ]
    return root, out


def mask(text: str, port: bool, folder: Path) -> str:
    """The messages without what differs between the sides: the time (``--all-info``'s report
    and the fill's), the case's folder, the candidate's notices."""
    text = STAMP.sub("TimeStamp: <time>", text)
    text = TOOK.sub("filling took <t> minutes", text)
    text = text.replace(str(folder), "<folder>")
    if port:
        # the notices follow at the end, also after a line cut short by a crash
        text = re.sub(r"larmorx: upstream defect [^\n]*\n", "", text)
    return text


def status(code: int | None) -> int | None:
    """An exit status as a shell reports it: a process ended by signal ``n`` as ``128 + n``;
    ``None`` for a run stopped at its time limit."""
    if code is None:
        return None
    return 128 - code if code < 0 else code


def notices(text: str) -> list[str]:
    return sorted({m.group(1) for m in re.finditer(r"larmorx: upstream defect (\S+) ", text)})


def listing(folder: Path) -> dict[str, tuple[int, int]]:
    return {
        str(p.relative_to(folder)): (p.stat().st_size, p.stat().st_mtime_ns)
        for p in sorted(folder.rglob("*"))
        if p.is_file()
    }


def written(folder: Path, before: dict[str, tuple[int, int]]) -> dict[str, Path]:
    """The files the run made or changed."""
    return {k: folder / k for k, v in listing(folder).items() if before.get(k) != v}


def prepare(src: Path, d: Path, case: Case) -> dict[str, tuple[int, int]]:
    if d.exists():
        unlock(d)
        shutil.rmtree(d)
    shutil.copytree(src / case.folder, d)
    for p in d.glob(".complete*"):
        p.unlink()
    before = listing(d)
    if case.readonly:
        d.chmod(stat.S_IRUSR | stat.S_IXUSR | stat.S_IRGRP | stat.S_IXGRP)
    return before


def unlock(d: Path) -> None:
    d.chmod(0o755)
    for p in d.rglob("*"):
        if p.is_dir():
            p.chmod(0o755)


def run_case(
    i: int, case: Case, src: Path, work: Path, env: dict[str, str], candidate: list[str],
    modes: tuple[str, ...], ctx: fsfiles.Context, runner: str,
) -> dict:  # fmt: skip
    if any(DIAG_ENV.match(k) for k in case.env) or any(DIAG_ARG.match(a) for a in case.args):
        raise SystemExit(f"case {i}: a diagnostic setting on the official program")
    sides = {}
    for side in ("official", *modes):
        d = work / f"{i:04d}" / side
        before = prepare(src, d, case)
        e = dict(env)
        e.update(case.env)
        if "SUBJECTS_DIR" in case.env:
            e["SUBJECTS_DIR"] = str(d)
        if side == "official":
            given = sorted(p for p in d.rglob("*") if p.is_file())
            done = oracle_cache.run(
                ["mri_fill", *case.args],
                env=e,
                cwd=d,
                outputs=[d],
                policy=POLICY,
                timeout=900,
                runner=runner,
                inputs=given,
            )
            out = (status(done.returncode), done.stdout, done.stderr)
        else:
            p = subprocess.run(
                [*(w.replace("{mode}", side) for w in candidate), *case.args],
                env=e,
                cwd=d,
                capture_output=True,
                text=True,
                errors="surrogateescape",
                timeout=900,
            )
            out = (p.returncode, p.stdout, p.stderr)
        unlock(d)
        sides[side] = (*out, d, before)
    oc, oo, oe, od, ob = sides["official"]
    row: dict = {"case": i, "args": case.args, "folder": case.folder, "env": case.env,
                 "official_exit": oc}  # fmt: skip
    fo = written(od, ob)
    for side in modes:
        pc, po, pe, pd, pb = sides[side]
        diffs = []
        if pc != oc:
            diffs.append(f"exit {pc} != {oc}")
        if mask(po, True, pd) != mask(oo, False, od):
            diffs.append("stdout")
        if mask(pe, True, pd) != mask(oe, False, od):
            diffs.append("stderr")
        fp = written(pd, pb)
        if sorted(fo) != sorted(fp):
            diffs.append(f"files {sorted(fo)} != {sorted(fp)}")
        for name in sorted(set(fo) & set(fp)):
            cmp = fsfiles.compare(fo[name], fp[name], ctx, name=Path(name).name)
            if not cmp.same:
                diffs.append(f"{name}: {cmp.verdict} {cmp.detail[:200]}")
        row[side] = {"exit": pc, "differ": diffs, "notices": notices(pe)}
        if diffs:
            row[side]["tails"] = {"official_out": oo[-300:], "candidate_out": po[-300:],
                                  "official_err": oe[-300:], "candidate_err": pe[-400:]}  # fmt: skip
    for side in (*modes, "official"):
        shutil.rmtree(work / f"{i:04d}" / side, ignore_errors=True)
    return row


def verdict(row: dict, modes: tuple[str, ...], skip: frozenset[str]) -> dict[str, str]:
    out = {}
    rep_ids = row[modes[0]]["notices"]
    for side in modes:
        r = row[side]
        if row["official_exit"] is None:
            out[side] = f"{side}-official-timeout"
        elif any(x in skip for x in rep_ids):
            out[side] = f"{side}-skip"
        elif side == modes[0]:
            out[side] = "identical" if not r["differ"] else "different"
        else:
            out[side] = ("fix-identical" if not r["differ"] else
                         ("fix-explained" if r["notices"] else "fix-unexplained"))  # fmt: skip
    return out


def run(
    seed: int,
    candidate: list[str],
    *,
    modes: tuple[str, ...] = ("reproduce", "fix"),
    only: str | None = None,
    jobs: int = 4,
    with_heads: bool = False,
    skip: frozenset[str] = NOT_COMPARABLE,
    tag: str = "candidate",
) -> dict:
    """Every case of ``seed``: the official program against ``candidate`` in each of ``modes``
    (the first is the one that must give FreeSurfer's results); the counts and each case's row."""
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    work = ws / "oracles" / "inputs" / "fs-fill-prog" / f"work-{tag}-{seed}"
    work.mkdir(parents=True, exist_ok=True)
    env = base_env(work)
    src = inputs(ws, seed)
    todo = [(i, c, src) for i, c in enumerate(cases())]
    if with_heads:
        hsrc, hcases = heads(ws)
        todo += [(1000 + i, c, hsrc) for i, c in enumerate(hcases)]
    keep = {int(x) for x in only.split(",")} if only else None
    todo = [t for t in todo if keep is None or t[0] in keep]
    ctx = fsfiles.Context(any_directory=True)
    verdicts: Counter = Counter()
    rows = []
    with concurrent.futures.ThreadPoolExecutor(max(1, jobs)) as pool:
        futures = {
            pool.submit(run_case, i, c, s, work, env, candidate, modes, ctx, "fs-fill-prog"): i
            for i, c, s in todo
        }
        for fut in concurrent.futures.as_completed(futures):
            row = fut.result()
            v = verdict(row, modes, skip)
            row["verdict"] = v
            verdicts.update(v.values())
            rows.append(row)
            print(f"{row['case']:4d} exit {row['official_exit']} " + " ".join(
                f"{m} {'ok' if not row[m]['differ'] else 'DIFF ' + ','.join(row[m]['notices'])}"
                for m in modes), flush=True)  # fmt: skip
            for m in modes:
                shown = m == modes[0] or not row[m]["notices"]
                if row[m]["differ"] and shown and not v[m].endswith(("-skip", "-timeout")):
                    print(f"  {m}: {row['args']} {row[m]['differ']}\n  "
                              f"{row[m].get('tails')}", flush=True)  # fmt: skip
    rows.sort(key=lambda r: r["case"])
    return {"seed": seed, "cases": len(rows), "verdicts": dict(verdicts), "rows": rows}


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--candidate", required=True, help="the command line; {mode} is replaced")
    ap.add_argument("--only", help="comma-separated case numbers (the heads' from 1000)")
    ap.add_argument("--jobs", type=int, default=4, help="cases run at once")
    ap.add_argument("--heads", action="store_true", help="add the recorded heads' cases")
    ap.add_argument("--json", type=Path)
    a = ap.parse_args(argv)
    os.environ.pop("FSLDIR", None)
    summary = run(a.seed, a.candidate.split(), only=a.only, jobs=a.jobs, with_heads=a.heads)
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    v = summary["verdicts"]
    return 1 if v.get("different", 0) + v.get("fix-unexplained", 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
