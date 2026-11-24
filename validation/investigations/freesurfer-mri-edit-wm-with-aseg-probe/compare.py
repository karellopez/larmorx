# SPDX-License-Identifier: Apache-2.0
"""mri_edit_wm_with_aseg as a black box: FreeSurfer 8.2.0's official program against a candidate.

An open probe (``README.md``): it writes seeded synthetic volumes into
``<workspace>/oracles/inputs/fs-editwm-prog/<seed>/``: a 32^3 coronal head of two hemispheres
whose segmentation (``aseg.presurf.mgz``, ``int``) holds every label the program edits by
(cerebral white matter and cortex, lateral and inferior lateral ventricles, hippocampus,
amygdala, choroid plexus, thalamus, caudate, putamen, pallidum, accumbens, ventral DC, vessels,
cerebellum, brain stem, optic chiasm, lesions and hypointensities, unknown pockets), a white-matter
volume (``wm.seg.mgz``, bytes, with holes, stray voxels and the edit values 255 and 1), a T1
(``brain.mgz``), the entorhinal labels (``entowm.mgz``), lesion labels (``wmsa.mgz``); the
segmentation also as bytes (the search for paths to cortex then works: FS-1234) and as ``float``;
a ``float`` T1 with NaNs and infinities (by their bits), a ``float`` white matter, volumes of
another extent, a head touching the volume's first slice. It runs each case's command line through
the official program (``$FREESURFER_HOME/bin/mri_edit_wm_with_aseg``, found on the ``PATH`` by its
name as recon-all runs it) by the oracle-output cache at one thread, and through the candidate
(``--candidate``, a command line whose ``{mode}`` becomes ``reproduce`` and then ``fix``), each in
a folder of its own holding a copy of the case's files. Compared: the exit status, standard output
and error (the run's time, the case's folder and the candidate's ``larmorx:`` notices aside) and
every file written (FreeSurfer's provenance aside: ``larmorx_validation.freesurfer.files``).
``reproduce`` must give FreeSurfer's on every case; ``fix`` may differ only with a notice.
``--heads`` adds cases on recorded heads with options recon-all does not give, and recon-all's
call with the segmentation as bytes.

No case gives the official program a diagnostic setting (any ``DIAG*`` variable, or an option
starting ``-diag``): its licence check prints the licence file from a diagnostic level of 1. The
probe refuses such a case.

    python -P validation/investigations/freesurfer-mri-edit-wm-with-aseg-probe/compare.py \\
        --seed 1 --candidate '<program> --upstream-defects {mode} freesurfer mri_edit_wm_with_aseg'
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
NOT_COMPARABLE = frozenset({"FS-52", "FS-53", "FS-1236"})
#: FreeSurfer's runs compared with their entries: ``--all-info``'s report holds the run's time.
POLICY = oracle_cache.Policy(
    threads=1, family="freesurfer", formats="freesurfer", volatile=(r"TimeStamp: \S+",)
)
STAMP = re.compile(r"TimeStamp: \S+")
TIMING = re.compile(r"^(auto filling took) .* minutes$", re.M)
#: Never a diagnostic setting on the official program (an option or the variables).
DIAG_ARG = re.compile(r"^--?diag", re.I)
DIAG_ENV = re.compile(r"^(DIAG|diag)")
N = 32
#: The recorded heads (``oracles/freesurfer-8.2.0/runs/<run>/``) for ``--heads``.
HEAD_RUNS = ("ds000005-sub-01-classic", "ds004173-sub-603309-v8")

# the segmentation's labels (include/cma.h)
UNK, CTX, WM, LV, ILV, TH, CA, PU, PA = 0, 3, 2, 4, 5, 10, 11, 12, 13
HI, AM, ACC, VDC, VES, CP = 17, 18, 26, 28, 30, 31
CBWM, CBCX, BS, LES = 7, 8, 16, 25
RIGHT = {CTX: 42, WM: 41, LV: 43, ILV: 44, TH: 49, CA: 50, PU: 51, PA: 52, HI: 53, AM: 54,
         ACC: 58, VDC: 60, VES: 62, CP: 63, CBWM: 46, CBCX: 47, LES: 57}  # fmt: skip


def lia(shape: tuple[int, int, int], size: float = 1.0) -> np.ndarray:
    """A coronal (LIA) grid of ``shape`` voxels of ``size`` mm centred on the origin."""
    a, b, c = (n / 2 * size for n in shape)
    return np.array([[-size, 0, 0, a], [0, 0, size, -c], [0, -size, 0, b], [0, 0, 0, 1.0]])


def save(path: Path, data: np.ndarray, affine: np.ndarray) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    nib.save(nib.MGHImage(data, affine), str(path))
    return path


def head(
    rng: np.random.Generator, shape: tuple[int, int, int] = (N, N, N), z0: int = 2
) -> np.ndarray:
    """A segmentation of two hemispheres (the left one at the larger columns, as in a coronal
    head): boxes of each structure, their edges moved by the seed by up to one voxel."""
    seg = np.zeros(shape, np.int32)
    nx, ny, nz = shape
    i, j, k = np.meshgrid(*(np.arange(n) for n in shape), indexing="ij")
    c = (nx / 2, ny / 2, nz / 2 + (z0 - 2))
    r = np.sqrt((i - c[0]) ** 2 + (j - c[1]) ** 2 + (k - c[2]) ** 2 * 0.8)
    seg[(r < 14) & (k >= z0)] = CTX
    seg[(r < 11.5) & (k >= z0)] = WM

    def j1() -> int:
        return int(rng.integers(-1, 2))

    # (label, x range of the right hemisphere, y range, z range): the left one is the mirror
    boxes = [
        (LV, (10, 13), (8, 12), (10, 21)),
        (TH, (12, 15), (12, 16), (12, 17)),
        (CA, (9, 12), (6, 9), (16, 21)),
        (PU, (5, 9), (10, 14), (14, 20)),
        (PA, (8, 10), (12, 15), (14, 19)),
        (ACC, (8, 12), (15, 17), (19, 22)),
        (VDC, (12, 15), (16, 19), (12, 16)),
        (VES, (6, 8), (16, 18), (18, 20)),
        (HI, (6, 11), (18, 22), (8, 15)),
        (AM, (6, 11), (17, 22), (15, 19)),
        (ILV, (4, 6), (17, 19), (9, 14)),
        (CP, (10, 13), (13, 15), (12, 14)),
        (CBCX, (5, 14), (24, 29), (3, 10)),
        (CBWM, (7, 12), (25, 28), (4, 9)),
        (LES, (3, 5), (8, 10), (20, 22)),
    ]
    for lab, (x0, x1), (y0, y1), (za, zb) in boxes:
        x0, y0, za = x0 + j1(), y0 + j1(), max(z0 if lab in (HI, ILV) else 0, za + j1() + (z0 - 2))
        x1, y1, zb = x1 + j1(), y1 + j1(), zb + j1() + (z0 - 2)
        seg[x0:x1, y0:y1, za:zb] = RIGHT.get(lab, lab)
        seg[nx - x1 : nx - x0, y0:y1, za:zb] = lab
    # cortex just below the hippocampus and amygdala, unknown pockets below them
    seg[6:11, 22:24, 8:19] = 42
    seg[nx - 11 : nx - 6, 22:24, 8:19] = CTX
    seg[7:9, 24:25, 10:14] = UNK
    seg[nx - 9 : nx - 7, 24:25, 10:14] = UNK
    # the midline: brain stem, optic chiasm; hypointensities and lesions in the white matter
    seg[nx // 2 - 2 : nx // 2 + 2, 20:29, 8 + (z0 - 2) : 13 + (z0 - 2)] = BS
    seg[nx // 2 - 1 : nx // 2 + 1, 19:21, 20:23] = 85
    for lab, x in ((77, 4), (78, nx - 5), (79, 5), (80, 6), (81, nx - 7), (82, 7)):
        seg[x, 10:12, 22:24] = lab
    return seg


def volumes(rng: np.random.Generator, seg: np.ndarray) -> tuple[np.ndarray, np.ndarray]:
    """The white matter (110 in the white matter and the subcortical structures, holes, stray
    voxels in the cortex and the background, edits 255 and 1) and a T1 by label, with noise."""
    shape = seg.shape
    wm = np.zeros(shape, np.uint8)
    white = np.isin(seg, (2, 41, 10, 49, 28, 60, 16))
    wm[white & (rng.uniform(size=shape) > 0.12)] = 110
    wm[np.isin(seg, (3, 42)) & (rng.uniform(size=shape) > 0.93)] = 110
    wm[(seg == 0) & (rng.uniform(size=shape) > 0.985)] = 110
    wm[np.isin(seg, (17, 53, 18, 54)) & (rng.uniform(size=shape) > 0.85)] = 110
    wm[(rng.uniform(size=shape) > 0.995) & white] = 255
    wm[(rng.uniform(size=shape) > 0.995) & (seg > 0)] = 1
    t1 = np.full(shape, 4.0)
    for labs, v in (((2, 41), 110), ((3, 42), 70), ((4, 43, 5, 44), 30), ((17, 53), 76),
                    ((18, 54), 72), ((10, 49, 11, 50, 12, 51, 13, 52, 26, 58, 28, 60), 88),
                    ((7, 46), 100), ((8, 47), 68), ((16,), 95), ((31, 63, 30, 62), 60),
                    ((77, 78, 79, 80, 81, 82, 25, 57, 85), 62)):  # fmt: skip
        t1[np.isin(seg, labs)] = v
    t1 = t1 + rng.normal(0, 4, shape) * (seg > 0)
    return wm, np.clip(np.rint(t1), 0, 255).astype(np.uint8)


def with_bits(data: np.ndarray, places: list[tuple[tuple[int, int, int], int]]) -> np.ndarray:
    """``float32`` data with the given voxels set from their bit patterns (no float conversion)."""
    bits = data.astype(np.float32).view(np.uint32).copy()
    for (x, y, z), b in places:
        bits[x, y, z] = b
    return bits.view(np.float32)


def small(d: Path, rng: np.random.Generator) -> None:
    """The inputs in ``s``, ``u8`` (the segmentation as bytes), ``f`` (as ``float``) and ``edge``
    (a head down to the first slice): see the module's text."""
    shape = (N, N, N)
    aff = lia(shape)
    seg = head(rng)
    wm, t1 = volumes(rng, seg)
    for name, sg in (("s", seg), ("u8", seg.astype(np.uint8)), ("f", seg.astype(np.float32))):
        f = d / name
        save(f / "aseg.presurf.mgz", sg, aff)
        save(f / "wm.seg.mgz", wm, aff)
        save(f / "brain.mgz", t1, aff)
    s = d / "s"
    save(s / "brain.finalsurfs.mgz", t1, aff)
    ento = np.zeros(shape, np.int32)
    ento[seg == 42] = 0
    ento[N - 11 : N - 7, 21:23, 9:13] = 3006
    ento[N - 13 : N - 11, 21:23, 9:13] = 3201
    ento[7:11, 21:23, 9:13] = 4006
    ento[11:13, 21:23, 9:13] = 4201
    save(s / "entowm.mgz", ento, aff)
    wmsa = np.zeros(shape, np.int32)
    for lab, x in ((77, 20), (78, 21), (79, 22), (99, 23), (80, 24)):
        wmsa[x, 8:10, 18:20] = lab
    save(s / "wmsa.mgz", wmsa, aff)
    old = wm.copy()
    old[20:22, 9:11, 15:17] = 255
    old[10:12, 9:11, 15:17] = 1
    save(s / "keep.mgz", old, aff)
    nan = [((16, 16, 16), 0x7FC00000), ((10, 16, 16), 0x7F800001), ((20, 12, 16), 0xFFC00000),
           ((16, 10, 16), 0x7F800000), ((16, 22, 16), 0xFF800000), ((12, 12, 12), 0x7FA00000)]  # fmt: skip
    save(s / "brain_float.mgz", with_bits(t1.astype(np.float32) * 1.5 + 0.25, nan), aff)
    save(s / "wm_float.mgz", wm.astype(np.float32), aff)
    save(s / "aseg_small.mgz", seg[:16, :16, :16].copy(), lia((16, 16, 16)))
    save(s / "brain_small.mgz", t1[:16, :16, :16].copy(), lia((16, 16, 16)))
    # a head down to the first slice (its hippocampus and inferior lateral ventricle at slice 0)
    e = d / "edge"
    sg = head(rng, z0=0)
    w2, t2 = volumes(rng, sg)
    save(e / "aseg.presurf.mgz", sg, aff)
    save(e / "wm.seg.mgz", w2, aff)
    save(e / "brain.mgz", t2, aff)


def official() -> Path:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    return ws / FS_HOME / "bin" / "mri_edit_wm_with_aseg"


def base_env(home: Path) -> dict[str, str]:
    exe = official()
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(home), "USER": "probe",
           "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    return env


def inputs(ws: Path, seed: int) -> Path:
    """The inputs of ``seed`` (written once: a marker file says they are complete)."""
    d = ws / "oracles" / "inputs" / "fs-editwm-prog" / str(seed)
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


IO = ["wm.seg.mgz", "brain.mgz", "aseg.presurf.mgz", "out.mgz"]
V8 = ["-keep-in", "-fix-ento-wm", "entowm.mgz", "3", "255", "255", "-fix-acj",
      "aseg.presurf.mgz", "255", "255", "-fill-seg-wm", "-fix-scm-ha", "1"]  # fmt: skip
#: Each option that takes values with fewer than its values, last (FS-1239).
SHORT = [
    ["-fillven"], ["-fix-scm-ha"],
    ["-fix-scm-ha-only"], ["-fix-scm-ha-only", "aseg.presurf.mgz"],
    ["-fix-scm-ha-only", "aseg.presurf.mgz", "wm.seg.mgz"],
    ["-fix-scm-ha-only", "aseg.presurf.mgz", "wm.seg.mgz", "1"],
    ["-fix-ento-wm"], ["-fix-ento-wm", "entowm.mgz"], ["-fix-ento-wm", "entowm.mgz", "3", "255"],
    ["-sa-fix-ento-wm", "entowm.mgz", "2", "255", "255"],
    ["-sa-fix-ento-wm", "entowm.mgz", "2", "255", "255", "brain.finalsurfs.mgz"],
    ["-label-acj"], ["-label-acj", "aseg.presurf.mgz"],
    ["-fix-acj"], ["-fix-acj", "aseg.presurf.mgz", "255"],
    ["-sa-fix-acj", "aseg.presurf.mgz", "255", "255"],
    ["-sa-fix-acj", "aseg.presurf.mgz", "255", "255", "wm.seg.mgz"],
    ["-keep-hailvcp"], ["-keep-hailvcp", "5"],
    ["-debug_voxel"], ["-debug_voxel", "1", "2"],
    ["-wmsa"],
]  # fmt: skip


def cases() -> list[Case]:
    c: list[Case] = [
        # the command line
        Case([]),
        Case(["wm.seg.mgz"]),
        Case(IO[:3]),
        Case([*IO, "extra.mgz"]),
        Case(["--help"]),
        Case(["--usage"]),
        Case(["-help"]),
        Case(["-u"]),
        Case(["-?"]),
        Case(["-h"]),
        Case(["--version"]),
        Case(["--all-info"]),
        Case(["-zzz", *IO]),
        Case(["-", *IO]),
        Case(["-dilate", *IO]),
        Case(["-FILLVEN", "0", *IO]),
        Case(["missing.mgz", *IO[1:]]),
        Case([IO[0], "missing.mgz", *IO[2:]]),
        Case([*IO[:2], "missing.mgz", IO[3]]),
        Case(["wm_float.mgz", *IO[1:]]),
        Case([*IO[:3], "nodir/out.mgz"]),
        Case(IO, readonly=True),
        *[Case(a) for a in SHORT],
        # the runs
        Case(IO),
        Case(["-keep-in", *IO]),
        Case([*V8, *IO]),
        Case(["-keep", *IO[:3], "keep.mgz"]),
        Case(["-keep", *IO]),
        Case(["-KEEP-IN", "-keep", *IO]),
        Case(["-fcd", *IO]),
        Case(["-lh", *IO]),
        Case(["-rh", *IO]),
        Case(["-lh", "-rh", *IO]),
        Case(["-fillven", "0", *IO]),
        Case(["-fillven", "2", *IO]),
        Case(["-fix-scm-ha", "0", *IO]),
        Case(["-fix-scm-ha", "2", *IO]),
        Case(["-fix-scm-ha-only", "aseg.presurf.mgz", "wm.seg.mgz", "1", "o.mgz"]),
        Case(["-fix-scm-ha-only", "aseg.presurf.mgz", "wm.seg.mgz", "1", "nodir/o.mgz"]),
        Case(["-fix-ento-wm", "entowm.mgz", "3", "255", "255", *IO]),
        Case(["-fix-ento-wm", "entowm.mgz", "1", "200", "100", *IO]),
        Case(["-fix-ento-wm", "entowm.mgz", "2", "abc", "7", *IO]),
        Case(["-fix-ento-wm", "missing.mgz", "3", "255", "255", *IO]),
        Case(["-sa-fix-ento-wm", "entowm.mgz", "2", "255", "255", "brain.finalsurfs.mgz", "o.mgz"]),
        Case(["-sa-fix-ento-wm", "entowm.mgz", "3", "255", "127", "wm.seg.mgz", "o.mgz"]),
        Case(["-sa-fix-ento-wm", "entowm.mgz", "3", "255", "255", "missing.mgz", "o.mgz"]),
        Case(["-sa-fix-ento-wm", "entowm.mgz", "3", "255", "255", "wm.seg.mgz", "nodir/o.mgz"]),
        Case(["-label-acj", "aseg.presurf.mgz", "acj.mgz"]),
        Case(["-label-acj", "missing.mgz", "acj.mgz"]),
        Case(["-fix-acj", "aseg.presurf.mgz", "255", "255", *IO]),
        Case(["-fix-acj", "aseg.presurf.mgz", "x", "1e3", *IO]),
        Case(["-sa-fix-acj", "aseg.presurf.mgz", "255", "255", "brain.finalsurfs.mgz", "o.mgz"]),
        Case(["-sa-fix-acj", "aseg.presurf.mgz", "255", "255", "missing.mgz", "o.mgz"]),
        Case(["-fill-seg-wm", *IO]),
        Case(["-fill-seg-wm", "-no-fill-seg-wm", *IO]),
        Case(["-keep-hailvcp", "100", "200", *IO]),
        Case(["-keep-hailvcp", "-5", "-6", *IO]),
        Case(["-wmsa", "wmsa.mgz", *IO]),
        Case(["-wmsa", "missing.mgz", *IO]),
        Case(["-debug_voxel", "9", "20", "12", *IO]),
        Case(["-debug_voxel", "22", "20", "12", *IO]),
        # the types and extents
        Case(IO, folder="u8"),
        Case(["-keep-in", *IO], folder="u8"),
        Case([*V8, *IO], folder="u8"),
        Case(IO, folder="f"),
        Case([*V8, *IO], folder="f"),
        Case(["wm.seg.mgz", "brain_float.mgz", *IO[2:]]),
        Case([*IO[:2], "aseg_small.mgz", IO[3]]),
        Case(["wm.seg.mgz", "brain_small.mgz", *IO[2:]]),
        Case(IO, folder="edge"),
        Case(["-keep-in", *IO], folder="edge"),
        Case([*IO[:3], "out.nii"]),
    ]
    return c


def heads(ws: Path) -> tuple[Path, list[Case]]:
    """The recorded heads: for each run a folder with its ``wm.seg.mgz``, ``brain.mgz``,
    ``aseg.presurf.mgz`` (and as bytes, ``aseg_u8.mgz``), ``entowm.mgz`` (v8) and
    ``brain.finalsurfs.mgz``; options recon-all does not give, and recon-all's call with the
    segmentation as bytes (FS-1234's search for paths to cortex then runs)."""
    root = ws / "oracles" / "inputs" / "fs-editwm-prog" / "heads"
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
            for f in ("wm.seg.mgz", "brain.mgz", "aseg.presurf.mgz", "entowm.mgz",
                      "brain.finalsurfs.mgz"):  # fmt: skip
                if (rec / f).exists():
                    shutil.copy(rec / f, m)
            a = nib.load(str(m / "aseg.presurf.mgz"))
            u8 = np.asarray(a.dataobj).astype(np.uint8)
            nib.save(nib.MGHImage(u8, a.affine, a.header), str(m / "aseg_u8.mgz"))
            done.write_text("complete\n", encoding="utf-8")
        u8io = ["wm.seg.mgz", "brain.mgz", "aseg_u8.mgz", "out.mgz"]
        out += [
            Case(["-keep-in", *u8io], folder=run),
            Case(["-keep-in", "-fcd", "-lh", *IO], folder=run),
            Case(["-keep-in", "-fill-seg-wm", "-fillven", "0", *IO], folder=run),
        ]
        if "v8" in run:
            out.append(Case([*V8, *u8io], folder=run))
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
                ["mri_edit_wm_with_aseg", *case.args],
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
    work = ws / "oracles" / "inputs" / "fs-editwm-prog" / f"work-{tag}-{seed}"
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
            pool.submit(run_case, i, c, s, work, env, candidate, modes, ctx, "fs-editwm-prog"): i
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
