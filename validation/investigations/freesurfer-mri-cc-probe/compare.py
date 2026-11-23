# SPDX-License-Identifier: Apache-2.0
"""mri_cc as a black box: FreeSurfer 8.2.0's official program against a candidate, case by case.

An open probe (``README.md``): it writes seeded synthetic subjects (folders ``<subject>/mri/``)
into ``<workspace>/oracles/inputs/fs-cc-prog/<seed>/``: small ones of 32^3 voxels (a white-matter
volume named ``wm`` as an uncompressed NIfTI file, as a gzipped MGH file and in ``float`` with
NaNs and infinities; segmentations with both hemispheres, one only, none, one holding the
callosum's label; intensity volumes; Talairach transforms), a conformed 256^3 head-like phantom
(two hemispheres of white matter and cortex, a callosum between them, ventricles; its intensity
volume, its white matter as a NIfTI file named ``wm`` and as a COR folder, a Talairach transform)
and the same phantom with voxels of 1.4 mm; then runs each case's command line through the official
program (``$FREESURFER_HOME/bin/mri_cc``, found on the ``PATH`` by its name as recon-all runs it)
by the oracle-output cache at one thread, and through the candidate (``--candidate``, a command
line whose ``{mode}`` becomes ``reproduce`` and then ``fix``), each in a folder of its own that
holds a copy of the case's subjects. Compared: the exit status, standard output and error (the
run's time, its peak memory, the case's folder and the candidate's ``larmorx:`` notices aside)
and every file written (FreeSurfer's provenance aside: ``larmorx_validation.freesurfer.files``).
``reproduce`` must give FreeSurfer's on every case; ``fix`` may differ only with a notice.
``--heads`` adds the recorded heads' cases (recon-all's call on each run's segmentation, and the
white-matter path on its white matter written as a COR folder).

    python -P validation/investigations/freesurfer-mri-cc-probe/compare.py --seed 1 \\
        --candidate '<program> --upstream-defects {mode} freesurfer mri_cc' [--heads]
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
#: or writes memory it does not own there, where the spec gives an error in both modes or fills
#: what FreeSurfer would read past a volume (FS-53, FS-1201, FS-1206).
NOT_COMPARABLE = frozenset({"FS-53", "FS-1201", "FS-1206"})
#: FreeSurfer's runs compared with their entries: ``--all-info``'s report holds the run's time.
POLICY = oracle_cache.Policy(
    threads=1, family="freesurfer", formats="freesurfer", volatile=(r"TimeStamp: \S+",)
)
STAMP = re.compile(r"TimeStamp: \S+")
TIMING = re.compile(r"^(corpus callosum segmentation took) .* minutes$", re.M)
PEAK = re.compile(r"^#VMPC# mri_cc VmPeak .*$", re.M)
#: Never a diagnostic setting on the official program: its licence check prints the licence file
#: from a diagnostic level of 1 (the probe refuses such a case).
DIAG = re.compile(r"^(DIAG|diag)")
N = 32
#: The recorded heads (``oracles/freesurfer-8.2.0/runs/<head>-<stream>/``) for ``--heads``.
HEADS = (
    "ds000005-sub-01",
    "ds000228-sub-pixar008",
    "ds002422-sub-02",
    "ds004173-sub-603309",
    "ds005003-sub-RS001",
    "ds005454-sub-16",
)


def lia(n: int = N, size: float = 1.0) -> np.ndarray:
    """A coronal (LIA) grid of ``n^3`` voxels of ``size`` mm centred on the origin."""
    h = n / 2 * size
    return np.array([[-size, 0, 0, h], [0, 0, size, -h], [0, -size, 0, h], [0, 0, 0, 1.0]])


def save(path: Path, data: np.ndarray, affine: np.ndarray) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    nib.save(nib.MGHImage(data, affine), str(path))
    return path


def save_wm_nifti(path: Path, data: np.ndarray, affine: np.ndarray) -> Path:
    """An uncompressed single-file NIfTI written under a name without a suffix (``wm``)."""
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".nii")
    nib.save(nib.Nifti1Image(data, affine), str(tmp))
    tmp.rename(path)
    return path


def xfm(path: Path, m: np.ndarray) -> Path:
    """A Talairach transform (an MNI transform file) of the 3 x 4 matrix ``m``."""
    path.parent.mkdir(parents=True, exist_ok=True)
    rows = "\n".join(" ".join(f"{v:.10g}" for v in r) for r in m[:3])
    path.write_text(
        f"MNI Transform File\n% probe\n\nTransform_Type = Linear;\nLinear_Transform =\n{rows};\n",
        encoding="utf-8",
    )
    return path


def small_affine(rng: np.random.Generator) -> np.ndarray:
    """A small rotation about each axis (up to 2 degrees), a scaling of 0.95 to 1.05, a shift of
    up to 3 mm."""
    m = np.eye(4)
    for i, j in ((0, 1), (1, 2), (0, 2)):
        t = np.deg2rad(rng.uniform(-2, 2))
        r = np.eye(4)
        r[i, i] = r[j, j] = np.cos(t)
        r[i, j], r[j, i] = -np.sin(t), np.sin(t)
        m = r @ m
    m[:3, :3] *= rng.uniform(0.95, 1.05, 3)
    m[:3, 3] = rng.uniform(-3, 3, 3)
    return m


def small(d: Path, rng: np.random.Generator) -> None:
    """The small subjects (32^3 voxels of 1 mm): ``s`` (white matter, segmentation, intensities,
    transform), ``ro`` (a copy of ``s`` whose ``mri`` folder the cases make read-only), ``n`` (a
    ``float`` white matter with NaNs and infinities), ``z`` (the white matter as a gzipped MGH file
    named ``wm``), ``w`` (the transform only), ``x`` (the white matter only), ``c`` (a segmentation
    that holds the callosum's central label 253), ``e0`` (an empty segmentation), ``el`` (only the
    left hemisphere's labels), ``hv`` (a white matter of 64^3 voxels of 0.45 mm)."""
    s = (N, N, N)
    x, y, z = np.meshgrid(np.arange(N), np.arange(N), np.arange(N), indexing="ij")
    gap = int(rng.integers(13, 16))
    inside = (y >= 4) & (y < 28) & (z >= 6) & (z < 26)
    hemi = ((x >= 4) & (x < gap)) | ((x >= gap + 4) & (x < 28))
    bridge = (x >= gap) & (x < gap + 4) & (y >= 10) & (y < 14) & (z >= 8) & (z < 24)
    wm = np.where(inside & hemi | bridge, 110, 0).astype(np.uint8)
    fill = rng.random(s) < 0.15
    wm[fill & (wm > 0)] = rng.choice([200, 210, 220, 230, 240, 250], int((fill & (wm > 0)).sum()))
    a = lia()
    seg = np.zeros(s, np.int32)
    box = inside & (x >= 3) & (x < 29)
    shell = (y == 4) | (y == 27) | (z == 6) | (z == 25) | (x == 3) | (x == 28)
    left = x >= gap + 2
    seg[box & left & ~shell] = 2
    seg[box & left & shell] = 3
    seg[box & ~left & ~shell] = 41
    seg[box & ~left & shell] = 42
    norm = np.where(seg > 0, rng.integers(45, 130, s), 0).astype(np.uint8)
    for subj in ("s", "ro"):
        m = d / subj / "mri"
        save_wm_nifti(m / "wm", wm, a)
        save(m / "aseg.mgz", seg, a)
        save(m / "norm.mgz", norm, a)
        xfm(m / "transforms" / "talairach.xfm", small_affine(rng))
    wf = wm.astype(np.float32) + np.float32(0.25)
    bits = wf.view(np.uint32).reshape(-1).copy()
    k = rng.choice(bits.size, 400, replace=False)
    # NaNs and infinities by their bits: quiet and signalling NaNs of both signs, +-infinity
    bits[k[:100]] = 0x7FC00000
    bits[k[100:200]] = 0xFFC00001
    bits[k[200:250]] = 0x7FA00001
    bits[k[250:325]] = 0x7F800000
    bits[k[325:]] = 0xFF800000
    m = d / "n" / "mri"
    save_wm_nifti(m / "wm", bits.view(np.float32).reshape(s), a)
    xfm(m / "transforms" / "talairach.xfm", small_affine(rng))
    m = d / "z" / "mri"
    save(m / "wm.mgz", wm, a).rename(m / "wm")
    xfm(m / "transforms" / "talairach.xfm", np.eye(4))
    xfm(d / "w" / "mri" / "transforms" / "talairach.xfm", np.eye(4))
    save_wm_nifti(d / "x" / "mri" / "wm", wm, a)
    cc = seg.copy()
    cc[gap + 1, 12, 16] = 253
    save(d / "c" / "mri" / "aseg.mgz", cc, a)
    save(d / "c" / "mri" / "norm.mgz", norm, a)
    save(d / "e0" / "mri" / "aseg.mgz", np.zeros(s, np.int32), a)
    save(d / "e0" / "mri" / "norm.mgz", norm, a)
    el = np.where(left, seg, 0).astype(np.int32)
    save(d / "el" / "mri" / "aseg.mgz", el, a)
    save(d / "el" / "mri" / "norm.mgz", norm, a)
    hx = np.zeros((64, 64, 64), np.uint8)
    hx[10:54, 12:50, 8:56] = 110
    save_wm_nifti(d / "hv" / "mri" / "wm", hx, lia(64, 0.45))
    xfm(d / "hv" / "mri" / "transforms" / "talairach.xfm", np.eye(4))


def phantom(d: Path, rng: np.random.Generator, v: float = 1.0, n: int = 256) -> None:
    """A head-like phantom of ``n^3`` coronal voxels of ``v`` mm centred on the origin: two
    hemispheres of white matter (labels 2 and 41, intensity about 110) inside cortex (3 and 42,
    about 70) with flat medial faces a voxel or two apart, a callosum (an arc in the sagittal
    planes, 7 mm thick, 32 mm wide, white matter of both labels interleaved at the midline), two
    ventricles below it (4 and 43, about 25), the whole rotated about the vertical axis by up to 3
    degrees and moved by up to 2 mm; noise on the intensities; ``mri/aseg.auto_noCCseg.mgz``,
    ``mri/norm.mgz``, ``mri/wm`` (an uncompressed NIfTI file: 110 in the white matter) and
    ``mri/transforms/talairach.xfm``."""
    u = (np.arange(n) - n / 2) * v + 128
    x, y, z = np.meshgrid(u, u, u, indexing="ij", sparse=True)
    ang = np.deg2rad(rng.uniform(-3, 3))
    c, s = np.cos(ang), np.sin(ang)
    xc = 128 + rng.uniform(-2, 2)
    xr = (x - xc) * c - (z - 128) * s + xc
    zr = (x - xc) * s + (z - 128) * c + 128
    seg = np.zeros((n, n, n), np.int32)

    def ell(cx: float, cy: float, cz: float, rx: float, ry: float, rz: float) -> np.ndarray:
        return ((xr - cx) / rx) ** 2 + ((y - cy) / ry) ** 2 + ((zr - cz) / rz) ** 2 <= 1

    outer = ell(xc, 118, 128, 70, 50, 75)
    inner = ell(xc, 118, 128, 64, 44, 68)
    for side, wml, ctx in ((1, 2, 3), (-1, 41, 42)):
        half = side * (xr - xc) >= 1
        halfw = side * (xr - xc) >= 4
        seg[outer & half & ~(inner & halfw)] = ctx
        seg[inner & halfw] = wml
    dz = (zr - 128) / rng.uniform(33, 39)
    top = rng.uniform(110, 114) - 12 * np.sqrt(np.clip(1 - dz**2, 0, None))
    arc = (np.abs(dz) <= 1) & (y >= top) & (y < top + 7) & (np.abs(xr - xc) <= 16)
    mix = (np.abs(xr - xc) < 2) & ((np.floor(y) + np.floor(z)) % 2 == 0)
    seg[arc & ((xr >= xc) ^ mix)] = 2
    seg[arc & ((xr < xc) ^ mix)] = 41
    seg[ell(xc + 6, 124, 128, 4, 6, 25)] = 4
    seg[ell(xc - 6, 124, 128, 4, 6, 25)] = 43
    level = {2: 110, 41: 110, 3: 70, 42: 70, 4: 25, 43: 25}
    norm = np.zeros((n, n, n), np.float32)
    for k, val in level.items():
        norm[seg == k] = val
    norm += (seg > 0) * rng.normal(0, 4, (n, n, n)).astype(np.float32)
    a = lia(n, v)
    m = d / "mri"
    save(m / "aseg.auto_noCCseg.mgz", seg, a)
    save(m / "norm.mgz", np.clip(norm, 0, 255).astype(np.uint8), a)
    save_wm_nifti(m / "wm", np.where((seg == 2) | (seg == 41), 110, 0).astype(np.uint8), a)
    t = np.eye(4)
    t[:3, 3] = rng.uniform(-2, 2, 3)
    xfm(m / "transforms" / "talairach.xfm", t)


def official() -> Path:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    return ws / FS_HOME / "bin" / "mri_cc"


def base_env(home: Path) -> dict[str, str]:
    exe = official()
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(home), "USER": "probe",
           "FS_DISABLE_LANG": "1"}  # fmt: skip
    env.update(freesurfer_env())
    return env


def inputs(ws: Path, seed: int) -> Path:
    """The subjects of ``seed`` (written once: a marker file says they are complete)."""
    d = ws / "oracles" / "inputs" / "fs-cc-prog" / str(seed) / "subjects"
    done = d / ".complete-v2"
    if done.exists():
        return d
    if d.exists():
        shutil.rmtree(d)
    rng = np.random.default_rng(seed)
    small(d, rng)
    phantom(d / "p", rng)
    phantom(d / "pv", rng, 1.4)
    # the phantom's segmentation holding the callosum's central label (status 77, or -force)
    pc = d / "pc" / "mri"
    pc.mkdir(parents=True)
    img = nib.load(str(d / "p" / "mri" / "aseg.auto_noCCseg.mgz"))
    seg = np.asarray(img.dataobj).copy()
    seg[128, 112, 128] = 253
    save(pc / "aseg.auto_noCCseg.mgz", seg, img.affine)
    shutil.copy(d / "p" / "mri" / "norm.mgz", pc)
    # the phantom's white matter as a COR folder, written by the official mri_convert
    pk = d / "pk" / "mri"
    (pk / "transforms").mkdir(parents=True)
    shutil.copy(d / "p" / "mri" / "transforms" / "talairach.xfm", pk / "transforms")
    shutil.copy(d / "p" / "mri" / "wm", pk / "wm.nii")
    (pk / "wm").mkdir()
    made = oracle_cache.run(
        ["mri_convert", "wm.nii", "wm", "-ot", "cor"],
        env=base_env(d),
        cwd=pk,
        outputs=[pk / "wm"],
        policy=POLICY,
        timeout=900,
        runner="fs-cc-prog",
        inputs=[pk / "wm.nii"],
    )
    if made.returncode != 0:
        raise SystemExit(f"mri_convert -ot cor failed: {made.stderr[-500:]}")
    (pk / "wm.nii").unlink()
    # a copy of the phantom whose mri folder the cases make read-only
    shutil.copytree(d / "p", d / "pro")
    done.write_text("complete\n", encoding="utf-8")
    return d


@dataclass
class Case:
    """A command line, the subjects its folder holds, the variables it adds, the subjects whose
    ``mri`` folders are made read-only."""

    args: list[str]
    subjects: tuple[str, ...] = ("s",)
    env: dict[str, str] = field(default_factory=dict)
    readonly: tuple[str, ...] = ()


#: The options that take values, each given last (FS-1203).
LAST = ("-sdir", "-aseg", "-norm", "-lta", "-o", "-m", "-d", "-s", "-a", "-t", "-DEBUG_VOXEL")


def cases() -> list[Case]:
    seg2cc = ["-aseg", "aseg.auto_noCCseg.mgz", "-o", "aseg.auto.mgz"]
    lta = ["-lta", "p/mri/transforms/cc_up.lta"]
    a0 = ["-sdir", ".", "-a", "0"]
    sd = ["-sdir", "."]
    c: list[Case] = [
        # the command line
        Case([]),
        Case(["s"], env={"SUBJECTS_DIR": "."}),
        Case(["s"]),
        Case([*sd, "s", "t"]),
        Case(["--help"]),
        Case(["--usage"]),
        Case(["-help"]),
        Case(["-u"]),
        Case(["-?"]),
        Case(["-HELP"]),
        Case(["--version"]),
        Case(["--all-info"]),
        Case(["-version"]),
        Case(["-x", "s"]),
        Case(["-", "s"]),
        Case(["--sdir", ".", "s"]),
        Case([*sd]),
        Case([*sd, "nosubject"]),
        # every option given last, or before too few values (FS-1203)
        *[Case([*sd, opt]) for opt in LAST],
        Case([*sd, "-DEBUG_VOXEL", "1", "2"]),
        Case([*sd, "-DEBUG_VOXEL", "1", "2", "3", "s"]),
        # options by their first letter, names without case
        Case(["-SDIR", ".", "-Aseg", "aseg.mgz", "-LH", "s"]),
        Case([*sd, "-fornix", "-lh", "s"]),
        Case([*sd, "-mx", "2", "-lh", "s"]),
        Case([*sd, "-output", "x.mgz", "-rh", "s"]),
        Case([*sd, "-dd", "3", "-lh", "s"]),
        Case([*sd, "-skip", "1", "-lh", "s"]),
        Case([*sd, "-thick", "1", "-lh", "s"]),
        Case([*sd, "-ax", "1", "-lh", "s"]),
        Case([*sd, "-usage", "s"]),
        Case([*sd, "-hh", "s"]),
        Case([*sd, "-force", "-lh", "s"]),
        Case([*sd, "-rh", "-o", "rh.mgz", "s"]),
        # the aseg path on the small subjects: no white matter found in the upright frame
        Case([*sd, "-aseg", "aseg.mgz", "s"]),
        Case([*sd, "-aseg", "aseg.mgz", "-m", "1", "-s", "3", "s"]),
        Case([*sd, "-aseg", "aseg.mgz", "c"], subjects=("c",)),
        Case([*sd, "-aseg", "aseg.mgz", "-force", "-m", "1", "c"], subjects=("c",)),
        Case([*sd, "-aseg", "aseg.mgz", "e0"], subjects=("e0",)),
        Case([*sd, "-aseg", "aseg.mgz", "el"], subjects=("el",)),
        Case([*sd, "-aseg", "no.mgz", "s"]),
        Case([*sd, "-aseg", "aseg.mgz", "-norm", "no.mgz", "s"]),
        Case([*sd, "-aseg", "aseg.mgz", "-norm", "aseg.mgz", "-m", "1", "s"]),
        Case([*sd, "-aseg", "aseg.mgz", "-o", "nodir/x.mgz", "-lh", "s"]),
        Case([*sd, "-aseg", "aseg.mgz", "-lh", "ro"], subjects=("ro",), readonly=("ro",)),
        Case([*sd, "-aseg", "aseg.mgz", "-d", "51", "s"]),
        Case([*sd, "-d", "0", "-aseg", "aseg.mgz", "-m", "1", "s"]),
        Case([*sd, "-d", "-1", "-aseg", "aseg.mgz", "-m", "1", "s"]),
        Case([*sd, "-d", "abc", "-aseg", "aseg.mgz", "-m", "1", "s"]),
        Case([*sd, "-m", "nan", "-aseg", "aseg.mgz", "s"]),
        # names of 1,023 and 1,024 bytes (FS-1212)
        Case([*sd, "-aseg", "aseg.mgz", "-o", "a" * 1023, "-lh", "s"]),
        Case([*sd, "-aseg", "aseg.mgz", "-o", "a" * 1024, "-lh", "s"]),
        Case([*sd, "-aseg", "a" * 1024, "s"]),
        Case([*sd, "-norm", "a" * 1024, "s"]),
        Case([*sd, "-lta", "a" * 1024, "s"]),
        Case(["-sdir", "./" + "/" * 1022, "s"]),
        Case(["s"], env={"SUBJECTS_DIR": "./" + "/" * 1022}),
        # the white-matter path on the small subjects (FS-1210: no column qualifies)
        Case([*a0, "s"]),
        Case([*a0, "-d", "3", "-t", "1", "s"]),
        Case([*a0, "-d", "1", "s"]),
        Case([*a0, "-d", "50", "-t", "0", "s"]),
        Case([*a0, "-d", "51", "s"]),
        Case([*a0, "-d", "0", "s"]),
        Case([*a0, "-t", "-1", "s"]),
        Case([*a0, "-t", "20", "s"]),
        Case([*a0, "-t", "1000000", "s"]),
        Case([*a0, "-m", "nan", "-s", "2", "s"]),
        Case([*a0, "-f", "-lh", "-o", "x.mgz", "s"]),
        Case([*a0, "-DEBUG_VOXEL", "16", "16", "16", "s"]),
        Case([*a0, "n"], subjects=("n",)),
        Case([*a0, "n"], subjects=("n",), env={"FS_LEAVE_NANS": "1"}),
        Case([*a0, "z"], subjects=("z",)),
        Case([*a0, "w"], subjects=("w",)),
        Case([*a0, "x"], subjects=("x",)),
        Case([*a0, "hv"], subjects=("hv",)),
        Case([*a0, "ro"], subjects=("ro",), readonly=("ro",)),
        Case(["-a", "0", "s"], env={"SUBJECTS_DIR": "."}),
        Case(["-a", "0", "s"]),
        # the phantom: recon-all's call and its options
        Case([*sd, *seg2cc, *lta, "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-lta", "cc_up.lta", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-f", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-d", "7", "-t", "3", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-d", "1", "-t", "0", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-d", "0", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-t", "-1", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-t", "200", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-s", "2", "-m", "3", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-m", "0", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-force", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-lh", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-norm", "aseg.auto_noCCseg.mgz", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "-lta", "nodir/cc.lta", "p"], subjects=("p",)),
        Case([*sd, *seg2cc, "pc"], subjects=("pc",)),
        Case([*sd, *seg2cc, "-force", "pc"], subjects=("pc",)),
        Case([*sd, *seg2cc, "-lta", "pro/mri/cc.lta", "pro"], ("pro",), readonly=("pro",)),
        Case([*sd, *seg2cc, *lta, "p"], subjects=("p",), env={"FS_LEAVE_NANS": "1"}),
        # the white-matter path on the phantom
        Case([*a0, "p"], subjects=("p",)),
        Case([*a0, "-d", "7", "-t", "3", "p"], subjects=("p",)),
        Case([*a0, "-t", "200", "p"], subjects=("p",)),
        Case([*a0, "pk"], subjects=("pk",)),
        Case([*a0, "pv"], subjects=("pv",)),
        Case([*a0, "-m", "2", "pro"], subjects=("pro",), readonly=("pro",)),
    ]
    return c


def mask(text: str, port: bool, folder: Path) -> str:
    """The messages without what differs between the sides: the time (``--all-info``'s report
    and the timing line), the peak memory, the case's folder, the candidate's notices."""
    text = STAMP.sub("TimeStamp: <time>", text)
    text = TIMING.sub(r"\1 <time> minutes", text)
    text = PEAK.sub("#VMPC# mri_cc VmPeak <peak>", text)
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
        for p in d.rglob("*"):
            if p.is_dir():
                p.chmod(0o755)
        shutil.rmtree(d)
    d.mkdir(parents=True)
    for s in case.subjects:
        shutil.copytree(src / s, d / s)
    before = listing(d)
    for s in case.readonly:
        for p in [d / s / "mri", *(q for q in (d / s / "mri").rglob("*") if q.is_dir())]:
            p.chmod(stat.S_IRUSR | stat.S_IXUSR | stat.S_IRGRP | stat.S_IXGRP)
    return before


def unlock(d: Path) -> None:
    for p in d.rglob("*"):
        if p.is_dir():
            p.chmod(0o755)


def run_case(
    i: int, case: Case, src: Path, work: Path, env: dict[str, str], candidate: list[str],
    modes: tuple[str, ...], ctx: fsfiles.Context, runner: str,
) -> dict:  # fmt: skip
    if any(DIAG.match(k) for k in case.env):
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
                ["mri_cc", *case.args],
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
    row: dict = {"case": i, "args": [a if len(a) < 80 else f"<{len(a)} bytes>" for a in case.args],
                 "subjects": list(case.subjects), "env": case.env, "official_exit": oc}  # fmt: skip
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


def heads(ws: Path, env: dict[str, str]) -> tuple[Path, list[Case]]:
    """The recorded heads: for each run, a subject ``h`` holding its segmentation without the
    callosum, its intensity volume, its white matter as a COR folder (the official
    ``mri_convert -ot cor``) and its Talairach transform; recon-all's call and the white-matter
    path on each."""
    root = ws / "oracles" / "inputs" / "fs-cc-prog" / "heads"
    out: list[Case] = []
    for head in HEADS:
        for stream in ("classic", "v8"):
            run = f"{head}-{stream}"
            subjects = ws / "oracles" / "freesurfer-8.2.0" / "runs" / run / "subjects"
            rec = next(p for p in sorted(subjects.iterdir()) if p.name != "fsaverage") / "mri"
            m = root / run / "mri"
            done = root / run / ".complete-v1"
            if not done.exists():
                if m.exists():
                    shutil.rmtree(m)
                (m / "transforms").mkdir(parents=True)
                (m / "wm").mkdir()
                for f in ("aseg.auto_noCCseg.mgz", "norm.mgz"):
                    shutil.copy(rec / f, m)
                shutil.copy(rec / "transforms" / "talairach.xfm", m / "transforms")
                made = oracle_cache.run(
                    ["mri_convert", str(rec / "wm.mgz"), "wm", "-ot", "cor"],
                    env=env,
                    cwd=m,
                    outputs=[m / "wm"],
                    policy=POLICY,
                    timeout=900,
                    runner="fs-cc-prog",
                    inputs=[rec / "wm.mgz"],
                )
                if made.returncode != 0:
                    raise SystemExit(f"{run}: mri_convert -ot cor failed: {made.stderr[-500:]}")
                done.write_text("complete\n", encoding="utf-8")
            out.append(Case(["-sdir", ".", "-aseg", "aseg.auto_noCCseg.mgz", "-o",
                             "aseg.auto.mgz", "-lta", f"{run}/mri/transforms/cc_up.lta", run],
                            subjects=(run,)))  # fmt: skip
            out.append(Case(["-sdir", ".", "-a", "0", run], subjects=(run,)))
    return root, out


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
    work = ws / "oracles" / "inputs" / "fs-cc-prog" / f"work-{tag}-{seed}"
    work.mkdir(parents=True, exist_ok=True)
    env = base_env(work)
    src = inputs(ws, seed)
    todo = [(i, c, src) for i, c in enumerate(cases())]
    if with_heads:
        hsrc, hcases = heads(ws, env)
        todo += [(1000 + i, c, hsrc) for i, c in enumerate(hcases)]
    keep = {int(x) for x in only.split(",")} if only else None
    todo = [t for t in todo if keep is None or t[0] in keep]
    ctx = fsfiles.Context(any_directory=True)
    verdicts: Counter = Counter()
    rows = []
    with concurrent.futures.ThreadPoolExecutor(max(1, jobs)) as pool:
        futures = {
            pool.submit(run_case, i, c, s, work, env, candidate, modes, ctx, "fs-cc-prog"): i
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
