# SPDX-License-Identifier: Apache-2.0
"""ImageMath and the ANTs filters' evidence (ImageMath, ThresholdImage, MultiplyImages,
SmoothImage, ResampleImage, ResampleImageBySpacing): the calls sMRIPrep's brain extraction
made in fMRIPrep 25.2.6's oracle run, on their own inputs, by fMRIPrep, ANTs 2.6.5 (1 and 8
threads, correctly rounded maths) and larmorx (both modes); the record cases of every
program, one explorer each; the five ImageMath cases that pass within a tolerance, run again
to classify them; speed.

fMRIPrep 25.2.6 calls ImageMath, ThresholdImage and MultiplyImages (sMRIPrep's brain
extraction); none of the five oracle runs calls SmoothImage, ResampleImage or
ResampleImageBySpacing, so those three have no worked example on a pipeline's input.
"""

from __future__ import annotations

import re
import shlex
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "ants-programs"
RUN = "ds000005-sub-01-noreconall"
BE = "work/fmriprep_25_2_wf/sub_01_wf/anat_fit_wf/brain_extraction_wf"
MASK = "out/sub-01/anat/sub-01_desc-brain_mask.nii.gz"
#: The T1w grid's nodes: output id -> (node, label)
T1W_NODES = {
    "truncate": (
        "truncate_images/mapflow/_truncate_images0",
        "ImageMath TruncateImageIntensity 0.01 0.999 256",
    ),
    "laplacian": ("lap_target", "ImageMath Laplacian 1.5 1"),
    "threshold": ("thr_brainmask", "ThresholdImage 0.5 1 1 0"),
    "dilate": ("atropos_wf/dil_brainmask", "ImageMath MD 2"),
}
#: The padded grid's nodes (atropos_wf pads the segmentation by 10 voxels)
PAD_NODES = {
    "largest": ("atropos_wf/06_get_gm", "ImageMath GetLargestComponent"),
    "fill": ("atropos_wf/07_fill_gm", "ImageMath FillHoles 2"),
    "multiply": ("atropos_wf/08_mult_gm", "MultiplyImages"),
    "md": ("atropos_wf/21_md_7_2", "ImageMath MD 5"),
    "me": ("atropos_wf/22_me_7_2", "ImageMath ME 5"),
}
PROGRAMS = {
    "image-math": ("ImageMath", "ants-image-math", "ants_image_math"),
    "threshold-image": ("ThresholdImage", "ants-threshold-image", "ants_threshold_image"),
    "multiply-images": ("MultiplyImages", "ants-multiply-images", "ants_multiply_images"),
    "smooth-image": ("SmoothImage", "ants-smooth-image", "ants_smooth_image"),
    "resample-image": ("ResampleImage", "ants-resample-image", "ants_resample_image"),
    "resample-image-by-spacing": (
        "ResampleImageBySpacing",
        "ants-resample-image-by-spacing",
        "ants_resample_image_by_spacing",
    ),
}
WITHIN = (
    "arithmetic/^-image",
    "arithmetic/^-scalar-2",
    "arithmetic/^-scalar-0.37",
    "grayscale-morphology/GD-signed-zeros",
    "grayscale-morphology/GC-signed-zeros",
)
WITHIN_THREADS = (1, 2, 4, 12)


def _node_argv(node_dir: Path, out: Path) -> tuple[str, list[str], list[Path]]:
    """A node's program, its arguments with the run's paths on this machine and the output
    redirected to ``out``, and its input files."""
    run = common.fmriprep_run(RUN)
    words = shlex.split((node_dir / "command.txt").read_text(encoding="utf-8"))
    program, args = words[0], words[1:]
    inputs = []
    res = []
    produced = sorted(p.name for p in node_dir.glob("*.nii.gz"))
    for a in args:
        if a.startswith("/work/") or a.startswith("/out/"):
            p = run / a.lstrip("/")
            inputs.append(p)
            res.append(str(p))
        elif a in produced:
            res.append(str(out))
        else:
            res.append(a)
    return program, res, inputs


def _compute_nodes(nodes: dict):
    def compute(e: fcache.Entry) -> None:
        from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

        run = common.fmriprep_run(RUN)
        b, lib = ants_bin(), crmath_preload()
        if b is None or lib is None:
            raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload")
        for oid, (node, _label) in nodes.items():
            d = run / BE / node
            for label, threads, cr in (
                ("ants-1", 1, False),
                ("ants-8", 8, False),
                ("ants-1-cr", 1, True),
            ):
                prog, args, inputs = _node_argv(d, e.dir / f"{oid}-{label}.nii.gz")
                env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
                if cr:
                    env["LD_PRELOAD"] = str(lib)
                e.run(f"{oid}-{label}", [str(b / prog), *args], env=env, unset=common.CLEAN_ENV)
            for label, mode in (("larmorx", "reproduce"), ("larmorx-fix", "fix")):
                prog, args, inputs = _node_argv(d, e.dir / f"{oid}-{label}.nii.gz")
                e.run(
                    f"{oid}-{label}",
                    fcache.larmorx_cli("ants", prog, *args),
                    env={
                        "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "8",
                        "LARMORX_UPSTREAM_DEFECTS": mode,
                    },
                    unset=common.CLEAN_ENV,
                )
            for i, p in enumerate(inputs):
                e.input(f"{oid} input {i + 1} ({p.name})", p)
            e.input(f"{oid}: fMRIPrep's output", sorted(d.glob("*.nii.gz"))[0])
        e.finish(
            versions={"ants": ants_version()},
            threads={"ants": [1, 8], "ants_crmath": 1, "larmorx": 8},
            settings="sMRIPrep's brain-extraction nodes of fMRIPrep 25.2.6's run "
            f"{RUN} (their command.txt and input files): ANTs 2.6.5 at 1 and 8 threads and with "
            "CORE-MATH at 1, larmorx in reproduce and default modes at 8 threads",
        )

    return compute


def _compute_within(e: fcache.Entry) -> None:
    from larmorx_validation.parity.ants_image_math import all_cases
    from larmorx_validation.parity.ants_programs import (
        _resolve,
        ants_bin,
        ants_version,
        crmath_preload,
    )

    b, lib = ants_bin(), crmath_preload()
    defs = {c.id: c for c in all_cases()}
    for cid in WITHIN:
        c = defs[cid]
        key = cid.replace("/", "_").replace("^", "pow")
        tmp = e.dir / key
        tmp.mkdir(exist_ok=True)
        runs = [(f"ants-{t}", t, False) for t in WITHIN_THREADS] + [("ants-1-cr", 1, True)]
        for label, threads, cr in runs:
            out = tmp / label
            out.mkdir(exist_ok=True)
            args = [_resolve(a, tmp, out) for a in c.args]
            env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
            if cr:
                env["LD_PRELOAD"] = str(lib)
            e.run(f"{key}-{label}", [str(b / c.program), *args], env=env, unset=common.CLEAN_ENV)
        for label, threads in (("larmorx", 4), ("larmorx-1", 1)):
            out = tmp / label
            out.mkdir(exist_ok=True)
            args = [_resolve(a, tmp, out) for a in c.args]
            e.run(
                f"{key}-{label}",
                fcache.larmorx_cli("ants", c.program, *args),
                env={
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                    "LARMORX_UPSTREAM_DEFECTS": "reproduce",
                },
                unset=common.CLEAN_ENV,
            )
    e.finish(
        versions={"ants": ants_version()},
        settings="the ImageMath record's five cases that pass within a tolerance, run again: "
        "ANTs at 1, 2, 4 and 12 threads and with CORE-MATH at 1; larmorx (reproduce) at 4 and 1",
    )


def _entry(ctx, case: str, description: str, compute) -> fcache.Entry:
    return fcache.entry(ctx.figures_context(), TOOL, case, description, compute)


def entries(ctx: core.Context) -> dict[str, fcache.Entry]:
    return {
        "t1w": _entry(
            ctx,
            "fmriprep-ds000005-brain-extraction-t1w",
            "sMRIPrep's brain-extraction calls on the T1w grid (ds000005 sub-01): "
            "fMRIPrep, ANTs 2.6.5, larmorx",
            _compute_nodes(T1W_NODES),
        ),
        "pad": _entry(
            ctx,
            "fmriprep-ds000005-brain-extraction-padded",
            "sMRIPrep's brain-extraction mask clean-up on its padded grid (ds000005 "
            "sub-01): fMRIPrep, ANTs 2.6.5, larmorx",
            _compute_nodes(PAD_NODES),
        ),
        "within": _entry(
            ctx,
            "image-math-within-tolerance",
            "the ImageMath cases within a tolerance: ANTs at several thread "
            "counts and with CORE-MATH, larmorx",
            _compute_within,
        ),
    }


# ----------------------------------------------------------------------------------- export

RUNS = [
    ("larmorx", "larmorx", "larmorx", "larmorx (port)", "reproduce mode, 8 threads"),
    ("larmorx-fix", "larmorx-fix", "larmorx", "larmorx (port)", "default mode, 8 threads"),
    (
        "ants-1-cr",
        "ants-1-cr",
        "reference",
        "ANTs 2.6.5, correctly rounded maths",
        "1 thread, CORE-MATH preloaded",
    ),
    ("ants-1", "ants-1", "original", "ANTs 2.6.5, 1 thread", "as built (GCC 14.3)"),
    ("ants-8", "ants-8", "original", "ANTs 2.6.5, 8 threads", "as built"),
    ("fmriprep", None, "fmriprep", "fMRIPrep 25.2.6, as it ran", "ANTs 2.6.2 in its image"),
]


def _example(ctx: core.Context, e: fcache.Entry, nodes: dict, which: str, reg: dict) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    info = e.info()
    run = common.fmriprep_run(RUN)
    first = next(iter(nodes))
    ref_img = nib.load(str(sorted((run / BE / nodes[first][0]).glob("*.nii.gz"))[0]))
    brain = common.load(str(run / MASK)) > 0
    if which == "pad":
        brain = np.pad(brain, 10)
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(ref_img, show)
    ex = exm.Example(ctx, grid, show)
    inputs = {}
    for oid, (node, _label) in nodes.items():
        _prog, _args, ins = _node_argv(run / BE / node, Path("out.nii.gz"))
        inputs[oid] = common.load(str(ins[0]))
    ex.add(
        exm.Panel(
            "input",
            "input",
            "The call's input",
            "the file fMRIPrep gave the node (its first input)",
            inputs,
        )
    )
    for pid, label, role, text, sub in RUNS:
        arrays = {}
        for oid, (node, _l) in nodes.items():
            path = (
                sorted((run / BE / node).glob("*.nii.gz"))[0]
                if label is None
                else e.dir / f"{oid}-{label}.nii.gz"
            )
            arrays[oid] = common.load(str(path))
        ex.add(exm.Panel(pid, role, text, sub, arrays))
    outputs = [{"id": oid, "label": lab, "unit": ""} for oid, (_n, lab) in nodes.items()]
    dd = common.d
    diffs = [
        dd(
            "lx-ants",
            "larmorx",
            "ants-1",
            "main",
            "larmorx − ANTs 2.6.5, 1 thread, as built",
            common.ROLE_LX,
        ),
        dd(
            "lx-cr",
            "larmorx",
            "ants-1-cr",
            "main",
            "larmorx − ANTs 2.6.5, correctly rounded maths",
            common.ROLE_LX,
        ),
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
            "check",
            "larmorx, default − reproduce mode",
            common.ROLE_SELF,
        ),
        dd(
            "ants8-ants1",
            "ants-8",
            "ants-1",
            "variability",
            "ANTs 2.6.5, 8 threads − 1 thread",
            common.ROLE_ORIG,
        ),
        dd(
            "fmriprep-ants",
            "fmriprep",
            "ants-8",
            "variability",
            "fMRIPrep's own (ANTs 2.6.2) − ANTs 2.6.5, 8 threads",
            common.ROLE_ORIG,
        ),
    ]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    per_out = {x["id"]: {o: m["identical"] for o, m in x["metrics"].items()} for x in diffs}
    for x in diffs:
        i, links = x["id"], []
        if x["kind"] == "variability":
            x["verdict"] = None
            x["reason"] = (
                "The same bytes: the thread count changes nothing in these calls."
                if i == "ants8-ants1" and same[i]
                else "ANTs' version and build play no part."
                if same[i]
                else "These runs differ in "
                + ", ".join(o for o, s in per_out[i].items() if not s)
                + "."
            )
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "lx-ants": "Every call gives ANTs' bytes.",
                "lx-cr": "Every call gives ANTs' bytes.",
                "lx-fmriprep": "Every call gives the bytes fMRIPrep wrote in its run.",
                "fix-lx": "No correction of the register applies to these calls.",
            }[i]
        elif i == "lx-ants" and same["lx-cr"]:
            x["verdict"] = "identical-cr"
            x["reason"] = (
                "Only where glibc's maths is not correctly rounded ("
                + ", ".join(o for o, s in per_out[i].items() if not s)
                + "): with CORE-MATH preloaded ANTs gives larmorx's bytes."
            )
        elif i == "fix-lx":
            x["verdict"] = "d13"
            x["reason"] = (
                "A correction of the register changes "
                + ", ".join(o for o, s in per_out[i].items() if not s)
                + "."
            )
        elif i == "lx-fmriprep" and same.get("lx-cr") and not same["fmriprep-ants"]:
            x["verdict"] = "within-variability"
            x["reason"] = "fMRIPrep's ANTs differs from ANTs 2.6.5 by as much (reference rows)."
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example."
        x["links"] = links
    out0 = outputs[0]["id"] if which == "t1w" else "md"
    at, _k = common.centre_view(ex, "axial")
    default = {
        "output": out0,
        "plane": "axial",
        "voxel": at,
        "panels": ["input", "fmriprep", "ants-1", "larmorx"],
        "diffs": ["lx-ants", "lx-fmriprep"],
        "compare": {"a": "input", "b": "larmorx"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        outputs,
        default=default,
        window_panel="larmorx",
        caption="each call's output in its own window; choose the call above the panels (Output).",
    )
    vers = info.get("versions", {})
    calls = "; ".join(
        f"`{lab}` ({node.split('/')[-1].split('/')[0]})" for _oid, (node, lab) in nodes.items()
    )
    n_vox = int(np.prod(ref_img.shape))
    if which == "t1w":
        title = "sMRIPrep's brain extraction, step by step: the calls on the T1w grid"
        summary = (
            "Four calls fMRIPrep 25.2.6 made in sMRIPrep's brain extraction of OpenNeuro "
            "ds000005 sub-01, each with its own input file (choose it under Output): the "
            "intensity truncation before N4, the Laplacian of the N4-corrected T1w for the "
            "registration to OASIS, the threshold of the warped brain probability map, its "
            "dilation. Each by fMRIPrep in its run, by ANTs 2.6.5 and by larmorx."
        )
    else:
        title = "The mask clean-up of the Atropos segmentation, on its padded grid"
        summary = (
            "sMRIPrep pads the segmentation by 10 voxels and cleans its grey- and white-matter "
            "masks with ImageMath and MultiplyImages: the largest component, holes filled, the "
            "product of two masks, a dilation and an erosion of radius 5 (each with its own "
            "input under Output)."
        )
    return {
        "id": f"brain-extraction-{which}",
        "title": title,
        "summary": summary,
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01's T1w "
                f"({common.DS000005['licence']}); shown inside fMRIPrep's brain mask, dilated by "
                "2 voxels"
                + (", padded by 10 voxels as the node's images" if which == "pad" else ""),
            },
            {"label": "Calls", "text": calls},
            {
                "label": "Programs",
                "text": f"{vers.get('ants', '?')}; ANTs 2.6.2 in fMRIPrep's "
                f"image (its run); larmorx at commit `{info['larmorx_commit'][:12]}`",
            },
            {"label": "Grid", "text": common.describe_grid(ref_img, grid.shape)},
        ],
        **out,
        "chain": None,
        "metrics_note": f"Δ is A − B over all {n_vox:,} voxels of each call's "
        f"{' × '.join(map(str, ref_img.shape))} output; one row per comparison where every "
        "call gives the same bytes.",
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [common.DS000005]),
    }


# ------------------------------------------------------------------------------------ cases


def _within(ctx: core.Context) -> dict[str, tuple[str, str]]:
    """The five ImageMath cases within a tolerance, classified from their runs."""
    import nibabel as nib

    e = entries(ctx)["within"]
    out = {}
    for cid in WITHIN:
        key = cid.replace("/", "_").replace("^", "pow")
        d = e.dir / key

        def bits(label: str, d: Path = d) -> np.ndarray:
            a = np.asanyarray(nib.load(str(d / label / "out.nii.gz")).dataobj)
            return a.view(np.uint32) if a.dtype == np.float32 else a

        lx = bits("larmorx")
        ants = {t: bits(f"ants-{t}") for t in WITHIN_THREADS}
        cr = bits("ants-1-cr")
        n1 = int(np.count_nonzero(lx != ants[1]))
        ncr = int(np.count_nonzero(lx != cr))
        nthreads = int(np.count_nonzero(lx != bits("larmorx-1")))
        ants_var = {t: int(np.count_nonzero(ants[t] != ants[1])) for t in WITHIN_THREADS[1:]}
        values_equal = bool(
            np.array_equal(
                np.asanyarray(nib.load(str(d / "larmorx" / "out.nii.gz")).dataobj),
                np.asanyarray(nib.load(str(d / "ants-1" / "out.nii.gz")).dataobj),
            )
        )
        if ncr == 0 and n1:
            out[cid] = (
                "identical-cr",
                f"{n1:,} values differ from ANTs as built by one float "
                "ulp (glibc's powf); bit-identical to ANTs with CORE-MATH preloaded",
            )
        elif n1 and values_equal and any(ants_var.values()):
            out[cid] = (
                "within-variability",
                f"{n1:,} zeros differ in sign only (-0.0 against +0.0, equal values); "
                "ANTs' own result changes them with its thread count ("
                + ", ".join(f"{t} threads: {v:,}" for t, v in ants_var.items())
                + " against 1); larmorx gives one sign on any number of threads"
                + ("" if nthreads == 0 else " (it does not here)"),
            )
        elif n1 and values_equal:
            out[cid] = (
                "divergence",
                f"{n1:,} zeros differ in sign only (-0.0 against +0.0, "
                "equal values): ITK keeps the sign of whichever zero its histogram met "
                "first; larmorx returns +0.0 for a zero maximum and -0.0 for a zero "
                "minimum",
            )
        else:
            out[cid] = (
                "open",
                f"{n1:,} values differ from ANTs as built, {ncr:,} from ANTs with CORE-MATH",
            )
    return out


def _cases(ctx: core.Context) -> list[dict]:
    import importlib

    within = _within(ctx)
    out = []
    for impl, (program, name, module) in PROGRAMS.items():
        rec = records.load(ctx.repo, name)
        mod = importlib.import_module(f"larmorx_validation.parity.{module}")
        defs = {c.id: c for c in mod.all_cases()}
        nb = set()
        m = re.search(r"Not bit-identical[^:]*: (.*)$", rec.text, re.M)
        if m:
            nb = set(records.case_list(m.group(0)))
        for r in rec.results:
            c = defs.get(r["case"])
            v = common.outcome_verdict(r) or common.known_correction(c, r, program)
            rep = r.get("reported", {})
            if v is None:
                if r["case"] in within and impl == "image-math":
                    v = within[r["case"]]
                elif "values not compared" in rep or (c is not None and c.undefined_values):
                    v = (
                        "divergence",
                        "Values not compared: ANTs' depend on memory it never "
                        "wrote; exit status, header, type, shape and affine are ANTs'"
                        + (
                            f" ({c.undefined_values})"
                            if c is not None and c.undefined_values
                            else ""
                        ),
                    )
                elif r["case"] in nb:
                    v = ("open", "within the record's tolerance, not classified")
                else:
                    v = ("bit-identical", "values and header bytes")
            out.append(
                {
                    "id": f"{program}/{r['case']}",
                    "category": r["category"],
                    "description": c.description if c else "",
                    "command": _command(c) if c else "",
                    "results": {
                        impl: {
                            "outcome": r["status"],
                            "verdict": v[0],
                            "checks": f"{r['checks_passed']}/{r['checks_total']}",
                            "reason": common.clip(v[1], 400),
                        }
                    },
                }
            )
    return out


def _command(c) -> str:
    def sub(a: str) -> str:
        a = re.sub(r"\{in:([^}]+)\}", lambda m: m.group(1).split("/")[-1], a)
        a = re.sub(r"\{gen:([^}]+)\}", lambda m: f"<generated: {m.group(1)}>", a)
        a = re.sub(r"\{missing:([^}]+)\}", lambda m: m.group(1), a)
        a = re.sub(r"\{out(\d*)\}", lambda m: f"out{m.group(1)}.nii.gz", a)
        return a.replace("{tmp}/", "")

    return common.command_text(c.program, c.args, sub)


def _speed(ctx: core.Context) -> dict | None:
    from larmorx_validation.figures.ants_programs import SPEED

    rows, sources = [], []
    series = [
        {"id": "ANTs 2.6.5|1", "label": "ANTs, 1 thread", "mark": "original"},
        {"id": "ANTs 2.6.5|12", "label": "ANTs, 12 threads", "mark": "original-open"},
        {"id": "larmorx|1", "label": "larmorx, 1 thread", "mark": "larmorx"},
        {"id": "larmorx|12", "label": "larmorx, 12 threads", "mark": "larmorx-open"},
    ]
    env = {}
    for name, labels in SPEED.items():
        bench = records.load_bench(ctx.repo, name)
        if bench is None:
            continue
        sources += bench.source()
        s = common.speed(
            bench,
            series,
            labels,
            title="",
            base=("ANTs 2.6.5|1", "larmorx|1"),
            note="",
            inputs=list(labels),
        )
        if s:
            rows += s["rows"]
            env = bench.data.get("environment", {})
    if not rows:
        return None
    axis, ticks = common.ticks_log([v for r in rows for v in r["seconds"].values()], " s")
    return {
        "title": "Seconds for the whole command, median of repeated runs, log scale; the "
        "pipelines' own calls first; the factor is larmorx's speed-up at one thread",
        "axis": axis,
        "ticks": ticks,
        "series": series,
        "rows": rows,
        "note": "From the benchmark reports `docs/benchmarks/ants-programs.json`, "
        f"`ants-gaussian.json` and `ants-morphology.json` ({env.get('cpu', '?')}, "
        f"{env.get('logical_cpus', '?')} logical CPUs).",
        "sources": sources,
    }


#: The ANTS-17 example: ``ME`` with a label value, on fMRIPrep's segmentation of ds000005
A17_DATA = {
    "dataset": "OpenNeuro ds000005 (mixed-gambles task), v00001",
    "subject": "sub-01",
    "licence": "PDDL-1.0",
}
DSEG = "out/sub-01/anat/sub-01_dseg.nii.gz"


def _a17_spec(ctx: core.Context):
    from larmorx_validation.evidence import corrections

    run = common.fmriprep_run(RUN)

    def show():
        from scipy import ndimage

        return ndimage.binary_dilation(common.load(str(run / MASK)) > 0, iterations=2)

    def effect(e) -> str:
        dseg = common.load(str(run / DSEG))
        a = common.load(str(e.dir / "ants-1-eroded.nii.gz"))
        b = common.load(str(e.dir / "larmorx-eroded.nii.gz"))
        wm = dseg == 2
        return (
            f"of the {int(wm.sum()):,} white-matter voxels, larmorx's eroded mask keeps "
            f"{int(b.sum()):,}; ANTs' output is 1 on {int(a.sum()):,} voxels, the eroded white "
            f"matter and every voxel of the other labels ({int(((dseg == 1) | (dseg == 3)).sum()):,}"
            " of CSF and grey matter)"
        )

    return corrections.Spec(
        tool=TOOL,
        case="ds000005-dseg-me-2-ants17",
        entry="ANTS-17",
        title="ANTS-17: eroding the white-matter label of fMRIPrep's segmentation",
        summary=(
            "`ImageMath ME` with a label value, on the segmentation fMRIPrep 25.2.6 wrote for "
            "OpenNeuro ds000005 sub-01 (1 grey matter, 2 white matter, 3 CSF): an eroded white-"
            "matter mask, as for a CompCor or a boundary mask. ANTs thresholds the eroded image at "
            "0.5, so the other labels come back as 1 beside the eroded white matter; larmorx's "
            "default gives the eroded white matter alone, its reproduce mode ANTs' output."
        ),
        program="ImageMath",
        args=("3", "{out:eroded}", "ME", "{in:dseg}", "1", "2"),
        inputs={"dseg": ("fMRIPrep's segmentation (dseg)", run / DSEG)},
        panels=(("dseg", "ds000005 sub-01 segmentation", "fMRIPrep's dseg: 1 GM, 2 WM, 3 CSF"),),
        outputs=({"id": "eroded", "label": "Eroded label 2 (WM)", "unit": ""},),
        data=(A17_DATA,),
        show=show,
        reason=(
            "ANTS-17, corrected by default with a warning: `ME r value` erodes the voxels "
            "holding `value`; ANTs then keeps every voxel above 0.5, so on a label map the other "
            "labels survive as 1 and the result is no erosion of anything. larmorx gives 1 where "
            "the eroded label remains (SciPy's binary erosion gives the same voxels: the parity "
            "case `binary-morphology/ME-labels-known`); reproduce mode gives ANTs' output."
        ),
        effect=effect,
        settings="ImageMath ME with radius 1 and value 2 on fMRIPrep 25.2.6's dseg of ds000005 "
        "sub-01 (no fMRIPrep node: fMRIPrep erodes binary masks only)",
        focus=lambda: common.load(str(run / DSEG)) == 1,
        focus_text="in the grey matter, which ANTs' output keeps as 1",
    )


def export(ctx: core.Context) -> dict:
    from larmorx_validation.evidence import corrections

    reg = core.register(ctx.repo)
    cases = _cases(ctx)
    a17 = _a17_spec(ctx)
    ents = entries(ctx)
    impls, sources = [], []
    for impl, (program, name, _m) in PROGRAMS.items():
        rec = records.load(ctx.repo, name)
        sources += rec.source()
        impls.append(
            common.implementation(
                cases,
                impl,
                f"{program} (port)",
                program,
                f"docs/validation/{name}.md",
                "validated",
                name=name,
            )
        )
    speed = _speed(ctx)
    bench_sources = speed.pop("sources") if speed else []
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "ImageMath and the filters",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": sources,
            "benchmarks": bench_sources,
            "cache": [
                *(fcache.provenance(e) for e in ents.values()),
                fcache.provenance(corrections.entry(ctx, a17)),
            ],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": impls,
        "cases_per_impl": True,
        "cases": cases,
        "cases_total": len(cases),
        "speed": speed,
        "examples": [
            _example(ctx, ents["t1w"], T1W_NODES, "t1w", reg),
            _example(ctx, ents["pad"], PAD_NODES, "pad", reg),
            corrections.example(ctx, a17, reg),
        ],
    }


if __name__ == "__main__":
    from larmorx_validation.evidence import corrections
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    for k, e in entries(c).items():
        print("done", k, e.dir)
    print("done", corrections.entry(c, _a17_spec(c)).dir)
