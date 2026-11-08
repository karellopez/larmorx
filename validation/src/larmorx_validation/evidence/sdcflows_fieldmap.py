# SPDX-License-Identifier: Apache-2.0
"""The evidence of ``lx.sdcflows``, the port of sdcflows 2.17.1's field-map estimation (field
maps, part A), against sdcflows itself run in-process and fMRIPrep 25.2.6's own files.

- **fMRIPrep's phase-difference estimators** of the oracle runs (``fmap_preproc_wf/<est>``),
  step by step on the run's own inputs: ``PhaseMap2rads`` and ``SubtractPhases`` (the phase
  difference in radians PRELUDE is given), ``Phasediff2Fieldmap`` of fMRIPrep's own PRELUDE
  output (the field map in Hz), ``BSplineApprox`` of fMRIPrep's own field map with its brain
  mask and parameters (``bs_filter``: the fitted, extrapolated field, the fitting error and the
  B-spline coefficients), by sdcflows 2.17.1 with the newest numerical releases (larmorx's
  reference) and with the shared environment's (the originals' own step), and by larmorx at 1
  and 14 threads in its reproduce and default modes; larmorx's whole ``phdiff_workflow`` with
  ``lx.fsl.prelude``'s clean-room original beside fMRIPrep's own field map. Three are shown
  (ds005454, ds002603, ds002422's first), all seven in a table.
- **The coefficients** of ds005454's fit on their control grid, fMRIPrep's own file beside.
- **The unwarping**: ``ApplyCoeffsField`` on ds005454's BOLD series (its first volumes) with
  fMRIPrep's coefficients and registration.
- **SDC-10, corrected by default**: ``B0FieldTransform.fit`` with ``approx`` on ds002603's
  oblique BOLD references with fMRIPrep's registration, against the exact field where the
  transform puts the target; **SDC-06**: ``BSplineApprox`` on fMRIPrep's own 4D-of-one field
  maps (a table).
- The record's cases, the originals' own step (LAPACK 3.12.0 -> 3.12.1), random testing's
  trial, the memory peaks (the memory rule's calls) and the benchmark.

Every sdcflows and larmorx run is a separate process of
``larmorx_validation.evidence.sdcflows_fieldmap_run`` in a kept Python environment
(``parity.python_versions``), numpy's OpenBLAS on its Haswell kernels at one thread, as
fMRIPrep's nodes run it. Every real image is shown inside the brain mask of the step that used
it, dilated by two voxels (one for the BOLD series): no face can show.

    python -m larmorx_validation.evidence.sdcflows_fieldmap   (computes the cache entries)
"""

from __future__ import annotations

import copy
import hashlib
import json
import re
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "sdcflows-fieldmap"
RECORD = "sdcflows-fieldmap"
#: Random testing's trial of this part (``<workspace>/oracles/difftest/<trial>/summary.json``).
TRIAL = "2026-11-08-sdcflows-fieldmap"
FINDINGS = "docs/findings/sdcflows-fieldmap.md"
WORK = "work/fmriprep_25_2_wf"
#: fMRIPrep's phase-difference estimators: key, oracle run, the estimator's workflow, what it
#: is. The first three are shown as examples; every one is in the estimators' table.
ESTIMATORS = [
    ("ds005454", "ds005454-sub-16-noreconall", "sub_16_wf/fmap_preproc_wf/wf_gre16",
     "a GRE phase difference at 7 T (Siemens), 112 × 112 × 70 voxels of 2 mm"),
    ("ds002603", "ds002603-sub-01-noreconall", "sub_01_wf/fmap_preproc_wf/wf_auto_00000",
     "a Philips phase difference at 3 T (`acq-Mm2`), 128 × 128 × 128 voxels of 2 mm"),
    ("ds002422-run01", "ds002422-sub-02-noreconall", "sub_02_wf/fmap_preproc_wf/wf_auto_00000",
     "the first of two phase differences at 1.5 T (`run-01`)"),
    ("ds002422-run02", "ds002422-sub-02-noreconall", "sub_02_wf/fmap_preproc_wf/wf_auto_00001",
     "the second phase difference at 1.5 T (`run-02`), on another grid"),
    ("ds001600-v1", "ds001600-sub-1-noreconall", "sub_1_wf/fmap_preproc_wf/wf_auto_00000",
     "two phase images (`acq-v1`, phase1 and phase2): sdcflows subtracts them"),
    ("ds001600-v2", "ds001600-sub-1-noreconall", "sub_1_wf/fmap_preproc_wf/wf_auto_00001",
     "two phase images of a second protocol (`acq-v2`)"),
    ("ds001600-v4", "ds001600-sub-1-noreconall", "sub_1_wf/fmap_preproc_wf/wf_auto_00002",
     "a phase difference (`acq-v4`)"),
]  # fmt: skip
SHOWN = ("ds005454", "ds002603", "ds002422-run01")
#: The runs of every estimator entry: label, environment, implementation, threads, D13 mode.
RUNS = [
    ("sdcflows", "newest", "sdcflows", 1, "reproduce"),
    ("sdcflows-shared", "shared", "sdcflows", 1, "reproduce"),
    ("larmorx", "newest", "larmorx", 1, "reproduce"),
    ("larmorx-14", "newest", "larmorx", 14, "reproduce"),
    ("larmorx-fix", "newest", "larmorx", 14, "fix"),
    ("workflow", "newest", "larmorx-workflow", 14, "fix"),
]
#: The unwarping example: ds005454's BOLD series, its first volumes.
APPLY_RUN = ("ds005454-sub-16-noreconall", "sub_16_wf")
APPLY_VOLUMES = 10
APPLY_SHOWN = 4
#: SDC-10: ds002603's BOLD references (sorted; the example shows the first).
SDC10_RUN = "ds002603-sub-01-noreconall"
#: SDC-06: fMRIPrep's own field maps (4D, one volume) refitted.
REFIT_RUNS = ("ds005454-sub-16-noreconall", "ds002603-sub-01-noreconall",
              "ds002422-sub-02-noreconall")  # fmt: skip
#: The memory rule's calls (tests/parity/test_memory_peak.py's SDCFLOWS_CASES).
MEMORY_CALLS = [
    ("fit-ds002422", "fit", "ds002422-run01"),
    ("fit-ds005454", "fit", "ds005454"),
    ("apply-ds005454", "apply", None),
]
MEMORY_VOLUMES = 20
ENV = {"OPENBLAS_CORETYPE": "Haswell", "OPENBLAS_NUM_THREADS": "1", "OMP_NUM_THREADS": "1"}
UNSET = ("LARMORX_UPSTREAM_DEFECTS", "PYTHONPATH")


# ----------------------------------------------------------------------- the oracle runs


def _bids(run: str) -> Path:
    return Path.home() / ".cache" / "larmorx-testdata" / "data" / "openneuro" / run.split("-")[0]


def _host(run: str, p: str) -> Path:
    """A path of fMRIPrep's container (``/work``, ``/out``, ``/data``) on this machine."""
    base = common.fmriprep_run(run)
    for c, h in (("/work/", base / "work"), ("/out/", base / "out"), ("/data/", _bids(run))):
        if p.startswith(c):
            return h / p[len(c) :]
    return Path(p)


def _pkl(path: Path) -> dict:
    from nipype.utils.filemanip import loadpkl

    return loadpkl(path)


def _estimator(key: str) -> tuple[str, str, str, str]:
    return next(x for x in ESTIMATORS if x[0] == key)


def _data(run: str) -> dict:
    from larmorx_validation.evidence.ants_panel import data

    return data(run.split("-sub-")[0])


def node(key: str) -> dict:
    """fMRIPrep's estimator ``key``: the inputs of its phase steps and of ``bs_filter`` (paths
    on this machine), and fMRIPrep's own files."""
    _k, run, est, _w = _estimator(key)
    wf = common.fmriprep_run(run) / WORK / est
    if not (wf / "bs_filter").is_dir():
        raise FileNotFoundError(f"fMRIPrep's estimator {est} of {run}")
    ph = wf / "phdiff_wf"
    phases = [
        str(_host(run, _pkl(d / "_inputs.pklz")["in_file"]))
        for d in sorted((ph / "phmap2rads" / "mapflow").glob("_phmap2rads*"))
    ]
    calc = _pkl(ph / "calc_phdiff" / "_inputs.pklz")
    comp = _pkl(ph / "compfmap" / "_inputs.pklz")
    pre = _pkl(ph / "prelude" / "_inputs.pklz")
    bs = _pkl(wf / "bs_filter" / "_inputs.pklz")
    params = {k: bs[k] for k in ("bs_spacing", "ridge_alpha", "recenter", "extrapolate",
                                  "zooms_min")}  # fmt: skip
    own = {
        "rads": _host(run, pre["phase_file"]),
        "fmap": _host(run, bs["in_data"]),
        "extrapolated": next((wf / "bs_filter").glob("*_extra.nii*")),
        "coeff": next((wf / "bs_filter").glob("*_coeff000.nii*")),
    }
    return {
        "run": run,
        "est": est,
        "spec": {
            "phases": phases,
            "metas": list(calc["in_meta"]),
            "metadata": dict(comp["metadata"]),
            "unwrapped": str(_host(run, comp["in_file"])),
            "fmap": str(_host(run, bs["in_data"])),
            "mask": str(_host(run, bs["in_mask"])),
            "magnitude": str(_host(run, pre["magnitude_file"])),
            "prelude_mask": str(_host(run, pre["mask_file"])),
            "bs": {**params, "bs_spacing": [list(s) for s in params["bs_spacing"]]},
        },
        "own": own,
    }


# --------------------------------------------------------------------------- the runs


def _python(name: str) -> str:
    from larmorx_validation.parity import python_versions as pv

    p = pv.prefix(name)
    if p is None:
        raise FileNotFoundError(f"the {name} Python environment ({pv.ENVS[name]})")
    return str(p / "bin" / "python")


def _runner(e: fcache.Entry, label: str, task: str, spec: Path, env_name: str, impl: str,
            threads: int, mode: str) -> dict:  # fmt: skip
    """One run of ``sdcflows_fieldmap_run`` into ``<entry>/<label>``; its ``outputs.json``."""
    repo = e.ctx.repo
    out = e.dir / label
    e.run(
        label,
        [_python(env_name), "-P", "-m", "larmorx_validation.evidence.sdcflows_fieldmap_run",
         task, str(spec), str(out), impl, "--threads", str(threads), "--defects", mode],
        env={**ENV, "PYTHONPATH": f"{repo / 'python'}:{repo / 'validation' / 'src'}"},
        unset=UNSET,
    )  # fmt: skip
    return json.loads((out / "outputs.json").read_text(encoding="utf-8"))


def _versions() -> dict[str, str]:
    from larmorx_validation.parity import python_versions as pv

    return {
        "newest": f"sdcflows 2.17.1, scikit-learn 1.9.1, nibabel 5.4.2, {pv.RELEASES[pv.NEWEST]} "
        "(oracles/python-newest: larmorx's reference)",
        "shared": f"sdcflows 2.17.1, scikit-learn 1.9.1, nibabel 5.4.2, {pv.RELEASES[pv.SHARED]} "
        "(the shared .venv)",
    }


def _compute_estimator(key: str):
    def compute(e: fcache.Entry) -> None:
        n = node(key)
        spec = n["spec"]
        for i, p in enumerate(spec["phases"]):
            e.input(f"phase map {i} (phmap2rads' in_file)", Path(p))
        for k in ("unwrapped", "fmap", "mask", "magnitude", "prelude_mask"):
            e.input(f"{k} (fMRIPrep's)", Path(spec[k]))
        for k, p in n["own"].items():
            e.input(f"fMRIPrep's own {k}", p)
        sp = e.dir / "spec.json"
        sp.write_text(json.dumps(spec, indent=1), encoding="utf-8")
        outs = {}
        for label, env_name, impl, threads, mode in RUNS:
            outs[label] = _runner(e, label, "estimator", sp, env_name, impl, threads, mode)
        e.finish(
            versions=_versions(),
            node=f"{WORK}/{n['est']}",
            outputs={
                k: {x: v[x] for x in ("files", "named", "seconds", "releases")}
                for k, v in outs.items()
            },
            settings="fMRIPrep 25.2.6's phase-difference estimator of the run, step by step on "
            "the inputs its nodes were given (their _inputs.pklz): PhaseMap2rads and "
            "SubtractPhases of the phase maps, Phasediff2Fieldmap of fMRIPrep's own PRELUDE "
            "output, BSplineApprox of fMRIPrep's own field map with its brain mask and "
            "parameters; sdcflows 2.17.1 in both kept environments, larmorx's lx.sdcflows at 1 "
            "and 14 threads (reproduce) and 14 (default), and lx.sdcflows.phdiff_workflow with "
            "lx.fsl.prelude's clean-room original (default); OPENBLAS_CORETYPE=Haswell, "
            "OPENBLAS_NUM_THREADS=1",
        )

    return compute


def estimator_entry(ctx: core.Context, key: str) -> fcache.Entry:
    _k, run, est, _w = _estimator(key)
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        f"estimator-{key}",
        f"fMRIPrep's phase-difference estimator {est.rsplit('/', 1)[-1]} of {run}: sdcflows "
        "2.17.1 in both environments and larmorx (1 and 14 threads, both modes, the workflow)",
        _compute_estimator(key),
    )


def _apply_inputs() -> dict:
    run, sub = APPLY_RUN
    w = common.fmriprep_run(run) / WORK / sub
    coeffs = sorted(w.glob("fmap_preproc_wf/wf_gre16/bs_filter/*_coeff000.nii.gz"))
    xfm = w / "bold_task_rest_wf/bold_fit_wf/fmapreg_wf/coregister/transform0GenericAffine.mat"
    rs = _pkl(w / "bold_task_rest_wf/bold_native_wf/boldref_bold/_inputs.pklz")
    bold = _bids(run) / "sub-16" / "func" / "sub-16_task-rest_bold.nii.gz"
    mask = common.fmriprep_run(run) / "out/sub-16/func/sub-16_task-rest_desc-brain_mask.nii.gz"
    for p in (*coeffs[:1], xfm, bold, mask):
        if not p.is_file():
            raise FileNotFoundError(p)
    return {"coeffs": coeffs[:1], "xfm": xfm, "bold": bold, "mask": mask,
            "ro_time": float(rs["ro_time"]), "pe_dir": str(rs["pe_dir"])}  # fmt: skip


def _bold_head(src: Path, dst: Path, n: int) -> Path:
    """The first ``n`` volumes of ``src``, its header kept (check_apply.py's ``bold_head``)."""
    import nibabel as nib

    img = nib.load(src)
    sub = img.slicer[..., :n]
    nib.Nifti1Image(np.asanyarray(sub.dataobj), img.affine, img.header).to_filename(dst)
    return dst


def _compute_apply(e: fcache.Entry) -> None:
    a = _apply_inputs()
    e.input("BOLD series (BIDS)", a["bold"])
    e.input("fMRIPrep's coefficients (bs_filter)", a["coeffs"][0])
    e.input("fMRIPrep's registration (fmapreg_wf/coregister)", a["xfm"])
    (e.dir / "inputs").mkdir(exist_ok=True)
    head = _bold_head(a["bold"], e.dir / "inputs" / a["bold"].name, APPLY_VOLUMES)
    spec = {
        "bold": str(head),
        "coeffs": [str(c) for c in a["coeffs"]],
        "fmap2data_xfm": str(a["xfm"]),
        "ro_time": a["ro_time"],
        "pe_dir": a["pe_dir"],
    }
    sp = e.dir / "spec.json"
    sp.write_text(json.dumps(spec, indent=1), encoding="utf-8")
    outs = {}
    for label, env_name, impl, threads, mode in (
        ("sdcflows", "newest", "sdcflows", 8, "reproduce"),
        ("sdcflows-shared", "shared", "sdcflows", 8, "reproduce"),
        ("larmorx", "newest", "larmorx", 1, "reproduce"),
        ("larmorx-14", "newest", "larmorx", 14, "reproduce"),
        ("larmorx-fix", "newest", "larmorx", 14, "fix"),
    ):
        outs[label] = _runner(e, label, "apply", sp, env_name, impl, threads, mode)
    e.finish(
        versions=_versions(),
        outputs={k: {x: v[x] for x in ("files", "named", "seconds")} for k, v in outs.items()},
        settings=f"ApplyCoeffsField on the first {APPLY_VOLUMES} volumes of ds005454 sub-16's "
        "BOLD series with fMRIPrep's bs_filter coefficients, its registration of the BOLD "
        "reference to the field map (fmap2data_xfm) and the readout time and phase-encoding "
        "direction of its ResampleSeries node; sdcflows 2.17.1 (num_threads 8) in both kept "
        "environments, larmorx's lx.sdcflows.apply_coeffs_field at 1 and 14 threads "
        "(reproduce) and 14 (default)",
    )


def apply_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "apply-ds005454",
        f"ApplyCoeffsField on the first {APPLY_VOLUMES} volumes of ds005454 sub-16's BOLD: "
        "sdcflows 2.17.1 in both environments and larmorx", _compute_apply,
    )  # fmt: skip


def _sdc10_targets() -> list[dict]:
    import nitransforms as nt

    base = common.fmriprep_run(SDC10_RUN) / "out" / "sub-01"
    cs = sorted((base / "fmap").glob("*desc-coeff_fieldmap.nii.gz"))
    out = []
    for bref in sorted((base / "func").glob("*desc-hmc_boldref.nii.gz"))[:2]:
        prefix = bref.name.split("_desc-")[0]
        xp = sorted((base / "func").glob(f"{prefix}_from-boldref_to-*_desc-fmap_xfm.txt"))
        mask = base / "func" / f"{prefix}_desc-brain_mask.nii.gz"
        if not cs or not xp or not mask.is_file():
            raise FileNotFoundError(f"{SDC10_RUN}: {prefix}'s coefficients or registration")
        m = np.linalg.inv(nt.linear.Affine.from_filename(str(xp[0]), fmt="itk").matrix)
        out.append({"prefix": prefix, "target": bref, "coeffs": cs, "xfm": m, "xfm_file": xp[0],
                    "mask": mask})  # fmt: skip
    return out


def _compute_sdc10(e: fcache.Entry) -> None:
    from larmorx_validation.parity.sdcflows_fieldmap import SCRIPTS

    done = {}
    for t in _sdc10_targets():
        p = t["prefix"]
        e.input(f"{p}: BOLD reference", t["target"])
        e.input(f"{p}: fMRIPrep's registration", t["xfm_file"])
        e.input(f"{p}: brain mask", t["mask"])
        spec = {
            "coeffs": [str(c) for c in t["coeffs"]],
            "target": str(t["target"]),
            "xfm": t["xfm"].tolist(),
            "scripts": str(SCRIPTS),
        }
        sp = e.dir / f"{p}.spec.json"
        sp.write_text(json.dumps(spec, indent=1), encoding="utf-8")
        runs = {}
        for label, env_name, impl, threads, mode in (
            ("sdcflows", "newest", "sdcflows", 1, "reproduce"),
            ("sdcflows-shared", "shared", "sdcflows", 1, "reproduce"),
            ("larmorx", "newest", "larmorx", 14, "reproduce"),
            ("larmorx-fix", "newest", "larmorx", 14, "fix"),
            ("exact", "newest", "exact", 1, "reproduce"),
        ):
            r = _runner(e, f"{p}/{label}", "fit", sp, env_name, impl, threads, mode)
            runs[label] = {"named": r["named"], "seconds": r["seconds"]}
        done[p] = runs
    e.finish(
        versions=_versions(),
        targets=done,
        settings="B0FieldTransform.fit(target, xfm_data2fmap, approx=True), as ApplyCoeffsField "
        "and sdcflows' unwarping call it, with fMRIPrep's coefficients of ds002603 sub-01 on "
        "its two oblique BOLD references (desc-hmc_boldref) and the inverse of fMRIPrep's "
        "registration (from-boldref_to-auto00000 desc-fmap_xfm): sdcflows 2.17.1 in both "
        "environments, larmorx at 14 threads in both modes, and the exact field where the "
        "transform puts the target (the investigation's numpy construction, "
        "measure_sdc01_02.exact_field: every voxel's B-spline weights at its own position)",
    )


def sdc10_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "sdc10-ds002603",
        "SDC-10: B0FieldTransform.fit with approx on ds002603's oblique BOLD references: "
        "sdcflows, larmorx in both modes and the exact field", _compute_sdc10,
    )  # fmt: skip


def _refit_inputs() -> list[dict]:
    out = []
    for run in REFIT_RUNS:
        sub = "sub-" + run.split("-sub-")[1].split("-")[0]
        d = common.fmriprep_run(run) / "out" / sub / "fmap"
        for f in sorted(d.glob("*desc-preproc_fieldmap.nii.gz")):
            stem = f.name.split("_desc-")[0]
            mask = d / f"{stem}_desc-brain_mask.nii.gz"
            if mask.is_file():
                out.append({"run": run, "name": stem, "fmap": f, "mask": mask})
    if not out:
        raise FileNotFoundError("fMRIPrep's desc-preproc_fieldmap outputs")
    return out


def _compute_refit(e: fcache.Entry) -> None:
    import nibabel as nib

    done = {}
    (e.dir / "inputs").mkdir(exist_ok=True)
    for x in _refit_inputs():
        name = x["name"]
        e.input(f"{name}: fMRIPrep's field map", x["fmap"])
        e.input(f"{name}: fMRIPrep's brain mask", x["mask"])
        img = nib.load(x["fmap"])
        three = e.dir / "inputs" / f"{name}_3d.nii.gz"
        data = np.asanyarray(img.dataobj)
        nib.Nifti1Image(data.reshape(data.shape[:3]), img.affine, img.header).to_filename(three)
        specs = {
            "4d": {"fmap": str(x["fmap"]), "mask": str(x["mask"])},
            "3d": {"fmap": str(three), "mask": str(x["mask"])},
        }
        for k, s in specs.items():
            (e.dir / f"{name}.{k}.spec.json").write_text(json.dumps(s), encoding="utf-8")
        runs = {}
        for label, which, impl, mode in (
            ("sdcflows-4d", "4d", "sdcflows", "reproduce"),
            ("sdcflows-3d", "3d", "sdcflows", "reproduce"),
            ("larmorx-4d", "4d", "larmorx", "reproduce"),
            ("larmorx-fix-4d", "4d", "larmorx", "fix"),
        ):
            sp = e.dir / f"{name}.{which}.spec.json"
            r = _runner(e, f"{name}/{label}", "refit", sp, "newest", impl, 14, mode)
            runs[label] = {k: r.get(k) for k in ("files", "named", "error", "seconds")}
        done[name] = {"run": x["run"], "shape": list(img.shape), "runs": runs}
    e.finish(
        versions=_versions(),
        refits=done,
        settings="BSplineApprox with fMRIPrep's parameters on fMRIPrep's own field maps "
        "(desc-preproc_fieldmap: 4D, one volume) with their brain masks (desc-brain_mask), and "
        "on the same maps written as 3D images: sdcflows 2.17.1 (newest environment) and "
        "larmorx at 14 threads in both modes on the 4D files",
    )


def refit_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "sdc06-refit",
        "SDC-06: BSplineApprox on fMRIPrep's own 4D field maps of one volume: sdcflows on "
        "them and on their 3D copies, larmorx in both modes", _compute_refit,
    )  # fmt: skip


def _compute_memory(e: fcache.Entry) -> None:
    from larmorx_validation.parity.sdcflows_fieldmap import SCRIPTS

    script = SCRIPTS / "memory_call.py"
    repo = e.ctx.repo
    (e.dir / "inputs").mkdir(exist_ok=True)
    peaks: dict[str, dict] = {}
    for name, call, key in MEMORY_CALLS:
        if call == "fit":
            s = node(key)["spec"]
            inputs = [s["fmap"], s["mask"]]
        else:
            a = _apply_inputs()
            head = _bold_head(a["bold"], e.dir / "inputs" / a["bold"].name, MEMORY_VOLUMES)
            inputs = [str(head), str(a["coeffs"][0]), str(a["xfm"]), str(a["ro_time"]),
                      a["pe_dir"]]  # fmt: skip
        for p in inputs:
            if Path(p).is_file():
                e.input(f"{name}: {Path(p).name}", Path(p))
        peaks[name] = {}
        for impl, mode in (("sdcflows", None), ("larmorx", "reproduce"), ("larmorx", "fix")):
            label = f"{name}-{impl}" + (f"-{mode}" if mode else "")
            cwd = e.dir / label
            cwd.mkdir(exist_ok=True)
            argv = ["/usr/bin/time", "-v", _python("newest"), "-P", str(script), impl, call,
                    *inputs]  # fmt: skip
            env = dict(ENV)
            if impl == "larmorx":
                argv += ["--defects", mode]
                env["PYTHONPATH"] = str(repo / "python")
            e.run(label, argv, env=env, unset=UNSET, cwd=cwd)
            log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
            kb = int(re.search(r"Maximum resident set size \(kbytes\): (\d+)", log).group(1))
            peaks[name][f"{impl}" + (f"-{mode}" if mode else "")] = kb / 1024.0
            for f in cwd.iterdir():  # the outputs are not kept (the record compares them)
                if f.is_file():
                    f.unlink()
    e.finish(
        versions=_versions(),
        peaks_mib=peaks,
        settings="The memory rule's calls (tests/parity/test_memory_peak.py, SDCFLOWS_CASES): "
        "validation/investigations/sdcflows-fieldmap/memory_call.py, one call per process, "
        "its peak resident memory from /usr/bin/time -v; fMRIPrep's bs_filter of ds002422's "
        f"first estimator and of ds005454, ApplyCoeffsField on {MEMORY_VOLUMES} volumes of "
        "ds005454's BOLD; sdcflows 2.17.1 and larmorx (one thread, both modes) in the newest "
        "environment",
    )


def memory_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "memory",
        "The memory rule's three calls: sdcflows' and larmorx's peak resident memory",
        _compute_memory,
    )  # fmt: skip


# ------------------------------------------------------------------------- the examples


def _files(e: fcache.Entry, label: str) -> dict[str, Path]:
    rec = json.loads((e.dir / label / "outputs.json").read_text(encoding="utf-8"))
    return {k: e.dir / label / v for k, v in rec["files"].items()}


def _named(e: fcache.Entry, label: str) -> list[str]:
    rec = json.loads((e.dir / label / "outputs.json").read_text(encoding="utf-8"))
    return [n for n in rec.get("named", []) if n]


def _stored(path: Path) -> np.ndarray:
    """A file's values as stored (no scaling: every output here is float32)."""
    import nibabel as nib

    return np.asanyarray(nib.load(str(path)).dataobj)


def _findings_links(repo: Path) -> dict[str, str]:
    """``SDC-xx`` -> the link of its section in the findings document."""
    out = {}
    for line in (repo / FINDINGS).read_text(encoding="utf-8").splitlines():
        m = re.match(r"### (SDC-\d+):", line)
        if m:
            out[m.group(1)] = f"{FINDINGS}#{core.github_anchor(line.lstrip('#').strip())}"
    return out


def _link(links: dict[str, str], ids) -> list[dict]:
    return [{"label": i, "href": links[i]} for i in ids if i in links]


def _show(mask: np.ndarray, iterations: int) -> np.ndarray:
    from scipy import ndimage

    return ndimage.binary_dilation(mask > 0, iterations=iterations)


def _peak_view(ex, panel: str, output: str) -> list[int]:
    """The default view: the axial slice with the most of the shown region, the crosshair
    where ``panel``'s ``output`` is largest in magnitude inside it (its edge eroded)."""
    from scipy import ndimage

    v = np.abs(ex.values(ex.key(panel, output)))
    if v.ndim == 4:
        v = v[..., 0]
    k = int(np.argmax(ex.m.sum(axis=(0, 1))))
    inner = ndimage.binary_erosion(ex.m, iterations=2)
    region = inner if inner[..., k].any() else ex.m
    sl = np.where(region[..., k], v[..., k], -1.0)
    i, j = np.unravel_index(int(np.argmax(sl)), sl.shape)
    return [int(i), int(j), k]


def _diff_count(a: np.ndarray, b: np.ndarray) -> int:
    """How many values differ (as stored; NaN alike), or -1 when the shapes differ."""
    if a.shape != b.shape:
        return -1
    if a.dtype == b.dtype and a.tobytes() == b.tobytes():
        return 0
    x, y = np.asarray(a, np.float64), np.asarray(b, np.float64)
    return int(np.count_nonzero(~((x == y) | (np.isnan(x) & np.isnan(y)))))


OUTPUTS = [
    {"id": "rads", "label": "Phase difference (rad)", "unit": "rad"},
    {"id": "fmap", "label": "Field map (Hz)", "unit": "Hz"},
    {"id": "extrapolated", "label": "Fitted field, extrapolated (Hz)", "unit": "Hz"},
    {"id": "error", "label": "Fitting error (Hz)", "unit": "Hz"},
]
#: The estimator examples' panels of runs: id, run label, role, label, sub.
RUN_PANELS = [
    ("sdcflows", "sdcflows", "original", "sdcflows 2.17.1",
     "numpy 2.5.4, SciPy 1.18.1, scikit-learn 1.9.1: larmorx's reference"),
    ("larmorx", "larmorx", "larmorx", "larmorx (port), 1 thread", "reproduce mode"),
    ("larmorx-14", "larmorx-14", "larmorx", "larmorx (port), 14 threads", "reproduce mode"),
    ("larmorx-fix", "larmorx-fix", "larmorx", "larmorx (port), 14 threads",
     "default mode (D13 corrections)"),
    ("sdcflows-shared", "sdcflows-shared", "original", "sdcflows 2.17.1",
     "numpy 2.3.5, SciPy 1.15.3: the shared environment's releases"),
]  # fmt: skip


def _estimator_arrays(e: fcache.Entry, own: dict) -> tuple[dict, dict, dict]:
    """Every run's outputs as stored: the panels' (id -> output -> array), fMRIPrep's own
    files and larmorx's workflow (compared over their files only), and the coefficients."""
    runs, coeffs = {}, {}
    for pid, label, *_ in RUN_PANELS:
        f = _files(e, label)
        runs[pid] = {o["id"]: _stored(f[o["id"]]) for o in OUTPUTS}
        coeffs[pid] = _stored(f["coeff"])
    wf = _files(e, "workflow")
    full = {
        "fmriprep": {k: _stored(own[k]) for k in ("rads", "fmap", "extrapolated")},
        "workflow": {k: _stored(wf[k]) for k in ("rads", "fmap")},
    }
    coeffs["fmriprep"] = _stored(own["coeff"])
    return runs, full, coeffs


def _estimator_diffs() -> list[dict]:
    d = common.d
    return [
        d("lx-sdc", "larmorx", "sdcflows", "main", "larmorx − sdcflows 2.17.1", common.ROLE_LX),
        d("lx-fmriprep", "larmorx", "fmriprep", "fmriprep",
          "larmorx − fMRIPrep 25.2.6's own files", common.ROLE_FMRIPREP),
        d("wf-fmriprep", "workflow", "fmriprep", "fmriprep",
          "larmorx's whole workflow (PRELUDE: lx.fsl.prelude's clean-room original) − fMRIPrep's "
          "own files", common.ROLE_FMRIPREP),
        d("lx14-lx1", "larmorx-14", "larmorx", "check", "larmorx: 14 threads − 1 thread",
          common.ROLE_SELF),
        d("fix-lx", "larmorx-fix", "larmorx-14", "check", "larmorx: default − reproduce mode",
          common.ROLE_SELF),
        d("newest-shared", "sdcflows", "sdcflows-shared", "variability",
          "sdcflows: numpy 2.5.4 / SciPy 1.18.1 − numpy 2.3.5 / SciPy 1.15.3",
          "the original against itself: the version step"),
    ]  # fmt: skip


def _estimator_verdicts(diffs, same, named, links) -> None:
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if i == "newest-shared":
            x["verdict"] = None
            x["reason"] = (
                "The version step, a reference row (the original against itself): with numpy "
                "2.5.4 (OpenBLAS 0.3.34) and SciPy 1.18.1 sdcflows gives "
                + (
                    "the same values as with numpy 2.3.5 (OpenBLAS 0.3.30) and SciPy 1.15.3, "
                    "every one: sdcflows 2.15.1 to 2.17.1 and the libraries' releases change "
                    "nothing on these paths."
                    if same[i]
                    else "other values than with numpy 2.3.5 and SciPy 1.15.3 (see the record's "
                    "step)."
                )
            )
            x["links"].append({"label": "the record", "href": f"docs/validation/{RECORD}.md"})
            continue
        if same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "lx-sdc": "Every value of the four outputs is sdcflows': the phase in radians "
                "(numpy's float64 rescaling rounded to float32), the field map in Hz, the ridge "
                "regression's coefficients (scikit-learn's sparse_cg, SciPy's conjugate "
                "gradients with OpenBLAS's float32 dot products, in their order) and the field "
                "they give inside and outside the mask, the fitting error.",
                "lx-fmriprep": "fMRIPrep 25.2.6's own files of this estimator (sdcflows 2.15.1 "
                "in its image) hold the same values: the phase difference PRELUDE was given, "
                "the field map, the extrapolated field (`*_extra`) the run used.",
                "wf-fmriprep": "larmorx's whole phase-difference workflow, PRELUDE included "
                "(lx.fsl.prelude's clean-room original, written from the specification), gives "
                "fMRIPrep's field map in every voxel.",
                "lx14-lx1": "The same bytes at 1 and 14 threads (at one thread the fit stores "
                "the collocation matrix, at more it evaluates the weights at every product: the "
                "same sums in the same order).",
                "fix-lx": "No correction of the register applies to fMRIPrep's call: both modes "
                "give the same values."
                if not named
                else "No correction changes this call's values: the default mode names "
                f"{', '.join(named)}, with the same values in both modes.",
            }[i]
        elif i == "fix-lx" and named:
            x["verdict"] = "d13"
            x["reason"] = f"The default mode corrects {', '.join(named)} and names it."
            x["links"] += _link(links, named)
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example (see the record)."


def _example_estimator(ctx: core.Context, key: str, links: dict) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    _k, run, est, what = _estimator(key)
    e = estimator_entry(ctx, key)
    info = e.info()
    n = node(key)
    spec, own = n["spec"], n["own"]
    fimg = nib.load(spec["fmap"])
    mask = common.load(spec["mask"]) > 0
    show = _show(mask, 2)
    grid = core.Grid.around(fimg, show)
    ex = exm.Example(ctx, grid, show)
    ds = run.split("-noreconall")[0]
    two = len(spec["phases"]) == 2
    raw, rs, ri = common.stored(spec["phases"][0])
    ex.add(exm.Panel("phase", "input", f"{ds}: phase map" + (" 1" if two else ""),
                     "the scanner's phase difference as BIDS stores it (PhaseMap2rads' input)",
                     {"all": raw}, scale=rs, inter=ri))  # fmt: skip
    mag, ms, mi = common.stored(spec["magnitude"])
    ex.add(exm.Panel("mag", "input", f"{ds}: magnitude",
                     "sdcflows' clipped magnitude, PRELUDE's --abs", {"all": mag}, scale=ms,
                     inter=mi))  # fmt: skip
    mk, ks, ki = common.stored(spec["mask"])
    from larmorx_validation.evidence.fsl_bet import window_of

    ex.add(exm.Panel("mask", "input", f"{ds}: brain mask",
                     "sdcflows' brain mask of the magnitude (PRELUDE's mask, bs_filter's "
                     "in_mask), over the magnitude", {"all": mk}, scale=ks, inter=ki,
                     overlay={o["id"]: {"on": "mag", "window": window_of(ex, "mag")}
                              for o in OUTPUTS}))  # fmt: skip
    uw, us, ui = common.stored(spec["unwrapped"])
    ex.add(exm.Panel("unwrapped", "input", f"{ds}: unwrapped phase (rad)",
                     "fMRIPrep's own PRELUDE output (FSL in its image), Phasediff2Fieldmap's "
                     "input", {"all": uw}, scale=us, inter=ui))  # fmt: skip
    runs, full, coeffs = _estimator_arrays(e, own)
    for pid, _label, role, label, sub in RUN_PANELS:
        ex.add(exm.Panel(pid, role, label, sub, runs[pid]))
    diffs = _estimator_diffs()
    outputs = copy.deepcopy(OUTPUTS)
    common.compare_all(ex, diffs, outputs, full=full)
    same = common.identical(diffs)
    named = _named(e, "larmorx-fix")
    _estimator_verdicts(diffs, same, named, links)
    at = _peak_view(ex, "sdcflows", "fmap")
    default = {
        "output": "fmap",
        "plane": "axial",
        "voxel": at,
        "panels": ["phase", "unwrapped", "sdcflows", "larmorx"],
        "diffs": ["lx-sdc", "lx-fmriprep", "wf-fmriprep"],
        "compare": {"a": "sdcflows", "b": "larmorx"},
    }
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="sdcflows",
        caption="the crosshair where the field is strongest in the slice; choose the output "
        "(the phase difference, the field map, the fitted field, its error) above the images.",
    )  # fmt: skip
    nc = {k: _diff_count(coeffs["larmorx"], v) for k, v in coeffs.items()}
    rec = json.loads((e.dir / "larmorx" / "outputs.json").read_text(encoding="utf-8"))
    cimg = nib.load(str(own["coeff"]))
    data = _data(run)
    n_vox = int(np.prod(fimg.shape[:3]))
    meta = spec["metadata"]
    dte = (
        f"echo times {meta['EchoTime1'] * 1000:.3g} and {meta['EchoTime2'] * 1000:.3g} ms"
        if "EchoTime1" in meta and "EchoTime2" in meta
        else "the phase images' echo times"
    )
    coeff_text = (
        f"the {cimg.shape[0]} × {cimg.shape[1]} × {cimg.shape[2]} B-spline coefficients "
        f"({int(np.prod(cimg.shape[:3])):,} values, 16 × 16 × 10 mm spacing): larmorx's equal "
        + ("sdcflows' and fMRIPrep's own `*_coeff000` file, every one"
           if nc["sdcflows"] == 0 and nc["fmriprep"] == 0
           else f"sdcflows' but {nc['sdcflows']} and fMRIPrep's but {nc['fmriprep']}")
        + (", at 1 and 14 threads and in both modes"
           if nc["larmorx-14"] == 0 and nc["larmorx-fix"] == 0 else "")
        + f"; {rec.get('n_inside', 0):,} voxels inside the mask, {rec.get('n_iter', 0)} "
        "conjugate-gradient iterations"
    )  # fmt: skip
    return {
        "id": f"estimator-{key}",
        "title": f"fMRIPrep's field map from {ds}'s phase difference, and its B-spline fit",
        "summary": f"fMRIPrep 25.2.6's phase-difference estimator of {ds} ({what}; sdcflows' "
        f"`{est.rsplit('/', 1)[-1]}`), step by step on the inputs its nodes were given: the "
        "scanner's phase map rescaled to radians"
        + (" and the two phases subtracted" if two else "")
        + " (the phase difference PRELUDE unwraps), fMRIPrep's own unwrapped phase turned into "
        "Hz (the field map), and that field map approximated by cubic B-splines inside the brain "
        "mask (`bs_filter`): the smooth fitted field, extrapolated beyond the mask, and the "
        "fitting error. sdcflows 2.17.1 and larmorx compute every output; fMRIPrep's own files "
        "and larmorx's whole workflow (with PRELUDE) are compared with them value by value.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']}): the field "
                "map's phase and magnitude as fMRIPrep's nodes were given them; shown inside "
                "sdcflows' brain mask of the magnitude, dilated by 2 voxels, so no face shows",
            },
            {
                "label": "Nodes",
                "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}`: "
                "`phdiff_wf/phmap2rads`, `calc_phdiff`, `compfmap` and `bs_filter`",
            },
            {
                "label": "Fit",
                "text": "BSplineApprox as `bs_filter` calls it: spacing "
                f"{spec['bs']['bs_spacing']} mm, ridge alpha {spec['bs']['ridge_alpha']:g}, "
                f"recentred by the {spec['bs']['recenter']}, extrapolated, zooms_min "
                f"{spec['bs']['zooms_min']:g}; Δ TE from the {dte}",
            },
            {"label": "Coefficients", "text": coeff_text},
            {
                "label": "Programs",
                "text": "sdcflows 2.17.1 in both kept environments (in "
                "process, its nipype interfaces); larmorx's `lx.sdcflows` at commit "
                f"`{info['larmorx_commit'][:12]}`; numpy's OpenBLAS on its Haswell kernels, one "
                "thread, as fMRIPrep's nodes",
            },
            {"label": "Grid", "text": common.describe_grid(fimg, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against sdcflows"],
            ["fmriprep", "LARMORX against fMRIPrep's own files"],
            ["check", "LARMORX against itself: threads and modes"],
            ["variability", "sdcflows against itself: the version step"],
        ],
        "metrics_note": f"Δ is A − B over all {n_vox:,} voxels of each output file, not only "
        "those shown; fMRIPrep's own files and larmorx's workflow are compared over their "
        "files (fMRIPrep keeps no fitting error, the workflow ends at the field map).",
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
    }


def _example_coefficients(ctx: core.Context) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    key = SHOWN[0]
    _k, run, _est, _w = _estimator(key)
    e = estimator_entry(ctx, key)
    info = e.info()
    own = node(key)["own"]
    cimg = nib.load(str(own["coeff"]))
    show = np.ones(cimg.shape[:3], bool)
    grid = core.Grid.around(cimg, show, margin=0)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("fmriprep", "fmriprep", "fMRIPrep 25.2.6's own coefficients",
                     "`bs_filter`'s `*_coeff000` file (sdcflows 2.15.1 in its image)",
                     {"coeff": _stored(own["coeff"])}))  # fmt: skip
    for pid, label, role, lab, sub in RUN_PANELS:
        ex.add(exm.Panel(pid, role, lab, sub, {"coeff": _stored(_files(e, label)["coeff"])}))
    d = common.d
    diffs = [
        d("lx-sdc", "larmorx", "sdcflows", "main", "larmorx − sdcflows 2.17.1", common.ROLE_LX),
        d("lx-fmriprep", "larmorx", "fmriprep", "fmriprep", "larmorx − fMRIPrep's own file",
          common.ROLE_FMRIPREP),
        d("lx14-lx1", "larmorx-14", "larmorx", "check", "larmorx: 14 threads − 1 thread",
          common.ROLE_SELF),
        d("fix-lx", "larmorx-fix", "larmorx-14", "check", "larmorx: default − reproduce mode",
          common.ROLE_SELF),
        d("newest-shared", "sdcflows", "sdcflows-shared", "variability",
          "sdcflows: numpy 2.5.4 / SciPy 1.18.1 − numpy 2.3.5 / SciPy 1.15.3",
          "the original against itself: the version step"),
    ]  # fmt: skip
    outputs = [{"id": "coeff", "label": "B-spline coefficients (Hz)", "unit": "Hz"}]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    _estimator_verdicts(diffs, same, _named(e, "larmorx-fix"), {})
    for x in diffs:
        if x["id"] == "lx-sdc" and same["lx-sdc"]:
            x["reason"] = (
                "Every coefficient is sdcflows': scikit-learn's ridge regression (sparse_cg, "
                "SciPy's conjugate gradients) on the field map inside the mask, its float32 "
                "dot products as OpenBLAS's Haswell kernel sums them."
            )
        if x["id"] == "lx-fmriprep" and same["lx-fmriprep"]:
            x["reason"] = "fMRIPrep 25.2.6's own coefficient file holds the same values."
    at, _k2 = common.centre_view(ex, "axial")
    default = {
        "output": "coeff",
        "plane": "axial",
        "voxel": at,
        "panels": ["fmriprep", "sdcflows", "larmorx"],
        "diffs": ["lx-sdc", "lx-fmriprep"],
        "compare": {"a": "sdcflows", "b": "larmorx"},
    }
    out = common.finish(ctx, ex, diffs, outputs, default=default, window_panel="sdcflows",
                        caption="every coefficient of the control grid.")  # fmt: skip
    data = _data(run)
    ds = run.split("-noreconall")[0]
    return {
        "id": f"coefficients-{key}",
        "title": f"The B-spline coefficients of {ds}'s field map",
        "summary": "What the fit computes: one coefficient per control point of a grid 16 mm "
        "apart in x and y and 10 mm in z, spread over the field map's field of view; the field "
        "anywhere is the sum of the cubic B-splines of the nearest 4 × 4 × 4 control points, "
        "weighted by these coefficients. fMRIPrep keeps them (`desc-coeff_fieldmap`) and "
        "evaluates them on every grid it resamples the BOLD series onto.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} "
                f"({data['licence']}): the coefficients of the field map above (no image of the "
                "head: a smooth field on a coarse grid)",
            },
            {
                "label": "Grid",
                "text": f"{' × '.join(map(str, cimg.shape[:3]))} control points "
                f"of {' × '.join(f'{z:g}' for z in cimg.header.get_zooms()[:3])} mm, every one "
                "shown",
            },
            {
                "label": "Programs",
                "text": "fMRIPrep 25.2.6's own file; sdcflows 2.17.1 in both "
                f"kept environments; larmorx at commit `{info['larmorx_commit'][:12]}`",
            },
        ],
        **out,
        "chain": None,
        "metrics_note": f"Δ is A − B over all {int(np.prod(cimg.shape[:3])):,} coefficients.",
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
    }


def _example_apply(ctx: core.Context, links: dict) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    e = apply_entry(ctx)
    info = e.info()
    a = _apply_inputs()
    head = e.dir / "inputs" / a["bold"].name
    bimg = nib.load(str(head))
    n = int(bimg.shape[3])
    shown = [int(t) for t in np.linspace(0, n - 1, APPLY_SHOWN).round()]
    mask = common.load(str(a["mask"])) > 0
    if mask.shape != bimg.shape[:3]:
        raise ValueError(f"{a['mask'].name} is not on the BOLD series' grid")
    show = _show(mask, 1)
    grid = core.Grid.around(bimg, show)
    ex = exm.Example(ctx, grid, show)

    def vols(x: np.ndarray) -> np.ndarray:
        return np.ascontiguousarray(x[..., shown])

    raw, rs, ri = common.stored(head)
    ex.add(exm.Panel("bold", "input", "ds005454 sub-16 BOLD, task rest",
                     f"as BIDS stores it (the first {n} volumes), distorted along the phase "
                     "encoding", {"all": vols(raw)}, scale=rs, inter=ri))  # fmt: skip
    labels = [
        ("sdcflows", "original", "sdcflows 2.17.1, ApplyCoeffsField",
         "numpy 2.5.4, SciPy 1.18.1: larmorx's reference"),
        ("larmorx", "larmorx", "larmorx (port), 1 thread", "reproduce mode"),
        ("larmorx-14", "larmorx", "larmorx (port), 14 threads", "reproduce mode"),
        ("larmorx-fix", "larmorx", "larmorx (port), 14 threads",
         "default mode (D13 corrections)"),
        ("sdcflows-shared", "original", "sdcflows 2.17.1, ApplyCoeffsField",
         "numpy 2.3.5, SciPy 1.15.3: the shared environment's releases"),
    ]  # fmt: skip
    full, scales, fields = {}, {}, {}
    for pid, role, label, sub in labels:
        f = _files(e, pid)
        st, sc, it = common.stored(f["corrected"])
        full[pid], scales[pid] = st, (sc, it)
        fields[pid] = _stored(f["field"])
        ex.add(exm.Panel(pid, role, label, sub, {"corrected": vols(st)}, scale=sc, inter=it))
    d = common.d
    diffs = [
        d("lx-sdc", "larmorx", "sdcflows", "main", "larmorx − sdcflows 2.17.1", common.ROLE_LX),
        d("lx14-lx1", "larmorx-14", "larmorx", "check", "larmorx: 14 threads − 1 thread",
          common.ROLE_SELF),
        d("fix-lx", "larmorx-fix", "larmorx-14", "check", "larmorx: default − reproduce mode",
          common.ROLE_SELF),
        d("newest-shared", "sdcflows", "sdcflows-shared", "variability",
          "sdcflows: numpy 2.5.4 / SciPy 1.18.1 − numpy 2.3.5 / SciPy 1.15.3",
          "the original against itself: the version step"),
    ]  # fmt: skip
    outputs = [{"id": "corrected", "label": "Unwarped series", "unit": ""}]
    for x in diffs:
        sa, sb = scales[x["a"]], scales[x["b"]]
        same_scale = sa == sb
        x["metrics"] = {
            "corrected": core.compare(full[x["a"]], full[x["b"]],
                                      **({"scale": sa[0] or 1.0, "inter": sa[1]}
                                         if same_scale and sa[0] else {}))
        }  # fmt: skip
        if not same_scale:
            x["metrics"]["corrected"]["identical"] = False
        r = ex.compare(x["a"], x["b"], "corrected")
        x["display"] = {"corrected": r["display"]}
    same = common.identical(diffs)
    named = _named(e, "larmorx-fix")
    _estimator_verdicts(diffs, same, named, links)
    field_same = {k: _diff_count(fields["larmorx"], v) for k, v in fields.items()}
    fd = np.abs(
        fields["larmorx-fix"].astype(np.float64).reshape(mask.shape)
        - fields["larmorx-14"].astype(np.float64).reshape(mask.shape)
    )[mask]
    sd = np.abs(full["larmorx-fix"].astype(np.float64) - full["larmorx-14"])
    for x in diffs:
        if x["id"] == "fix-lx" and x["verdict"] == "d13" and named == ["SDC-02"]:
            x["reason"] = (
                "SDC-02, corrected by default with a notice (rule A): fMRIPrep's registration "
                "of the BOLD reference to the field map rotates the series' grid against the "
                "coefficients' grid, where sdcflows multiplies per-axis weights each taken from "
                "one index of the voxel (a product exact only for aligned grids, with its "
                "warning \"Image's and B-Spline's grids are not aligned\"); larmorx evaluates "
                "the B-spline at each voxel's own position. The field on the series' grid "
                f"changes by {float(np.sqrt(np.mean(fd * fd))):.2f} Hz RMS (largest "
                f"{float(fd.max()):.1f} Hz) inside the brain mask, the unwarped series in "
                f"{int(np.count_nonzero(sd)):,} of its {sd.size:,} values. "
                "`upstream_defects='reproduce'` gives sdcflows' series."
            )
        if x["id"] == "lx-sdc" and same["lx-sdc"]:
            x["reason"] = (
                f"Every value of the {n} unwarped volumes is sdcflows': the field evaluated on "
                "the series' grid through the registration's inverse, the voxel shifts along "
                "the phase encoding, SciPy's cubic B-spline interpolation and the Jacobian, "
                "written back into the series' int16 type as nibabel scales it."
            )
    t0 = 0
    at = _peak_view(ex, "bold", "corrected")
    default = {
        "output": "corrected",
        "plane": "axial",
        "voxel": at,
        "t": t0,
        "panels": ["bold", "sdcflows", "larmorx", "larmorx-fix"],
        "diffs": ["lx-sdc", "fix-lx", "newest-shared"],
        "compare": {"a": "bold", "b": "larmorx"},
    }
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="sdcflows",
        input_windows={"bold": "brain"},
        caption=f"file volume {shown[t0]}; compare the distorted series (before) with the "
        "unwarped one (after) with Swipe or Blink.",
    )  # fmt: skip
    out["grid"]["t_index"] = shown
    data = _data(APPLY_RUN[0])
    n_val = int(np.prod(full["sdcflows"].shape))
    return {
        "id": "apply-ds005454",
        "title": "The unwarping of a real BOLD series with fMRIPrep's field map",
        "summary": "sdcflows' `ApplyCoeffsField` (its own unwarping, the interface sdcflows' "
        "workflows use; fMRIPrep 25.2.6 itself unwarps inside its one-shot resampler) on the "
        f"first {n} volumes of OpenNeuro ds005454 sub-16's resting-state series (7 T, "
        "multiband): fMRIPrep's B-spline coefficients evaluated on the series' grid through "
        "fMRIPrep's registration of the BOLD reference to the field map, each voxel moved "
        "back along the phase-encoding axis by the field times the readout time, and its "
        f"intensity corrected by the Jacobian. {len(shown)} of the {n} volumes are shown; the "
        "metrics cover every volume.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']}, task rest "
                f"({data['licence']}); shown inside fMRIPrep's BOLD brain mask, dilated by one "
                "voxel",
            },
            {
                "label": "Inputs",
                "text": f"PE `{a['pe_dir']}`, readout {a['ro_time']} s (the "
                "values of fMRIPrep's ResampleSeries node); the coefficients of fMRIPrep's "
                "`bs_filter`; `fmap2data_xfm` fMRIPrep's `fmapreg_wf/coregister` transform",
            },
            {
                "label": "Field",
                "text": "the field on the series' grid (`out_field`): larmorx's "
                + (
                    "equals sdcflows' in every voxel"
                    if field_same["sdcflows"] == 0
                    else f"differs from sdcflows' in {field_same['sdcflows']} voxels"
                )
                + (
                    ", at 1 and 14 threads and in both modes"
                    if field_same["larmorx-14"] == 0 and field_same["larmorx-fix"] == 0
                    else ""
                ),
            },
            {
                "label": "Programs",
                "text": "sdcflows 2.17.1 (num_threads 8) in both kept "
                "environments; larmorx's `lx.sdcflows.apply_coeffs_field` at commit "
                f"`{info['larmorx_commit'][:12]}`",
            },
            {"label": "Volumes", "text": "shown: file volumes " + ", ".join(map(str, shown))},
            {"label": "Grid", "text": common.describe_grid(bimg, grid.shape)},
        ],
        **out,
        "chain": None,
        "metrics_note": f"Δ is A − B over all {n_val:,} values of the "
        f"{' × '.join(map(str, full['sdcflows'].shape))} series (every volume), in the image's "
        "units (stored integers × scale factor).",
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
    }


def _sdc10_stats(e: fcache.Entry, t: dict) -> dict:
    p = t["prefix"]
    m = common.load(str(t["mask"])) > 0
    f = {k: _stored(e.dir / p / k / "field.nii.gz").astype(np.float64)
         for k in ("sdcflows", "larmorx", "larmorx-fix", "exact")}  # fmt: skip
    out = {"voxels": int(m.sum())}
    for k in ("sdcflows", "larmorx-fix"):
        d = (f[k] - f["exact"])[m]
        out[k] = {"rms": float(np.sqrt(np.mean(d * d))), "max": float(np.abs(d).max()),
                  "median": float(np.median(np.abs(d)))}  # fmt: skip
    out["same"] = _diff_count(f["larmorx"], f["sdcflows"]) == 0
    return out


def _example_sdc10(ctx: core.Context, links: dict) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    e = sdc10_entry(ctx)
    info = e.info()
    t = _sdc10_targets()[0]
    p = t["prefix"]
    timg = nib.load(str(t["target"]))
    mask = common.load(str(t["mask"])) > 0
    show = _show(mask, 1)
    grid = core.Grid.around(timg, show)
    ex = exm.Example(ctx, grid, show)
    ref, rs, ri = common.stored(t["target"])
    ex.add(exm.Panel("bref", "input", f"ds002603 sub-01: BOLD reference ({p.split('_')[-1]})",
                     "fMRIPrep's `desc-hmc_boldref`, an oblique grid", {"all": ref}, scale=rs,
                     inter=ri))  # fmt: skip
    arrays = {
        k: _stored(e.dir / p / k / "field.nii.gz")
        for k in ("exact", "sdcflows", "larmorx", "larmorx-fix", "sdcflows-shared")
    }
    for pid, role, label, sub in [
        ("exact", "reference", "The exact field where the transform puts the target",
         "every voxel's B-spline weights at its registered position (numpy, SciPy)"),
        ("sdcflows", "original", "sdcflows 2.17.1, B0FieldTransform.fit",
         "approx=True: the grid built and sampled where the target lies unregistered"),
        ("larmorx", "larmorx", "larmorx (port), reproduce mode", "sdcflows' result"),
        ("larmorx-fix", "larmorx", "larmorx (port), default mode",
         "SDC-10 corrected: the grid built and sampled where the transform puts the target"),
        ("sdcflows-shared", "original", "sdcflows 2.17.1, B0FieldTransform.fit",
         "numpy 2.3.5, SciPy 1.15.3: the shared environment's releases"),
    ]:  # fmt: skip
        ex.add(exm.Panel(pid, role, label, sub, {"field": arrays[pid]}))
    d = common.d
    diffs = [
        d("lx-sdc", "larmorx", "sdcflows", "main", "larmorx (reproduce) − sdcflows 2.17.1",
          common.ROLE_LX),
        d("fix-lx", "larmorx-fix", "larmorx", "main", "larmorx: default − reproduce mode (SDC-10)",
          "larmorx's correction"),
        d("sdc-exact", "sdcflows", "exact", "reference", "sdcflows − the exact field",
          "against the reference"),
        d("fix-exact", "larmorx-fix", "exact", "reference", "larmorx's default − the exact field",
          "against the reference"),
        d("newest-shared", "sdcflows", "sdcflows-shared", "variability",
          "sdcflows: numpy 2.5.4 / SciPy 1.18.1 − numpy 2.3.5 / SciPy 1.15.3",
          "the original against itself: the version step"),
    ]  # fmt: skip
    outputs = [{"id": "field", "label": "Field map on the BOLD grid (Hz)", "unit": "Hz"}]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    st = _sdc10_stats(e, t)
    named = info["targets"][p]["larmorx-fix"]["named"]
    for x in diffs:
        i = x["id"]
        x["links"] = _link(links, ["SDC-10"]) if i != "newest-shared" else []
        if i == "lx-sdc":
            x["verdict"] = "bit-identical" if same[i] else "open"
            x["reason"] = (
                "In its reproduce mode larmorx gives sdcflows' field, every value."
                if same[i]
                else "Not accounted for (see the record)."
            )
        elif i == "sdc-exact":
            s = st["sdcflows"]
            x["verdict"] = None
            x["reason"] = (
                f"sdcflows' field departs from the exact one by {s['rms']:.2f} Hz RMS (median "
                f"{s['median']:.2f}, largest {s['max']:.1f} Hz) in the {st['voxels']:,} voxels of "
                "the BOLD brain mask: it is the field where the target lies before the "
                "registration, not where the transform puts it."
            )
        elif i == "fix-exact":
            s = st["larmorx-fix"]
            x["verdict"] = None
            x["reason"] = (
                f"larmorx's default gives the exact field: {s['rms']:.2g} Hz RMS, largest "
                f"{s['max']:.2g} Hz, in the brain mask (the float32 sums of the B-splines taken "
                "in another order)."
                if s["max"] < 0.05
                else f"larmorx's default stays at the exact field: {s['rms']:.3f} Hz RMS "
                f"(median {s['median']:.3f}, largest {s['max']:.2f} Hz) in the brain mask, the "
                "size of the approximation's own interpolation (a fine aligned grid, then cubic "
                "B-splines onto the target)."
            )
        elif i == "fix-lx":
            better = st["larmorx-fix"]["rms"] < st["sdcflows"]["rms"]
            x["verdict"] = "d13" if better and "SDC-10" in named else "open"
            x["reason"] = (
                "SDC-10, corrected by default with a notice (rule A): sdcflows' approximation "
                "builds its fine aligned grid around the target as given and samples it at the "
                "target's own coordinates, so `xfm_data2fmap` changes nothing but the choice of "
                "the path; larmorx builds and samples it where the transform puts the target. "
                f"The default names {', '.join(named) or 'nothing'}; "
                "`upstream_defects='reproduce'` gives sdcflows' field."
            )
            if "SDC-01" in named:
                x["links"] += _link(links, ["SDC-01"])
        else:
            x["verdict"] = None
            x["reason"] = "The version step, a reference row: " + (
                "the same values with both releases." if same[i] else "other values."
            )
    at, _k = common.largest_in_slice(ex, "larmorx-fix", "larmorx", "field", "axial")
    default = {
        "output": "field",
        "plane": "axial",
        "voxel": at,
        "panels": ["bref", "exact", "sdcflows", "larmorx-fix"],
        "diffs": ["sdc-exact", "fix-exact", "fix-lx"],
        "compare": {"a": "sdcflows", "b": "larmorx-fix"},
    }
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="exact",
        caption="the crosshair where the correction changes the field most in the slice; the "
        "differences on one symmetric scale.",
    )  # fmt: skip
    data = _data(SDC10_RUN)
    n_vox = int(np.prod(timg.shape[:3]))
    xf = t["xfm"]
    ang = float(np.degrees(np.arccos(np.clip((np.trace(xf[:3, :3]) - 1) / 2, -1, 1))))
    return {
        "id": "sdc10-ds002603",
        "title": "SDC-10: the field on an oblique BOLD reference, sdcflows' and the corrected one",
        "summary": "sdcflows evaluates a field map on an EPI target with `B0FieldTransform.fit` "
        "(the first step of `ApplyCoeffsField` and of its unwarping workflow). When the target "
        "is not aligned with the coefficients' grid it takes an approximation: a fine aligned "
        "grid around the target, then cubic B-splines onto the target. sdcflows builds that "
        "grid around the target as given and samples it there, ignoring the registration "
        "(`xfm_data2fmap`); the field is taken where the target lies before it is registered. "
        "fMRIPrep's own reconstruction never takes this path (it refuses grids that are not "
        "aligned); sdcflows' unwarping does. ds002603's oblique BOLD reference with fMRIPrep's "
        "registration to the field map, against the exact field.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {data['subject']} ({data['licence']})"
                f": fMRIPrep's coefficients, BOLD reference `{p}` and registration; inside "
                "fMRIPrep's BOLD brain mask, dilated by one voxel",
            },
            {
                "label": "Registration",
                "text": f"fMRIPrep's `from-boldref_to-auto00000` "
                f"transform (inverted, as sdcflows takes it): a rotation of {ang:.2f}° and a shift "
                f"of {float(np.linalg.norm(xf[:3, 3])):.2f} mm",
            },
            {
                "label": "Defect",
                "text": f"[SDC-10](repo:{links.get('SDC-10', FINDINGS)}): the "
                "approximation ignores the transform (rule A)",
            },
            {
                "label": "Programs",
                "text": "sdcflows 2.17.1 in both kept environments; larmorx "
                f"at commit `{info['larmorx_commit'][:12]}`, 14 threads; the exact field by "
                "validation/investigations/sdcflows-fieldmap/measure_sdc01_02.py's `exact_field`",
            },
            {"label": "Grid", "text": common.describe_grid(timg, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against sdcflows, and its correction"],
            ["reference", "Each field against the exact one"],
            ["variability", "sdcflows against itself: the version step"],
        ],
        "metrics_note": f"Δ is A − B over all {n_vox:,} voxels of the BOLD reference's grid; "
        "the RMS, median and largest differences in the reasons are those inside the brain "
        f"mask ({st['voxels']:,} voxels).",
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
    }


# ------------------------------------------------------------------------------ tables


def _text(n: int) -> dict:
    return common.cell("identical" if n == 0 else ("shapes differ" if n < 0 else f"{n:,} differ"))


def _estimators_table(ctx: core.Context) -> dict:
    """Every phase-difference estimator of the oracle runs, output by output."""
    rows = []
    for key, run, est, what in ESTIMATORS:
        e = estimator_entry(ctx, key)
        n = node(key)
        runs, full, coeffs = _estimator_arrays(e, n["own"])
        ref = runs["larmorx"]
        sdc = {k: _diff_count(ref[k], runs["sdcflows"][k]) for k in ref}
        own = {k: _diff_count(ref[k], full["fmriprep"][k]) for k in full["fmriprep"]}
        own["coeff"] = _diff_count(coeffs["larmorx"], coeffs["fmriprep"])
        wf = {k: _diff_count(full["workflow"][k], full["fmriprep"][k]) for k in ("rads", "fmap")}
        thr = sum(_diff_count(ref[k], runs["larmorx-14"][k]) for k in ref)
        fix = sum(_diff_count(runs["larmorx-14"][k], runs["larmorx-fix"][k]) for k in ref)
        step = sum(_diff_count(runs["sdcflows"][k], runs["sdcflows-shared"][k]) for k in ref)
        c_sdc = _diff_count(coeffs["larmorx"], coeffs["sdcflows"])
        cells = [
            _text(sdc["rads"]), _text(sdc["fmap"]), _text(c_sdc),
            _text(sdc["extrapolated"]), _text(sdc["error"]),
            _text(sum(own.values())), _text(sum(wf.values())), _text(thr), _text(fix),
            _text(step),
        ]  # fmt: skip
        ok = all(v == 0 for v in (*sdc.values(), *own.values(), *wf.values(), thr, fix, c_sdc))
        cells.append({"verdict": "bit-identical" if ok else "open"})
        img = n["spec"]["fmap"]
        import nibabel as nib

        shape = nib.load(img).shape
        rows.append({
            "head": f"{run.split('-noreconall')[0]} `{est.rsplit('/', 1)[-1]}`",
            "sub": f"{what}; {' × '.join(map(str, shape[:3]))} voxels",
            "cells": cells,
            "verdict": "bit-identical" if ok else "open",
        })  # fmt: skip
    return common.table(
        "estimators",
        "fMRIPrep's seven phase-difference estimators: larmorx against sdcflows and fMRIPrep",
        [
            "Estimator",
            "Phase (rad)",
            "Field map",
            "Coefficients",
            "Fitted field",
            "Error",
            "fMRIPrep's files",
            "Workflow",
            "1 vs 14 threads",
            "Default vs reproduce",
            "sdcflows' step",
            "Verdict",
        ],
        rows,
        note="Each estimator of the five oracle runs that have one, step by step on the inputs "
        "fMRIPrep's nodes were given (the examples above show three): larmorx (1 thread, "
        "reproduce mode) against sdcflows 2.17.1 (newest releases) output by output; against "
        "fMRIPrep 25.2.6's own files (the phase difference, the field map, the extrapolated "
        "field and the coefficients); larmorx's whole workflow with lx.fsl.prelude's clean-room "
        "original against fMRIPrep's phase difference and field map; larmorx at 14 threads and "
        "in its default mode against itself; sdcflows with the shared environment's releases "
        "against the newest (the originals' own step). Values counted as stored (NaN alike).",
    )


def _refit_table(ctx: core.Context, links: dict) -> dict:
    e = refit_entry(ctx)
    info = e.info()
    rows = []
    for name, r in info["refits"].items():
        runs = r["runs"]
        same = True
        for role in ("extrapolated", "coeff"):
            fa = runs["larmorx-fix-4d"]["files"].get(role)
            fb = runs["sdcflows-3d"]["files"].get(role)
            if not fa or not fb:
                same = False
                continue
            a = _stored(e.dir / name / "larmorx-fix-4d" / fa)
            b = _stored(e.dir / name / "sdcflows-3d" / fb)
            if a.size == b.size:  # the field keeps the input's 4D shape of one volume
                a = a.reshape(b.shape)
            same &= _diff_count(a, b) == 0
        err_s = runs["sdcflows-4d"].get("error") or "fits"
        err_l = runs["larmorx-4d"].get("error") or "fits"
        named = runs["larmorx-fix-4d"].get("named") or []
        ok = same and err_s == err_l and runs["sdcflows-4d"].get("error") and "SDC-06" in named
        rows.append({
            "head": f"{r['run'].split('-noreconall')[0]} `{name}`",
            "sub": f"{' × '.join(map(str, r['shape']))} voxels",
            "cells": [common.cell(common.clip(err_s, 120)),
                      common.cell("the same error" if err_s == err_l else common.clip(err_l, 120)),
                      common.cell(("the 3D map's fit, every value (stored in the input's 4D shape)" if same else "differs")
                                  + (f"; names {', '.join(named)}" if named else "")),
                      {"verdict": "d13" if ok else "open"}],
            "verdict": "d13" if ok else "open",
        })  # fmt: skip
    return common.table(
        "sdc06",
        "SDC-06: BSplineApprox on fMRIPrep's own field maps (4D, one volume)",
        [
            "Field map",
            "sdcflows 2.17.1",
            "larmorx, reproduce mode",
            "larmorx, default mode",
            "Verdict",
        ],
        rows,
        note="fMRIPrep writes its field maps (`desc-preproc_fieldmap`) as 4D images of one "
        "volume; fitting one again with sdcflows' BSplineApprox (fMRIPrep's parameters, the "
        "map's own brain mask) fails in `bspline_grid`, which broadcasts the header's four "
        "zooms against three. SDC-06 (rule A): larmorx's default takes the first three zooms "
        "and gives the fit of the same map stored as a 3D image (sdcflows' own result there), "
        "the coefficients and the extrapolated field; its reproduce mode raises sdcflows' error. "
        f"[SDC-06](repo:{links.get('SDC-06', FINDINGS)}).",
    )


def _sdc10_table(ctx: core.Context, links: dict) -> dict:
    e = sdc10_entry(ctx)
    info = e.info()
    rows = []
    for t in _sdc10_targets():
        st = _sdc10_stats(e, t)
        named = info["targets"][t["prefix"]]["larmorx-fix"]["named"]
        ok = st["same"] and st["larmorx-fix"]["rms"] < st["sdcflows"]["rms"] and ("SDC-10" in named)
        rows.append({
            "head": f"ds002603 `{t['prefix']}`",
            "sub": f"{st['voxels']:,} voxels in the BOLD brain mask",
            "cells": [common.cell(st["sdcflows"]["rms"], "sig"),
                      common.cell(st["sdcflows"]["max"], "sig"),
                      common.cell(st["larmorx-fix"]["rms"], "sig"),
                      common.cell(st["larmorx-fix"]["max"], "sig"),
                      common.cell("identical" if st["same"] else "differs"),
                      common.cell(", ".join(named) or "–"),
                      {"verdict": "d13" if ok else "open"}],
            "verdict": "d13" if ok else "open",
        })  # fmt: skip
    return common.table(
        "sdc10",
        "SDC-10 on ds002603's two oblique BOLD references (Hz, inside the brain mask)",
        [{"label": "sdcflows − exact, RMS", "num": True}, {"label": "largest", "num": True},
         {"label": "larmorx default − exact, RMS", "num": True}, {"label": "largest", "num": True},
         "larmorx reproduce = sdcflows", "Named", "Verdict"],
        rows,
        note="B0FieldTransform.fit(approx=True) with fMRIPrep's coefficients and its "
        "registration of each BOLD reference to the field map, against the exact field where "
        "the transform puts the target. "
        f"[SDC-10](repo:{links.get('SDC-10', FINDINGS)}).",
    )  # fmt: skip


def _memory_table(ctx: core.Context) -> dict:
    e = memory_entry(ctx)
    peaks = e.info()["peaks_mib"]
    labels = {
        "fit-ds002422": "BSplineApprox, ds002422's first field map (64 × 64 × 24)",
        "fit-ds005454": "BSplineApprox, ds005454's field map (112 × 112 × 70)",
        "apply-ds005454": f"ApplyCoeffsField, {MEMORY_VOLUMES} volumes of ds005454's BOLD "
        "(160 × 160 × 96)",
    }
    rows = []
    for name, p in peaks.items():
        sdc = p["sdcflows"]
        allow = 1.1 * sdc + 8.0
        worst = max(p["larmorx-reproduce"], p["larmorx-fix"])
        rows.append({
            "head": labels.get(name, name),
            "cells": [common.cell(round(sdc), "count"),
                      common.cell(round(p["larmorx-reproduce"]), "count"),
                      common.cell(round(p["larmorx-fix"]), "count"),
                      common.cell(worst / sdc, "sig")],
            "verdict": "bit-identical" if worst <= allow else "open",
        })  # fmt: skip
    for r in rows:  # a resource row, not a comparison of values
        r.pop("verdict")
    return common.table(
        "memory",
        "Peak memory (MiB): the memory rule's calls, one thread",
        [
            "Call",
            {"label": "sdcflows", "num": True},
            {"label": "larmorx, reproduce", "num": True},
            {"label": "larmorx, default", "num": True},
            {"label": "larmorx ÷ sdcflows", "num": True},
        ],
        rows,
        note="Each call in a process of its own (validation/investigations/sdcflows-fieldmap/"
        "memory_call.py, the calls of tests/parity/test_memory_peak.py), its peak resident "
        "memory from /usr/bin/time -v, in the newest environment at one thread. The rule: at "
        "most 10 % above the original's peak plus 8 MiB. sdcflows holds the collocation matrix "
        "at least three times (its matrix, scikit-learn's copy, the adjoint's transpose); "
        "larmorx stores it once at one thread (and evaluates the weights at every product with "
        "more threads), and unwarps the series volume by volume.",
    )


def _step_table(rec) -> dict:
    """The originals' own step from the record: lines compared, identical, changed."""
    from larmorx_validation.parity import python_versions as pv

    reason = re.search(r"every change has a known reason \((LAPACK-[\d.]+)", rec.text)
    rows = []
    total = same = 0
    for r in rec.results:
        lines = {k: v for k, v in r.get("measured", {}).items() if k.startswith(pv.STEP_CHECK)}
        if not lines:
            continue
        ch = [k for k, v in lines.items() if v == "changed"]
        total += len(lines)
        same += len(lines) - len(ch)
        if ch:
            rows.append({
                "head": f"`{r['case']}`",
                "cells": [common.cell(len(lines), "count"), common.cell(len(ch), "count"),
                          common.cell(reason.group(1) if reason else "see the record")],
            })  # fmt: skip
    rows.insert(0, {
        "head": "every case",
        "cells": [common.cell(total, "count"), common.cell(total - same, "count"),
                  common.cell("–")],
    })  # fmt: skip
    return common.table(
        "step",
        "The originals' own step: sdcflows with numpy 2.3.5 / SciPy 1.15.3 and with numpy "
        "2.5.4 / SciPy 1.18.1",
        [
            "Case",
            {"label": "Lines compared", "num": True},
            {"label": "Changed", "num": True},
            "Reason",
        ],
        rows,
        note="The record's step (python_versions): every line of every case whose script "
        "passed the originals' outputs to stepdigest.oracle, compared between the shared "
        "releases (two runs) and the newest. The changes have one reason, LAPACK-3.12.1: "
        "numpy's OpenBLAS 0.3.31 and later bundle LAPACK 3.12.1, which moves the last bits of "
        "numpy's svd and eigh, so nibabel writes other NIfTI quaternions for some oblique "
        "affines and its io_orientation decides a tied rotation otherwise (larmorx follows "
        "3.12.1, the newest, and reproduces 3.12.0 for the shared environment's comparison).",
    )


def trial(ctx: core.Context) -> tuple[list[dict], dict]:
    """Random testing's trial of this part (``summary.json``) and its triage: two tables."""
    import tomllib

    path = common.oracles() / "difftest" / TRIAL / "summary.json"
    raw = path.read_bytes()
    s = json.loads(raw)
    t = s["tools"][TOOL]
    o, im = t["options"], t["implementations"]["port"]
    calls = ("bspline_approx", "fit", "apply", "displacements", "topup", "phase")
    rows = [
        ("Random cases, each compared with sdcflows 2.17.1", t["cases"]),
        ("bit-identical (every output, the messages, the default's notices)", im.get("agree", 0)),
        ("differing (triaged below: fixed in larmorx, or SDC-11 registered)", im.get("differ", 0)),
        ("a notice without a visible change (SCI-02; triaged below)", im.get("notice", 0)),
        ("calls: " + ", ".join(calls), [o.get(c, 0) for c in calls]),
        ("fits: one, two control grids", [o.get("1 level(s)", 0), o.get("2 level(s)", 0)]),
        ("targets: own grid, aligned, rotated", [o.get("target own grid", 0),
                                                 o.get("target aligned", 0),
                                                 o.get("target rotated", 0)]),
        ("transforms: none, identity, rigid", [o.get("transform none", 0),
                                               o.get("transform identity", 0),
                                               o.get("transform rigid", 0)]),
        ("recentring: median, mean, mode, none", [o.get("recenter median", 0),
                                                  o.get("recenter mean", 0),
                                                  o.get("recenter mode", 0),
                                                  o.get("recenter False", 0)]),
        ("4D field maps of one volume (SDC-06)", o.get("4D field map of one volume (SDC-06)", 0)),
        ("distinct options and settings drawn", t["options_covered"]),
        ("length (minutes; 12 jobs at one thread each)", round(s["elapsed_s"] / 60, 1)),
    ]  # fmt: skip
    out_rows = []
    for head, v in rows:
        if isinstance(v, list):
            cell = common.cell(" / ".join(f"{x:,}" for x in v))
        elif isinstance(v, float):
            cell = common.cell(v, "sig")
        else:
            cell = common.cell(int(v), "count")
        out_rows.append({"head": head, "cells": [cell]})
    peaks = max(float(p.split()[0]) for p in s["memory"]["peaks"])
    table = common.table(
        "trial",
        f"Random testing: the trial of {s['date']}",
        ["", {"label": "Cases", "num": True}],
        out_rows,
        note=f"Differential random testing's trial of this part (seed `{s['seed']}`, larmorx "
        f"`{s['larmorx_commit']}`): random field maps, masks, grids and orientations (oblique, "
        "permuted, qform-only headers, NIfTI-2), float32, float64 and scaled int16 images; "
        "BSplineApprox's options, B0FieldTransform's fit and apply on own, aligned and rotated "
        "targets with and without transforms, ApplyCoeffsField's options, the displacement "
        "fields, TOPUPCoeffReorient with FSL and BIDS letters, the phase steps; sdcflows "
        "in-process, larmorx in its reproduce mode compared output by output, its default mode "
        f"checked by D13's rule. Every case in a memory box of {s['memory']['case']} (peaks "
        f"below {peaks:.2f} GiB).",
    )
    tri = tomllib.loads(
        (ctx.repo / "validation" / "difftest" / "triage.toml").read_text(encoding="utf-8")
    )
    trows = []
    for d in tri.get("difference", []):
        if d.get("tool") != TOOL:
            continue
        trows.append({
            "head": d.get("status", ""),
            "cells": [common.cell(common.clip(d.get("explanation", ""), 420)),
                      common.cell(d.get("defect", "–"))],
            "sub": d.get("link", ""),
        })  # fmt: skip
    triage = common.table(
        "triage",
        "Random testing: every difference of the trial, triaged",
        ["Status", "What it was", "Register"],
        trows,
        note="validation/difftest/triage.toml (block sdcflows-fieldmap, 2026-11-08); each "
        "difference has a permanent case (validation/difftest/cases/sdcflows-fieldmap/), run "
        "by the record. The two short probes before the trial (relays 6 and 7) found larmorx's "
        "mistakes in nibabel's header arithmetic, SciPy's float64 stacks and numpy's NaN, all "
        "fixed, each with a permanent case.",
    )
    prov = {"trial": f"oracles/difftest/{TRIAL}/summary.json",
            "sha256": hashlib.sha256(raw).hexdigest()}  # fmt: skip
    return [table, triage], prov


def _speed(bench) -> dict | None:
    """The speed chart from the benchmark report (its own format: one row per call)."""
    if bench is None:
        return None
    series = [
        {"id": "sdcflows|1", "label": "sdcflows, 1 thread", "mark": "original"},
        {"id": "sdcflows|14", "label": "sdcflows, 14 threads (ApplyCoeffsField)",
         "mark": "original-open"},
        {"id": "larmorx|1", "label": "larmorx, 1 thread", "mark": "larmorx"},
        {"id": "larmorx|14", "label": "larmorx, 14 threads", "mark": "larmorx-open"},
    ]  # fmt: skip
    rows = []
    for r in bench.data["results"]:
        sec = {}
        if r["call"] == "BSplineApprox":
            sec["sdcflows|1"] = r["sdcflows_s"]
        else:
            sec["sdcflows|1"] = r["sdcflows_1_s"]
            sec["sdcflows|14"] = r["sdcflows_14_s"]
        sec["larmorx|1"] = r["larmorx_1_s"]
        sec["larmorx|14"] = r["larmorx_14_s"]
        label = (f"{r['call']}: {r['case']}" if r["call"] != "BSplineApprox"
                 else f"bs_filter: {r['case']}")  # fmt: skip
        rows.append({
            "label": f"{label} ({' × '.join(map(str, r['shape']))})",
            "input": r["case"],
            "seconds": sec,
            "speedup": sec["sdcflows|1"] / sec["larmorx|1"],
        })  # fmt: skip
    axis, ticks = common.ticks_log([v for row in rows for v in row["seconds"].values()], " s")
    env = bench.data.get("environment", {})
    return {
        "title": "Seconds per call (median of 3), log scale; the factor is larmorx's speed-up "
        "over sdcflows at one thread",
        "axis": axis,
        "ticks": ticks,
        "series": series,
        "rows": rows,
        "note": "From the benchmark report `docs/benchmarks/sdcflows-fieldmap.json` (commit "
        f"{env.get('commit', '?')}, {env.get('cpus', '?')} logical CPUs, the newest "
        "environment, OPENBLAS_NUM_THREADS=1 as fMRIPrep's nodes; every output byte-identical). "
        "BSplineApprox has no thread option: sdcflows' fit runs on one thread; larmorx's at "
        "one thread stores the collocation matrix, at 14 evaluates the weights at every "
        "product (the same bits). ds001600's three one-thread fits ran after the benchmark's "
        "first calls in the same process and took longer than in a fresh process (the "
        "report's footnote 1: the stored matrix's speed depends on what the process ran "
        "before, an open follow-up).",
    }


# --------------------------------------------------------------------------- the cases


def _cases(ctx: core.Context, rec, links: dict) -> list[dict]:
    from larmorx_validation.difftest import permanent
    from larmorx_validation.parity import sdcflows_fieldmap as suite

    perm = {f"difftest/{p['id']}": p for p in permanent.load(TOOL)}
    out = []
    for r in rec.results:
        cid = r["case"]
        v = common.outcome_verdict(r)
        p = perm.get(cid)
        if cid in suite.CASES:
            script, _tier, desc = suite.CASES[cid]
            command = f"python -P validation/investigations/sdcflows-fieldmap/{script}"
        else:
            desc = p["description"] if p else ""
            command = ""
        if v is None:
            fixed = (r.get("reported") or {}).get("D13 fix pass")
            lines = r["checks_total"]
            if p and p.get("defect") and fixed:
                v = ("d13", common.clip(
                    f"{p['defect']}, corrected by default (rule {p.get('rule', '?')}): in its "
                    f"reproduce mode larmorx gives sdcflows' outputs bit for bit; the default "
                    f"changes {int(fixed):,} values and names {p['defect']}.", 400))  # fmt: skip
            elif p and p.get("defect"):
                v = ("bit-identical", common.clip(
                    f"Bit-identical to sdcflows in reproduce mode; {p['defect']}'s case: the "
                    "default's result checked by D13's rule.", 400))  # fmt: skip
            elif p:
                v = (
                    "bit-identical",
                    "Bit-identical to sdcflows (a mistake of larmorx's found "
                    "by random testing, fixed; its permanent case).",
                )
            else:
                v = (
                    "bit-identical",
                    f"Every one of its {lines} checks: the outputs "
                    "bit-identical to sdcflows' (or fMRIPrep's own files), the messages too, "
                    "and the originals' step per line.",
                )
        out.append({
            "id": cid,
            "category": r["category"],
            "description": desc,
            "command": command,
            "results": {
                "port": {
                    "outcome": r["status"],
                    "verdict": v[0],
                    "checks": f"{r['checks_passed']}/{r['checks_total']}",
                    "reason": v[1],
                }
            },
        })  # fmt: skip
    return out


def _corrections_table(ctx: core.Context, links: dict, cases: list[dict]) -> dict:
    """Every sdcflows defect of the findings: its rule, where this page or the record shows
    it."""
    text = (ctx.repo / FINDINGS).read_text(encoding="utf-8")
    shown = {
        "SDC-01": "named in the SDC-10 example (sampling positions beyond the grid)",
        "SDC-02": "the unwarping example (fMRIPrep's registration rotates ds005454's grid)",
        "SDC-06": "the SDC-06 table (fMRIPrep's own field maps)",
        "SDC-10": "the SDC-10 example and table (ds002603's BOLD references)",
    }
    rows = []
    for m in re.finditer(r"^### (SDC-\d+): (.*?) \(rule ([^)]*)\)\s*$", text, re.M):
        sid, title, rule = m.group(1), m.group(2), m.group(3)
        case = next((c["id"] for c in cases if c["id"].startswith(f"difftest/{sid.lower().replace('-', '')}")),
                    None)  # fmt: skip
        rows.append({
            "head": f"[{sid}](repo:{links.get(sid, FINDINGS)})",
            "cells": [common.cell(title.replace("`", "")), common.cell(rule),
                      common.cell(shown.get(sid, "–")),
                      common.cell(f"`{case}`" if case else "–")],
        })  # fmt: skip
    return common.table(
        "corrections",
        "sdcflows' defects found by the port (SDC-01 to SDC-11)",
        ["Entry", "What", "Rule", "On this page", "The record's case"],
        rows,
        note="Each decided by rules A to D (A: corrected by default; B: a clear error; C: kept "
        "and documented; D: measured first), each with `upstream_defects='reproduce'` giving "
        "sdcflows' behaviour; the findings: docs/findings/sdcflows-fieldmap.md. fMRIPrep's "
        "calls meet none of them (its reconstruction refuses grids that are not aligned; it "
        "passes FSL's letters to TOPUPCoeffReorient); sdcflows' own interfaces and workflows "
        "do. SDC-03 waits for FreeSurfer's mri_robust_register (CheckRegister's two-magnitude "
        "branch).",
    )


# -------------------------------------------------------------------------------- export


def entries(ctx: core.Context) -> list[fcache.Entry]:
    return [
        *(estimator_entry(ctx, k) for k, *_ in ESTIMATORS),
        apply_entry(ctx),
        sdc10_entry(ctx),
        refit_entry(ctx),
        memory_entry(ctx),
    ]


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, RECORD)
    bench = records.load_bench(ctx.repo, RECORD)
    links = _findings_links(ctx.repo)
    ents = entries(ctx)
    cases = _cases(ctx, rec, links)
    examples = [_example_estimator(ctx, k, links) for k in SHOWN]
    examples.append(_example_coefficients(ctx))
    examples.append(_example_apply(ctx, links))
    examples.append(_example_sdc10(ctx, links))
    trial_tables, tprov = trial(ctx)
    tables = [
        _estimators_table(ctx),
        _sdc10_table(ctx, links),
        _refit_table(ctx, links),
        _corrections_table(ctx, links, cases),
        _step_table(rec),
        _memory_table(ctx),
        *trial_tables,
    ]
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "sdcflows' field-map estimation",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": bench.source() if bench else [],
            "cache": [fcache.provenance(x) for x in ents],
            "difftest": tprov,
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cases,
                "port",
                "larmorx (port), against sdcflows 2.17.1 (standard tier)",
                "port",
                f"docs/validation/{RECORD}.md",
                "validated",
            )
        ],
        "cases": cases,
        "cases_total": len(cases),
        "speed": _speed(bench),
        "tables": tables,
        "examples": examples,
    }


if __name__ == "__main__":  # compute the cache entries only
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    for x in entries(c):
        print("done", x.dir, flush=True)
