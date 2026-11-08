# SPDX-License-Identifier: Apache-2.0
"""sdcflows' PEPOLAR estimation (``init_topup_wf``, both wirings) and its steps against
``lx.sdcflows`` / ``lx.niworkflows`` (``upstream_defects='reproduce'``): every NIfTI file each
nipype node writes against larmorx's file of that node (data and header bytes), the steps'
outputs, nipype's command lines and encoding files.

The programs under sdcflows' graphs run through the oracle cache: stand-ins on ``PATH`` (each
with a recursion guard) run AFNI 26.3.05's ``3dvolreg``, FSL 6.0.7.23's ``topup`` and ANTs
2.6.5's ``N4BiasFieldCorrection`` (one thread, CORE-MATH; ``check_workflows.py``'s); larmorx
runs its 3dvolreg replica, its topup replica (``LARMORX_NC_BIN``) and its N4. AFNI writes its
history (the command line, the date) into an extension of its NIfTI files and FSL its version
and the time into ``descrip``; files carrying them are compared without those fields (counted
apart: ``same but descrip/extension``).

Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``, ``LARMORX_NC_BIN``; boxed):

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_pepolar.py \
        [--sections units,ravg,wf,image] [--runs ds005040,ds001600,...] [--threads N]

- ``units``: ``get_trt`` (sdcflows' doctest metadata and every branch, its errors),
  ``GetReadoutTime``, ``SelectPEVolumes``, ``SortPEBlips``, ``UniformGrid``, ``MergeSeries``,
  ``PadSlices``, ``ReorientImageAndMetadata``, ``ReorientImage``, ``_select_topup_config``,
  ``_topup_mats_to_world``, nipype's ``TOPUP`` command line and encoding file, numpy's
  ``mean(axis=(0, 1, 2))`` of RobustAverage's series;
- ``ravg``: niworkflows' ``RobustAverage`` (every option, the one-volume, 3D and error paths)
  on the oracle runs' PEPOLAR EPIs;
- ``wf``: ``init_topup_wf`` of sdcflows 2.17.1 (installed) and of 2.15.1 (``tags/``: fMRIPrep's
  wiring) on the PEPOLAR estimators of the oracle runs (their ``inputnode``), 2.17.1 also with
  ``debug`` and ``sloppy`` on the first;
- ``image``: larmorx's steps with fMRIPrep 25.2.6's libraries (numpy 2.2.6) on the oracle runs'
  own node inputs against fMRIPrep's own node files (runwise and set averages, regrid, merge;
  ``fix_coeff``, ``from_las`` and ``ref_average`` fed fMRIPrep's own topup outputs); fMRIPrep's
  topup is an older FSL's (reported only: INFO).

Prints one line per check (cases, identical), INFO lines, the times, and ``ALL OK`` or
``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import ast
import importlib.util
import os
import stat
import sys
import tempfile
import time
import warnings
from pathlib import Path

os.environ.setdefault("FSLOUTPUTTYPE", "NIFTI_GZ")

import nibabel as nb
import numpy as np
import scipy

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_workflows as cw  # noqa: E402

W = cw.W
RUNS = cw.RUNS
NP = np.__version__
SP = scipy.__version__
TAGS = W / "tags"

VOLREG_BODY = """from pathlib import Path
from larmorx_validation.parity.afni_replica import run_afni
home = Path(os.environ["LX_SHIM_HOME"])
env = {"HOME": str(home), "AFNI_DONT_LOGFILE": "YES", "OMP_NUM_THREADS": "1",
       "PATH": "/usr/bin:/bin"}
r = run_afni("3dvolreg", sys.argv[1:], env, Path.cwd())
sys.stdout.write(r.out)
sys.stderr.write(r.err)
sys.exit(r.code)
"""
TOPUP_BODY = """from pathlib import Path
from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_versions
env = {**os.environ, "FSLOUTPUTTYPE": "NIFTI_GZ", "FSLDIR": str(fsl_versions.fsldir()),
       **fsl_versions.openblas_env()}
r = oracle_cache.run([str(fsl_versions.program("topup")), *sys.argv[1:]], env=env,
                     cwd=Path.cwd(), policy=oracle_cache.Policy(threads=1, family="fsl"),
                     runner="sdcflows-pepolar", timeout=3600)
sys.stdout.write(r.stdout)
sys.stderr.write(r.stderr)
sys.exit(r.returncode)
"""


def install_shims(folder: Path) -> None:
    cw.install_shims(folder)
    paths = [p for p in sys.path if p and ("validation/src" in p or p.endswith("/python"))]
    home = folder / "home"
    home.mkdir(exist_ok=True)
    os.environ["LX_SHIM_HOME"] = str(home)
    for name, body in (("3dvolreg", VOLREG_BODY), ("topup", TOPUP_BODY)):
        p = folder / name
        p.write_text(cw.SHIM.format(python=sys.executable, name=name, paths=paths, body=body))
        p.chmod(p.stat().st_mode | stat.S_IXUSR)


def compare(a, b) -> str:
    """``same``, ``same but descrip/extension`` (data, the header without ``descrip``,
    ``vox_offset`` and extensions), or what differs."""
    if not Path(b).exists():
        return "missing"
    x, y = nb.load(a), nb.load(b)
    dx, dy = np.asanyarray(x.dataobj), np.asanyarray(y.dataobj)
    if dx.dtype != dy.dtype or dx.shape != dy.shape:
        return f"dtype/shape {dx.dtype}{dx.shape} vs {dy.dtype}{dy.shape}"
    if np.ascontiguousarray(dx).tobytes() != np.ascontiguousarray(dy).tobytes():
        same = (
            np.sum((dx == dy) | (np.isnan(dx) & np.isnan(dy)))
            if dx.dtype.kind == "f"
            else np.sum(dx == dy)
        )
        return f"data ({same}/{dx.size} equal)"
    hx, hy = x.header.copy(), y.header.copy()
    if hx.binaryblock == hy.binaryblock and [
        (e.get_code(), e.get_content()) for e in hx.extensions
    ] == [(e.get_code(), e.get_content()) for e in hy.extensions]:
        return "same"
    for h in (hx, hy):
        h["descrip"] = b""
        h["vox_offset"] = 0
    if hx.binaryblock == hy.binaryblock:
        return "same but descrip/extension"
    diff = [k for k in hx if np.asarray(hx[k]).tobytes() != np.asarray(hy[k]).tobytes()]
    return f"header {diff}"


def _quaternion_rounding(a, b) -> bool:
    """Whether two headers differ only in quaternion fields, by less than 1e-12 (a value that is
    zero up to rounding, as numpy's BLAS kernels give it)."""
    hx, hy = nb.load(a).header.copy(), nb.load(b).header.copy()
    for h in (hx, hy):
        h["descrip"] = b""
        h["vox_offset"] = 0
    keys = [k for k in hx.keys() if np.asarray(hx[k]).tobytes() != np.asarray(hy[k]).tobytes()]  # noqa: SIM118 (a nibabel header)
    return bool(keys) and all(
        k in ("quatern_b", "quatern_c", "quatern_d") and abs(float(hx[k]) - float(hy[k])) < 1e-12
        for k in keys
    )


def tally_file(t: cw.Tally, what: str, label: str, ref, mine, quaternion_ok: bool = False) -> None:
    r = compare(ref, mine)
    if quaternion_ok and r.startswith("header") and _quaternion_rounding(ref, mine):
        t.add(what, True)
        t.add(f"{what} (of which quaternion rounding only)", True)
        print(f"INFO {label}: {what}: the data identical; {r} by less than 1e-12", flush=True)
        return
    t.add(what, r.startswith("same"), f"{label}: {r}")
    if r == "same but descrip/extension":
        t.add(f"{what} (of which descrip/extension only)", True)


def compare_tree(t: cw.Tally, label: str, ref_root: Path, mine_root: Path, what: str) -> None:
    for f in sorted(ref_root.rglob("*.nii*")):
        rel = f.relative_to(ref_root)
        if any(p.startswith("_report") or p == "crash" for p in rel.parts):
            continue
        tally_file(t, f"{what}: node files", f"{label}: {rel}", f, mine_root / rel)


# ------------------------------------------------------------------------------------------
# the oracle runs' PEPOLAR estimators


def estimators(names: list[str] | None) -> list[tuple[str, Path, list[str], list[dict]]]:
    """(label, the estimator's folder, in_data, metadata) of every finished PEPOLAR estimator."""
    out = []
    for run in sorted(RUNS.iterdir()):
        if names and not any(run.name.startswith(n) for n in names):
            continue
        for wf in sorted(run.glob("work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/wf_*")):
            if not (wf / "runwise_avg").is_dir():
                continue
            subs = sorted((wf / "readout_time" / "mapflow").glob("_readout_time*"))
            ravgs = sorted((wf / "runwise_avg" / "mapflow").glob("_runwise_avg*"))
            if not subs or not all((s / "_report" / "report.rst").exists() for s in subs):
                continue
            metas = [ast.literal_eval(cw.node_inputs(s)["metadata"]) for s in subs]
            files = [cw.host_path(r, cw.node_inputs(r)["in_file"]) for r in ravgs]
            out.append((f"{run.name}/{wf.name}", wf, files, metas))
    return out


def topup_215():
    """sdcflows 2.15.1's ``init_topup_wf`` (its ``workflows/fit/pepolar.py`` from ``tags/``, run
    against the installed sdcflows' interfaces, which are 2.15.1's on this path)."""
    path = TAGS / "sdcflows-2.15.1" / "sdcflows" / "workflows" / "fit" / "pepolar.py"
    spec = importlib.util.spec_from_file_location("sdcflows.workflows.fit._pepolar_2_15_1", path)
    mod = importlib.util.module_from_spec(spec)
    mod.__package__ = "sdcflows.workflows.fit"
    spec.loader.exec_module(mod)
    return mod.init_topup_wf


def fsldir() -> str:
    from larmorx_validation.parity import fsl_versions

    return str(fsl_versions.fsldir())


# ------------------------------------------------------------------------------------------
# units


def sec_units(t: cw.Tally, nt: int, ests) -> None:
    from nipype.interfaces.fsl.epi import TOPUP
    from sdcflows.interfaces.epi import GetReadoutTime, SelectPEVolumes, SortPEBlips
    from sdcflows.interfaces.utils import PadSlices, ReorientImageAndMetadata, UniformGrid
    from sdcflows.utils.epimanip import get_trt as sd_trt
    from sdcflows.workflows.fit.pepolar import _select_topup_config, _topup_mats_to_world

    from larmorx import sdcflows as lxs
    from larmorx.niworkflows import merge_series, reorient_file
    from larmorx.niworkflows.images import _gs_drift

    def both(fn_ref, fn_mine, label, what):
        try:
            r = ("ok", fn_ref())
        except Exception as e:
            r = ("err", type(e).__name__, str(e))
        try:
            m = ("ok", fn_mine())
        except Exception as e:
            m = ("err", type(e).__name__, str(e))
        t.add(what, r == m, f"{label}: {r} vs {m}")

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        ep = tmp / "epi.nii.gz"
        nb.Nifti1Image(np.zeros((70, 80, 30), np.int16), np.eye(4)).to_filename(ep)
        cases = [
            {"TotalReadoutTime": 0.05251},
            {"TotalReadoutTime": 0},
            {"TotalReadoutTime": "0.05"},
            {"EstimatedTotalReadoutTime": 0.05251},
            {"PhaseEncodingDirection": "j", "EffectiveEchoSpacing": 0.00059},
            {"PhaseEncodingDirection": "i-", "EffectiveEchoSpacing": 0.00059},
            {"PhaseEncodingDirection": "k", "EffectiveEchoSpacing": 0.0},
            {"PhaseEncodingDirection": "j", "EstimatedEffectiveEchoSpacing": 0.00059},
            {
                "PhaseEncodingDirection": "j",
                "EchoSpacing": 0.00119341,
                "ParallelReductionFactorInPlane": 2,
            },
            {
                "PhaseEncodingDirection": "j",
                "EchoSpacing": 0.001,
                "ParallelReductionFactorInPlane": 1.5,
            },
            {
                "PhaseEncodingDirection": "j",
                "WaterFatShift": 9.2227266,
                "EPIFactor": 35,
                "ImagingFrequency": 127.7325,
            },
            {
                "PhaseEncodingDirection": "j",
                "WaterFatShift": 9.2227266,
                "EPIFactor": 35,
                "MagneticFieldStrength": 3,
            },
            {"PhaseEncodingDirection": "j", "WaterFatShift": 9.2227266, "EPIFactor": 35},
            {"PhaseEncodingDirection": "j"},
            {"EffectiveEchoSpacing": 0.00059},
            {},
        ]
        for i, meta in enumerate(cases):
            for est in (False, True):
                for fb in (None, 0.0, 0.03125):
                    for f in (str(ep), None):
                        both(
                            lambda m=meta, e=est, b=fb, f=f: sd_trt(
                                m, f, use_estimate=e, fallback=b
                            ),
                            lambda m=meta, e=est, b=fb, f=f: lxs.get_trt(
                                m, f, use_estimate=e, fallback=b
                            ),
                            f"case {i} est={est} fb={fb} file={f is not None}",
                            "get_trt",
                        )
        for i, meta in enumerate(cases[:3] + cases[4:6]):
            for fb in (None, 0.03125):

                def ref(meta=meta, fb=fb):
                    gi = GetReadoutTime(metadata=meta, in_file=str(ep))
                    if fb is not None:
                        gi.inputs.fallback = fb
                    o = gi.run(cwd=str(tmp)).outputs
                    return (o.readout_time, o.pe_direction, o.pe_dir_fsl)

                def mine(meta=meta, fb=fb):
                    r = lxs.get_readout_time(meta, str(ep), fallback=fb)
                    return (r.readout_time, r.pe_direction, r.pe_dir_fsl)

                both(ref, mine, f"meta {i} fb {fb}", "GetReadoutTime")

        # SortPEBlips
        for dirs, ros in (
            (["y-", "y", "y-", "y"], [0.03, 0.03, 0.02, 0.04]),
            (["x", "x-"], [0.05, 0.05]),
            (["z-", "y", "x"], [0.01, 0.02, 0.03]),
        ):
            files = [str(ep)] * len(dirs)

            def ref(d=dirs, r=ros, f=files):
                o = SortPEBlips(in_data=f, pe_dirs_fsl=d, readout_times=r).run(cwd=str(tmp)).outputs
                return (
                    list(o.out_data) if isinstance(o.out_data, list) else [o.out_data],
                    o.pe_dirs,
                    o.pe_dirs_fsl,
                    o.readout_times,
                )

            def mine(d=dirs, r=ros, f=files):
                o = lxs.sort_pe_blips(f, d, r)
                return (
                    list(o.out_data),
                    list(o.pe_dirs),
                    list(o.pe_dirs_fsl),
                    list(o.readout_times),
                )

            both(ref, mine, f"{dirs}", "SortPEBlips")

        # nipype's TOPUP: command line and encoding file
        for dirs, ros in (
            (["y", "y-"], [0.0426603, 0.0346501]),
            (["x-", "x", "x"], [0.05]),
            (["z", "z-"], [0.1, 0.2, 0.3]),
        ):
            four = tmp / "four.nii.gz"
            nb.Nifti1Image(np.zeros((4, 4, 4, len(dirs)), np.float32), np.eye(4)).to_filename(four)
            cwd = tmp / f"tu{len(dirs)}{dirs[0]}"
            cwd.mkdir()
            here = os.getcwd()
            os.chdir(cwd)
            try:
                tu = TOPUP(
                    config="b02b0.cnf",
                    in_file=str(four),
                    encoding_direction=dirs,
                    readout_times=ros,
                )
                try:
                    ref_cmd = tu.cmdline
                    ref_enc = (cwd / "four_encfile.txt").read_text()
                except Exception as e:
                    ref_cmd, ref_enc = f"err {e}", ""
            finally:
                os.chdir(here)
            mdir = tmp / f"m{len(dirs)}{dirs[0]}"
            mdir.mkdir()
            try:
                lxs.pepolar._encfile(str(mdir / "four_encfile.txt"), dirs, ros)
                mine_enc = (mdir / "four_encfile.txt").read_text()
                mine_cmd = " ".join(["topup", "--config=b02b0.cnf", f"--datain={cwd}/four_encfile.txt", f"--imain={four}",
                    "--out=four_base", "--iout=four_corrected.nii.gz", "--fout=four_field.nii.gz", "--jacout=jac",
                    "--logout=four_topup.log", "--rbmout=xfm", "--dfout=warpfield"])  # fmt: skip
            except Exception as e:
                mine_cmd, mine_enc = f"err {e}", ""
            t.add(
                "TOPUP encoding file = nipype's",
                ref_enc == mine_enc,
                f"{dirs}: {ref_enc!r} vs {mine_enc!r}",
            )
            if not ref_cmd.startswith("err"):
                t.add(
                    "TOPUP command line = nipype's", ref_cmd == mine_cmd, f"{ref_cmd} vs {mine_cmd}"
                )
            else:
                t.add(
                    "TOPUP command line = nipype's",
                    mine_cmd.startswith("err") and "thelength" in mine_cmd,
                    f"{ref_cmd} vs {mine_cmd}",
                )

        # the drift's mean
        rng = np.random.default_rng(5)
        for shape in ((7, 9, 11, 3), (64, 64, 40, 2), (80, 80, 60, 5), (31, 33, 17, 4)):
            parts = [rng.normal(500, 200, shape[:3]).astype(np.float64) for _ in range(shape[3])]
            arr = np.rollaxis(np.stack(parts), 0, 4)
            d = np.asarray(arr, dtype=np.float32)
            ref = np.mean(d, axis=(0, 1, 2))
            mine = _gs_drift(d, NP)
            t.add("RobustAverage drift mean = numpy's", ref.tobytes() == mine.tobytes(), f"{shape}")

        # steps on the estimators' own files
        for label, wf, files, _metas in ests:
            ravgs = sorted((wf / "runwise_avg" / "mapflow").glob("_runwise_avg*/*_average.nii.gz"))
            for ref_index in range(len(ravgs)):
                rd, md = tmp / f"ug{ref_index}", tmp / f"ugm{ref_index}"
                o = (
                    UniformGrid(in_data=[str(p) for p in ravgs], reference=ref_index)
                    .run(cwd=str(rd))
                    .outputs
                )
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    m = lxs.uniform_grid(
                        [str(p) for p in ravgs], ref_index, out_dir=md, n_threads=nt
                    )
                ok = [Path(a).name for a in o.out_data] == [Path(b).name for b in m.out_data]
                t.add("UniformGrid outputs = sdcflows'", ok, f"{label} ref {ref_index}")
                for a, b in zip(o.out_data, m.out_data, strict=True):
                    if a != str(ravgs[0]) and a not in map(str, ravgs):
                        tally_file(
                            t, "UniformGrid files = sdcflows'", f"{label} ref {ref_index}", a, b
                        )
            for k, f in enumerate(files):
                for maxv in (1, 3, 10):
                    sd, sm = tmp / f"sel{k}{maxv}", tmp / f"selm{k}{maxv}"
                    o = (
                        SelectPEVolumes(
                            in_data=[f, f],
                            pe_dirs_fsl=["y", "y-"],
                            readout_times=[0.03, 0.04],
                            max_vols_per_pe=maxv,
                        )
                        .run(cwd=str(sd))
                        .outputs
                    )
                    m = lxs.select_pe_volumes([f, f], ["y", "y-"], [0.03, 0.04], maxv, out_dir=sm)
                    ref_out = o.out_data if isinstance(o.out_data, list) else [o.out_data]
                    t.add(
                        "SelectPEVolumes outputs = sdcflows'",
                        [Path(a).name for a in ref_out] == [Path(b).name for b in m.out_data],
                        f"{label} {k} {maxv}",
                    )
                    for a, b in zip(ref_out, m.out_data, strict=False):
                        tally_file(t, "SelectPEVolumes files = sdcflows'", f"{label}", a, b)
            # MergeSeries, PadSlices, ReorientImageAndMetadata, ReorientImage on the merged pair
            from niworkflows.interfaces.nibabel import MergeSeries, ReorientImage

            cat = wf / "concat_blips"
            merged = sorted(cat.glob("*_merged.nii.gz"))
            src = [cw.host_path(cat, p) for p in ast.literal_eval(cw.node_inputs(cat)["in_files"])]
            for tol in (1e-4, None):
                rd = tmp / f"ms{tol}"
                kw = {} if tol is None else {"affine_tolerance": tol}
                both(
                    lambda kw=kw, rd=rd, src=src: (
                        Path(MergeSeries(in_files=src, **kw).run(cwd=str(rd)).outputs.out_file).name
                    ),
                    lambda tol=tol, rd=rd, src=src: (
                        Path(
                            merge_series(src, affine_tolerance=tol, out_dir=Path(str(rd) + "m"))
                        ).name
                    ),
                    f"{label} tol {tol}",
                    "MergeSeries outputs = niworkflows'",
                )
                ref_f = sorted(rd.glob("*_merged.nii.gz"))
                if ref_f:
                    tally_file(
                        t,
                        "MergeSeries files = niworkflows'",
                        label,
                        ref_f[0],
                        Path(str(rd) + "m") / ref_f[0].name,
                    )
            if merged:
                for ax in (0, 1, 2):
                    pd, pm = tmp / f"pad{ax}", tmp / f"padm{ax}"
                    o = PadSlices(in_file=str(merged[0]), axis=ax).run(cwd=str(pd)).outputs
                    m = lxs.pad_slices(merged[0], ax, out_dir=pm)
                    t.add(
                        "PadSlices outputs = sdcflows'",
                        (Path(o.out_file).name, bool(o.padded)) == (Path(m[0]).name, m[1]),
                        f"{label} {ax}",
                    )
                    if m[1]:
                        tally_file(
                            t, "PadSlices files = sdcflows'", f"{label} {ax}", o.out_file, m[0]
                        )
                for code in ("LAS", "RAS", "LPI", "PIR", "las"):
                    rd, rm = tmp / f"ro{code}", tmp / f"rom{code}"

                    def ref(code=code, rd=rd, merged=merged):
                        o = (
                            ReorientImageAndMetadata(
                                in_file=str(merged[0]), target_orientation=code, pe_dir=["j", "j-"]
                            )
                            .run(cwd=str(rd))
                            .outputs
                        )
                        return (Path(o.out_file).name, list(o.pe_dir))

                    def mine(code=code, rm=rm, merged=merged):
                        f, p = lxs.reorient_image_and_metadata(
                            merged[0], code, ["j", "j-"], out_dir=rm, upstream_defects="reproduce"
                        )
                        return (Path(f).name, list(p))

                    both(ref, mine, f"{label} {code}", "ReorientImageAndMetadata = sdcflows'")
                    for f in sorted(rd.glob("*.nii.gz")):
                        tally_file(
                            t,
                            "ReorientImageAndMetadata files = sdcflows'",
                            f"{label} {code}",
                            f,
                            rm / f.name,
                        )
                    od, om = tmp / f"rf{code}", tmp / f"rfm{code}"
                    if code.isupper():
                        o = (
                            ReorientImage(in_file=str(merged[0]), target_orientation=code)
                            .run(cwd=str(od))
                            .outputs
                        )
                        m = reorient_file(merged[0], target_ornt=code, newpath=om)
                        tally_file(
                            t,
                            "ReorientImage files = niworkflows'",
                            f"{label} {code}",
                            o.out_file,
                            m,
                        )
                m0 = str(merged[0])
                for sloppy in (False, True):
                    both(lambda m0=m0, sl=sloppy: _select_topup_config(m0, sl).split("/")[-1],
                         lambda m0=m0, sl=sloppy: lxs.select_topup_config(m0, sl, fsldir=fsldir()).split("/")[-1],
                         label, "_select_topup_config = sdcflows'")  # fmt: skip
            mats = sorted((wf / "topup").glob("xfm_*.mat"))
            las = sorted((wf / "setwise_avg").glob("*_sliced_volreg.nii.gz"))
            if mats and las:
                ref = _topup_mats_to_world([str(m) for m in mats], str(las[0]))
                mine = lxs.topup_mats_to_world(mats, las[0])
                t.add(
                    "_topup_mats_to_world = sdcflows'",
                    np.asarray(ref).tobytes() == np.asarray(mine).tobytes(),
                    label,
                )
        # synthetic grids for UniformGrid: equal, near (allclose), shifted, rotated
        base = np.eye(4) * 2.0
        base[3, 3] = 1.0
        rngd = rng.normal(100, 30, (20, 22, 18)).astype(np.float32)
        variants = {
            "equal": base,
            "near": base + 1e-7,
            "shifted": base
            + np.array([[0, 0, 0, 0.7], [0, 0, 0, -0.4], [0, 0, 0, 0.2], [0, 0, 0, 0]]),
        }
        rot = base.copy()
        c, s = np.cos(0.1), np.sin(0.1)
        rot[:3, :3] = np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]]) @ base[:3, :3]
        variants["rotated"] = rot
        for dtype, slope in ((np.float32, None), (np.int16, 0.37), (np.uint8, None)):
            img = nb.Nifti1Image((rngd[:, :, :17] * 0.5).astype(dtype), base)
            if slope is not None:
                img.header.set_slope_inter(slope, 3.0)
            odd = tmp / f"odd_{dtype.__name__}.nii.gz"
            img.to_filename(odd)
            for ax in (0, 1, 2):
                pd, pm = tmp / f"po{ax}{dtype.__name__}", tmp / f"pom{ax}{dtype.__name__}"
                o = PadSlices(in_file=str(odd), axis=ax).run(cwd=str(pd)).outputs
                m = lxs.pad_slices(odd, ax, out_dir=pm)
                t.add(
                    "PadSlices outputs = sdcflows'",
                    (Path(o.out_file).name, bool(o.padded)) == (Path(m[0]).name, m[1]),
                    f"odd {ax}",
                )
                if m[1]:
                    tally_file(
                        t,
                        "PadSlices files = sdcflows'",
                        f"odd {dtype.__name__} {ax}",
                        o.out_file,
                        m[0],
                    )
        for name, aff in variants.items():
            for dtype in (np.float32, np.int16):
                ref_p = tmp / f"g_ref_{name}_{dtype.__name__}.nii.gz"
                mov_p = tmp / f"g_mov_{name}_{dtype.__name__}.nii.gz"
                nb.Nifti1Image(rngd.astype(dtype), base).to_filename(ref_p)
                nb.Nifti1Image((rngd * 1.1).astype(dtype), aff).to_filename(mov_p)
                rd, md = tmp / f"gd_{name}{dtype.__name__}", tmp / f"gm_{name}{dtype.__name__}"
                with warnings.catch_warnings():
                    warnings.simplefilter("ignore")
                    o = UniformGrid(in_data=[str(ref_p), str(mov_p)]).run(cwd=str(rd)).outputs
                    m = lxs.uniform_grid([ref_p, mov_p], out_dir=md, n_threads=nt)
                t.add(
                    "UniformGrid outputs = sdcflows'",
                    [Path(a).name for a in o.out_data] == [Path(b).name for b in m.out_data],
                    name,
                )
                if o.out_data[1] != str(mov_p):
                    tally_file(
                        t,
                        "UniformGrid files = sdcflows'",
                        f"{name} {dtype.__name__}",
                        o.out_data[1],
                        m.out_data[1],
                    )


# ------------------------------------------------------------------------------------------
# RobustAverage


def sec_ravg(t: cw.Tally, nt: int, ests) -> None:
    from niworkflows.interfaces.images import RobustAverage

    from larmorx.niworkflows import robust_average

    def run_pair(label, in_file, kw, tmp):
        rd, md = tmp / "ref", tmp / "mine"
        rd.mkdir(exist_ok=True)
        try:
            o = RobustAverage(in_file=in_file, **kw).run(cwd=str(rd)).outputs
            ref = (
                "ok",
                Path(o.out_file).name if o.out_file else None,
                [round(x, 12) for x in (o.out_drift or [])],
            )
        except Exception as e:
            ref = ("err", type(e).__name__, str(e))
        try:
            m = robust_average(
                in_file,
                out_dir=md,
                numpy_version=NP,
                upstream_defects="reproduce",
                n_threads=nt,
                **kw,
            )
            mine = (
                "ok",
                Path(m.out_file).name if m.out_file else None,
                [round(x, 12) for x in m.out_drift],
            )
        except Exception as e:
            mine = ("err", type(e).__name__, str(e))
        t.add("RobustAverage outputs = niworkflows'", ref == mine, f"{label}: {ref} vs {mine}")
        t.add(f"RobustAverage {ref[0]} cases", True)
        for f in sorted(rd.glob("*.nii*")):
            tally_file(t, "RobustAverage files = niworkflows'", f"{label} {kw}", f, md / f.name)

    for label, _wf, files, _metas in ests:
        for k, f in enumerate(files):
            n = nb.load(f).shape[3] if nb.load(f).ndim == 4 else 1
            opts = [{}, {"num_threads": 4}]
            if k == 0:
                opts += [
                    {"two_pass": False},
                    {"nonnegative": False},
                    {"mc_method": None},
                    {"t_mask": [True] + [False] * (n - 1)},
                    {"t_mask": [i % 2 == 0 for i in range(n)]},
                    {"t_mask": [False] * n},
                    {"t_mask": [True] * (n + 1)},
                ]
            for kw in opts:
                with tempfile.TemporaryDirectory() as tmp:
                    run_pair(f"{label} run {k}", f, kw, Path(tmp))
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        rng = np.random.default_rng(3)
        three = tmp / "three.nii.gz"
        nb.Nifti1Image(rng.normal(100, 10, (20, 20, 10)).astype(np.float32), np.eye(4)).to_filename(
            three
        )
        one = tmp / "one.nii.gz"
        nb.Nifti1Image(
            rng.normal(100, 10, (20, 20, 10, 1)).astype(np.int16), np.eye(4)
        ).to_filename(one)
        for f in (three, one):
            with tempfile.TemporaryDirectory() as sub:
                run_pair(f.name, str(f), {}, Path(sub))


# ------------------------------------------------------------------------------------------
# the workflows


def sec_wf(t: cw.Tally, nt: int, ests, times) -> None:
    from sdcflows.workflows.fit.pepolar import init_topup_wf as wf217

    from larmorx.sdcflows import topup_workflow

    wf215 = topup_215()
    variants = [("2.15", {}), ("2.17", {})]
    for i, (label, _wf, files, metas) in enumerate(ests):
        vs = variants + ([("2.17", {"debug": True}), ("2.17", {"sloppy": True})] if i == 0 else [])
        for wiring, kw in vs:
            with tempfile.TemporaryDirectory() as tmp:
                t0 = time.perf_counter()
                wf = (wf215 if wiring == "2.15" else wf217)(omp_nthreads=1, **kw)
                wf.inputs.inputnode.in_data = files
                wf.inputs.inputnode.metadata = metas
                ref = cw.run_nipype(wf, Path(tmp) / "ref")
                t1 = time.perf_counter()
                mine = Path(tmp) / "mine"
                what = f"topup_workflow {wiring}{''.join(f' {k}' for k in kw)}"
                try:
                    with warnings.catch_warnings():
                        warnings.simplefilter("ignore")
                        res = topup_workflow(
                        files, metas, wiring=wiring, out_dir=mine, fsldir=fsldir(),
                        numpy_version=NP, scipy_version=SP, topup_implementation="replica",
                        volreg_implementation="replica", upstream_defects="reproduce",
                        n_threads=nt, **kw,
                    )  # fmt: skip
                except Exception as e:
                    import traceback

                    traceback.print_exc()
                    t.add(f"{what}: ran", False, f"{label}: {type(e).__name__}: {e}")
                    continue
                times[0] += t1 - t0
                times[1] += time.perf_counter() - t1
                compare_tree(t, label, ref, mine, what)
                outn = wf.get_node("outputnode")
                t.add(f"{what}: method", res.method == outn.inputs.method, label)
                print(
                    f"INFO {label} {what}: config {res.topup_config or res.nodes['topup_config']}",
                    flush=True,
                )


# ------------------------------------------------------------------------------------------
# fMRIPrep's own node files


def sec_image(t: cw.Tally, nt: int, ests) -> None:
    from larmorx import sdcflows as lxs
    from larmorx.niworkflows import merge_series, reorient_file, robust_average

    for label, wf, files, _metas in ests:
        if not (wf / "brainextraction_wf" / "masker" / "result_masker.pklz").exists():
            print(f"INFO {label}: fMRIPrep's estimation not finished; skipped", flush=True)
            continue
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            ravgs = sorted((wf / "runwise_avg" / "mapflow").glob("_runwise_avg*"))
            for k, (node, f) in enumerate(zip(ravgs, files, strict=True)):
                m = robust_average(f, num_threads=8, out_dir=tmp / node.name, numpy_version="2.2.6",
                                   volreg_implementation="replica", upstream_defects="reproduce", n_threads=nt)  # fmt: skip
                for ref in sorted(node.glob("*_average.nii.gz")):
                    tally_file(
                        t, "image: runwise_avg = fMRIPrep's", f"{label} {k}", ref, m.out_file
                    )
            # regrid on fMRIPrep's own averages
            rg = wf / "regrid"
            avgs = [cw.host_path(rg, p) for p in ast.literal_eval(cw.node_inputs(rg)["in_data"])]
            ug = lxs.uniform_grid(avgs, 0, out_dir=tmp / "regrid", n_threads=nt)
            for ref in sorted((wf / "regrid").glob("*.nii.gz")):
                tally_file(
                    t,
                    "image: regrid = fMRIPrep's",
                    label,
                    ref,
                    tmp / "regrid" / ref.name,
                    quaternion_ok=True,
                )
            cat = wf / "concat_blips"
            src = [cw.host_path(cat, p) for p in ast.literal_eval(cw.node_inputs(cat)["in_files"])]
            merged = merge_series(src, affine_tolerance=1e-4, out_dir=tmp / "concat_blips")
            for ref in sorted(cat.glob("*_merged.nii.gz")):
                tally_file(t, "image: concat_blips = fMRIPrep's", label, ref, merged)
            pad = wf / "pad_blip_slices"
            pad_in = cw.host_path(pad, cw.node_inputs(pad)["in_file"])
            padded, _ = lxs.pad_slices(pad_in, out_dir=tmp / "pad_blip_slices")
            for ref in sorted(pad.glob("*_padded.nii.gz")):
                tally_file(t, "image: pad_blip_slices = fMRIPrep's", label, ref, padded)
            sw = wf / "setwise_avg"
            sw_in = cw.host_path(sw, cw.node_inputs(sw)["in_file"])
            s = robust_average(sw_in, num_threads=8, out_dir=tmp / "setwise_avg", numpy_version="2.2.6",
                               volreg_implementation="replica", upstream_defects="reproduce", n_threads=nt)  # fmt: skip
            for ref in sorted(sw.glob("*.nii.gz")):
                tally_file(
                    t, "image: setwise_avg = fMRIPrep's", label, ref, tmp / "setwise_avg" / ref.name
                )
            del s, ug
            # topup: an older FSL in fMRIPrep's image (INFO); later nodes from fMRIPrep's own topup
            tu = wf / "topup"
            for ref in sorted(tu.glob("*_fieldcoef.nii.gz")):
                fix_ref = sorted((wf / "fix_coeff").glob("*_fixed.nii.gz"))
                fx = cw.node_inputs(wf / "fix_coeff")
                fmap_ref = cw.host_path(wf / "fix_coeff", fx["fmap_ref"])
                pe = fx["pe_dir"].strip("'\"")
                res = lxs.topup_coeff_reorient(
                    ref, fmap_ref, pe, out_dir=tmp / "fix_coeff", upstream_defects="reproduce"
                )
                if fix_ref:
                    tally_file(
                        t,
                        "image: fix_coeff (fMRIPrep's topup) = fMRIPrep's",
                        label,
                        fix_ref[0],
                        res.out_coeff[0],
                    )
            for node in ("from_las", "from_las_fmap"):
                n = wf / node
                ni = cw.node_inputs(n)
                out = reorient_file(
                    cw.host_path(n, ni["in_file"]),
                    target_file=cw.host_path(n, ni["target_file"]),
                    newpath=tmp / node,
                )
                tally_file(
                    t,
                    f"image: {node} (fMRIPrep's topup) = fMRIPrep's",
                    label,
                    n / "reoriented.nii.gz",
                    out,
                )
            ra = wf / "ref_average"
            r = robust_average(cw.host_path(ra, cw.node_inputs(ra)["in_file"]), num_threads=8, out_dir=tmp / "ref_average",
                               numpy_version="2.2.6", volreg_implementation="replica", upstream_defects="reproduce", n_threads=nt)  # fmt: skip
            for ref in sorted(ra.glob("*_average.nii.gz")):
                tally_file(t, "image: ref_average = fMRIPrep's", label, ref, r.out_file)
            # topup itself (INFO): larmorx's replica (FSL 6.0.7.23) on fMRIPrep's own topup input
            ti = cw.node_inputs(tu)
            las = cw.host_path(tu, ti["in_file"])
            ros = ast.literal_eval(ti["readout_times"])
            dirs = ast.literal_eval(ti["encoding_direction"])
            cfg = lxs.topup_config_file("b02b0.cnf")
            mine_tu = lxs.topup_node(
                las,
                dirs,
                ros,
                cfg,
                out_dir=tmp / "topup",
                implementation="replica",
                upstream_defects="reproduce",
                n_threads=nt,
            )
            # FSL 6.0.7.23's own topup on the same input (through the oracle cache: the topup
            # stand-in on PATH, the node's command line), against the replica: identical
            fsl_dir = tmp / "topup-fsl"
            fsl_dir.mkdir()
            import shutil
            import subprocess

            shutil.copy(mine_tu.out_enc_file, fsl_dir / Path(mine_tu.out_enc_file).name)
            argv = [a.replace(str(tmp / "topup"), str(fsl_dir)) for a in mine_tu.cmdline]
            subprocess.run(
                argv,
                cwd=fsl_dir,
                check=True,
                capture_output=True,
                env={**os.environ, "FSLOUTPUTTYPE": "NIFTI_GZ"},
            )
            for f in sorted(fsl_dir.glob("*.nii.gz")):
                tally_file(
                    t,
                    "image: topup replica = FSL 6.0.7.23 on fMRIPrep's input",
                    label,
                    f,
                    tmp / "topup" / f.name,
                )
            for f in sorted(fsl_dir.glob("xfm_*.mat")):
                t.add(
                    "image: topup replica matrices = FSL 6.0.7.23's",
                    f.read_text() == (tmp / "topup" / f.name).read_text(),
                    f"{label} {f.name}",
                )
            # fMRIPrep ran an older FSL (its image): its files are FSL 6.0.7.23's or not (INFO)
            same = [
                compare(ref, tmp / "topup" / ref.name).startswith("same")
                for ref in sorted(tu.glob("*.nii.gz"))
            ]
            mats = [
                ref.read_text() == (tmp / "topup" / ref.name).read_text()
                for ref in sorted(tu.glob("xfm_*.mat"))
            ]
            print(
                f"INFO {label} topup, FSL 6.0.7.23 vs fMRIPrep's older FSL: {sum(same)}/{len(same)} images "
                f"identical (descrip apart), {sum(mats)}/{len(mats)} matrices",
                flush=True,
            )
            for ref in sorted(tu.glob("*_field.nii.gz")):
                print(
                    f"INFO {label} topup field: {cw.info_diff(str(ref), str(tmp / 'topup' / ref.name))}",
                    flush=True,
                )
            cmd = (tu / "command.txt").read_text().split()

            def flags(argv):
                return [(a.partition("=")[0], Path(a.partition("=")[2]).name) for a in argv]

            t.add("image: topup command line = fMRIPrep's (directories apart)",
                  flags(cmd) == flags(mine_tu.cmdline), f"{label}: {cmd} vs {mine_tu.cmdline}")  # fmt: skip


def _patch_nipype_cwd() -> None:
    """Let an interface's ``run(cwd=...)`` make its directory (nipype's needs it to exist)."""
    import contextlib

    import nipype.interfaces.base.core as core

    real = core.indirectory

    @contextlib.contextmanager
    def indirectory(path):
        Path(path).mkdir(parents=True, exist_ok=True)
        with real(path):
            yield

    core.indirectory = indirectory


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="units,ravg,wf,image")
    ap.add_argument("--runs", default="")
    ap.add_argument("--threads", type=int, default=4)
    args = ap.parse_args()
    from larmorx.io.nibabel_header import use_lapack

    sys.path.insert(0, str(HERE.parent / "sdcflows-fieldmap"))
    from stepdigest import numpy_lapack

    use_lapack(numpy_lapack())
    os.environ.setdefault("FSLDIR", fsldir())
    _patch_nipype_cwd()
    print(f"numpy {NP}, SciPy {SP}", flush=True)
    t = cw.Tally()
    secs = args.sections.split(",")
    ests = estimators([r for r in args.runs.split(",") if r] or None)
    print(f"estimators: {[e[0] for e in ests]}", flush=True)
    shims = Path(tempfile.mkdtemp(prefix="lx-shims-"))
    install_shims(shims)
    times = [0.0, 0.0]
    t0 = time.perf_counter()
    if "units" in secs:
        sec_units(t, args.threads, ests)
    if "ravg" in secs:
        sec_ravg(t, args.threads, ests)
    if "wf" in secs:
        sec_wf(t, args.threads, ests, times)
    if "image" in secs:
        sec_image(t, args.threads, ests)
    bad = t.report()
    print(
        f"sdcflows' graphs {times[0]:.1f} s (programs through the oracle cache), larmorx "
        f"{times[1]:.1f} s ({args.threads} threads); total {time.perf_counter() - t0:.1f} s"
    )
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
