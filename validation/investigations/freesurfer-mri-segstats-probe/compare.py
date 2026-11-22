# SPDX-License-Identifier: Apache-2.0
"""mri_segstats as a black box: FreeSurfer 8.2.0's official program against a candidate, case by case.

An open probe (``README.md``): it writes seeded synthetic inputs (MGZ volumes: segmentations of a
head-like pattern with FreeSurfer's labels as int, uchar and float, intensity volumes in uchar and
float, a three-frame float volume, a mask, volumes of other sizes; a colour table; replacement
lists; a statistics table; a registration in both of FreeSurfer's formats; a Talairach transform;
two subjects' folders with the files the subject options read: a ribbon, the brain-volume
statistics, closed and open surfaces, an annotation, labels, the volumes of --qa-stats) into
``<workspace>/oracles/inputs/fs-segstats-prog/<seed>/``, runs each case's command line through the
official program (``$FREESURFER_HOME/bin/mri_segstats``, found on the ``PATH`` by its name as the
scripts run it) by the oracle-output cache at one thread, and through the candidate
(``--candidate``, a command line whose ``{mode}`` becomes ``reproduce`` and then ``fix``), each in a
folder of its own (outputs are written there, temporary files beside it). Compared: the exit
status, standard output and error (``--all-info``'s time and the candidate's ``larmorx:`` notices
aside) and every file written (FreeSurfer's provenance aside: ``larmorx_validation.freesurfer.files``).
``reproduce`` must give FreeSurfer's on every case but those whose reproduce run notes a defect of
``NOT_COMPARABLE``, which are counted; ``fix`` may differ only with a notice.

    python -P validation/investigations/freesurfer-mri-segstats-probe/compare.py --seed 1 \\
        --candidate '<program> --upstream-defects {mode} freesurfer mri_segstats'
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
#: reads memory it never wrote or does not own, writes past its buffers, or crashes or aborts
#: there (FS-53, FS-58, FS-469 and the program's own).
NOT_COMPARABLE = frozenset(
    {"FS-53", "FS-58", "FS-469", "FS-1102", "FS-1103", "FS-1104", "FS-1109", "FS-1111", "FS-1112",
     "FS-1113", "FS-1114", "FS-1115", "FS-1116", "FS-1117"}
)  # fmt: skip
#: FreeSurfer's runs compared with their entries: ``--all-info``'s report holds the run's time.
POLICY = oracle_cache.Policy(
    threads=1, family="freesurfer", formats="freesurfer", volatile=(r"TimeStamp: \S+",)
)
#: ``--all-info``'s time of the run (UTC, to the second).
STAMP = re.compile(r"TimeStamp: \S+")


def segmentation(rng: np.random.Generator, shape: tuple[int, int, int]) -> np.ndarray:
    """FreeSurfer's labels in a head-like pattern: white matter 2 and 41, cortex 3 and 42,
    ventricles 4 and 43, hippocampi 17 and 53, the corpus callosum 251 and 252, cerebellar white
    matter 7 and 46, CSF 24, vessels 30 and 62, scattered 77 and 80, 1000-range cortical labels,
    background 0."""
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
    seg[(np.abs(idx[0] - c[0]) < 1) & (r < 0.15)] = 252
    cb = (idx[2] < c[2] * 0.6) & (r < 0.5)
    seg[cb] = np.where(left, 7, 46)[cb]
    ctx = (seg == 3) & (rng.random(shape) < 0.3)
    seg[ctx] = 1000 + rng.integers(1, 35, shape)[ctx]
    seg[(seg == 2) & (rng.random(shape) < 0.03)] = 77
    seg[(seg == 41) & (rng.random(shape) < 0.02)] = 80
    ves = (seg == 24) & (rng.random(shape) < 0.04)
    seg[ves] = np.where(left, 30, 62)[ves]
    return seg


def head(rng: np.random.Generator, seg: np.ndarray) -> np.ndarray:
    """Intensities by tissue (white matter near 110, grey matter 75, CSF 35, background 4)."""
    base = {2: 110, 41: 110, 7: 105, 46: 105, 77: 90, 80: 85, 251: 115, 252: 115, 3: 75,
            42: 75, 17: 70, 53: 70, 4: 30, 43: 30, 24: 35, 30: 50}  # fmt: skip
    v = np.full(seg.shape, 4.0)
    for k, b in base.items():
        v[seg == k] = b
    v[(seg >= 1000) & (seg < 2000)] = 72
    return np.clip(v + rng.normal(0.0, 6.0, seg.shape), 0, None)


def save(path: Path, data: np.ndarray, affine: np.ndarray) -> Path:
    nib.save(nib.MGHImage(data, affine), str(path))
    return path


def vol_info(dims: tuple[int, int, int], name: str) -> str:
    """An LTA's volume geometry block for the probe's 1 mm grid centred at 0."""
    e = "1.000000000000000e+00"
    z = "0.000000000000000e+00"
    return (
        f"valid = 1  # volume info valid\nfilename = {name}\nvolume = {dims[0]} {dims[1]} {dims[2]}\n"
        f"voxelsize = {e} {e} {e}\nxras   = {e} {z} {z}\nyras   = {z} {e} {z}\nzras   = {z} {z} {e}\n"
        f"c_ras  = {z} {z} {z}\n"
    )


def octahedron(radius: float, open_one: bool = False) -> tuple[np.ndarray, np.ndarray]:
    """A closed surface of 6 vertices and 8 faces (Euler number 2), or with one face left out
    (an open surface: Euler number 1)."""
    r = radius
    v = np.array([[r, 0, 0], [-r, 0, 0], [0, r, 0], [0, -r, 0], [0, 0, r], [0, 0, -r]], float)
    fa = np.array(
        [[0, 2, 4], [2, 1, 4], [1, 3, 4], [3, 0, 4], [2, 0, 5], [1, 2, 5], [3, 1, 5], [0, 3, 5]]
    )
    return v, (fa[1:] if open_one else fa)


def subject(d: Path, name: str, rng: np.random.Generator, aff: np.ndarray, full: bool) -> None:
    """A subject's folder: the ribbon, the brain-volume statistics, the Talairach transform, the
    surfaces before the topology fix (``lh`` closed, ``rh`` open), the white surfaces, the
    annotation, the cortex labels; ``full`` adds the volumes of ``--qa-stats``."""
    s = d / "subjects" / name
    for sub in ("mri/transforms", "surf", "label", "stats", "tmp"):
        (s / sub).mkdir(parents=True, exist_ok=True)
    seg = segmentation(rng, (32, 32, 32))
    save(s / "mri" / "ribbon.mgz", (seg % 7).astype(np.int32), aff)
    names = ["BrainSeg, BrainSegVol, Brain Segmentation Volume",
             "BrainSegNotVent, BrainSegVolNotVent, Brain Segmentation Volume Without Ventricles",
             "SupraTentorial, SupraTentorialVol, Supratentorial volume",
             "SupraTentorialNotVent, SupraTentorialVolNotVent, Supratentorial volume",
             "SubCortGray, SubCortGrayVol, Subcortical gray matter volume",
             "lhCortex, lhCortexVol, Left hemisphere cortical gray matter volume",
             "rhCortex, rhCortexVol, Right hemisphere cortical gray matter volume",
             "Cortex, CortexVol, Total cortical gray matter volume",
             "TotalGray, TotalGrayVol, Total gray matter volume",
             "lhCerebralWhiteMatter, lhCerebralWhiteMatterVol, Left hemisphere cerebral white matter volume",
             "rhCerebralWhiteMatter, rhCerebralWhiteMatterVol, Right hemisphere cerebral white matter volume",
             "CerebralWhiteMatter, CerebralWhiteMatterVol, Total cerebral white matter volume",
             "Mask, MaskVol, Mask Volume",
             "SupraTentorialNotVentVox, SupraTentorialVolNotVentVox, Supratentorial volume voxel count",
             "BrainSegNotVentSurf, BrainSegVolNotVentSurf, Brain Segmentation Volume Without Ventricles from Surf",
             "VentricleChoroidVol, VentricleChoroidVol, Volume of ventricles and choroid plexus"]  # fmt: skip
    vals = rng.uniform(1000.0, 30000.0, len(names))
    (s / "stats" / "brainvol.stats").write_text(
        "".join(f"# Measure {n}, {v:.12f}, mm^3\n" for n, v in zip(names, vals, strict=True)),
        encoding="utf-8",
    )
    (s / "mri" / "transforms" / "talairach.xfm").write_text(
        "MNI Transform File\n%probe\n\nTransform_Type = Linear;\nLinear_Transform =\n"
        "   1.05   0.01   0.02   2.5\n  -0.03   0.98   0.05  -1.25\n   0.01  -0.04   1.10   3.0 ;\n",
        encoding="utf-8",
    )
    for hemi, open_one in (("lh", False), ("rh", True)):
        v, fa = octahedron(6.0, open_one)
        nib.freesurfer.write_geometry(str(s / "surf" / f"{hemi}.orig.nofix"), v, fa)
        v2, fa2 = octahedron(7.0)
        nib.freesurfer.write_geometry(str(s / "surf" / f"{hemi}.white"), v2, fa2)
        labels = np.array([1, 2, 1, 3, 2, 0])
        ctab = np.array(
            [[25, 5, 25, 0, 0], [25, 100, 40, 0, 0], [220, 20, 100, 0, 0], [220, 180, 140, 0, 0]],
            np.int32,
        )
        ctab[:, 4] = ctab[:, 0] + ctab[:, 1] * 256 + ctab[:, 2] * 65536
        nib.freesurfer.write_annot(
            str(s / "label" / f"{hemi}.aparc.annot"), labels, ctab,
            [b"unknown", b"bankssts", b"caudalanteriorcingulate", b"caudalmiddlefrontal"],
        )  # fmt: skip
        (s / "label" / f"{hemi}.cortex.label").write_text(
            "#!ascii label  , from subject probe\n4\n0  7.0 0.0 0.0 0.5\n2  0.0 7.0 0.0 0.1\n"
            "3  0.0 -7.0 0.0 0.9\n4  0.0 0.0 7.0 0.3\n",
            encoding="utf-8",
        )
        if full:
            save(
                s / "surf" / f"{hemi}.w-g.pct.mgh",
                rng.normal(20.0, 4.0, (6, 1, 1)).astype(np.float32),
                np.eye(4),
            )
    if full:
        t = head(rng, seg)
        save(s / "mri" / "norm.mgz", np.clip(np.rint(t), 0, 255).astype(np.uint8), aff)
        wm = np.where(np.isin(seg, [2, 41, 77]), 110, 0).astype(np.uint8)
        wm[rng.random(wm.shape) < 0.002] = 1
        wm[rng.random(wm.shape) < 0.002] = 255
        save(s / "mri" / "wm.mgz", wm, aff)
        bm = np.clip(np.rint(t), 0, 255).astype(np.uint8)
        save(s / "mri" / "brainmask.auto.mgz", bm, aff)
        bm2 = bm.copy()
        bm2[rng.random(bm.shape) < 0.003] = 1
        bm2[rng.random(bm.shape) < 0.002] = 77
        save(s / "mri" / "brainmask.mgz", bm2, aff)
        save(s / "mri" / "aseg.auto.mgz", seg, aff)
        seg2 = seg.copy()
        seg2[rng.random(seg.shape) < 0.01] = 41
        save(s / "mri" / "aseg.mgz", seg2, aff)
        # solid white matter (the ventricles and hippocampi filled) that survives 3 erosions
        apas = seg.copy()
        apas[np.isin(apas, [4, 43, 17, 53, 24])] = 2
        save(s / "mri" / "aparc+aseg.mgz", apas, aff)
        (s / "tmp" / "control.dat").write_text(
            "10 12 -3\n-4 5 6\n1 1 1\ninfo\nnumpoints 3\nuseRealRAS 0\n", encoding="utf-8"
        )


def inputs(ws: Path, seed: int) -> dict[str, Path]:
    d = ws / "oracles" / "inputs" / "fs-segstats-prog" / str(seed)
    d.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(seed)
    aff = np.diag([1.0, 1.0, 1.0, 1.0])
    aff[:3, 3] = [-16.0, -16.0, -16.0]
    f: dict[str, Path] = {}
    seg = segmentation(rng, (32, 32, 32))
    f["seg"] = save(d / "seg_32.mgz", seg, aff)
    f["segu8"] = save(d / "seg_u8_32.mgz", np.where(seg > 255, 3, seg).astype(np.uint8), aff)
    segf = seg.astype(np.float32) + rng.uniform(0.0, 0.9, seg.shape).astype(np.float32)
    f["segf"] = save(d / "seg_f32_32.mgz", segf, aff)
    t = head(rng, seg)
    f["u8"] = save(d / "t1_u8_32.mgz", np.clip(np.rint(t), 0, 255).astype(np.uint8), aff)
    f["f32"] = save(d / "t1_f32_32.mgz", (t - 40.0).astype(np.float32), aff)
    f["f3"] = save(
        d / "t1_f32_32x3.mgz",
        np.stack([t, t * 1.1 + 3, t * 0.9 - 2], axis=-1).astype(np.float32),
        aff,
    )
    f["mask"] = save(d / "mask_f32_32.mgz", rng.random((32, 32, 32)).astype(np.float32), aff)
    f["mask2"] = save(d / "mask_f32_32x2.mgz", rng.random((32, 32, 32, 2)).astype(np.float32), aff)
    aff2 = np.diag([1.2, 0.9, 1.1, 1.0])
    aff2[:3, 3] = [-20.0, -15.0, -14.0]
    f["segb"] = save(d / "seg_40x36x28.mgz", segmentation(rng, (40, 36, 28)), aff2)
    f["meas"] = save(
        d / "measure_6.mgz", rng.normal(20.0, 4.0, (6, 1, 1)).astype(np.float32), np.eye(4)
    )
    lut = [(0, "Unknown"), (2, "Left-Cerebral-White-Matter"), (3, "Left-Cerebral-Cortex"),
           (4, "Left-Lateral-Ventricle"), (17, "Left-Hippocampus"),
           (41, "Right-Cerebral-White-Matter"), (42, "Right-Cerebral-Cortex"),
           (43, "Right-Lateral-Ventricle"), (53, "Right-Hippocampus"), (88, "Not-In-The-Volume"),
           (120, "Last")]  # fmt: skip
    (d / "lut.txt").write_text(
        "".join(f"{k:4d}  {n:<32s} {(k * 37) % 256:3d} {(k * 91) % 256:3d} {(k * 53) % 256:3d}   0\n"
                for k, n in lut),
        encoding="utf-8",
    )  # fmt: skip
    f["lut"] = d / "lut.txt"
    (d / "replace.txt").write_text("# probe\n2 41\n3 42\n\t17   53\n", encoding="utf-8")
    f["rep"] = d / "replace.txt"
    (d / "replace_bad.txt").write_text("2 41\n3 42 7\n", encoding="utf-8")
    f["repbad"] = d / "replace_bad.txt"
    (d / "replace_empty.txt").write_text("# nothing\n", encoding="utf-8")
    f["repempty"] = d / "replace_empty.txt"
    (d / "sum_in.stats").write_text(
        "# Title Segmentation Statistics\n# a comment\n"
        "  1   2     1200     1200.0  Left-Cerebral-White-Matter   110.5   5.25  90  130  40\n"
        "  2  41      950      950.0  Right-Cerebral-White-Matter  111.0   4.75  91  129  38\n"
        "  3   4  oops\n",
        encoding="utf-8",
    )
    f["sumin"] = d / "sum_in.stats"
    (d / "register.dat").write_text(
        "probe\n1.000000\n1.000000\n0.150000\n1 0 0 1.5\n0 1 0 -2\n0 0 1 0.5\n0 0 0 1\nround\n",
        encoding="utf-8",
    )
    f["regdat"] = d / "register.dat"
    (d / "reg.lta").write_text(
        "# transform file\ntype      = 1 # LINEAR_RAS_TO_RAS\nnxforms   = 1\n"
        "mean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n"
        "1 0 0 1.5\n0 1 0 -2\n0 0 1 0.5\n0 0 0 1\n"
        f"src volume info\n{vol_info((32, 32, 32), 'src.mgz')}"
        f"dst volume info\n{vol_info((32, 32, 32), 'dst.mgz')}",
        encoding="utf-8",
    )
    f["reglta"] = d / "reg.lta"
    f["xfm"] = d / "subjects" / "subj" / "mri" / "transforms" / "talairach.xfm"
    (d / "tiv.dat").write_text("123456.75\n", encoding="utf-8")
    f["tiv"] = d / "tiv.dat"
    subject(d, "subj", rng, aff, True)
    subject(d, "bare", rng, aff, False)
    (d / "subjects" / "bare" / "mri" / "ribbon.mgz").unlink()
    f["sd"] = d / "subjects"
    f["label"] = d / "subjects" / "subj" / "label" / "lh.cortex.label"
    f["annot"] = d / "subjects" / "subj" / "label" / "lh.aparc.annot"
    return f


def cases(f: dict[str, Path], fs_home: Path) -> list[tuple[list[str], dict[str, str | None]]]:
    """The command lines (``@x`` an input's absolute name; outputs are relative) and each one's
    changes to the environment (``None``: unset)."""
    o = ["--sum", "out.stats"]
    sd = {"SUBJECTS_DIR": str(f["sd"])}
    nosd = {"SUBJECTS_DIR": None}
    gca = str(fs_home / "average" / "face.gca")
    # fmt: off
    c: list[tuple[list[str], dict[str, str | None]]] = [
        ([], {}),
        (["--help"], {}),
        (["--version"], {}),
        (["--all-info"], {}),
        (["--bogus"], {}),
        (["-seg", "@seg"], {}),
        (["--seg"], {}),
        (["--seg", "@seg"], {}),
        (["--seg", "@seg", *o], {}),
        (["--seg", "@segu8", *o], {}),
        (["--seg", "@segf", *o, "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--in", "@f32", "--snr"], {}),
        (["--seg", "@seg", *o, "--snr"], {}),
        (["--seg", "@seg", *o, "--in", "@u8", "--in-intensity-name", "norm",
          "--in-intensity-units", "MR"], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut"], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut", "--empty"], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut", "--empty", "--nonempty"], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut", "--in", "@u8", "--non-empty", "--snr"], {}),
        (["--seg", "@seg", *o, "--id", "2", "41", "3", "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--id", "2", "--id", "999", "--empty"], {}),
        (["--seg", "@seg", *o, "--id", "x", "2", "--empty"], {}),
        (["--seg", "@seg", *o, "--id"], {}),
        (["--seg", "@seg", *o, "--id", "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut", "--id", "2", "77"], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut", "--id", "2", "17", "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut", "--id", "2", "500", "--empty"], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut", "--id", "500"], {}),
        (["--seg", "@seg", *o, "--excludeid", "0", "2"], {}),
        (["--seg", "@seg", *o, "--exclude", "3", "--excl-ctxgmwm", "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--excludeid"], {}),
        (["--seg", "@seg", *o, "--mask", "@mask", "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--mask", "@mask", "--maskthresh", "0.3", "--masksign", "pos"], {}),
        (["--seg", "@seg", *o, "--mask", "@mask", "--maskthresh", "0.7", "--masksign", "NEGATIVE"], {}),
        (["--seg", "@seg", *o, "--mask", "@mask", "--masksign", "bogus"], {}),
        (["--seg", "@seg", *o, "--mask", "@mask", "--maskinvert", "--maskerode", "1"], {}),
        (["--seg", "@seg", *o, "--mask", "@mask2", "--maskframe", "1", "--maskinvert"], {}),
        (["--seg", "@seg", *o, "--mask", "@mask", "--maskframe", "1"], {}),
        (["--seg", "@seg", *o, "--mask", "@mask", "--maskthresh", "2"], {}),
        (["--seg", "@seg", *o, "--mask", "@segb"], {}),
        (["--seg", "@seg", *o, "--pv", "@u8", "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--pv", "@u8", "--no-segborder", "--ctab", "@lut"], {}),
        (["--seg", "@seg", *o, "--pv", "@f32", "--segborder", "--id", "2", "17"], {}),
        (["--seg", "@seg", *o, "--pv", "@segb"], {}),
        (["--seg", "@seg", *o, "--in", "@segb"], {}),
        (["--seg", "@seg", *o, "--in", "@f3", "--frame", "2"], {}),
        (["--seg", "@seg", *o, "--in", "@f3", "--frame", "3"], {}),
        (["--seg", "@seg", *o, "--in", "@u8", "--robust", "10"], {}),
        (["--seg", "@seg", *o, "--in", "@f32", "--robust", "x"], {}),
        (["--seg", "@seg", *o, "--in", "@u8", "--mul", "2", "--div", "8"], {}),
        (["--seg", "@seg", *o, "--in", "@u8", "--mul", "x"], {}),
        (["--seg", "@seg", *o, "--in", "@f32", "--abs"], {}),
        (["--seg", "@seg", *o, "--in", "@f32", "--sqr"], {}),
        (["--seg", "@seg", *o, "--in", "@f32", "--sqrt"], {}),
        (["--seg", "@seg", *o, "--in", "@u8", "--acc"], {}),
        (["--seg", "@seg", "--in", "@f3", "--avgwf", "wf.dat"], {}),
        (["--seg", "@seg", "--in", "@f3", "--sumwf", "wf.dat", *o], {}),
        (["--seg", "@seg", "--in", "@f3", "--sfavg", "sf.dat", "--avgwfvol", "wf.mgz"], {}),
        (["--seg", "@seg", "--in", "@f3", "--avgwf", "wf.dat", "--avgwf-remove-mean"], {}),
        (["--seg", "@seg", "--in", "@f3", "--avgwf", "wf.dat", "--avgwf-norm-mean", "100",
          "--id", "2", "41"], {}),
        (["--seg", "@seg", "--avgwf", "wf.dat"], {}),
        (["--seg", "@seg", "--in", "@f3", "--sfavg", "nodir/sf.dat", *o], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut", "--ctab-out", "out.ctab"], {}),
        (["--seg", "@seg", *o, "--ctab-out", "out.ctab"], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut", "--ctab-out", "@lut"], {}),
        (["--seg", "@seg", *o, "--ctab", "@lut", "--ctab-out", "nodir/out.ctab"], {}),
        (["--seg", "@seg", *o, "--seg-erode", "1", "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--vox", "10", "12", "14", "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--vox", "10", "12"], {}),
        (["--seg", "@seg", *o, "--vox"], {}),
        (["--seg", "@seg", *o, "--replace", "2", "41", "--replace", "3", "42"], {}),
        (["--seg", "@seg", *o, "--replace", "2"], {}),
        (["--seg", "@seg", *o, "--replace-file", "@rep", "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--replace", "4", "43", "--replace-file", "@rep"], {}),
        (["--seg", "@seg", *o, "--replace", "4", "43", "--replace-file", "@repempty"], {}),
        (["--seg", "@seg", *o, "--replace-file", "@repbad"], {}),
        (["--seg", "@seg", *o, "--replace-file", "@repempty"], {}),
        (["--seg", "@seg", *o, "--replace-file", "nofile.txt"], {}),
        (["--seg", "@seg", *o, "--gtm-default-seg-merge"], {}),
        (["--seg", "@seg", *o, "--gtm-default-seg-merge-choroid", "--empty", "--id", "62", "80",
          "24"], {}),
        (["--seg", "@seg", *o, "--gtm-default-seg-merge-choroid", "--replace-file", "@rep"], {}),
        (["--seg-from-input", "--in", "@u8", *o], {}),
        (["--seg-from-input", *o], {}),
        (["--seg-from-input", "--in", "nofile.mgz", *o], {}),
        (["--seg", "@seg", *o, "--dontrun", "--in", "@u8"], {}),
        (["--seg", "@seg", *o, "--no-newprint", "--in", "@u8", "--snr", "--ctab", "@lut"], {}),
        (["--seg", "@seg", *o, "--no-newprint", "--newprint"], {}),
        (["--seg", "@seg", *o, "--debug", "--in", "@u8", "--snr"], {}),
        (["--seg", "@seg", *o, "--subject", "subj", "--etiv"], sd),
        (["--seg", "@seg", *o, "--etiv"], sd),
        (["--etiv-only", "--subject", "subj"], sd),
        (["--old-etiv-only", "--subject", "subj"], sd),
        (["--etiv-only", "--talxfm", "@xfm"], {}),
        (["--seg", "@seg", *o, "--subject", "subj"], nosd),
        (["--sd", "@sd", "--seg", "@seg", *o, "--subject", "subj", "--etiv"], nosd),
        (["--seg", "@seg", *o, "--subject", "subj", "--brain-vol-from-seg", "--etiv",
          "--brainmask", "@u8", "--subcortgray", "--in", "@u8"], sd),
        (["--seg", "@seg", *o, "--subject", "subj", "--surf-wm-vol", "--surf-ctx-vol",
          "--totalgray", "--supratent"], sd),
        (["--seg", "@seg", *o, "--subject", "subj", "--supratent"], sd),
        (["--seg", "@seg", *o, "--subject", "subj", "--totalgray"], sd),
        (["--seg", "@seg", *o, "--surf-ctx-vol"], sd),
        (["--seg", "@seg", *o, "--subject", "bare", "--surf-wm-vol"], sd),
        (["--seg", "@seg", *o, "--subject", "subj", "--surf-wm-vol", "--no-global-stats"], sd),
        (["--seg", "@seg", *o, "--subject", "subj", "--euler"], sd),
        (["--seg", "@seg", *o, "--euler"], sd),
        (["--seg", "@seg", *o, "--subject", "nobody", "--euler"], sd),
        (["--seg", "@seg", *o, "--stiv", "1234.5"], {}),
        (["--seg", "@seg", *o, "--stiv", "@tiv"], {}),
        (["--seg", "@seg", *o, "--stiv"], {}),
        (["--annot", "subj", "lh", "aparc", *o], sd),
        (["--annot", "subj", "lh", "aparc", *o, "--in", "@meas", "--snr"], sd),
        (["--annot", "subj", "rh", "aparc", *o, "--segbase", "100", "--surf", "orig.nofix"], sd),
        (["--annot", "subj", "lh", "@annot", *o], sd),
        (["--annot", "subj", "lh", "nothere", *o], sd),
        (["--annot", "subj", "lh"], sd),
        (["--slabel", "subj", "lh", "@label", *o], sd),
        (["--slabel", "subj", "lh", "@label", *o, "--label-thresh", "0.4", "--in", "@meas"], sd),
        (["--slabel", "subj", "lh"], sd),
        (["--sum-in", "@sumin"], {}),
        (["--sum-in", "nofile.stats"], {}),
        (["--xfm2etiv", "@xfm", "nofile"], {}),
        (["--xfm2etiv", "@xfm", "etiv.txt"], {}),
        (["--xfm2etiv", "@xfm"], {}),
        (["--rescale-by-seg", "@u8", "@seg", "100", "rescaled.mgz", "2", "41"], {}),
        (["--rescale-by-seg", "@f3", "@seg", "50", "rescaled.mgz", "3"], {}),
        (["--rescale-by-seg", "@u8", "@seg", "100", "rescaled.mgz", "999"], {}),
        (["--rescale-by-seg", "@u8", "@seg", "100", "rescaled.mgz", "--seg", "@seg"], {}),
        (["--rescale-by-seg", "@u8", "@seg", "100"], {}),
        (["--qa-stats", "subj", "qa.txt"], sd),
        (["--qa-stats", "bare", "qa.txt"], sd),
        (["--qa-stats", "subj"], sd),
        (["--seg", "@seg", *o, "--in", "@u8", "--reg", "@regdat"], {}),
        (["--seg", "@seg", *o, "--in", "@u8", "--reg", "@reglta"], {}),
        (["--seg", "@seg", *o, "--in", "@u8", "--regheader"], {}),
        (["--seg", "@seg", *o, "--in", "@segb", "--regheader"], {}),
        (["--seg", "@seg", *o, "--in", "@u8", "--reg", "nofile.dat"], {}),
        (["--seg", "@seg", *o, "--ctab-default"], {}),
        (["--seg", "@seg", *o, "--ctab-gca", gca], {}),
        (["--seg", "@seg", *o, "--ctab-gca", "nofile.gca"], {}),
        (["--seg", "@seg", *o, "--ctab-gca", gca, "--ctab", "@lut"], {}),
        (["--seg", "@seg", *o, "--seed", "5", "--synth", "3"], {}),
        (["--seg", "@seg", "--sum", "nodir/out.stats"], {}),
        (["--seg", "@seg", "--in", "@f3", "--avgwf", "nodir/wf.dat"], {}),
        (["--seg", "nofile.mgz", *o], {}),
        (["--seg", "@seg", *o, "--in", "nofile.mgz"], {}),
        (["--seg", "@seg", *o, "--ctab", "nofile.txt"], {}),
    ]
    # fmt: on
    return [([str(f[a[1:]]) if a.startswith("@") else a for a in x], e) for x, e in c]


def mask(text: str, port: bool, folder: Path) -> str:
    # a relative name the program makes absolute names the side's own folder
    text = STAMP.sub("TimeStamp: <time>", text).replace(str(folder), "<folder>")
    if port:
        text = "".join(x for x in text.splitlines(True) if not x.startswith("larmorx: "))
    return text


def notices(text: str) -> list[str]:
    return sorted(
        {m.group(1) for m in re.finditer(r"^larmorx: upstream defect (\S+) ", text, re.M)}
    )


def written(folder: Path) -> dict[str, Path]:
    return {str(p.relative_to(folder)): p for p in sorted(folder.rglob("*")) if p.is_file()}


def official() -> Path:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    return ws / FS_HOME / "bin" / "mri_segstats"


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
    work = ws / "oracles" / "inputs" / "fs-segstats-prog" / f"work-{tag}-{seed}"
    ctx = fsfiles.Context(any_directory=True)
    # the subjects' files the subject options read without naming them (the cache keys them)
    subject_files = sorted(p for p in f["sd"].rglob("*") if p.is_file())
    rows = []
    verdicts: Counter = Counter()
    shown = 0
    for i, (args, extra) in enumerate(all_cases):
        if i not in keep:
            continue
        sides = {}
        for side in ("official", *modes):
            d = work / f"{i:03d}" / side
            t = work / f"{i:03d}" / f"{side}-tmp"
            for x in (d, t):
                if x.exists():
                    shutil.rmtree(x)
                x.mkdir(parents=True)
            # argv[0] as the scripts give it (the program's messages print it): the name on the PATH
            env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
                   "OMP_NUM_THREADS": "1", "HOME": str(work), "USER": "probe",
                   "TMPDIR": str(t)}  # fmt: skip
            env.update(freesurfer_env())
            for k, v in extra.items():
                if v is None:
                    env.pop(k, None)
                else:
                    env[k] = v
            if side == "official":
                done = oracle_cache.run(
                    ["mri_segstats", *args],
                    env=env,
                    cwd=d,
                    outputs=[d],
                    policy=POLICY,
                    timeout=1800,
                    runner="fs-segstats-prog",
                    inputs=subject_files,
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
        row = {"case": i, "args": args, "env": extra, "official_exit": oc}
        for side in modes:
            pc, po, pe, pd = sides[side]
            diffs = []
            if pc != oc:
                diffs.append(f"exit {pc} != {oc}")
            if mask(po, True, pd) != mask(oo, False, od):
                diffs.append("stdout")
            if mask(pe, True, pd) != mask(oe, False, od):
                diffs.append("stderr")
            fo, fp = written(od), written(pd)
            if sorted(fo) != sorted(fp):
                diffs.append(f"files {sorted(fo)} != {sorted(fp)}")
            for name in sorted(set(fo) & set(fp)):
                cmp = fsfiles.compare(fo[name], fp[name], ctx, name=name)
                if not cmp.same:
                    diffs.append(f"{name}: {cmp.verdict} {cmp.detail[:200]}")
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
