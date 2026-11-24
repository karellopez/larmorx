# SPDX-License-Identifier: Apache-2.0
"""mri_pretess as a black box: FreeSurfer 8.2.0's official program against a candidate.

An open probe (``README.md``): it writes seeded synthetic volumes into
``<workspace>/oracles/inputs/fs-pretess-prog/<seed>/``: 32^3 coronal volumes of 1 mm: a white
matter segmentation (``wm.mgz``, bytes: random blobs with voxels that touch only at an edge or a
corner in every direction, the edit values 255 and 1, values below 5 and exactly 5), a filled
volume (``filled.mgz``, bytes: the labels 255 and 127 of two halves and other values), an
intensity volume (``norm.mgz``, bytes, with many equal values) and its ``float`` (with NaNs,
infinities and a negative zero by their bits) and ``int`` forms, the white matter as ``float``
(fractional values; values outside 0 to 255) and with two frames, the filled volume as ``int``
(labels above 255), an empty segmentation, earlier outputs for ``-keep`` (bytes, ``float``, of
another extent), volumes of another extent and of 2^3. It runs each case's command line through
the official program (``$FREESURFER_HOME/bin/mri_pretess``, found on the ``PATH`` by its name as
recon-all runs it) by the oracle-output cache at one thread, and through the candidate
(``--candidate``, a command line whose ``{mode}`` becomes ``reproduce`` and then ``fix``), each in
a folder of its own holding a copy of the case's files. Compared: the exit status, standard output
and error (the case's folder, ``--all-info``'s time and the candidate's ``larmorx:`` notices
aside) and every file written (FreeSurfer's provenance aside:
``larmorx_validation.freesurfer.files``). ``reproduce`` must give FreeSurfer's on every case;
``fix`` may differ only with a notice. ``--heads`` adds cases on recorded heads with options
recon-all does not give.

No case gives the official program a diagnostic setting: no ``DIAG*`` variable, no option
starting ``-diag`` and no option starting ``-w`` (mri_pretess's own switch of the diagnostic
writes, ``-w`` in its help): the licence check prints the licence file from a diagnostic level of
1. The probe refuses such a case.

    python -P validation/investigations/freesurfer-mri-pretess-probe/compare.py \\
        --seed 1 --candidate '<program> --upstream-defects {mode} freesurfer mri_pretess'
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
#: or writes memory it does not own there (voxels outside the volumes).
NOT_COMPARABLE = frozenset({"FS-52", "FS-53"})
#: FreeSurfer's runs compared with their entries: ``--all-info``'s report holds the run's time.
POLICY = oracle_cache.Policy(
    threads=1, family="freesurfer", formats="freesurfer", volatile=(r"TimeStamp: \S+",)
)
STAMP = re.compile(r"TimeStamp: \S+")
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


def blobs(rng: np.random.Generator, shape: tuple[int, int, int], p: float) -> np.ndarray:
    """A random binary volume: voxels on with probability ``p``, then each voxel on when most of
    its face neighbours are (one smoothing pass), so that blobs touch at edges and corners."""
    on = rng.uniform(size=shape) < p
    pad = np.pad(on, 1).astype(np.int32)
    s = sum(
        pad[1 + dx : 1 + dx + shape[0], 1 + dy : 1 + dy + shape[1], 1 + dz : 1 + dz + shape[2]]
        for dx, dy, dz in ((0, 0, 0), (1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0),
                           (0, 0, 1), (0, 0, -1))
    )  # fmt: skip
    return s >= 4


def small(d: Path, rng: np.random.Generator) -> None:
    """The inputs in ``s``: see the module's text."""
    shape = (N, N, N)
    aff = lia(shape)
    s = d / "s"
    on = blobs(rng, shape, 0.5)
    wm = np.zeros(shape, np.uint8)
    wm[on] = 110
    u = rng.uniform(size=shape)
    wm[on & (u < 0.03)] = 255
    wm[on & (u > 0.97)] = 250
    wm[~on & (u < 0.02)] = 1
    wm[~on & (u > 0.98)] = rng.integers(2, 6, size=shape)[~on & (u > 0.98)]
    # isolated voxels meeting only at an edge or a corner, in every direction
    for x, y, z, dx, dy, dz in ((3, 3, 3, 1, 1, 0), (3, 8, 3, -1, 1, 0), (8, 3, 3, 0, 1, 1),
                                (8, 8, 4, 0, 1, -1), (13, 3, 3, 1, 0, 1), (13, 8, 3, -1, 0, 1),
                                (18, 3, 3, 1, 1, 1), (18, 8, 4, 1, 1, -1), (23, 4, 4, 1, -1, -1),
                                (23, 9, 3, 1, -1, 1)):  # fmt: skip
        wm[x - 1 : x + 3, y - 1 : y + 3, z - 1 : z + 3] = 0
        wm[x, y, z] = 110
        wm[x + dx, y + dy, z + dz] = 110
    # a background voxel whose corner neighbour is background inside white matter
    wm[24:30, 22:28, 22:28] = 110
    wm[26, 24, 24] = 0
    wm[27, 25, 25] = 0
    save(s / "wm.mgz", wm, aff)
    filled = np.zeros(shape, np.uint8)
    fon = blobs(rng, shape, 0.5)
    filled[fon] = 255
    filled[fon & (np.arange(N)[:, None, None] >= N // 2)] = 127
    filled[(rng.uniform(size=shape) > 0.99)] = 80
    save(s / "filled.mgz", filled, aff)
    norm = (rng.integers(0, 12, size=shape) * 20).astype(np.uint8)
    save(s / "norm.mgz", norm, aff)
    nan = [((x, y, z), b) for (x, y, z, b) in
           ((4, 3, 3, 0x7FC00000), (3, 4, 3, 0x7F800001), (8, 4, 3, 0xFFC00000),
            (8, 3, 4, 0x7F800000), (19, 3, 3, 0xFF800000), (18, 4, 3, 0x80000000),
            (19, 4, 3, 0x7FA00000), (14, 3, 3, 0x7FC00001))]  # fmt: skip
    save(s / "norm_float.mgz", with_bits(norm.astype(np.float32) * 0.5 + 0.25, nan), aff)
    save(s / "norm_int.mgz", norm.astype(np.int32) - 100, aff)
    wf = wm.astype(np.float32)
    wf[on & (u > 0.5) & (u < 0.52)] = 127.5
    wf[on & (u > 0.52) & (u < 0.54)] = 4.5
    save(s / "wm_float.mgz", wf, aff)
    wo = wf.copy()
    wo[1, 1, 1] = 300.0
    wo[2, 1, 1] = -2.0
    save(s / "wm_float_out.mgz", wo, aff)
    save(s / "wm_2f.mgz", np.stack([wm, wm[::-1]], axis=-1), aff)
    fi = filled.astype(np.int32)
    fi[fon & (np.arange(N)[None, :, None] < 6)] = 1000
    save(s / "filled_int.mgz", fi, aff)
    save(s / "filled_2f.mgz", np.stack([filled, filled[:, ::-1]], axis=-1), aff)
    save(s / "empty.mgz", np.zeros(shape, np.uint8), aff)
    old = wm.copy()
    old[10:12, 20:22, 10:12] = 255
    old[20:22, 10:12, 20:22] = 1
    save(s / "old.mgz", old, aff)
    save(s / "old_float.mgz", old.astype(np.float32), aff)
    save(s / "old_small.mgz", old[:16, :16, :16].copy(), lia((16, 16, 16)))
    save(s / "norm_small.mgz", norm[:16, :16, :16].copy(), lia((16, 16, 16)))
    save(s / "wm_2.mgz", wm[:2, :2, :2].copy(), lia((2, 2, 2)))
    save(s / "norm_2.mgz", norm[:2, :2, :2].copy(), lia((2, 2, 2)))
    # the white matter as float with NaNs and infinities by their bits (read as they are with
    # FS_LEAVE_NANS, else made 0)
    wn = [((x, y, z), b) for (x, y, z, b) in
          ((3, 3, 3, 0x7FC00000), (4, 4, 3, 0xFFC00001), (18, 3, 3, 0x7F800001),
           (19, 4, 4, 0x7F800000), (26, 24, 24, 0xFF800000), (8, 3, 3, 0x7FA00000))]  # fmt: skip
    save(s / "wm_nan.mgz", with_bits(wm.astype(np.float32), wn), aff)


def official() -> Path:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    return ws / FS_HOME / "bin" / "mri_pretess"


def base_env(home: Path) -> dict[str, str]:
    exe = official()
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(home), "USER": "probe",
           "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    return env


def inputs(ws: Path, seed: int) -> Path:
    """The inputs of ``seed`` (written once: a marker file says they are complete)."""
    d = ws / "oracles" / "inputs" / "fs-pretess-prog" / str(seed)
    done = d / ".complete-v2"
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


IO = ["wm.mgz", "wm", "norm.mgz", "out.mgz"]
FILLED = ["filled.mgz", "255", "norm.mgz", "out.mgz"]


def cases() -> list[Case]:
    def io(seg: str = "wm.mgz", label: str = "wm", norm: str = "norm.mgz", out: str = "out.mgz"):
        return [seg, label, norm, out]

    c: list[Case] = [
        # the command line
        Case([]),
        Case(["wm.mgz"]),
        Case(IO[:3]),
        Case([*IO, "extra.mgz"]),
        Case([*IO, "-keep"]),
        Case(["--help"]),
        Case(["--usage"]),
        Case(["-help"]),
        Case(["-u"]),
        Case(["-?"]),
        Case(["-h"]),
        Case(["--version"]),
        Case(["--all-info"]),
        Case(["--version", *IO]),
        Case([*IO, "--version"]),
        Case(["-zzz", *IO]),
        Case(["--zzz", *IO]),
        Case(["-", *IO]),
        Case(["-NOCORNERS", *IO]),
        Case(["-Keep", *IO[:3], "old.mgz"]),
        Case(["-debug_voxel"]),
        Case(["-debug_voxel", "1"]),
        Case(["-debug_voxel", "1", "2"]),
        Case(["-keep", "-debug_voxel", "1", "2"]),
        Case(["-debug_voxel", "1", "2", "3"]),
        Case(["-debug_voxel", "3", "3", "3", *IO]),
        Case(["-DEBUG_VOXEL", "-1", "x", "40", *IO]),
        Case(["missing.mgz", *IO[1:]]),
        Case([*IO[:2], "missing.mgz", IO[3]]),
        Case([*IO[:2], "norm_small.mgz", IO[3]]),
        Case([*IO[:3], "nodir/out.mgz"]),
        Case(IO, readonly=True),
        # the labels
        Case(io(label="WM")),
        Case(io(label="Wm")),
        Case(io(label="wmx")),
        Case(FILLED),
        Case(io("filled.mgz", "127")),
        Case(io("filled.mgz", "80")),
        Case(io("filled.mgz", "0")),
        Case(io(label="110")),
        Case(io(label="128")),
        Case(io(label="255")),
        Case(io(label="256")),
        Case(io(label="-1")),
        Case(io(label="-5")),
        Case(io(label="abc")),
        Case(io(label="12abc")),
        Case(io(label="")),
        Case(io(label=" 110")),
        Case(io("empty.mgz")),
        Case(io("empty.mgz", "255")),
        # the runs and options
        Case(IO),
        Case(["-nocorners", *IO]),
        Case(["-nocorners", *FILLED]),
        Case(["-keep", *IO[:3], "old.mgz"]),
        Case(["-keep", *FILLED[:3], "old.mgz"]),
        Case(["-keep", *IO]),
        Case(["-keep", *IO[:3], "old_float.mgz"]),
        Case(["-keep", *IO[:3], "old_small.mgz"]),
        Case(["-test", *IO]),
        Case(["-test", "-keep", *IO[:3], "old.mgz"]),
        Case(["-test", *FILLED]),
        Case(["-test", "wm_2.mgz", "wm", "norm_2.mgz", "out.mgz"]),
        Case(["-test", "empty.mgz", "wm", "norm.mgz", "out.mgz"]),
        Case(["-test", "wm_float.mgz", "wm", "norm.mgz", "out.mgz"]),
        Case(["wm_2.mgz", "wm", "norm_2.mgz", "out.mgz"]),
        Case([*IO[:3], "wm.mgz"]),
        Case([*IO[:3], "out.nii"]),
        Case([*IO[:3], "out.nii.gz"]),
        # the types and frames
        Case(io(norm="norm_float.mgz")),
        Case(io("filled.mgz", "255", "norm_float.mgz")),
        Case(io(norm="norm_int.mgz")),
        Case(io("wm_float.mgz")),
        Case(io("wm_float.mgz", "110")),
        Case(io("wm_float.mgz", "128")),
        Case(io("wm_float_out.mgz")),
        Case(io("filled_int.mgz", "255")),
        Case(io("filled_int.mgz", "1000")),
        Case(io("wm_2f.mgz", "110")),
        Case(io("filled_2f.mgz", "255")),
        Case(io("wm_2f.mgz")),
        Case(io(norm="norm_float.mgz"), env={"FS_LEAVE_NANS": "1"}),
        Case(io("filled.mgz", "255", "norm_float.mgz"), env={"FS_LEAVE_NANS": "1"}),
        Case(["-nocorners", *io(norm="norm_float.mgz")], env={"FS_LEAVE_NANS": "1"}),
        Case(io("wm_nan.mgz")),
        Case(io("wm_nan.mgz"), env={"FS_LEAVE_NANS": "1"}),
        Case(io("wm_nan.mgz", "110"), env={"FS_LEAVE_NANS": "1"}),
        Case(["-test", *io("wm_nan.mgz")], env={"FS_LEAVE_NANS": "1"}),
    ]
    return c


def heads(ws: Path) -> tuple[Path, list[Case]]:
    """The recorded heads: for each run a folder with its ``wm.asegedit.mgz``, ``filled.mgz``,
    ``norm.mgz`` and ``wm.mgz``; options recon-all does not give: ``-nocorners``, ``-keep`` over
    the recorded ``wm.mgz`` (recon-all's call when it runs again), ``-test``, another label."""
    root = ws / "oracles" / "inputs" / "fs-pretess-prog" / "heads"
    out: list[Case] = []
    for run in HEAD_RUNS:
        subjects = ws / "oracles" / "freesurfer-8.2.0" / "runs" / run / "subjects"
        rec = next(p for p in sorted(subjects.iterdir()) if p.name != "fsaverage") / "mri"
        m = root / run
        done = m / ".complete-v1"
        if not done.exists():
            if m.exists():
                shutil.rmtree(m)
            m.mkdir(parents=True)
            for f in ("wm.asegedit.mgz", "filled.mgz", "norm.mgz", "wm.mgz"):
                shutil.copy(rec / f, m)
            done.write_text("complete\n", encoding="utf-8")
        out += [
            Case(["-nocorners", "wm.asegedit.mgz", "wm", "norm.mgz", "out.mgz"], folder=run),
            Case(["-keep", "wm.asegedit.mgz", "wm", "norm.mgz", "wm.mgz"], folder=run),
            Case(["-test", "filled.mgz", "127", "norm.mgz", "out.mgz"], folder=run),
            Case(["wm.asegedit.mgz", "110", "norm.mgz", "out.mgz"], folder=run),
        ]
    return root, out


def mask(text: str, port: bool, folder: Path) -> str:
    """The messages without what differs between the sides: the time (``--all-info``'s report
    and the timing line), the case's folder, the candidate's notices."""
    text = STAMP.sub("TimeStamp: <time>", text)
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
                ["mri_pretess", *case.args],
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
    work = ws / "oracles" / "inputs" / "fs-pretess-prog" / f"work-{tag}-{seed}"
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
            pool.submit(run_case, i, c, s, work, env, candidate, modes, ctx, "fs-pretess-prog"): i
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
