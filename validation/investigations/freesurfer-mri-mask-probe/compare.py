# SPDX-License-Identifier: Apache-2.0
"""mri_mask as a black box: FreeSurfer 8.2.0's official program against a candidate, case by case.

An open probe (``README.md``): it writes seeded synthetic inputs (coronal MGZ volumes of 32^3
voxels of 2 mm in ``float``, ``uchar``, ``short`` and ``int``, three frames, NaNs and infinities,
a NIfTI copy; masks of values 0 to 10, binary, negative, with NaNs, empty, full, of 0/1/2, shifted
by a millimetre, of another grid; segmentations, one with a colour table; volumes whose
``-BBEQ`` box takes each of its ways or none; odd and one-voxel volumes; linear transform arrays
with and without their volumes' geometry, inverted, mismatched, of two transforms that chain and
that do not, singular for the volumes' grid in single precision; an FSL matrix, an MNI transform,
a ``register.dat``; a text file) into ``<workspace>/oracles/inputs/fs-mask-prog/<seed>/``, runs
each case's command line through the official program (``$FREESURFER_HOME/bin/mri_mask``, found on
the ``PATH`` by its name as recon-all runs it) by the oracle-output cache at one thread, and
through the candidate (``--candidate``, a command line whose ``{mode}`` becomes ``reproduce`` and
then ``fix``), each in a folder of its own (outputs are written there). Compared: the exit status,
standard output and error (``--all-info``'s time, the case's folder and the candidate's
``larmorx:`` notices aside) and every file written (FreeSurfer's provenance aside:
``larmorx_validation.freesurfer.files``). ``reproduce`` must give FreeSurfer's on every case;
``fix`` may differ only with a notice.

    python -P validation/investigations/freesurfer-mri-mask-probe/compare.py --seed 1 \\
        --candidate '<program> --upstream-defects {mode} freesurfer mri_mask'
"""

from __future__ import annotations

import argparse
import gzip
import json
import re
import shutil
import struct
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

#: The defects (``larmorx_core::defects``) whose cases are counted, not compared: a transform file
#: FreeSurfer crashes on, or never returns from, where both modes end with an error (FS-307,
#: FS-309, FS-310: a text file named as an ``.lta``).
NOT_COMPARABLE = frozenset({"FS-307", "FS-309", "FS-310"})
#: FreeSurfer's runs compared with their entries: ``--all-info``'s report holds the run's time.
POLICY = oracle_cache.Policy(
    threads=1,
    family="freesurfer",
    formats="freesurfer",
    volatile=(r"TimeStamp: \S+",),
)
STAMP = re.compile(r"TimeStamp: \S+")
N = 32


def lia(n: int = N, size: float = 2.0, shift: float = 0.0) -> np.ndarray:
    """A coronal (LIA) grid of ``n^3`` voxels of ``size`` mm centred on the origin (moved by
    ``shift`` mm along x)."""
    h = n / 2 * size
    return np.array([[-size, 0, 0, h + shift], [0, 0, size, -h], [0, -size, 0, h], [0, 0, 0, 1.0]])


def save(path: Path, data: np.ndarray, affine: np.ndarray) -> Path:
    nib.save(nib.MGHImage(data, affine), str(path))
    return path


def save_with_ctab(path: Path, data: np.ndarray, affine: np.ndarray, labels: dict) -> Path:
    """An ``.mgz`` holding a colour table (the old colour-table tag, its second binary version:
    -2, the entry count, the file name, the number of entries stored, each entry's structure
    number, name and ``r g b 255-a``; big-endian) after the scan parameters."""
    raw = path.with_suffix(".mgh")
    nib.save(nib.MGHImage(data, affine), str(raw))
    body = raw.read_bytes()
    raw.unlink()

    def text(s: str) -> bytes:
        b = s.encode() + b"\0"
        return struct.pack(">i", len(b)) + b

    tab = struct.pack(">ii", 1, -2) + struct.pack(">i", max(labels) + 1) + text("probe.ctab")
    tab += struct.pack(">i", len(labels))
    for k, (name, rgb) in sorted(labels.items()):
        tab += struct.pack(">i", k) + text(name) + struct.pack(">iiii", *rgb, 0)
    with gzip.open(path, "wb") as f:
        f.write(body + tab)
    return path


def geometry(img_path: Path, name: str | None = None) -> str:
    """A transform file's volume information for the volume in ``img_path`` (valid)."""
    img = nib.load(str(img_path))
    shape = img.shape[:3]
    vs = img.header.get_zooms()[:3]
    m = img.affine[:3, :3] / np.array(vs)
    c = img.affine @ np.array([shape[0] / 2, shape[1] / 2, shape[2] / 2, 1.0])

    def e(v: float) -> str:
        return f"{v:.15e}"

    return (
        f"valid = 1  # volume info valid\nfilename = {name or img_path.name}\n"
        f"volume = {shape[0]} {shape[1]} {shape[2]}\n"
        f"voxelsize = {e(vs[0])} {e(vs[1])} {e(vs[2])}\n"
        f"xras   = {e(m[0, 0])} {e(m[1, 0])} {e(m[2, 0])}\n"
        f"yras   = {e(m[0, 1])} {e(m[1, 1])} {e(m[2, 1])}\n"
        f"zras   = {e(m[0, 2])} {e(m[1, 2])} {e(m[2, 2])}\n"
        f"cras   = {e(c[0])} {e(c[1])} {e(c[2])}\n"
    )


NOGEO = "valid = 0  # volume info invalid\n"


def lta(path: Path, ltype: int, mats: list, geos: list[tuple[str, str]]) -> Path:
    """A linear transform array of type ``ltype`` (0 voxel to voxel, 1 scanner to scanner): every
    matrix first, then each transform's source and destination information (as FreeSurfer
    writes several)."""
    name = "LINEAR_VOX_TO_VOX" if ltype == 0 else "LINEAR_RAS_TO_RAS"
    text = f"type      = {ltype} # {name}\nnxforms   = {len(mats)}\n"
    for m in mats:
        rows = "\n".join(" ".join(f"{float(v):.9e}" for v in r) for r in np.asarray(m))
        text += f"mean      = 0.0000 0.0000 0.0000\nsigma     = 1.0000\n1 4 4\n{rows}\n"
    for src, dst in geos:
        text += f"src volume info\n{src}dst volume info\n{dst}"
    path.write_text(text, encoding="utf-8")
    return path


#: Two matrices whose voxel-to-voxel form for the oblique 8^3 grid of ``inputs`` is singular in
#: single precision although they invert (found by a search; FS-1135): one voxel to voxel, one
#: scanner to scanner.
SINGULAR_V2V = [
    [9.556695819e-01, 6.027768254e-01, 1.558446407e00, 1.047930121e00],
    [-1.331979632e00, -1.767718554e00, -3.099698305e00, -8.762617111e-01],
    [6.139296293e-01, 3.470301032e-01, 9.609597325e-01, 5.985966325e-01],
    [0, 0, 0, 1],
]
SINGULAR_RAS = [
    [4.437906444e-01, 5.446658134e-01, 9.884564281e-01, 3.640021682e-01],
    [-1.427637935e00, 1.462778568e00, 3.514070809e-02, 7.867556810e-01],
    [2.226209641e-01, -5.013926029e-01, -2.787716687e-01, 8.877945542e-01],
    [0, 0, 0, 1],
]


def inputs(ws: Path, seed: int) -> dict[str, Path]:
    d = ws / "oracles" / "inputs" / "fs-mask-prog" / str(seed)
    d.mkdir(parents=True, exist_ok=True)
    rng = np.random.default_rng(seed)
    a = lia()
    s = (N, N, N)
    f: dict[str, Path] = {}
    v = rng.normal(80.0, 30.0, s)
    v[rng.random(s) < 0.05] = 0.0
    f["vf"] = save(d / "vf.mgz", v.astype(np.float32), a)
    f["vu"] = save(d / "vu.mgz", np.clip(v, 0, 255).astype(np.uint8), a)
    f["vs"] = save(d / "vs.mgz", rng.integers(-100, 400, s).astype(np.int16), a)
    f["vi"] = save(d / "vi.mgz", rng.integers(-500, 3000, s).astype(np.int32), a)
    f["v3"] = save(d / "v3.mgz", rng.normal(60.0, 20.0, (*s, 3)).astype(np.float32), a)
    vn = v.astype(np.float32).copy().reshape(-1)
    k = rng.choice(vn.size, 60, replace=False)
    vn[k[:20]], vn[k[20:40]], vn[k[40:]] = np.nan, np.inf, -np.inf
    f["vnan"] = save(d / "vnan.mgz", vn.reshape(s), a)
    nib.save(nib.Nifti1Image(v.astype(np.float32), a), str(d / "vf.nii.gz"))
    f["vnii"] = d / "vf.nii.gz"
    m = np.zeros(s, np.float32)
    m[8:24, 6:26, 10:22] = rng.uniform(0, 10, (16, 20, 12))
    f["mf"] = save(d / "mf.mgz", m, a)
    f["mu"] = save(d / "mu.mgz", (m > 3).astype(np.uint8), a)
    mn = np.zeros(s, np.float32)
    mn[5:27, 5:27, 5:27] = rng.uniform(-10, 10, (22, 22, 22))
    f["mneg"] = save(d / "mneg.mgz", mn, a)
    mnan = m.copy().reshape(-1)
    k = rng.choice(mnan.size, 90, replace=False)
    mnan[k[:30]], mnan[k[30:60]], mnan[k[60:]] = np.nan, np.inf, -np.inf
    f["mnan"] = save(d / "mnan.mgz", mnan.reshape(s), a)
    f["mzero"] = save(d / "mzero.mgz", np.zeros(s, np.float32), a)
    f["mones"] = save(d / "mones.mgz", np.ones(s, np.uint8), a)
    m012 = np.zeros(s, np.uint8)
    m012[6:26, 6:26, 6:26] = rng.integers(0, 3, (20, 20, 20))
    f["m012"] = save(d / "m012.mgz", m012, a)
    f["mshift"] = save(d / "mshift.mgz", m, lia(shift=1.0))
    big = np.zeros((40, 40, 40), np.float32)
    big[10:30, 8:32, 12:28] = rng.uniform(0, 10, (20, 24, 16))
    f["mgrid"] = save(d / "mgrid.mgz", big, lia(40, 1.6))
    labels = np.array([0, 2, 3, 4, 41, 42, 43, 7, 8, 46, 47, 24, 165, 130, 257, 258, 10, 49, 17,
                       53, 77, 251, 16, 85])  # fmt: skip
    seg = labels[rng.integers(0, len(labels), s)].astype(np.int32)
    f["aseg"] = save(d / "aseg.mgz", seg, a)
    names = {0: ("Unknown", (0, 0, 0)), 2: ("Left-Cerebral-White-Matter", (245, 245, 245)),
             3: ("Left-Cerebral-Cortex", (205, 62, 78)), 41: ("Right-Cerebral-White-Matter",
             (0, 225, 0)), 42: ("Right-Cerebral-Cortex", (205, 62, 78))}  # fmt: skip
    small = np.array([0, 2, 3, 41, 42])[rng.integers(0, 5, s)].astype(np.int32)
    f["asegct"] = save_with_ctab(d / "asegct.mgz", small, a, names)
    # -BBEQ's ways: a box square with odd slices (as it is), even slices (one added), even slices
    # over the whole depth from slice 1 (one taken before), columns fewer than rows (moved left
    # by half; asymmetric at an edge), rows fewer than columns (moved up by half; at an edge)
    for name, box in [
        ("bq_same", (slice(8, 20), slice(8, 20), slice(4, 15))),
        ("bq_even", (slice(8, 20), slice(8, 20), slice(4, 16))),
        ("bq_full", (slice(8, 20), slice(8, 20), slice(1, 32))),
        ("bq_dxlt", (slice(12, 18), slice(6, 26), slice(4, 15))),
        ("bq_dxedge", (slice(0, 6), slice(2, 30), slice(4, 15))),
        ("bq_dylt", (slice(6, 26), slice(12, 18), slice(4, 15))),
        ("bq_dyedge", (slice(2, 30), slice(26, 32), slice(4, 15))),
        ("bq_fits", (slice(4, 10), slice(4, 18), slice(4, 15))),
    ]:
        b = np.zeros(s, np.uint8)
        b[box] = 1
        f[name] = save(d / f"{name}.mgz", b, a)
    # -BBEQ without a box (FS-1133): columns more than rows, rows more than columns, an even
    # count of slices over the whole depth from the first
    for name, shape in [("nh", (32, 8, 9)), ("nw", (8, 32, 9)), ("nz", (16, 16, 32))]:
        f[name + "_in"] = save(d / f"{name}_in.mgz", rng.normal(80, 30, shape).astype(np.float32),
                               lia(shape[0]) if shape[0] == shape[1] == shape[2] else
                               np.array([[-2.0, 0, 0, shape[0]], [0, 0, 2.0, -shape[2]],
                                         [0, -2.0, 0, shape[1]], [0, 0, 0, 1]]))  # fmt: skip
        f[name + "_m"] = save(d / f"{name}_m.mgz", np.ones(shape, np.uint8),
                              nib.load(str(f[name + "_in"])).affine)  # fmt: skip
    f["vodd"] = save(d / "vodd.mgz", rng.normal(90, 25, (9, 7, 5)).astype(np.float32),
                     lia(9, 3.0))  # fmt: skip
    mo = np.zeros((9, 7, 5), np.uint8)
    mo[2:7, 1:6, 1:4] = 1
    f["modd"] = save(d / "modd.mgz", mo, nib.load(str(f["vodd"])).affine)
    one = np.array([[-1.0, 0, 0, 0], [0, 0, 1.0, 0], [0, -1.0, 0, 0], [0, 0, 0, 1.0]])
    f["vtiny"] = save(d / "vtiny.mgz", np.full((1, 1, 1), 42.0, np.float32), one)
    f["mtiny"] = save(d / "mtiny.mgz", np.ones((1, 1, 1), np.uint8), one)
    # the oblique grid of the singular transforms (FS-1135)
    vs8 = np.array([1.1, 0.7, 1.3])
    th, ph = 0.3, 0.2
    r = np.array([[np.cos(th), -np.sin(th), 0], [np.sin(th), np.cos(th), 0], [0, 0, 1]])
    r = r @ np.array([[1, 0, 0], [0, np.cos(ph), -np.sin(ph)], [0, np.sin(ph), np.cos(ph)]])
    ob = np.eye(4)
    ob[:3, :3] = r * vs8
    ob[:3, 3] = [3.1, -7.3, 11.9]
    g = np.random.default_rng(7)
    f["in8"] = save(d / "in8.mgz", g.uniform(0, 4, (8, 8, 8)).astype(np.float32), ob)
    f["m8"] = save(d / "m8.mgz", g.uniform(0, 4, (8, 8, 8)).astype(np.float32), ob)
    gm, gv = geometry(f["mf"]), geometry(f["vf"])
    shift = np.eye(4)
    shift[0, 3] = 1.0
    f["lv2v"] = lta(d / "v2v.lta", 0, [shift], [(gm, gv)])
    rot = np.eye(4)
    rot[:3, :3] = [[0.99, -0.05, 0.0], [0.05, 0.99, 0.02], [0.0, -0.02, 1.0]]
    rot[:3, 3] = [2.0, -1.5, 0.5]
    f["lras"] = lta(d / "ras.lta", 1, [rot], [(gm, gv)])
    f["linv"] = lta(d / "inv.lta", 1, [rot], [(gv, geometry(f["mshift"]))])
    other = geometry(f["mshift"], "other.mgz").replace("cras   = ", "cras   = 5", 1)
    f["lmis"] = lta(d / "mismatch.lta", 0, [np.eye(4)], [(other, other)])
    f["lnogeo"] = lta(d / "nogeo.lta", 1, [rot], [(NOGEO, NOGEO)])
    f["lnosrc"] = lta(d / "nosrc.lta", 1, [rot], [(NOGEO, gv)])
    g2 = geometry(f["mshift"], "mid.mgz")
    f["lchain"] = lta(d / "chain.lta", 1, [rot, shift], [(gm, g2), (g2, gv)])
    f["lchainv"] = lta(d / "chaininv.lta", 1, [rot, shift], [(gm, g2), (gv, g2)])
    f["lnochain"] = lta(d / "nochain.lta", 0, [np.eye(4), np.eye(4)], [(gm, gv), (other, other)])
    g8m, g8v = geometry(f["m8"]), geometry(f["in8"])
    f["lsingv"] = lta(d / "singv.lta", 0, [SINGULAR_V2V], [(g8m, g8v)])
    f["lsingr"] = lta(d / "singr.lta", 1, [SINGULAR_RAS], [(g8m, g8v)])
    zero = np.zeros((4, 4))
    zero[3, 3] = 1.0
    f["lzero"] = lta(d / "zero.lta", 0, [zero], [(gm, gv)])
    (d / "shift.fslmat").write_text("1 0 0 2\n0 1 0 0\n0 0 1 0\n0 0 0 1\n", encoding="utf-8")
    f["fsl"] = d / "shift.fslmat"
    (d / "tal.xfm").write_text(
        "MNI Transform File\n%probe\n\nTransform_Type = Linear;\nLinear_Transform =\n"
        "1 0 0 2\n0 1 0 0\n0 0 1 0 ;\n",
        encoding="utf-8",
    )
    f["xfm"] = d / "tal.xfm"
    (d / "register.dat").write_text(
        "probe\n2.0\n2.0\n0.15\n1 0 0 2\n0 1 0 0\n0 0 1 0\n0 0 0 1\nround\n", encoding="utf-8"
    )
    f["dat"] = d / "register.dat"
    (d / "notes.txt").write_text("not a transform\n", encoding="utf-8")
    f["txt"] = d / "notes.txt"
    (d / "notes.lta").write_text("not a transform\n", encoding="utf-8")
    f["txtlta"] = d / "notes.lta"
    (d / "folder").mkdir(exist_ok=True)
    f["dir"] = d / "folder"
    return f


def cases(f: dict[str, Path]) -> list[list[str]]:
    """The command lines (inputs by absolute name; outputs are relative)."""
    s = {k: str(v) for k, v in f.items()}
    vf, mf, mu, o = s["vf"], s["mf"], s["mu"], "o.mgz"
    base = [vf, mf, o]
    c: list[list[str]] = [
        [],
        [vf],
        [vf, mf],
        [*base, "extra"],
        ["--help"],
        ["--usage"],
        ["-help"],
        ["-USAGE"],
        ["--version"],
        ["-version"],
        ["--all-info"],
        ["-u"],
        ["-x", *base],
        ["-", *base],
        # every option given last, or before too few arguments (FS-1132)
        *[
            [opt]
            for opt in [
                "-dilate",
                "-erode",
                "-T",
                "-threshold",
                "-crop-to-fov",
                "-crop-to-fov-mm",
                "-BB",
                "-crop",
                "-boundingbox",
                "-crop-crs",
                "-BBM",
                "-boundingboxm",
                "-BBMM",
                "-boundingboxmm",
                "-BBEQ",
                "-transfer",
                "-oval",
                "-lta_src",
                "-src",
                "-lta_dst",
                "-dst",
                "-DEBUG_VOXEL",
                "-bgnoise",
            ]
        ],
        ["-crop-to-fov", "4", "4"],
        ["-bgnoise", "1"],
        ["-xform"],
        ["-xform", vf, mf],
        # recon-all's calls and the plain mask
        base,
        ["-T", "5", *base],
        [s["vu"], mu, o],
        [s["vs"], mf, o],
        [s["vi"], mf, o],
        [s["v3"], mf, o],
        [s["vnan"], mf, o],
        [s["vnii"], mf, o],
        [vf, s["vnii"], o],
        [vf, s["mnan"], o],
        ["-T", "nan", vf, s["mnan"], o],
        ["-T", "2", vf, s["mnan"], o],
        [vf, s["mzero"], o],
        [vf, s["mones"], o],
        # the threshold, the inversion, the absolute value
        ["-T", "0", *base],
        ["-T", "-1", *base],
        ["-threshold", "7.5", *base],
        ["-T", "1e40", *base],
        ["-T", "abc", *base],
        ["-invert", *base],
        ["-invert", "-T", "4", *base],
        ["-T", "4", "-invert", *base],
        ["-invert", "-no-invert", *base],
        ["-abs", vf, s["mneg"], o],
        ["-abs", "-T", "3", vf, s["mneg"], o],
        ["-T", "3", vf, s["mneg"], o],
        ["-abs", "-invert", vf, s["mneg"], o],
        # segmentations
        ["-samseg", vf, s["aseg"], o],
        ["-lh", vf, s["aseg"], o],
        ["-rh", vf, s["aseg"], o],
        ["-lh", "-rh", vf, s["aseg"], o],
        ["-lh", "-no_cerebellum", vf, s["aseg"], o],
        ["-rh", "-no_cerebellum", "-dilate", "1", vf, s["aseg"], o],
        ["-LH", vf, s["aseg"], o],
        [s["asegct"], mu, o],
        ["-T", "2", s["asegct"], s["asegct"], o],
        # dilation and erosion
        ["-dilate", "1", *base],
        ["-dilate", "2", vf, mu, o],
        ["-dilate", "0", *base],
        ["-dilate", "-1", *base],
        ["-erode", "1", vf, mu, o],
        ["-erode", "2", *base],
        ["-dilate", "1", "-erode", "1", vf, mu, o],
        ["-T", "3", "-dilate", "1", *base],
        # the crops
        ["-BB", "2", *base],
        ["-crop", "0", vf, mu, o],
        ["-boundingbox", "3", *base],
        ["-BB", "40", *base],
        ["-BB", "-3", *base],
        ["-crop-crs", "1", "2", "3", *base],
        ["-BBM", "1", "2", "3", *base],
        ["-boundingboxm", "0", "0", "0", vf, mu, o],
        ["-BBMM", "1", "2", "3", "4", "5", "6", *base],
        ["-boundingboxmm", "2", "0", "2", "0", "2", "0", vf, mu, o],
        ["-BB", "1", vf, s["mzero"], o],
        *[
            ["-BBEQ", "1", vf, s[k], o]
            for k in [
                "bq_same",
                "bq_even",
                "bq_full",
                "bq_dxlt",
                "bq_dxedge",
                "bq_dylt",
                "bq_dyedge",
                "bq_fits",
            ]
        ],
        ["-BBEQ", "0", *base],
        ["-BBEQ", "2", vf, s["mzero"], o],
        ["-BBEQ", "1", s["nh_in"], s["nh_m"], o],
        ["-BBEQ", "1", s["nw_in"], s["nw_m"], o],
        ["-BBEQ", "1", s["nz_in"], s["nz_m"], o],
        ["-BBEQ", "1", "-crop-to-fov", "8", "8", "8", s["nz_in"], s["nz_m"], o],
        ["-BB", "1", "-BBEQ", "1", *base],
        ["-crop-to-fov", "16", "16", "16", *base],
        ["-crop-to-fov", "20", "18", "10", "-T", "2", *base],
        ["-crop-to-fov", "40", "40", "40", *base],
        ["-crop-to-fov-mm", "30", "30", "20", *base],
        ["-crop-to-fov-mm", "31", "29.5", "7", "-T", "5", *base],
        ["-BB", "2", "-crop-to-fov", "10", "10", "10", *base],
        # the values
        ["-transfer", "2", vf, s["m012"], o],
        ["-transfer", "3.7", *base],
        ["-transfer", "-1", vf, s["m012"], o],
        ["-oval", "5", *base],
        ["-oval", "nan", *base],
        ["-oval", "-1e3", s["vu"], mu, o],
        ["-oval", "300", s["vu"], mu, o],
        ["-T", "nan", "-oval", "nan", vf, s["mnan"], o],
        ["-keep_mask_deletion_edits", vf, s["m012"], o],
        ["-keep_mask_deletion_edits", "-transfer", "2", vf, s["m012"], o],
        ["-bgnoise", "10", "0", *base],
        ["-bgnoise", "10", "1", *base],
        ["-bgnoise", "7.5", "2", vf, mu, o],
        ["-bgnoise", "0", "1", *base],
        ["-bgnoise", "x", "y", *base],
        ["-DEBUG_VOXEL", "1", "2", "3", *base],
        ["-DEBUG_VOXEL", "10", "10", "10", "-dilate", "1", *base],
        # geometry
        [vf, s["mshift"], o],
        ["-no-allow-diff-geom", vf, s["mshift"], o],
        [vf, s["mgrid"], o],
        ["-no-allow-diff-geom", vf, s["mgrid"], o],
        [s["vodd"], s["modd"], o],
        ["-BBEQ", "0", s["vodd"], s["modd"], o],
        [s["vtiny"], s["mtiny"], o],
        ["-dilate", "1", "-BB", "0", s["vtiny"], s["mtiny"], o],
        # transforms
        ["-xform", s["lv2v"], *base],
        ["-xform", s["lras"], *base],
        ["-xform", s["lras"], "-T", "4", "-dilate", "1", *base],
        ["-xform", s["linv"], vf, s["mshift"], o],
        ["-xform", s["lmis"], *base],
        ["-xform", s["lnogeo"], *base],
        ["-xform", s["lnogeo"], "-lta_src", mf, *base],
        ["-xform", s["lnogeo"], "-lta_src", mf, "-lta_dst", vf, *base],
        ["-xform", s["lnogeo"], "-src", mf, "-dst", vf, *base],
        ["-xform", s["lnosrc"], "-lta_src", mf, *base],
        ["-xform", s["lnogeo"], "-lta_src", "nofile.mgz", *base],
        ["-xform", s["lnogeo"], "-lta_src", s["txt"], *base],
        ["-xform", s["lchain"], *base],
        ["-xform", s["lchainv"], *base],
        ["-xform", s["lnochain"], *base],
        ["-xform", s["lnochain"], "-T", "3", *base],
        ["-xform", s["lzero"], *base],
        ["-xform", s["lsingv"], s["in8"], s["m8"], o],
        ["-xform", s["lsingr"], s["in8"], s["m8"], o],
        *[
            ["-xform", s["lsingv"], *opt, s["in8"], s["m8"], o]
            for opt in [
                ["-T", "1"],
                ["-invert"],
                ["-samseg"],
                ["-lh"],
                ["-BB", "1"],
                ["-BBEQ", "1"],
                ["-crop-to-fov", "4", "4", "4"],
                ["-crop-to-fov-mm", "4", "4", "4"],
                ["-dilate", "1"],
                ["-erode", "2"],
                ["-transfer", "2"],
                ["-keep_mask_deletion_edits", "-bgnoise", "1", "0", "-oval", "3"],
            ]
        ],
        ["-xform", s["fsl"], *base],
        ["-xform", s["fsl"], "-lta_src", mf, "-lta_dst", vf, *base],
        ["-xform", s["xfm"], *base],
        ["-xform", s["xfm"], "-lta_src", mf, "-lta_dst", vf, *base],
        ["-xform", s["dat"], "-lta_src", mf, "-lta_dst", vf, *base],
        ["-xform", "identity.nofile", *base],
        ["-xform", "nofile.lta", *base],
        ["-xform", s["txtlta"], *base],
        ["-xform", s["vf"], *base],
        # the input, the mask and the output
        ["nofile.mgz", mf, o],
        [vf, "nofile.mgz", o],
        [s["txt"], mf, o],
        [s["dir"], mf, o],
        [*base[:2], "o.mgh"],
        [*base[:2], "o.nii"],
        [*base[:2], "o.nii.gz"],
        [*base[:2], "o"],
        [*base[:2], "o.xyz"],
        [*base[:2], "nodir/o.mgz"],
        [*base[:2], "a.b/c"],
        # added 2026-11-23 (after the first 193): recon-all's spelling of the bounding box
        # without a pad, and with one per side of 0 after the mask on one axis only
        ["-bb", "0", *base],
        ["-BBMM", "2", "2", "2", "0", "2", "2", *base],
    ]
    return c


def mask(text: str, port: bool, folder: Path) -> str:
    """The messages without what differs between the sides: the time, the case's folder
    (messages that name a file by its absolute name), the candidate's notices."""
    text = STAMP.sub("TimeStamp: <time>", text)
    text = text.replace(str(folder), "<folder>")
    if port:
        text = "".join(x for x in text.splitlines(True) if not x.startswith("larmorx: "))
    return text


def status(code: int) -> int:
    """An exit status as a shell reports it: a process ended by signal ``n`` as ``128 + n``."""
    return 128 - code if code < 0 else code


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
    return ws / FS_HOME / "bin" / "mri_mask"


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
    all_cases = cases(f)
    keep = {int(x) for x in only.split(",")} if only else set(range(len(all_cases)))
    work = ws / "oracles" / "inputs" / "fs-mask-prog" / f"work-{tag}-{seed}"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "HOME": str(work), "USER": "probe",
           "FS_DISABLE_LANG": "1"}  # fmt: skip
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
                    ["mri_mask", *args],
                    env=env,
                    cwd=d,
                    outputs=[d],
                    policy=POLICY,
                    timeout=600,
                    runner="fs-mask-prog",
                )
                sides[side] = (done.returncode, done.stdout, done.stderr, d)
            else:
                p = subprocess.run(
                    [*(w.replace("{mode}", side) for w in candidate), *args],
                    env=env,
                    cwd=d,
                    capture_output=True,
                    text=True,
                    errors="surrogateescape",
                    timeout=600,
                )
                sides[side] = (p.returncode, p.stdout, p.stderr, d)
        oc, oo, oe, od = sides["official"]
        row = {"case": i, "args": args, "official_exit": status(oc)}
        for side in modes:
            pc, po, pe, pd = sides[side]
            diffs = []
            if pc != status(oc):
                diffs.append(f"exit {pc} != {status(oc)}")
            if mask(po, True, pd) != mask(oo, False, od):
                diffs.append("stdout")
            if mask(pe, True, pd) != mask(oe, False, od):
                diffs.append("stderr")
            fo, fp = written(od), written(pd)
            if sorted(fo) != sorted(fp):
                diffs.append(f"files {sorted(fo)} != {sorted(fp)}")
            for name in sorted(set(fo) & set(fp)):
                cmp = fsfiles.compare(fo[name], fp[name], ctx, name=Path(name).name)
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
                print(f"case {i} {side}: {row['args']}\n  {diffs}\n  official stdout tail: "
                      f"{oo[-300:]!r}\n  candidate stdout tail: {po[-300:]!r}\n  official "
                      f"stderr: {oe[-300:]!r}\n  candidate stderr: {pe[-400:]!r}",
                      flush=True)  # fmt: skip
        rows.append(row)
        print(f"{i:3d} exit {status(oc)} " + " ".join(
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
