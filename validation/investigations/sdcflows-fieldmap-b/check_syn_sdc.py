# SPDX-License-Identifier: Apache-2.0
"""sdcflows' fieldmap-less estimation (``init_syn_sdc_wf``, larmorx's
``lx.sdcflows.syn_sdc_workflow``) against fMRIPrep 25.2.6's own node files of its
``--use-syn-sdc`` runs, against ANTs 2.6.5 on one thread, and against sdcflows' own graph.

Run in each kept environment (``PYTHONPATH=<repo>/python:<repo>/validation/src``,
``LARMORX_WORKSPACE``; boxed):

    OPENBLAS_CORETYPE=Haswell OPENBLAS_NUM_THREADS=1 python -P check_syn_sdc.py \\
        [--sections nodes,syn,workflow] [--root <folder of runs>] [--runs a,b] \\
        [--syn-runs <run/wf_x,...>] [--threads N]

- ``nodes``: every node of every ANAT estimator's workflow (``fmap_preproc_wf/wf_*`` with a
  ``syn`` node), fed fMRIPrep's own inputs (its report's "Original Inputs"; container paths
  mapped), against its files and values: ``readout_time``, ``clip_epi``, ``lap_anat`` and
  ``lap_epi`` (ANTs' ``ImageMath Laplacian``, nipype's ``copy_header``; their command lines),
  ``lap_anat_norm``, ``lap_epi_norm``, ``anat_dilmsk``, ``amask2epi``, ``epi_umask``,
  ``find_zooms``, ``warp_dir``, ``vox_params``, ``syn`` (its command line from larmorx's
  values), ``extract_field``, ``zooms_field``, ``zooms_bmask``, ``bs_filter`` and ``unwarp``
  (each fed fMRIPrep's file before it: the registration ran on fMRIPrep's 4 threads, on which
  it depends). fMRIPrep 25.2.6's image's numpy (2.2.6) for the clip's percentiles;
  ``upstream_defects="reproduce"``. ``zooms_epi`` is not computed (SDC-15): its value only.
- ``syn``: the registration of the estimators named by ``--syn-runs`` (default: the first)
  on fMRIPrep's inputs, larmorx (``--threads``) against ANTs 2.6.5 (the GCC 14.3 oracle, one
  thread, correctly rounded maths, the run's seed; through the oracle cache): the forward and
  inverse warps byte for byte; fMRIPrep's own warp (4 threads, glibc) reported.
- ``workflow``: the whole function on the ``--syn-runs`` estimators' inputs against sdcflows'
  own ``init_syn_sdc_wf`` run by nipype (this environment's sdcflows, niworkflows and numpy;
  ANTs' programs at one thread with correctly rounded maths as stand-ins through the oracle
  cache): every NIfTI file of the graph's folders (``zooms_epi``'s: SDC-15, counted apart),
  and the outputs' values.

Prints one line per check, INFO lines and ``ALL OK`` or ``FAIL <n>``.
"""

from __future__ import annotations

import argparse
import ast
import json
import os
import sys
import tempfile
import time
import warnings
from pathlib import Path

os.environ.setdefault("FSLOUTPUTTYPE", "NIFTI_GZ")

import nibabel as nb
import numpy as np

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import check_bold_fmap as cbf  # noqa: E402
import check_syn_preprocessing as csp  # noqa: E402
import check_workflows as cw  # noqa: E402

IMAGE_NUMPY = "2.2.6"
NIWF_IMAGE = csp.NIWF_IMAGE
IMAGEMATH_BODY = """from larmorx_validation.parity.ants_programs import _run_ants
code, out, err = _run_ants("ImageMath", sys.argv[1:], threads=1, correctly_rounded=True)
sys.stdout.write(out)
sys.stderr.write(err)
sys.exit(code)
"""


def install_shims(folder: Path) -> None:
    """relay 9's stand-ins (ANTs' registration, ``antsApplyTransforms``, ``DenoiseImage``, N4,
    ...) and ANTs' ``ImageMath`` (one thread, correctly rounded maths, the oracle cache)."""
    import stat

    csp.install_shims(folder)
    paths = [p for p in sys.path if p and ("validation/src" in p or p.endswith("/python"))]
    p = folder / "ImageMath"
    p.write_text(
        cw.SHIM.format(python=sys.executable, name="ImageMath", paths=paths, body=IMAGEMATH_BODY)
    )
    p.chmod(p.stat().st_mode | stat.S_IXUSR)


def estimators(root: Path, runs: list[str] | None) -> list[Path]:
    out = []
    for run in sorted(root.iterdir()):
        if not run.name.endswith("-syn") or (runs and run.name not in runs):
            continue
        out += sorted(run.glob("work/fmriprep_25_2_wf/sub_*_wf/fmap_preproc_wf/wf_*"))
    return [w for w in out if (w / "unwarp" / "result_unwarp.pklz").exists()]


def label(wf: Path) -> str:
    return f"{cw.run_root(wf).name}/{wf.name}"


def out_value(node: Path, key: str):
    """A value of the node's "Execution Outputs" (numpy 2's scalar reprs read back)."""
    text = (node / "_report" / "report.rst").read_text(encoding="utf-8")
    section = text.split("Execution Outputs")[-1].split("Runtime info")[0]
    import re

    v = dict(re.findall(r"^\* (\w+) : (.*)$", section, flags=re.M))[key]
    try:
        return eval(v, {"__builtins__": {}}, {"np": np})
    except (NameError, SyntaxError):
        return v


def wf_inputs(wf: Path) -> dict:
    """The estimation workflow's inputs fMRIPrep gave (host paths)."""
    meta = ast.literal_eval(csp.inputs(wf / "readout_time")["metadata"])
    masks = csp.hosts(wf / "syn", "fixed_image_masks")
    return {
        "epi_ref": (csp.hosts(wf / "clip_epi", "in_file"), meta),
        "epi_mask": csp.hosts(wf / "amask2epi", "reference_image"),
        "anat_ref": csp.hosts(wf / "lap_anat", "op1"),
        "anat_mask": masks[0],
        "sd_prior": masks[1],
    }


def sec_nodes(t: cw.Tally, wfs: list[Path], nt: int, times: list[float]) -> None:
    from larmorx import _nipype
    from larmorx import niworkflows as nw
    from larmorx import sdcflows as sd
    from larmorx.sdcflows.syn import _apply_transforms, _laplacian

    msg = f"FixHeaderApplyTransforms (niworkflows v{NIWF_IMAGE})"
    for wf in wfs:
        lab = label(wf)
        t0 = time.perf_counter()
        inp = wf_inputs(wf)
        epi_file, meta = inp["epi_ref"]
        with tempfile.TemporaryDirectory() as tmp:
            tmpd = Path(tmp)
            n = wf / "readout_time"
            ro = sd.get_readout_time(meta, epi_file)
            t.add(
                "readout_time: readout_time, pe_direction",
                ro.readout_time == out_value(n, "readout_time")
                and ro.pe_direction == out_value(n, "pe_direction"),
                f"{lab}: {ro.readout_time} {ro.pe_direction}",
            )
            n = wf / "clip_epi"
            res = nw.intensity_clip(
                epi_file,
                p_min=35.0,
                p_max=99.9,
                out_dir=tmpd / "clip_epi",
                numpy_version=IMAGE_NUMPY,
                n_threads=nt,
            )
            csp.tally(
                t, "clip_epi (IntensityClip 35, 99.9)", lab, n / "clipped.nii.gz", res.out_file
            )
            for name in ("lap_anat", "lap_epi"):
                n = wf / name
                cwd = tmpd / name
                cwd.mkdir()
                mine, cmd = _laplacian(csp.hosts(n, "op1"), str(cwd), "reproduce", nt)
                ref = next(n.glob("*_maths.nii*"))
                csp.tally(t, f"{name} (ImageMath Laplacian 1.5 1, copy_header)", lab, ref, mine)
                csp._cmd_same(t, f"{name}: command line", lab, n, cmd)
                nn = wf / f"{name}_norm"
                mine = sd.norm_lap(
                    csp.hosts(nn, "in_file"),
                    out_dir=tmpd / f"{name}_norm",
                    upstream_defects="reproduce",
                )
                csp.tally(t, f"{name}_norm (_norm_lap)", lab, next(nn.glob("*_norm.nii*")), mine)
            n = wf / "anat_dilmsk"
            _, mine = sd.binary_dilation(csp.hosts(n, "in_file"), out_dir=tmpd / "anat_dilmsk")
            csp.tally(t, "anat_dilmsk", lab, next(n.glob("*_dil.nii*")), mine)
            n = wf / "amask2epi"
            (tmpd / "amask2epi").mkdir()
            mine, cmd = _apply_transforms(
                str(tmpd / "amask2epi"),
                csp.hosts(n, "input_image"),
                csp.hosts(n, "reference_image"),
                ["identity"],
                None,
                "MultiLabel",
                msg,
                "reproduce",
                nt,
            )
            csp.tally(
                t, "amask2epi (MultiLabel, identity)", lab, next(n.glob("*_trans.nii*")), mine
            )
            csp._cmd_same(t, "amask2epi: command line", lab, n, cmd)
            n = wf / "epi_umask"
            _, mine = sd.union(csp.hosts(n, "in1"), csp.hosts(n, "in2"), out_dir=tmpd / "epi_umask")
            csp.tally(t, "epi_umask (Union)", lab, next(n.glob("*_union.nii*")), mine)
            n = wf / "find_zooms"
            z = sd.adjust_zooms(csp.hosts(n, "in_anat"), csp.hosts(n, "in_epi"))
            ref_z = out_value(n, "out")
            t.add(
                "find_zooms (_adjust_zooms: values and types)",
                [repr(v) for v in z] == [repr(v) for v in ref_z],
                f"{lab}: {z} vs {ref_z}",
            )
            zin = csp.value(wf / "zooms_epi", "zooms")
            t.add(
                "zooms_epi's zooms (its file not computed: SDC-15)",
                [float(v) for v in z] == [float(v) for v in zin],
                lab,
            )
            n = wf / "warp_dir"
            rd = sd.warp_dir(
                csp.hosts(n, "moving_image"), csp.hosts(n, "fixed_image"), ro.pe_direction
            )
            t.add("warp_dir (_warp_dir)", rd == out_value(n, "out"), f"{lab}: {rd}")
            n = wf / "vox_params"
            cfg = json.loads(sd.registration_settings("sd_syn.json").read_text(encoding="utf-8"))
            vp = sd.mm2vox(
                csp.hosts(n, "moving_image"), csp.hosts(n, "fixed_image"), ro.pe_direction, cfg
            )
            ref_vp = out_value(n, "out")
            t.add(
                "vox_params (_mm2vox: values and types)",
                repr(vp) == repr(ref_vp),
                f"{lab}: {vp!r} vs {ref_vp!r}",
            )
            # syn: nipype's line from larmorx's values
            n = wf / "syn"
            reg = _nipype.registration(
                cwd=tmpd / "syn",
                from_file=sd.registration_settings("sd_syn.json"),
                output_warped_image=False,
                output_inverse_warped_image=False,
                restrict_deformation=rd,
                fixed_image=csp.hosts(n, "fixed_image"),
                fixed_image_masks=csp.hosts(n, "fixed_image_masks"),
                moving_image=csp.hosts(n, "moving_image"),
                moving_image_masks=csp.hosts(n, "moving_image_masks"),
                transform_parameters=vp,
                num_threads=4,
            )
            csp._cmd_same(t, "syn: command line", lab, n, reg.cmdline)
            # after the registration: fMRIPrep's warp
            n = wf / "extract_field"
            res = sd.displacements_field_to_fieldmap(
                csp.hosts(n, "transform"),
                csp.hosts(n, "epi"),
                csp.value(n, "ro_time"),
                csp.value(n, "pe_dir"),
                out_dir=tmpd / "extract_field",
            )
            csp.tally(
                t,
                "extract_field (DisplacementsField2Fieldmap)",
                lab,
                next(n.glob("*_Hz.nii*")),
                res.out_file,
            )
            for name, interp, args in (
                ("zooms_field", "BSpline", "-u float"),
                ("zooms_bmask", "MultiLabel", "-u uchar"),
            ):
                n = wf / name
                cwd = tmpd / name
                cwd.mkdir()
                mine, cmd = _apply_transforms(
                    str(cwd),
                    csp.hosts(n, "input_image"),
                    csp.hosts(n, "reference_image"),
                    ["identity"],
                    None,
                    interp,
                    msg,
                    "reproduce",
                    nt,
                    args=args,
                )
                csp.tally(
                    t,
                    f"{name} ({interp}, identity, {args})",
                    lab,
                    next(n.glob("*_trans.nii*")),
                    mine,
                )
                csp._cmd_same(t, f"{name}: command line", lab, n, cmd)
            n = wf / "bs_filter"
            fit = sd.bspline_approx(
                csp.hosts(n, "in_data"),
                None,
                bs_spacing=csp.value(n, "bs_spacing"),
                recenter=False,
                extrapolate=True,
                out_dir=tmpd / "bs_filter",
                n_threads=nt,
                upstream_defects="reproduce",
            )
            for f in sorted(n.glob("*.nii*")):
                csp.tally(
                    t,
                    "bs_filter (BSplineApprox, recenter=False)",
                    f"{lab}: {f.name}",
                    f,
                    tmpd / "bs_filter" / f.name,
                )
            n = wf / "unwarp"
            res = sd.apply_coeffs_field(
                csp.hosts(n, "in_data"),
                csp.hosts(n, "in_coeff"),
                csp.value(n, "ro_time"),
                csp.value(n, "pe_dir"),
                jacobian=False,
                out_dir=tmpd / "unwarp",
                n_threads=nt,
                upstream_defects="reproduce",
            )
            for suffix, mine in (("_sdc", res.out_corrected), ("_field", res.out_field)):
                csp.tally(
                    t,
                    f"unwarp {suffix[1:]} (ApplyCoeffsField)",
                    lab,
                    next(n.glob(f"*{suffix}.nii*")),
                    mine,
                )
            del fit
        times.append(time.perf_counter() - t0)


def syn_argv(wf: Path) -> tuple[list[str], int]:
    """fMRIPrep's ``syn`` line (its ``command.txt``) with this machine's paths, and its seed."""
    import shlex

    n = wf / "syn"
    run = cw.run_root(n)
    cmd = (n / "command.txt").read_text(encoding="utf-8").strip()
    cmd = cmd.replace("/work/", str(run / "work") + "/")
    return shlex.split(cmd)[1:], cbf.ants_seed(n)


def sec_syn(t: cw.Tally, wfs: list[Path], nt: int, times: list[float]) -> None:
    from larmorx_validation.parity.ants_programs import _run_ants

    from larmorx import _core

    for wf in wfs:
        lab = label(wf)
        argv, seed = syn_argv(wf)
        here = os.getcwd()
        with tempfile.TemporaryDirectory() as tmp:
            mine_dir, ora_dir = Path(tmp) / "lx", Path(tmp) / "ants"
            mine_dir.mkdir()
            ora_dir.mkdir()
            try:
                os.chdir(mine_dir)
                t0 = time.perf_counter()
                code, _o, err, _ = _core.ants_program(
                    "antsRegistration", [*argv, "--random-seed", str(seed)], {}, [], nt, "reproduce"
                )
                t1 = time.perf_counter()
                os.chdir(ora_dir)
                os.environ["ANTS_RANDOM_SEED"] = str(seed)
                ocode, _o2, oerr = _run_ants(
                    "antsRegistration", argv, threads=1, correctly_rounded=True
                )
                t2 = time.perf_counter()
            finally:
                os.chdir(here)
                os.environ.pop("ANTS_RANDOM_SEED", None)
            times.append(t1 - t0)
            print(
                f"INFO {lab}: larmorx {t1 - t0:.1f} s ({nt} threads), ANTs 2.6.5 {t2 - t1:.1f} s (one thread; 0 if cached)"
            )
            t.add("syn: both ran", code == 0 and ocode == 0, f"{lab}: {err[-300:]} | {oerr[-300:]}")
            for f in ("fmap_syn0Warp.nii.gz", "fmap_syn0InverseWarp.nii.gz"):
                a, b = ora_dir / f, mine_dir / f
                ok = (
                    a.exists()
                    and b.exists()
                    and (
                        np.asanyarray(nb.load(a).dataobj).tobytes()
                        == np.asanyarray(nb.load(b).dataobj).tobytes()
                        and csp.raw_header(a) == csp.raw_header(b)
                    )
                )
                t.add(
                    "syn = ANTs 2.6.5 (one thread, correctly rounded maths, the run's seed)",
                    ok,
                    f"{lab}: {f}",
                )
            a = np.asanyarray(nb.load(wf / "syn" / "fmap_syn0Warp.nii.gz").dataobj)
            b = np.asanyarray(nb.load(mine_dir / "fmap_syn0Warp.nii.gz").dataobj)
            print(
                f"INFO {lab}: the warp against fMRIPrep's (its 4 threads, glibc): max |d| "
                f"{np.max(np.abs(a - b)):.3g} mm, {np.sum(a == b)}/{a.size} equal"
            )


def sec_workflow(t: cw.Tally, wfs: list[Path], nt: int, times: list[float]) -> None:
    import niworkflows
    from sdcflows.workflows.fit.syn import init_syn_sdc_wf

    from larmorx import sdcflows as sd

    with tempfile.TemporaryDirectory() as shims:
        install_shims(Path(shims))
        for wf in wfs:
            lab = label(wf)
            inp = wf_inputs(wf)
            seed = cbf.ants_seed(wf / "syn")
            os.environ["ANTS_RANDOM_SEED"] = str(seed)
            with tempfile.TemporaryDirectory() as tmp:
                g = init_syn_sdc_wf(omp_nthreads=1, debug=False, sloppy=False, name=wf.name)
                for k, v in inp.items():
                    setattr(g.inputs.inputnode, k, v)
                t0 = time.perf_counter()
                ref_root = cw.run_nipype(g, Path(tmp) / "nipype")
                t1 = time.perf_counter()
                res = sd.syn_sdc_workflow(
                    **inp,
                    out_dir=Path(tmp) / "lx",
                    name=wf.name,
                    omp_nthreads=nt,
                    ants_random_seed=seed,
                    niworkflows_version=niworkflows.__version__,
                    numpy_version=np.__version__,
                    upstream_defects="reproduce",
                    n_threads=nt,
                )
                times.append(time.perf_counter() - t1)
                print(
                    f"INFO {lab}: sdcflows' graph {t1 - t0:.1f} s (one thread), larmorx {times[-1]:.1f} s ({nt} threads)"
                )
                mine_root = Path(tmp) / "lx" / wf.name
                for f in sorted(ref_root.rglob("*.nii*")):
                    rel = f.relative_to(ref_root)
                    if any(p.startswith("_report") or p == "crash" for p in rel.parts):
                        continue
                    if rel.parts[0] == "zooms_epi":
                        t.add(
                            "workflow: zooms_epi's file (not computed: SDC-15)",
                            not (mine_root / rel).exists(),
                            f"{lab}: {rel}",
                        )
                        continue
                    csp.tally(
                        t,
                        "workflow: node files = sdcflows' graph's",
                        f"{lab}: {rel}",
                        f,
                        mine_root / rel,
                    )
                t.add("workflow: method", res.method == 'FLB ("fieldmap-less", SyN-based)', lab)
                t.add(
                    "workflow: outputs' files",
                    Path(res.fmap).relative_to(mine_root).parts[0] == "unwarp"
                    and Path(res.fmap_mask).relative_to(mine_root).parts[0] == "zooms_bmask"
                    and len(res.fmap_coeff) == 1,
                    lab,
                )
            os.environ.pop("ANTS_RANDOM_SEED", None)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--sections", default="nodes")
    ap.add_argument("--root", default=str(cw.RUNS))
    ap.add_argument("--runs", default="")
    ap.add_argument("--syn-runs", default="")
    ap.add_argument("--threads", type=int, default=6)
    a = ap.parse_args()
    runs = [r for r in a.runs.split(",") if r] or None
    wfs = estimators(Path(a.root), runs)
    print(f"numpy {np.__version__}; SyN estimators: {[label(w) for w in wfs]}")
    picked = [w for w in wfs if label(w) in a.syn_runs.split(",")] or wfs[:1]
    t = cw.Tally()
    times: dict[str, list[float]] = {"nodes": [], "syn": [], "workflow": []}
    sections = a.sections.split(",")
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        if "nodes" in sections:
            sec_nodes(t, wfs, a.threads, times["nodes"])
        if "syn" in sections:
            sec_syn(t, picked, a.threads, times["syn"])
        if "workflow" in sections:
            sec_workflow(t, picked, a.threads, times["workflow"])
    bad = t.report()
    for k, v in times.items():
        if v:
            print(f"TIME {k}: " + ", ".join(f"{x:.1f} s" for x in v))
    print("ALL OK" if bad == 0 else f"FAIL {bad}")
    return 0 if bad == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
