# SPDX-License-Identifier: Apache-2.0
"""mri_segment as a black box: FreeSurfer 8.2.0's official program against a candidate, case by case.

An open probe (``README.md``): it writes seeded synthetic volumes into
``<workspace>/oracles/inputs/fs-segment-prog/<seed>/``: small brain-like volumes of 32^3 coronal
voxels (a white-matter core, a graded border, a gray shell, cerebrospinal fluid, a white plate one
voxel thick along the rows, a thicker plate, bright spots outside the brain, voxels touching only
diagonally) as bytes, as ``float`` with NaNs (quiet and signalling, written by their bits) and
infinities, as shorts, of another extent, not coronal, all 0 and constant; masks and
segmentations (``aseg.presurf.mgz``) of the same and of other extents; then runs each case's
command line through the official program (``$FREESURFER_HOME/bin/mri_segment``, found on the
``PATH`` by its name as recon-all runs it) by the oracle-output cache at one thread, and through
the candidate (``--candidate``, a command line whose ``{mode}`` becomes ``reproduce`` and then
``fix``), each in a folder of its own holding a copy of the case's files. Compared: the exit
status, standard output and error (the run's time, the case's folder and the candidate's
``larmorx:`` notices aside) and every file written (FreeSurfer's provenance aside:
``larmorx_validation.freesurfer.files``). ``reproduce`` must give FreeSurfer's on every case; ``fix``
may differ only with a notice. ``--heads`` adds cases on recorded heads (their ``antsdn.brain.mgz``,
``aseg.presurf.mgz`` and ``wm.mgz``) with options recon-all does not give.

No case gives the official program a diagnostic setting (``-diagno``, ``-diag_no``,
``-diag-write``, ``-diag-verbose`` or any ``DIAG*`` variable): its licence check prints the
licence file from a diagnostic level of 1. The probe refuses such a case.

    python -P validation/investigations/freesurfer-mri-segment-probe/compare.py --seed 1 \\
        --candidate '<program> --upstream-defects {mode} freesurfer mri_segment' [--heads]
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
#: or writes memory it does not own there, where the spec gives an error in both modes or reads 0
#: what FreeSurfer never wrote.
NOT_COMPARABLE = frozenset(
    {
        "FS-52",
        "FS-53",
        "FS-498",
        "FS-1216",
        "FS-1217",
        "FS-1218",
        "FS-1222",
        "FS-1223",
        "FS-1228",
        "FS-1231",
    }
)
#: FreeSurfer's runs compared with their entries: ``--all-info``'s report holds the run's time.
POLICY = oracle_cache.Policy(
    threads=1, family="freesurfer", formats="freesurfer", volatile=(r"TimeStamp: \S+",)
)
STAMP = re.compile(r"TimeStamp: \S+")
TIMING = re.compile(r"^(white matter segmentation took) .* minutes$", re.M)
#: Never a diagnostic setting on the official program (its own options or the variables).
DIAG_ARG = re.compile(r"^--?diag", re.I)
DIAG_ENV = re.compile(r"^(DIAG|diag)")
N = 32
#: The recorded heads (``oracles/freesurfer-8.2.0/runs/<run>/``) for ``--heads``.
HEAD_RUNS = ("ds000005-sub-01-classic", "ds004173-sub-603309-v8")


def lia(shape: tuple[int, int, int], size: float = 1.0) -> np.ndarray:
    """A coronal (LIA) grid of ``shape`` voxels of ``size`` mm centred on the origin."""
    a, b, c = (n / 2 * size for n in shape)
    return np.array([[-size, 0, 0, a], [0, 0, size, -c], [0, -size, 0, b], [0, 0, 0, 1.0]])


def save(path: Path, data: np.ndarray, affine: np.ndarray) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    nib.save(nib.MGHImage(data, affine), str(path))
    return path


def brain(rng: np.random.Generator, shape: tuple[int, int, int] = (N, N, N)) -> np.ndarray:
    """A brain-like volume of ``float`` intensities (0 to about 150)."""
    i, j, k = np.meshgrid(*(np.arange(n) for n in shape), indexing="ij")
    c = [n / 2 + rng.uniform(-1.5, 1.5) for n in shape]
    r = np.sqrt((i - c[0]) ** 2 + (j - c[1]) ** 2 + (k - c[2]) ** 2)
    rw = rng.uniform(6.5, 7.5)
    v = np.zeros(shape)
    v = np.where(r < 12, 35 + rng.normal(0, 4, shape), v)
    v = np.where(r < 10, 70 + rng.normal(0, 6, shape), v)
    edge = np.clip((r - rw) / 1.5, 0, 1)
    v = np.where(r < rw + 1.5, 110 - 40 * edge + rng.normal(0, 3, shape), v)
    # a white plate one voxel thick along the rows, out of the core
    jc = round(c[1])
    v[2 : int(c[0]), jc, 6 : shape[2] - 6] = 110 + rng.normal(0, 2, (int(c[0]) - 2, shape[2] - 12))
    # a plate two voxels thick
    v[int(c[0]) + 6 : shape[0] - 2, jc + 4 : jc + 6, 8 : shape[2] - 8] = 112
    # bright spots outside the brain, and voxels touching only along an edge or a corner
    v[shape[0] - 5 : shape[0] - 2, 2:5, 14:17] = 150
    v[3, 3, 3] = v[4, 4, 3] = v[5, 5, 4] = 110
    v[2, shape[1] - 4, shape[2] - 4] = v[3, shape[1] - 3, shape[2] - 3] = 108
    v += rng.uniform(0, 2, shape) * (v == 0)
    return v


def labels(v: np.ndarray, shape: tuple[int, int, int]) -> np.ndarray:
    """A segmentation of ``v``: white matter 2 and 41 (by hemisphere: the first axis), cortex 3
    and 42, by intensity."""
    i = np.meshgrid(*(np.arange(n) for n in shape), indexing="ij")[0]
    left = i < shape[0] / 2
    seg = np.zeros(shape, np.int32)
    seg = np.where((v >= 92) & (v < 140), np.where(left, 2, 41), seg)
    seg = np.where((v >= 50) & (v < 92), np.where(left, 3, 42), seg)
    return seg.astype(np.int32)


def with_bits(data: np.ndarray, places: list[tuple[tuple[int, int, int], int]]) -> np.ndarray:
    """``float32`` data with the given voxels set from their bit patterns (no float conversion)."""
    bits = data.astype(np.float32).view(np.uint32).copy()
    for (x, y, z), b in places:
        bits[x, y, z] = b
    return bits.view(np.float32)


def small(d: Path, rng: np.random.Generator) -> None:
    """The small inputs in ``s`` (and ``s2``, ``nw``, ``sag``): see the module's text."""
    shape = (N, N, N)
    aff = lia(shape)
    v = brain(rng)
    b = np.clip(np.rint(v), 0, 255).astype(np.uint8)
    s = d / "s"
    save(s / "in.mgz", b, aff)
    nan = [((16, 16, 16), 0x7FC00000), ((10, 16, 16), 0x7F800001), ((20, 12, 16), 0xFFC00000),
           ((16, 10, 16), 0x7F800000), ((16, 22, 16), 0xFF800000), ((12, 12, 12), 0x7FA00000)]  # fmt: skip
    save(s / "in_float.mgz", with_bits(v * 1.5 - 5, nan), aff)
    save(s / "in_short.mgz", np.rint(v * 4).astype(np.int16), aff)
    nc = (40, N, 24)
    save(s / "in_nc.mgz", np.clip(np.rint(brain(rng, nc)), 0, 255).astype(np.uint8), lia(nc))
    save(s / "zero.mgz", np.zeros(shape, np.uint8), aff)
    save(s / "const.mgz", np.full(shape, 110, np.uint8), aff)
    seg = labels(b.astype(float), shape)
    save(s / "aseg.presurf.mgz", seg, aff)
    save(s / "other.mgz", np.where(seg == 2, 41, np.where(seg == 41, 2, seg)).astype(np.int32), aff)
    save(s / "mask.mgz", np.where(np.isin(seg, (2, 41)), seg, 0).astype(np.uint8), aff)
    save(s / "mask_small.mgz", np.zeros((16, 16, 16), np.uint8), lia((16, 16, 16)))
    save(s / "mask_float.mgz", seg.astype(np.float32), aff)
    old = np.zeros(shape, np.uint8)
    old[10:14, 10:14, 10:14] = 255
    old[16:19, 16:19, 16:19] = 1
    save(s / "keep.mgz", old, aff)
    # the input written as a not coronal volume (an axial grid)
    sag = d / "sag"
    save(sag / "in.mgz", b, np.diag([1.0, 1.0, 1.0, 1.0]))
    # a segmentation smaller than the input (FS-1222)
    s2 = d / "s2"
    save(s2 / "in.mgz", b, aff)
    save(s2 / "aseg.presurf.mgz", np.zeros((16, 16, 16), np.int32), lia((16, 16, 16)))
    # a white-matter sheet darker than the cortex around it: none left after the relabelling
    nw = d / "nw"
    iv = np.zeros(shape, np.uint8)
    sg = np.zeros(shape, np.int32)
    iv[8:24, 15, 8:24], sg[8:24, 15, 8:24] = 100, 3
    iv[8:24, 16, 8:24], sg[8:24, 16, 8:24] = 60, 2
    iv[8:24, 17, 8:24], sg[8:24, 17, 8:24] = 100, 3
    save(nw / "in.mgz", iv, aff)
    save(nw / "aseg.presurf.mgz", sg, aff)


def official() -> Path:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    return ws / FS_HOME / "bin" / "mri_segment"


def base_env(home: Path) -> dict[str, str]:
    exe = official()
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(home), "USER": "probe",
           "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    return env


def inputs(ws: Path, seed: int) -> Path:
    """The inputs of ``seed`` (written once: a marker file says they are complete)."""
    d = ws / "oracles" / "inputs" / "fs-segment-prog" / str(seed)
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


#: The options that take values, each given last (FS-1219; ``-dat`` and ``-b`` read none there).
LAST = ("-slope", "-pslope", "-debug_voxel", "-nslope", "-ghi", "-gray_hi", "-glo", "-gray_low",
        "-wlo", "-wm_low", "-wm_low_factor", "-whi", "-wm_hi", "-nseg", "-thicken", "-polvwsize",
        "-min-wm-mask", "-polvlen", "-wsizemm", "-n", "-t", "-p", "-x", "-w", "-dat", "-b",
        "-seg")  # fmt: skip


def cases() -> list[Case]:
    io = ["in.mgz", "out.mgz"]
    rc = ["-wsizemm", "13", "-mprage"]
    seg = ["-seg", "aseg.presurf.mgz", "3", "3", "2", "2"]
    c: list[Case] = [
        # the command line
        Case([]),
        Case(["in.mgz"]),
        Case([*io, "x.mgz"]),
        Case(["--help"]),
        Case(["--usage"]),
        Case(["-help"]),
        Case(["-u"]),
        Case(["-?"]),
        Case(["-h"]),
        Case(["--version"]),
        Case(["--all-info"]),
        Case(["--VERSION"]),
        Case(["--Version", *io]),
        Case(["-version", *io]),
        Case(["-zzz", *io]),
        Case(["-", *io]),
        Case(["missing.mgz", "out.mgz"]),
        Case(["in.mgz", "-wsizemm", "13"]),
        # every option that takes values given last, or before too few values (FS-1219)
        *[Case([opt]) for opt in LAST],
        Case(["-debug_voxel", "1", "2"]),
        Case(["-seg", "x.mgz"]),
        Case(["-seg", "x.mgz", "1", "2", "3"]),
        # the runs
        Case(io),
        Case([*rc, *io]),
        Case(["-noauto", *io]),
        Case(["-auto", *io]),
        Case(["-auto", "-auto", *io]),
        Case(["-log", "-dat", "x.dat", *io]),
        Case(["-dat", "nodir/x.dat", *io]),
        Case(["-mgh_mprage", *io]),
        Case(["-MPRAGE", "-ghi", "95", *io]),
        Case(["-ghi", "95", "-mprage", *io]),
        Case(["-washu_mprage", *io]),
        Case(["-no1d_remove", *io]),
        Case(["-slope", "2", "-pslope", "1.5", "-nslope", "0.5", *io]),
        Case(["-gray_hi", "92", *io]),
        Case(["-glo", "40", *io]),
        Case(["-gray_low", "-5", *io]),
        Case(["-wlo", "85", *io]),
        Case(["-wm_low", "nan", *io]),
        Case(["-wm_low_factor", "0.5", *io]),
        Case(["-wm_low_factor", "0.5", *seg, *io]),
        Case(["-whi", "120", *io]),
        Case(["-wm_hi", "130", "-wlo", "70", *io]),
        Case(["-nseg", "3", *io]),
        Case(["-nseg", "0", *io]),
        Case(["-thicken", "0", *io]),
        Case(["-thicken", "2", *io]),
        Case(["-thickenonly", *io]),
        Case(["-thickenonly", "-no1d_remove", *io]),
        Case(["-fillbg", *io]),
        Case(["-fillv", *io]),
        Case(["-fillbg", "-fillv", *io]),
        Case(["-fill_bg", "-fill_bg", "-fill_ventricles", "-fill_ventricles", *io]),
        Case(["-b", "0.5", *io]),
        Case(["-b", "0", *io]),
        Case(["-blur", "2", *io]),
        Case(["-n", "2", *io]),
        Case(["-n", "0", *io]),
        Case(["-t", "2", *io]),
        Case(["-t", "0", *io]),
        Case(["-thick", "10", *io]),
        Case(["-v", *io]),
        Case(["-p", "0.5", *io]),
        Case(["-x", "3", *io]),
        Case(["-x", "abc", *io]),
        Case(["-w", "5", *io]),
        Case(["-w", "1", *io]),
        Case(["-W", "0", *io]),
        Case(["-wsizemm", "5", *io]),
        Case(["-wsizemm", "0", *io]),
        Case(["-polvwsize", "3", *io]),
        Case(["-polvwsize", "0", *io]),
        Case(["-polvwsize", "1", *io]),
        Case(["-polvwsize", "4", *io]),
        Case(["-polvwsize", "-1", *io]),
        Case(["-polvwsize", "17", *io]),
        Case(["-polvwsize", "abc", *io]),
        Case(["-PolvWsize", "3", *io]),
        Case(["-polvlen", "1", *io]),
        Case(["-polvlen", "0", *io]),
        Case(["-polvlen", "-1", *io]),
        Case(["-polvlen", "0.1", *io]),
        Case(["-polvlen", "30", *io]),
        Case(["-polvlen", "nan", *io]),
        Case([*rc, "-polvwsize", "7", "-polvlen", "2", *io]),
        Case(["-min-wm-mask", "mask.mgz", *io]),
        Case(["-min-wm-mask", "mask_small.mgz", *io]),
        Case(["-min-wm-mask", "mask_float.mgz", *io]),
        Case(["-min-wm-mask", "missing.mgz", *io]),
        Case([*seg, *io]),
        Case(["-seg", "other.mgz", "3", "3", "2", "2", *io]),
        Case(["-seg", "aseg.presurf.mgz", "2", "4", "1", "3", "-wlo", "80", *io]),
        Case(["-seg", "aseg.presurf.mgz", "a", "b", "c", "d", *io]),
        Case([*seg, *io], folder="s2"),
        Case([*seg, *io], folder="nw"),
        Case(["-keep", "in.mgz", "keep.mgz"]),
        Case(["-keep", "in.mgz", "new.mgz"]),
        Case(["in.mgz", "nodir/out.mgz"]),
        Case(["-thickenonly", "in.mgz", "nodir/out.mgz"]),
        Case(io, readonly=True),
        Case(["-debug_voxel", "1", "2", "3", *io]),
        Case(["in_float.mgz", "out.mgz"]),
        Case([*rc, "in_float.mgz", "out.mgz"]),
        Case(["in_short.mgz", "out.mgz"]),
        Case(["in_nc.mgz", "out.mgz"]),
        Case(["zero.mgz", "out.mgz"]),
        Case(["const.mgz", "out.mgz"]),
        Case(["-noauto", "zero.mgz", "out.mgz"]),
        Case(io, folder="sag"),
        Case(["-fillbg", *io], folder="sag"),
        Case(["-fillv", *io], folder="sag"),
        Case(["in.mgz", "out.nii"]),
    ]
    return c


def heads(ws: Path) -> tuple[Path, list[Case]]:
    """The recorded heads: for each run a folder with its ``antsdn.brain.mgz``,
    ``aseg.presurf.mgz`` and its ``wm.mgz`` as ``wm.seg.mgz`` (for ``-keep``); options recon-all
    does not give."""
    root = ws / "oracles" / "inputs" / "fs-segment-prog" / "heads"
    out: list[Case] = []
    io = ["antsdn.brain.mgz", "wm.seg.mgz"]
    rc = ["-wsizemm", "13", "-mprage"]
    for run in HEAD_RUNS:
        subjects = ws / "oracles" / "freesurfer-8.2.0" / "runs" / run / "subjects"
        rec = next(p for p in sorted(subjects.iterdir()) if p.name != "fsaverage") / "mri"
        m = root / run
        done = m / ".complete-v1"
        if not done.exists():
            if m.exists():
                shutil.rmtree(m)
            m.mkdir(parents=True)
            for f in ("antsdn.brain.mgz", "aseg.presurf.mgz"):
                shutil.copy(rec / f, m)
            shutil.copy(rec / "wm.mgz", m / "wm.seg.mgz")
            done.write_text("complete\n", encoding="utf-8")
        out += [
            Case([*rc, "-noauto", *io], folder=run),
            Case([*rc, "-seg", "aseg.presurf.mgz", "3", "3", "2", "2", *io], folder=run),
            Case([*rc, "-fillbg", "-fillv", *io], folder=run),
            Case([*rc, "-thicken", "0", *io], folder=run),
            Case([*rc, "-keep", *io], folder=run),
        ]
    return root, out


def mask(text: str, port: bool, folder: Path) -> str:
    """The messages without what differs between the sides: the time (``--all-info``'s report
    and the timing line), the case's folder, the candidate's notices."""
    text = STAMP.sub("TimeStamp: <time>", text)
    text = TIMING.sub(r"\1 <time> minutes", text)
    text = text.replace(str(folder), "<folder>")
    if port:
        text = "".join(x for x in text.splitlines(True) if not x.startswith("larmorx: "))
    return text


def status(code: int | None) -> int | None:
    """An exit status as a shell reports it: a process ended by signal ``n`` as ``128 + n``;
    ``None`` for a run stopped at its time limit."""
    if code is None:
        return None
    return 128 - code if code < 0 else code


def notices(text: str) -> list[str]:
    return sorted(
        {m.group(1) for m in re.finditer(r"^larmorx: upstream defect (\S+) ", text, re.M)}
    )


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
        d = work / f"{i:03d}" / side
        before = prepare(src, d, case)
        e = dict(env)
        e.update(case.env)
        if side == "official":
            given = sorted(p for p in d.rglob("*") if p.is_file())
            done = oracle_cache.run(
                ["mri_segment", *case.args],
                env=e,
                cwd=d,
                outputs=[d],
                policy=POLICY,
                timeout=1800,
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
                timeout=1800,
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
        shutil.rmtree(work / f"{i:03d}" / side, ignore_errors=True)
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
    work = ws / "oracles" / "inputs" / "fs-segment-prog" / f"work-{tag}-{seed}"
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
            pool.submit(run_case, i, c, s, work, env, candidate, modes, ctx, "fs-segment-prog"): i
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
