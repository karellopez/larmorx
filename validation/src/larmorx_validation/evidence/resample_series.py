# SPDX-License-Identifier: Apache-2.0
"""The one-shot BOLD resampler's evidence (``lx.transforms.resample_series`` against
fMRIPrep's ``ResampleSeries``): fMRIPrep's native resampling of a real BOLD run with its
field map (head motion, susceptibility distortion and the Jacobian in one step), by fMRIPrep
25.2.6 as it ran and by larmorx (1 and 8 threads, ``reproduce`` and default modes); PIPE-03,
fMRIPrep's Jacobian on another grid, corrected by default, with its measured effect; the
record's cases; speed.
"""

from __future__ import annotations

from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "resample-series"
CASE = "fmriprep-ds005040-native"
RUN = "ds005040-sub-002-noreconall"
NODE = "bold_native_wf/boldref_bold"
OUT = "sub-002_task-clip_run-1_bold_tshift_xformresampled.nii.gz"
MASK = "out/sub-002/func/sub-002_task-clip_run-1_desc-brain_mask.nii.gz"
JACOBIAN_CASE = "ds005040-sub-002-jacobian-mni"
N_SHOWN = 6


def _node():
    from larmorx_validation.stages.catalogue import normalise
    from larmorx_validation.stages.oracle import OracleRun

    root = common.fmriprep_run(RUN)
    run = OracleRun(root)
    for node in run.all_nodes:
        if node.interface == "resampling" and normalise(node.path)[0].endswith(NODE):
            from larmorx_validation.parity.resample_jacobian_oracle import _value

            return run, node, {k: _value(v) for k, v in node.inputs.items()}
    raise FileNotFoundError(f"{RUN}: no {NODE} node")


def _compute(e: fcache.Entry) -> None:
    import warnings

    import nibabel as nib

    import larmorx as lx

    run, _nd, inputs = _node()
    src = e.input("BOLD after slice timing (the node's in_file)", Path(run.host(inputs["in_file"])))
    ref = e.input("BOLD reference (ref_file)", Path(run.host(inputs["ref_file"])))
    fmap = e.input("field-map coefficients (fieldmap)", Path(run.host(inputs["fieldmap"])))
    xfms = [
        str(e.input(f"head motion (transforms[{i}])", Path(run.host(t))))
        for i, t in enumerate(inputs["transforms"])
    ]
    e.input("fMRIPrep's output", _fmriprep_out())
    for threads in (1, 8):
        for mode in ("fix", "reproduce"):
            label = f"larmorx-{threads}" + ("-reproduce" if mode == "reproduce" else "")
            with warnings.catch_warnings(record=True) as w:
                warnings.simplefilter("always")
                res = lx.transforms.resample_series(
                    str(src),
                    str(ref),
                    xfms,
                    inverse=inputs.get("inverse") or False,
                    fieldmap=str(fmap),
                    pe_dir=inputs.get("pe_dir"),
                    ro_time=inputs.get("ro_time"),
                    jacobian=bool(inputs.get("jacobian")),
                    order=int(inputs.get("order", 3)),
                    mode=str(inputs.get("mode", "grid-constant")),
                    cval=float(inputs.get("cval", 0.0)),
                    prefilter=bool(inputs.get("prefilter", True)),
                    n_threads=threads,
                    upstream_defects=mode,
                )
            nib.save(
                nib.Nifti1Image(np.asarray(res.data, np.float32), res.affine),
                e.dir / f"{label}.nii.gz",
            )
            (e.dir / f"{label}.warnings.txt").write_text(
                "\n".join(str(x.message) for x in w), encoding="utf-8"
            )
    e.finish(
        settings="fMRIPrep 25.2.6's ResampleSeries node bold_native_wf/boldref_bold of "
        f"{RUN} (task clip, run 1), its inputs read from the oracle run's working directory "
        "(the slice-time corrected BOLD, the BOLD reference, the head-motion transforms, the "
        "field map's coefficients, pe_dir, ro_time, jacobian, order, mode); larmorx's "
        "lx.transforms.resample_series in this process, reproduce and default modes, 1 and 8 "
        "threads (fMRIPrep ran 8)",
        node_inputs={k: str(v) for k, v in inputs.items() if k not in ("in_file", "ref_file")},
    )


def _fmriprep_out() -> Path:
    root = common.fmriprep_run(RUN)
    p = sorted((root / "work").glob(f"**/{NODE}/{OUT}"))
    if not p:
        raise FileNotFoundError(f"fMRIPrep's {NODE} output")
    return p[0]


#: The entry of fMRIPrep's resampler run in each kept Python environment (the version update's
#: part 3, 2026-10-29: numpy 2.5.4 and SciPy 1.18.1 beside numpy 2.3.5 and SciPy 1.15.3).
NUMERICS_CASE = "fmriprep-ds005040-native-numerics"


def _compute_numerics(e: fcache.Entry) -> None:
    import json

    from larmorx_validation.parity import python_versions as pv

    run, _nd, inputs = _node()
    spec = dict(inputs)
    for k in ("in_file", "ref_file", "fieldmap"):
        spec[k] = str(e.input(f"the node's {k}", Path(run.host(inputs[k]))))
    spec["transforms"] = [
        str(e.input(f"the node's transforms[{i}]", Path(run.host(t))))
        for i, t in enumerate(inputs["transforms"])
    ]
    spec.pop("num_threads", None)
    (e.dir / "inputs.json").write_text(json.dumps(spec, indent=1), encoding="utf-8")
    for name in (pv.NEWEST, pv.SHARED):
        py = pv.prefix(name)
        if py is None:
            raise FileNotFoundError(f"the {name} Python environment ({pv.ENVS[name]})")
        e.run(
            f"fmriprep-{name}",
            [
                str(py / "bin" / "python"),
                "-m",
                "larmorx_validation.evidence.resample_series_run",
                str(e.dir / "inputs.json"),
                str(e.dir / f"fmriprep-{name}.nii.gz"),
            ],
            env={
                "PYTHONPATH": str(e.ctx.repo / "validation" / "src"),
                "OPENBLAS_CORETYPE": "Haswell",
            },
        )
    e.finish(
        settings="fMRIPrep's ResampleSeries (nipype, fMRIPrep at 21a490fb as installed in each "
        "environment) on the node's own inputs, in oracles/python-newest (numpy 2.5.4 with "
        "OpenBLAS 0.3.34, SciPy 1.18.1) and in the shared .venv (numpy 2.3.5 with OpenBLAS "
        "0.3.30, SciPy 1.15.3), OpenBLAS's Haswell kernels",
    )


def numerics_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        NUMERICS_CASE,
        "fMRIPrep's resampler on ds005040 sub-002's native node, with the newest numerical "
        "releases and with the shared environment's",
        _compute_numerics,
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's native resampling (head motion, field map, Jacobian) of ds005040 sub-002 "
        "task-clip run 1: fMRIPrep 25.2.6 and larmorx",
        _compute,
    )


def jacobian_entry(ctx: core.Context) -> fcache.Entry:
    from larmorx_validation.figures.resample_series import _compute as compute

    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        JACOBIAN_CASE,
        "PIPE-03: the Jacobian factors of ds005040 sub-002's BOLD on the MNI grid",
        compute,
    )


# ----------------------------------------------------------------------------------- export


def _native(ctx: core.Context, reg: dict) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = entry(ctx)
    info = e.info()
    run, _nd, inputs = _node()
    root = common.fmriprep_run(RUN)
    fm = _fmriprep_out()
    fimg = nib.load(str(fm))
    fdata = np.asanyarray(fimg.dataobj)
    lx = {
        k: common.load(str(e.dir / f"{k}.nii.gz"))
        for k in ("larmorx-8-reproduce", "larmorx-1-reproduce", "larmorx-8", "larmorx-1")
    }
    n = fdata.shape[3]
    # the volumes shown: the ends, and those with the largest default/reproduce difference or
    # the most motion
    dv = (
        np.abs(lx["larmorx-8"].astype(np.float64) - lx["larmorx-8-reproduce"]).reshape(-1, n).max(0)
    )
    pick = list(dict.fromkeys([0, n - 1, int(np.argmax(dv))]))
    for t in np.linspace(0, n - 1, N_SHOWN).round().astype(int):
        if len(pick) >= N_SHOWN:
            break
        if int(t) not in pick:
            pick.append(int(t))
    shown = sorted(set(pick))[:N_SHOWN]
    brain = common.load(str(root / MASK)) > 0
    show = ndimage.binary_dilation(brain, iterations=1)
    grid = core.Grid.around(fimg, show)
    ex = exm.Example(ctx, grid, show)

    def vols(a):
        return np.ascontiguousarray(a[..., shown])

    raw, slope, inter = common.load_raw(run.host(inputs["in_file"]))
    ex.add(
        exm.Panel(
            "bold",
            "input",
            "ds005040 sub-002 BOLD, task clip run 1",
            "after fMRIPrep's slice-timing correction (the node's input), int16 × scale factor",
            {"all": vols(raw)},
            scale=slope,
            inter=inter,
        )
    )
    for pid, src, role, label, sub in [
        (
            "larmorx",
            lx["larmorx-8-reproduce"],
            "larmorx",
            "larmorx (port), 8 threads",
            "reproduce mode",
        ),
        (
            "larmorx-1",
            lx["larmorx-1-reproduce"],
            "larmorx",
            "larmorx (port), 1 thread",
            "reproduce mode",
        ),
        (
            "larmorx-fix",
            lx["larmorx-8"],
            "larmorx",
            "larmorx (port), 8 threads",
            "default mode (PIPE-03 and the rest corrected)",
        ),
        (
            "fmriprep",
            fdata,
            "fmriprep",
            "fMRIPrep 25.2.6, as it ran",
            "ResampleSeries in its image, 8 threads",
        ),
    ]:
        ex.add(exm.Panel(pid, role, label, sub, {"series": vols(src)}))
    # fMRIPrep's resampler on this machine with the newest numerical releases and with the
    # shared environment's (the version update's part 3).
    ne = numerics_entry(ctx)
    newest = common.load(str(ne.dir / "fmriprep-newest.nii.gz"))
    shared = common.load(str(ne.dir / "fmriprep-shared.nii.gz"))
    for pid, src, label, sub in [
        (
            "fmriprep-newest",
            newest,
            "fMRIPrep's resampler, numpy 2.5.4 / SciPy 1.18.1",
            "ResampleSeries here, the newest releases (oracles/python-newest), larmorx's reference",
        ),
        (
            "fmriprep-shared",
            shared,
            "fMRIPrep's resampler, numpy 2.3.5 / SciPy 1.15.3",
            "ResampleSeries here, the shared environment's releases",
        ),
    ]:
        ex.add(exm.Panel(pid, "fmriprep", label, sub, {"series": vols(src)}))
    dd = common.d
    diffs = [
        dd(
            "lx-fmriprep",
            "larmorx",
            "fmriprep",
            "fmriprep",
            "larmorx − fMRIPrep's own output",
            common.ROLE_FMRIPREP,
        ),
        dd(
            "fix-lx",
            "larmorx-fix",
            "larmorx",
            "main",
            "larmorx: default − reproduce mode",
            "larmorx's corrections",
        ),
        dd(
            "lx1-lx8",
            "larmorx-1",
            "larmorx",
            "check",
            "larmorx: 1 thread − 8 threads",
            common.ROLE_SELF,
        ),
        dd(
            "lx-newest",
            "larmorx",
            "fmriprep-newest",
            "main",
            "larmorx − fMRIPrep's resampler, numpy 2.5.4 / SciPy 1.18.1",
            common.ROLE_FMRIPREP,
        ),
        dd(
            "newest-shared",
            "fmriprep-newest",
            "fmriprep-shared",
            "variability",
            "numpy 2.5.4 / SciPy 1.18.1 − numpy 2.3.5 / SciPy 1.15.3 (fMRIPrep's resampler)",
            "the original against itself: the version step",
        ),
    ]
    outputs = [{"id": "series", "label": "Resampled series", "unit": ""}]
    full = {
        "larmorx": {"series": lx["larmorx-8-reproduce"]},
        "larmorx-1": {"series": lx["larmorx-1-reproduce"]},
        "larmorx-fix": {"series": lx["larmorx-8"]},
        "fmriprep": {"series": fdata},
        "fmriprep-newest": {"series": newest},
        "fmriprep-shared": {"series": shared},
    }
    # every volume compared (the panels hold a few), over the files
    for x in diffs:
        x["metrics"] = {"series": core.compare(full[x["a"]]["series"], full[x["b"]]["series"])}
        r = ex.compare(x["a"], x["b"], "series")
        x["display"] = {"series": r["display"]}
    same = common.identical(diffs)
    warned = (e.dir / "larmorx-8.warnings.txt").read_text(encoding="utf-8")
    for x in diffs:
        i, links = x["id"], []
        if i == "newest-shared":
            x["verdict"] = None
            x["reason"] = (
                "The version step, a reference row (the original against itself): with numpy "
                "2.5.4 and SciPy 1.18.1 fMRIPrep's resampler gives "
                + (
                    "the same values as with numpy 2.3.5 and SciPy 1.15.3, every one: this call "
                    "(mode `grid-constant`) meets none of the newest releases' changes (SciPy "
                    "1.18 changed only the reflect boundary's initialisation, SCI-03)."
                    if same[i]
                    else "other values than with numpy 2.3.5 and SciPy 1.15.3."
                )
            )
            links.append({"label": "the record", "href": f"docs/validation/{TOOL}.md"})
            x["links"] = links
            continue
        if same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "lx-fmriprep": "Every value of the 63 volumes is fMRIPrep's: head motion, the "
                "field map's displacement and its Jacobian, the cubic B-spline interpolation.",
                "fix-lx": "On the BOLD's own grid PIPE-03's Jacobian and the corrected one agree "
                "(the target's axes are the source's): the default mode gives the same bytes.",
                "lx1-lx8": "The same bytes on any number of threads.",
                "lx-newest": "Every value is that of fMRIPrep's resampler with the newest "
                "releases, numpy 2.5.4 (OpenBLAS 0.3.34) and SciPy 1.18.1, which larmorx "
                "follows since 2026-10-29.",
            }[i]
            if i == "fix-lx":
                links.append({"label": "PIPE-03", "href": reg["PIPE-03"]["link"]})
        elif i == "fix-lx":
            x["verdict"] = "d13"
            x["reason"] = "Corrections of the register change this run: " + common.clip(warned, 200)
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example."
        x["links"] = links
    t0 = shown.index(int(np.argmax(dv))) if int(np.argmax(dv)) in shown else 0
    at, _k = common.centre_view(ex, "axial")
    default = {
        "output": "series",
        "plane": "axial",
        "voxel": at,
        "t": t0,
        "panels": ["bold", "fmriprep", "larmorx", "larmorx-fix"],
        "diffs": ["lx-fmriprep", "fix-lx", "lx-newest"],
        "compare": {"a": "bold", "b": "larmorx"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        outputs,
        default=default,
        window_panel="fmriprep",
        input_windows={"bold": "brain"},
        caption=f"file volume {shown[t0]}; compare the input (before) with the resampled series "
        "(after) with Swipe or Blink.",
    )
    out["grid"]["t_index"] = shown
    ni = info.get("node_inputs", {})
    n_vox = int(np.prod(fdata.shape))
    return {
        "id": "fmriprep-native",
        "title": "fMRIPrep's one-shot resampling of a real BOLD run with its field map",
        "summary": (
            "The ResampleSeries node fMRIPrep 25.2.6 ran on OpenNeuro ds005040 sub-002 (task "
            "clip, run 1): the slice-time corrected series moved by its head-motion transforms "
            "and unwarped by its PEPOLAR field map (with the Jacobian) onto the BOLD reference "
            "in one cubic B-spline interpolation, by fMRIPrep and by larmorx. "
            f"{len(shown)} of the {n} volumes are shown; the metrics cover every volume."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS005040['dataset']}, {common.DS005040['subject']}, task clip run 1 "
                f"({common.DS005040['licence']}); shown inside fMRIPrep's BOLD brain mask, "
                "dilated by one voxel",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `bold_native_wf/boldref_bold` (`ResampleSeries`)",
            },
            {
                "label": "Inputs",
                "text": f"PE {ni.get('pe_dir', '?')}, readout {ni.get('ro_time', '?')} s, "
                f"Jacobian {ni.get('jacobian', '?')}, order {ni.get('order', '?')}, mode "
                f"`{ni.get('mode', '?')}`; the field map as B-spline coefficients "
                "(`desc-coeff_fieldmaprec`), the head motion as one ITK transform per volume",
            },
            {
                "label": "Programs",
                "text": "fMRIPrep 25.2.6's own output (its working directory); larmorx's "
                f"`lx.transforms.resample_series` at commit `{info['larmorx_commit'][:12]}`",
            },
            {"label": "Volumes", "text": "shown: file volumes " + ", ".join(map(str, shown))},
            {
                "label": "Versions",
                "text": "fMRIPrep's own output ran in its 25.2.6 image; fMRIPrep's resampler ran "
                "here too on the node's inputs, with the newest numerical releases (numpy 2.5.4 "
                "with OpenBLAS 0.3.34, SciPy 1.18.1: larmorx's reference since 2026-10-29) and "
                "with the shared environment's (numpy 2.3.5, SciPy 1.15.3), the version step a "
                "reference row",
            },
        ],
        **out,
        "chain": None,
        "metrics_note": (
            f"Δ is A − B over all {n_vox:,} values of the {' × '.join(map(str, fdata.shape))} "
            "series (every volume), not only the volumes and brain shown."
        ),
        "provenance": common.provenance(info, f"{TOOL}/{CASE}", [common.DS005040]),
    }


def _jacobian(ctx: core.Context, reg: dict) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = jacobian_entry(ctx)
    info = e.info()
    fix = nib.load(str(e.dir / "fix.nii.gz"))
    mask = common.load(str(e.dir / "mask.nii.gz")) > 0
    show = ndimage.binary_dilation(mask, iterations=1)
    grid = core.Grid.around(fix, show)
    ex = exm.Example(ctx, grid, show)
    arrays = {k: common.load(str(e.dir / f"{k}.nii.gz")) for k in ("reproduce", "fix", "reference")}
    ex.add(
        exm.Panel(
            "reference",
            "reference",
            "The factor on the BOLD's own grid, carried here",
            "where both methods agree; cubic B-splines, as fMRIPrep resamples",
            {"jacobian": arrays["reference"]},
        )
    )
    ex.add(
        exm.Panel(
            "fix",
            "larmorx",
            "larmorx's Jacobian (default mode)",
            "the derivative along the BOLD's phase-encoding axis, in its voxels",
            {"jacobian": arrays["fix"]},
        )
    )
    ex.add(
        exm.Panel(
            "reproduce",
            "fmriprep",
            "fMRIPrep's Jacobian (larmorx, reproduce mode)",
            "1 + np.gradient(vsm, axis=pe) on the target grid, bit-identical to "
            "fMRIPrep's in the record",
            {"jacobian": arrays["reproduce"]},
        )
    )
    dd = common.d
    diffs = [
        dd(
            "fix-ref",
            "fix",
            "reference",
            "reference",
            "larmorx's factor − the reference",
            "against the reference",
        ),
        dd(
            "repro-ref",
            "reproduce",
            "reference",
            "reference",
            "fMRIPrep's factor − the reference",
            "against the reference",
        ),
        dd(
            "fix-repro",
            "fix",
            "reproduce",
            "main",
            "larmorx's factor − fMRIPrep's (PIPE-03)",
            "larmorx's correction",
        ),
    ]
    outputs = [{"id": "jacobian", "label": "Jacobian factor", "unit": ""}]
    common.compare_all(ex, diffs, outputs)
    m = mask
    # outside the BOLD's field of view the factors mean nothing: the metrics are the brain's
    for x in diffs:
        mm = core.compare(arrays[x["a"]][m], arrays[x["b"]][m])
        mm.pop("at")
        mm["at_display"] = x["metrics"]["jacobian"]["at_display"]
        x["metrics"]["jacobian"] = mm
    err = {
        k: np.abs(arrays[k].astype(np.float64) - arrays["reference"])[m]
        for k in ("fix", "reproduce")
    }
    for x in diffs:
        x["links"] = [
            {"label": "PIPE-03", "href": reg["PIPE-03"]["link"]},
            {"label": "the Jacobian record", "href": "docs/validation/resample-series-jacobian.md"},
        ]
        if x["id"] == "repro-ref":
            x["verdict"] = None
            x["reason"] = (
                f"fMRIPrep's factor departs from the reference by a median "
                f"{np.median(err['reproduce']):.3g} (99th percentile "
                f"{np.percentile(err['reproduce'], 99):.3g}) in the brain: it is compressed "
                "towards 1 on a grid finer than the BOLD's or turned against its axes."
            )
        elif x["id"] == "fix-ref":
            x["verdict"] = None
            x["reason"] = (
                f"larmorx's factor stays at the reference: a median {np.median(err['fix']):.3g} "
                f"(99th percentile {np.percentile(err['fix'], 99):.3g}), the size of the "
                "interpolation carrying the reference here."
            )
        else:
            better = np.median(err["fix"]) < np.median(err["reproduce"])
            x["verdict"] = "d13" if better else "open"
            x["reason"] = (
                "PIPE-03, corrected by default with a warning: fMRIPrep differentiates the "
                "voxel-shift map along the target's axis in target voxels; larmorx along the "
                "BOLD's phase-encoding axis in BOLD voxels, as the factor is defined. "
                "upstream_defects='reproduce' gives fMRIPrep's factor."
            )
    at, _k = common.largest_in_slice(ex, "fix", "reproduce", "jacobian", "axial")
    default = {
        "output": "jacobian",
        "plane": "axial",
        "voxel": at,
        "panels": ["reference", "reproduce", "fix"],
        "diffs": ["repro-ref", "fix-ref", "fix-repro"],
        "compare": {"a": "reproduce", "b": "fix"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        outputs,
        default=default,
        window_panel="reference",
        caption="the crosshair where the two factors differ most in it; the differences on one "
        "symmetric scale.",
    )
    n_vox = int(np.prod(fix.shape))
    return {
        "id": "pipe-03",
        "title": "PIPE-03: the Jacobian on another grid, fMRIPrep's and the corrected one",
        "summary": (
            "fMRIPrep multiplies the unwarped BOLD by a Jacobian, the factor that undoes the "
            "intensity pile-up of the distortion. On the BOLD's own grid (above) its factor is "
            "right; on another grid (here the MNI152NLin2009cAsym 2 mm grid of the same run) it "
            "takes the derivative in the wrong voxels. The factors of ds005040's real field "
            "map, against the reference."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS005040['dataset']}, {common.DS005040['subject']}, task clip "
                f"run 1 ({common.DS005040['licence']}): fMRIPrep's field map, transforms and MNI "
                f"grid; inside the run's MNI brain mask, dilated; {common.MNI_NOTICE}",
            },
            {
                "label": "How",
                "text": "a volume of ones resampled as fMRIPrep resamples the BOLD (no head "
                "motion), so the output is the factor itself; the reference is the factor on "
                "the BOLD's own grid, where both methods agree, carried to this grid by cubic "
                "B-splines",
            },
            {
                "label": "Defect",
                "text": f"[PIPE-03](repo:{reg['PIPE-03']['link']}): {reg['PIPE-03']['title']}",
            },
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "The correction: larmorx's factor against fMRIPrep's"],
            ["reference", "Each factor against the reference"],
        ],
        "metrics_note": f"Δ is A − B over the {int(m.sum()):,} voxels of the run's brain mask on "
        f"the {' × '.join(map(str, fix.shape))} grid ({n_vox:,} voxels; outside the BOLD's field "
        "of view the factors have no meaning); the medians and percentiles in the reasons too.",
        "provenance": common.provenance(
            info, f"{TOOL}/{JACOBIAN_CASE}", [common.DS005040, common.MNI]
        ),
    }


def _cases(ctx: core.Context, rec) -> list[dict]:
    from larmorx_validation.parity import resample_series as suite

    defs = {c.id: c for c in suite.cases("full")}
    out = []
    for r in rec.results:
        c = defs.get(r["case"])
        v = common.outcome_verdict(r)
        if v is None:
            n = r.get("reported", {}).get("bit-identical")
            v = (
                ("bit-identical", "")
                if n in (None, 0, "0")
                else ("open", f"{n} values differ, within the record's thresholds")
            )
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": c.description if c else "",
                "command": "",
                "results": {
                    "port": {
                        "outcome": r["status"],
                        "verdict": v[0],
                        "checks": f"{r['checks_passed']}/{r['checks_total']}",
                        "reason": v[1],
                    }
                },
            }
        )
    return out


def export(ctx: core.Context) -> dict:
    reg = core.register(ctx.repo)
    rec = records.load(ctx.repo, TOOL)
    cases = _cases(ctx, rec)
    bench = records.load_bench(ctx.repo, TOOL)
    from larmorx_validation.figures.resample_series import SPEED_LABELS

    series = [
        {"id": "fMRIPrep|1 threads", "label": "fMRIPrep, 1 thread", "mark": "original"},
        {"id": "fMRIPrep|12 threads", "label": "fMRIPrep, 12 threads", "mark": "original-open"},
        {"id": "larmorx|1 threads", "label": "larmorx, 1 thread", "mark": "larmorx"},
        {"id": "larmorx|12 threads", "label": "larmorx, 12 threads", "mark": "larmorx-open"},
    ]
    speed = common.speed(
        bench,
        series,
        SPEED_LABELS,
        title="Seconds per resampling (without the file I/O), median of repeated runs, log "
        "scale; the factor is larmorx's speed-up over fMRIPrep at one thread",
        base=(series[0]["id"], series[2]["id"]),
        note="From the benchmark report `docs/benchmarks/resample-series.json` ({cpu}, {cpus} "
        "logical CPUs).",
        inputs=list(SPEED_LABELS),
    )
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "One-shot BOLD resampler",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": bench.source() if bench else [],
            "cache": [fcache.provenance(entry(ctx)), fcache.provenance(jacobian_entry(ctx))],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cases, "port", "larmorx (port)", "port", f"docs/validation/{TOOL}.md", "validated"
            )
        ],
        "cases": cases,
        "cases_total": len(cases),
        "speed": speed,
        "examples": [_native(ctx, reg), _jacobian(ctx, reg)],
    }


if __name__ == "__main__":
    from larmorx_validation.figures.core import REPO, default_cache

    print("done", entry(core.Context(repo=REPO, site=Path("."), cache=default_cache())).dir)
