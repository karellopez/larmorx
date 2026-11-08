# SPDX-License-Identifier: Apache-2.0
"""3dvolreg's clean-room original against AFNI 26.3.05's program, as black boxes.

Written by the clean-room implementer of ``specs/3dvolreg.md`` (it reads no AFNI source and no
replica code). AFNI runs through the oracle-output cache (``oracle_cache.run``, one thread, a
clean environment); larmorx's original runs from the command line
(``LARMORX_IMPLEMENTATION=original``), and the replica (a black box too) where a case asks for it.

    python -P blackbox.py inputs                     # make the synthetic inputs
    python -P blackbox.py check [case ...]           # compare the original with AFNI (reproduce)
    python -P blackbox.py run <prog> <case> <dir>    # one program on one case (prog: afni,
                                                     # original, replica; LARMORX_UPSTREAM_DEFECTS
                                                     # from the environment)

The comparison (``specs/3dvolreg.md`` §14.3): exit statuses, messages (AFNI's banner and
compile date, larmorx's own lines and the CPU times set aside), ``-1Dfile`` (the thresholds of the
parameters), ``-1Dmatrix_save`` (1e-5 on the rotation, ``x_t·max δ`` on the translations),
``-maxdisp1D`` (0.01 mm) and the registered volumes (1e-5 or 1e-4 of the largest value where the
parameters agree within 1e-4, else NCC ≥ 0.9999 and mean |diff| ≤ 1e-3·max). Inputs go to
``<workspace>/oracles/inputs/afni-volreg-original/`` (never committed).
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
AFNI = WORKSPACE / "oracles/afni-26.3.05/bin/3dvolreg"
LX = os.environ.get("LX_BIN", str(REPO / "target/release/larmorx"))
INPUTS = WORKSPACE / "oracles/inputs/afni-volreg-original"
FMRIPREP_INPUTS = WORKSPACE / "oracles/inputs/afni-volreg/fmriprep"
#: the replica part's derived inputs (data files: `-rotparent` datasets made by AFNI's own
#: 3dvolreg, gridparents, bases), named with a leading `%`
SUITE_INPUTS = WORKSPACE / "oracles/inputs/afni-volreg"

sys.path.insert(0, str(REPO / "validation/src"))


def phantom(shape, zooms, motions, rng_seed=5):
    """A smooth head-like phantom moved by rigid motions (degrees about x, y, z; mm)."""
    nx, ny, nz = shape
    rng = np.random.default_rng(rng_seed)
    blobs = [(0.0, 0.0, 0.0, 50.0, 56.0, 34.0, 1000.0)]
    for _ in range(14):
        c = rng.uniform(-25, 25, 3) * [1.0, 1.0, 0.6]
        r = rng.uniform(5, 14, 3)
        blobs.append((*c, *r, rng.uniform(-300, 400)))
    ii, jj, kk = np.meshgrid(np.arange(nx), np.arange(ny), np.arange(nz), indexing="ij")
    centre = (np.array(shape) - 1) / 2
    pos = np.stack([(ii - centre[0]) * zooms[0], (jj - centre[1]) * zooms[1], (kk - centre[2]) * zooms[2]])
    out = []
    for ax, ay, az, sx, sy, sz in motions:
        a = np.radians([ax, ay, az])
        rx = np.array([[1, 0, 0], [0, np.cos(a[0]), -np.sin(a[0])], [0, np.sin(a[0]), np.cos(a[0])]])
        ry = np.array([[np.cos(a[1]), 0, np.sin(a[1])], [0, 1, 0], [-np.sin(a[1]), 0, np.cos(a[1])]])
        rz = np.array([[np.cos(a[2]), -np.sin(a[2]), 0], [np.sin(a[2]), np.cos(a[2]), 0], [0, 0, 1]])
        r = rz @ ry @ rx
        p = np.einsum("ij,jxyz->ixyz", r.T, pos - np.array([sx, sy, sz])[:, None, None, None])
        v = np.zeros(shape)
        for cx, cy, cz, rx_, ry_, rz_, amp in blobs:
            d = ((p[0] - cx) / rx_) ** 2 + ((p[1] - cy) / ry_) ** 2 + ((p[2] - cz) / rz_) ** 2
            v += amp / (1.0 + d**4)
        v += 20.0 * np.cos(0.21 * p[0]) * np.cos(0.17 * p[1]) * (v > 50)
        out.append(v)
    return np.stack(out, axis=-1)


MOTIONS = [
    (0, 0, 0, 0, 0, 0),
    (0.5, -0.3, 0.8, 0.6, -0.4, 0.3),
    (1.0, -0.6, 1.6, 1.2, -0.8, 0.6),
    (1.5, -0.9, 2.4, 1.8, -1.2, 0.9),
    (2.0, -1.2, 3.2, 2.4, -1.6, 1.2),
]


def make_inputs() -> None:
    import nibabel as nib

    INPUTS.mkdir(parents=True, exist_ok=True)
    zooms = (3.0, 3.0, 3.5)
    data = phantom((40, 44, 24), zooms, MOTIONS).astype(np.float32)
    # LPS storage (R2L, A2P, I2S): x and y of NIfTI's RAS negated.
    lps = np.diag([-3.0, -3.0, 3.5, 1.0])
    lps[:3, 3] = [58.5, 64.5, -40.25]
    img = nib.Nifti1Image(data, lps)
    img.header.set_xyzt_units("mm", "sec")
    img.header["pixdim"][4] = 2.0
    img.set_qform(lps, 1)
    img.set_sform(lps, 1)
    nib.save(img, INPUTS / "lps_f32.nii.gz")
    # RAS storage (L2R, P2A, I2S), int16.
    ras = np.diag([3.0, 3.0, 3.5, 1.0])
    ras[:3, 3] = [-58.5, -64.5, -40.25]
    img = nib.Nifti1Image(np.round(data).astype(np.int16), ras)
    img.header.set_xyzt_units("mm", "sec")
    img.header["pixdim"][4] = 2.0
    img.set_qform(ras, 1)
    img.set_sform(ras, 1)
    nib.save(img, INPUTS / "ras_i16.nii.gz")
    # uint8 (AFNI-87), a thin series (6 slices: the coarse search's region is empty, AFNI-93),
    # a constant series (AFNI-89 with -twopass).
    u8 = np.clip(np.round(data * (250.0 / data.max())), 0, 255).astype(np.uint8)
    img = nib.Nifti1Image(u8, lps)
    img.set_qform(lps, 1)
    nib.save(img, INPUTS / "lps_u8.nii.gz")
    img = nib.Nifti1Image(np.ascontiguousarray(data[:, :, 9:15, :]), lps)
    img.set_qform(lps, 1)
    nib.save(img, INPUTS / "thin_f32.nii.gz")
    img = nib.Nifti1Image(np.full((20, 22, 12, 3), 100.0, np.float32), lps)
    img.set_qform(lps, 1)
    nib.save(img, INPUTS / "const_f32.nii.gz")
    # slice timing in the header (alt+z, -tshift)
    img = nib.Nifti1Image(data, lps)
    img.set_qform(lps, 1)
    img.header.set_xyzt_units("mm", "sec")
    img.header["pixdim"][4] = 2.0
    img.header.set_dim_info(slice=2)
    img.header["slice_code"] = 3
    img.header["slice_start"] = 0
    img.header["slice_end"] = data.shape[2] - 1
    img.header["slice_duration"] = 2.0 / data.shape[2]
    nib.save(img, INPUTS / "lps_f32_st.nii.gz")
    print("inputs in", INPUTS)


def make_more_inputs() -> None:
    """The inputs of relay 2's cases: an all-zero weight and a constant series (AFNI-89's
    candidates), on the grid of the suite's ``s1``."""
    import nibabel as nib

    ref = nib.load(SUITE_INPUTS / "s1.nii.gz")
    shape = ref.shape[:3]
    nib.save(nib.Nifti1Image(np.zeros(shape, np.float32), ref.affine), INPUTS / "zeros_w.nii.gz")
    const = np.full((*shape, 4), 100.0, np.float32)
    nib.save(nib.Nifti1Image(const, ref.affine), INPUTS / "const_big.nii.gz")
    half = np.zeros((*shape, 4), np.float32)
    half[..., : shape[2] // 2, :] = 100.0
    nib.save(nib.Nifti1Image(half, ref.affine), INPUTS / "halfconst.nii.gz")
    print("inputs in", INPUTS)


FMRIPREP = ["-Fourier", "-twopass", "-zpad", "4"]
OUTPUTS = ["-prefix", "out.nii", "-1Dfile", "p.1D", "-1Dmatrix_save", "m.aff12.1D", "-maxdisp1D", "md.1D"]

#: case name -> (input, options)
CASES: dict[str, tuple[str, list[str]]] = {
    "default": ("lps_f32.nii.gz", []),
    "linear": ("lps_f32.nii.gz", ["-linear"]),
    "cubic": ("lps_f32.nii.gz", ["-cubic"]),
    "quintic": ("lps_f32.nii.gz", ["-quintic"]),
    "fourier": ("lps_f32.nii.gz", ["-Fourier"]),
    "final-nn": ("lps_f32.nii.gz", ["-final", "NN"]),
    "noclip-zpad0": ("lps_f32.nii.gz", ["-noclip", "-zpad", "0"]),
    "base2": ("lps_f32.nii.gz", ["-base", "2"]),
    "twopass": ("lps_f32.nii.gz", ["-twopass"]),
    "fmriprep": ("lps_f32.nii.gz", FMRIPREP),
    "twopass-shiftonly": ("lps_f32.nii.gz", ["-twopass", "-coarse", "2", "1"]),
    "ras-i16": ("ras_i16.nii.gz", []),
    "ras-i16-fmriprep": ("ras_i16.nii.gz", FMRIPREP),
    "verbose": ("lps_f32.nii.gz", ["-verbose", "-twopass"]),
    "vv-linear": ("lps_f32.nii.gz", ["-verbose", "-verbose", "-linear"]),
    "vv-heptic": ("lps_f32.nii.gz", ["-verbose", "-verbose"]),
    # the spec model's synthetic series (validation/investigations/3dvolreg-spec/check.py makes
    # it; copied here as model_syn.nii.gz)
    "syn-linear": ("model_syn.nii.gz", ["-linear"]),
    "syn-default": ("model_syn.nii.gz", []),
    "u8": ("lps_u8.nii.gz", []),
    "tshift": ("lps_f32_st.nii.gz", ["-tshift", "0"]),
    "tshift-verbose-ignore": ("lps_f32_st.nii.gz", ["-verbose", "-tshift", "1"]),
    "tshift-none": ("lps_f32.nii.gz", ["-tshift", "0"]),
    "tshift-fmriprep": ("lps_f32_st.nii.gz", ["-tshift", "0", *FMRIPREP]),
    "thin-twopass": ("thin_f32.nii.gz", ["-twopass"]),
    "const-twopass": ("const_f32.nii.gz", ["-twopass"]),
    "nomaxdisp-twopass": ("lps_f32.nii.gz", ["-nomaxdisp", "-twopass"]),
    "ras-savedisp": ("ras_i16.nii.gz", ["-savedisp", "sd.nii"]),
    "ds000005": ("@ds000005-run-01_sliced.nii.gz", []),
    # relay 2: -twodup, -SAVEDISP, AFNI-89's candidates, -rotparent / -gridparent, -verbose x2
    "twodup": ("%s1.nii.gz", ["-twopass", "-base", str(SUITE_INPUTS / "base_shift.nii.gz"), "-twodup"]),
    "twodup-base3d": ("%s1.nii.gz", ["-twopass", "-base", str(SUITE_INPUTS / "base3d.nii.gz"), "-twodup"]),
    "twodup-onepass": ("%s1.nii.gz", ["-base", str(SUITE_INPUTS / "base_shift.nii.gz"), "-twodup"]),
    "savedisp-s1": ("%s1.nii.gz", ["-savedisp", "sd.nii"]),
    "SAVEDISP": ("%s1.nii.gz", ["-SAVEDISP", "sd.nii"]),
    "SAVEDISP-ras": ("ras_i16.nii.gz", ["-SAVEDISP", "sd.nii"]),
    "zero-weight": ("%s1.nii.gz", ["-weight", str(INPUTS / "zeros_w.nii.gz") + "[0]"]),
    "zero-weight-twopass": ("%s1.nii.gz", ["-twopass", "-weight", str(INPUTS / "zeros_w.nii.gz") + "[0]"]),
    "const-big-twopass": ("const_big.nii.gz", ["-twopass"]),
    "const-big": ("const_big.nii.gz", []),
    "halfconst-twopass": ("halfconst.nii.gz", ["-twopass"]),
    "vv-twopass": ("lps_f32.nii.gz", ["-verbose", "-verbose", "-twopass"]),
    "afni94": ("%e_big120.nii.gz", ["-twopass", "-coarse", "1", "2"]),
    "afni90-prefix": ("lps_f32.nii.gz", ["-prefix", "p" * 252 + ".nii"]),
    "afni90-dfile": ("lps_f32.nii.gz", ["-dfile", "d" * 300]),
    "afni90-1dfile": ("lps_f32.nii.gz", ["-1Dfile", "x" * 256]),
    "afni90-weight": ("lps_f32.nii.gz", ["-weight", "w" * 300 + ".nii[0]"]),
    "afni90-maxdisp": ("lps_f32.nii.gz", ["-maxdisp1D", "m" * 250 + ".1D"]),
    "afni98-input": ("lps_f32.nii.gz", ["-input", str(INPUTS / "lps_f32.nii.gz"), "-base", "9"]),
    "afni98-weight": ("lps_f32.nii.gz", ["-input", str(INPUTS / "lps_f32.nii.gz"), "-weight", str(SUITE_INPUTS / "gp27.nii.gz") + "[0]"]),
    "s1-linear-noclip": ("%s1.nii.gz", ["-linear", "-noclip"]),
    "s1-linear": ("%s1.nii.gz", ["-linear"]),
    "rp": ("%s1.nii.gz", ["-rotparent", str(SUITE_INPUTS / "rp.nii.gz")]),
    "rp-verbose": ("%s1.nii.gz", ["-verbose", "-rotparent", str(SUITE_INPUTS / "rp.nii.gz")]),
    "rp-fourier": ("%s1.nii.gz", ["-Fourier", "-rotparent", str(SUITE_INPUTS / "rp.nii.gz")]),
    "rp-linear-noclip": ("%s1.nii.gz", ["-linear", "-noclip", "-rotparent", str(SUITE_INPUTS / "rp.nii.gz")]),
    "rp-twopass": ("%s1.nii.gz", ["-twopass", "-rotparent", str(SUITE_INPUTS / "rp.nii.gz")]),
    "rp-gp24": ("%s1.nii.gz", ["-rotparent", str(SUITE_INPUTS / "rp.nii.gz"), "-gridparent", str(SUITE_INPUTS / "gp24.nii.gz")]),
    "rp-gp27": ("%s1.nii.gz", ["-verbose", "-rotparent", str(SUITE_INPUTS / "rp.nii.gz"), "-gridparent", str(SUITE_INPUTS / "gp27.nii.gz")]),
    "rp-gp27m": ("%s1.nii.gz", ["-rotparent", str(SUITE_INPUTS / "rp.nii.gz"), "-gridparent", str(SUITE_INPUTS / "gp27m.nii.gz")]),
    "rp-gp22": ("%s1.nii.gz", ["-rotparent", str(SUITE_INPUTS / "rp.nii.gz"), "-gridparent", str(SUITE_INPUTS / "gp22.nii.gz")]),
    "rp-gpsp": ("%s1.nii.gz", ["-rotparent", str(SUITE_INPUTS / "rp.nii.gz"), "-gridparent", str(SUITE_INPUTS / "gpsp.nii.gz")]),
    "rp-gpx": ("%s1.nii.gz", ["-rotparent", str(SUITE_INPUTS / "rp.nii.gz"), "-gridparent", str(SUITE_INPUTS / "gpx.nii.gz")]),
    "gp-alone": ("%s1.nii.gz", ["-gridparent", str(SUITE_INPUTS / "gp27.nii.gz")]),
    "rp-noattr": ("%s1.nii.gz", ["-rotparent", str(SUITE_INPUTS / "s1.nii.gz")]),
    "rp-st": ("%s6_st.nii.gz", ["-verbose", "-rotparent", str(SUITE_INPUTS / "rp_st.nii.gz")]),
    "rp-st-gp27": ("%s6_st.nii.gz", ["-rotparent", str(SUITE_INPUTS / "rp_st.nii.gz"), "-gridparent", str(SUITE_INPUTS / "gp27.nii.gz")]),
    "rp-obl": ("%s3_oblique.nii.gz", ["-rotparent", str(SUITE_INPUTS / "rp_obl.nii.gz")]),
    "rp-twodup": ("%s1.nii.gz", ["-twopass", "-base", str(SUITE_INPUTS / "base_shift.nii.gz"), "-twodup", "-rotparent", str(SUITE_INPUTS / "rp.nii.gz"), "-gridparent", str(SUITE_INPUTS / "gp27.nii.gz")]),
    "ds000005-fmriprep": ("@ds000005-run-01_sliced.nii.gz", FMRIPREP),
}


def input_path(name: str) -> Path:
    if name.startswith("@"):
        return FMRIPREP_INPUTS / name[1:]
    if name.startswith("%"):
        return SUITE_INPUTS / name[1:]
    return INPUTS / name


def clean_env(work: Path) -> dict[str, str]:
    home = work / "home"
    home.mkdir(parents=True, exist_ok=True)
    return {
        "HOME": str(home),
        "PATH": "/usr/bin:/bin",
        "AFNI_DONT_LOGFILE": "YES",
        "OMP_NUM_THREADS": "1",
    }


def run(prog: str, case: str, d: Path, mode: str = "reproduce", threads: str = "1"):
    from larmorx_validation import oracle_cache

    src, opts = CASES[case]
    if d.exists() and any(d.iterdir()):
        raise SystemExit(f"{d} is not empty: every run gets a fresh directory")
    d.mkdir(parents=True, exist_ok=True)
    outputs = OUTPUTS if "-nomaxdisp" not in opts else OUTPUTS[:-2]
    args = [*opts, *outputs, str(input_path(src))]
    env = clean_env(d)
    if prog == "afni":
        if mode != "reproduce":
            raise SystemExit("AFNI is compared in reproduce mode only")
        return oracle_cache.run(
            [str(AFNI), *args],
            env=env,
            cwd=d,
            outputs=[d],
            policy=oracle_cache.Policy(
                threads=1,
                family="afni",
                nifti_extensions="AFNI's history: the date and the output's path",
            ),
            runner="afni-volreg-original-blackbox",
            timeout=1200,
        )
    env = {**env, "OMP_NUM_THREADS": threads, "LARMORX_UPSTREAM_DEFECTS": mode,
           "LARMORX_IMPLEMENTATION": prog}
    return subprocess.run([LX, "afni", "3dvolreg", *args], cwd=d, env=env, capture_output=True,
                          timeout=1200)


def _out(r) -> bytes:
    return r.stdout_bytes if hasattr(r, "stdout_bytes") else r.stdout


def _err(r) -> bytes:
    return r.stderr_bytes if hasattr(r, "stderr_bytes") else r.stderr


def _lines(text: bytes) -> list[str]:
    out = []
    for line in text.decode(errors="replace").splitlines():
        if re.match(r"^\+\+ 3dvolreg: AFNI version", line) or "Program compile date" in line:
            continue
        if line.startswith("++ implementation:") or line.startswith("++ upstream defects:"):
            continue
        if "upstream defect AFNI-" in line or "upstream defect GLIBC-" in line:
            continue
        line = re.sub(r"CPU time for (\w+ pass|realignment)=\S+ s", r"CPU time for \1=0 s", line)
        line = re.sub(r"\[=\S+ s/sub-brick\]", "[=0 s/sub-brick]", line)
        out.append(line)
    return out


def _table(path: Path) -> np.ndarray:
    return np.loadtxt(path, comments="#", ndmin=2)


def compare(case: str, a: Path, b: Path, ra, rb) -> dict:
    import nibabel as nib

    res: dict = {"case": case}
    res["exit"] = (ra.returncode, rb.returncode)
    la = _lines(_err(ra))
    lb = _lines(_err(rb))
    res["messages_equal"] = la == lb
    if la != lb:
        res["messages"] = {"afni": la, "larmorx": lb}
    if ra.returncode != 0 or rb.returncode != 0:
        return res
    pa, pb = _table(a / "p.1D"), _table(b / "p.1D")
    d = np.abs(pa - pb)
    res["params_identical_lines"] = int(
        sum(x == y for x, y in zip((a / "p.1D").read_text().splitlines(), (b / "p.1D").read_text().splitlines()))
    )
    res["params_max_diff"] = float(d.max())
    res["params_median_diff"] = float(np.median(d))
    _, opts = CASES[case]
    xt, rt = 0.01, 0.02
    hdr = nib.load(a / "out.nii").header
    vox = np.array(hdr.get_zooms()[:3], float)
    ok = True
    for row_a, row_b in zip(pa, pb):
        for q in range(6):
            lim = (rt if q < 3 else xt * vox.max()) + 0.005 * abs(row_a[q])
            ok &= abs(row_a[q] - row_b[q]) <= lim
    res["params_within"] = bool(ok and np.median(d) <= 0.001)
    ma, mb = _table(a / "m.aff12.1D"), _table(b / "m.aff12.1D")
    rot = [0, 1, 2, 4, 5, 6, 8, 9, 10]
    res["matrix_rot_max"] = float(np.abs(ma[:, rot] - mb[:, rot]).max())
    res["matrix_trans_max"] = float(np.abs(ma[:, [3, 7, 11]] - mb[:, [3, 7, 11]]).max())
    if (a / "md.1D").exists() and (b / "md.1D").exists():
        res["maxdisp_max"] = float(np.abs(_table(a / "md.1D") - _table(b / "md.1D")).max())
        res["maxdelta_max"] = float(np.abs(_table(a / "md_delt.1D") - _table(b / "md_delt.1D")).max())
    names = sorted({f.name[3:-4] for f in [*a.glob("sd_*.nii"), *b.glob("sd_*.nii")]})
    for axis in names:
        fa, fb = a / f"sd_{axis}.nii", b / f"sd_{axis}.nii"
        if fa.exists() or fb.exists():
            if not (fa.exists() and fb.exists()):
                res[f"savedisp_{axis}"] = "missing"
                continue
            da = np.asarray(nib.load(fa).dataobj, dtype=np.float64)
            db = np.asarray(nib.load(fb).dataobj, dtype=np.float64)
            res[f"savedisp_{axis}_max"] = float(np.abs(da - db).max())
    va = np.asarray(nib.load(a / "out.nii").dataobj, dtype=np.float64)
    vb = np.asarray(nib.load(b / "out.nii").dataobj, dtype=np.float64)
    res["dtype"] = (str(nib.load(a / "out.nii").get_data_dtype()), str(nib.load(b / "out.nii").get_data_dtype()))
    if va.shape != vb.shape:
        res["shape"] = (va.shape, vb.shape)
        return res
    big = float(np.abs(va).max())
    res["volume_rel_max"] = float(np.abs(va - vb).max() / big)
    res["volume_rel_mean"] = float(np.abs(va - vb).mean() / big)
    res["volume_identical"] = bool(np.array_equal(va, vb))
    ha, hb = nib.load(a / "out.nii").header, nib.load(b / "out.nii").header
    keys = ["dim", "pixdim", "qform_code", "sform_code", "quatern_b", "quatern_c", "quatern_d",
            "qoffset_x", "qoffset_y", "qoffset_z", "srow_x", "srow_y", "srow_z", "datatype",
            "scl_slope", "scl_inter", "xyzt_units", "toffset", "slice_code", "slice_start",
            "slice_end", "slice_duration", "dim_info"]
    diff = [k for k in keys
            if not np.array_equal(np.asarray(ha[k]), np.asarray(hb[k]), equal_nan=True)]
    res["header_differs"] = diff
    return res


def main(argv: list[str]) -> int:
    if not argv or argv[0] == "inputs":
        make_inputs()
        return 0
    if argv[0] == "inputs2":
        make_more_inputs()
        return 0
    if argv[0] == "run":
        prog, case, d = argv[1], argv[2], Path(argv[3])
        r = run(prog, case, d, os.environ.get("LARMORX_UPSTREAM_DEFECTS", "reproduce"))
        sys.stdout.buffer.write(_out(r))
        sys.stderr.buffer.write(_err(r))
        return r.returncode or 0
    if argv[0] == "notices":
        # larmorx's own lines (notices, errors) of the replica and the original, both modes
        work = Path(tempfile.mkdtemp(prefix="volreg-notices_", dir=os.environ.get("VOLREG_WORK")))
        for case in argv[1:]:
            for mode in ("fix", "reproduce"):
                outs = {}
                for prog in ("replica", "original"):
                    r = run(prog, case, work / case / f"{prog}-{mode}", mode)
                    lines = [ln for ln in _err(r).decode(errors="replace").splitlines()
                             if "upstream defect" in ln or ln.startswith("**")]
                    outs[prog] = (r.returncode, lines)
                same = outs["replica"] == outs["original"]
                print(f"== {case} [{mode}] {'same' if same else 'DIFFERENT'}")
                for prog, (code, lines) in outs.items():
                    if not same or prog == "replica":
                        print(f"  {prog}: exit {code}")
                        for ln in lines:
                            print("    " + ln)
        return 0
    if argv[0] == "check":
        # REF=replica compares with the replica (a black box) in LARMORX_UPSTREAM_DEFECTS's mode
        cases = argv[1:] or list(CASES)
        work = Path(tempfile.mkdtemp(prefix="volreg-bb_", dir=os.environ.get("VOLREG_WORK")))
        rows = []
        for case in cases:
            a, b = work / case / "afni", work / case / "original"
            t0 = time.perf_counter()
            ref = os.environ.get("REF", "afni")
            mode = os.environ.get("LARMORX_UPSTREAM_DEFECTS", "reproduce")
            ra = run(ref, case, a, mode)
            t1 = time.perf_counter()
            rb = run("original", case, b, mode)
            t2 = time.perf_counter()
            res = compare(case, a, b, ra, rb)
            res["seconds"] = {"afni": round(t1 - t0, 2), "afni_cached": bool(getattr(ra, "cached", False)),
                              "original": round(t2 - t1, 2)}
            rows.append(res)
            print(json.dumps(res, default=str))
        return 0
    print(__doc__)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
