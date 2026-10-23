# SPDX-License-Identifier: Apache-2.0
"""antsApplyTransforms's evidence: fMRIPrep's resampling of a real T1w into MNI space through
its ``.h5`` composite transform (affine and SyN displacement field) with Lanczos
interpolation (and, in ``ants_apply_transforms_bspline``, a B-spline transform), as fMRIPrep ran it (``--float 1``) and in double precision, by fMRIPrep, by ANTs
2.6.5 (as built, with correctly rounded maths, at 1 and 8 threads) and by larmorx (1 and 12
threads, default and ``reproduce`` modes); the record's cases (images, ``--float 1``,
transform outputs); speed.

The worked example is the node fMRIPrep 25.2.6 ran to write the T1w in MNI152NLin2009cAsym
(``ds_std_volumes_wf/anat2std_t1w``), with its own arguments and inputs (the masked T1w it
wrote before, the template, the composite of its output folder)."""

from __future__ import annotations

import re
import shlex
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "ants-apply-transforms"
CASE = "fmriprep-ds000005-anat2std-t1w"
RUN = "ds000005-sub-01-noreconall"
WF = "work/fmriprep_25_2_wf/sub_01_wf/ds_std_volumes_wf"
NODE = f"{WF}/_in_tuple_MNI152NLin2009cAsym.res2/anat2std_t1w"
INPUT = f"{WF}/mask_anat/sub-01_desc-preproc_T1w_masked.nii.gz"
XFM = "out/sub-01/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5"
OUT = "sub-01_desc-preproc_T1w_masked_trans.nii.gz"
TPL = "templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_"
LX_THREADS = (1, 12)


def _argv(command: str, inp: str, ref: str, xfm: str, out: str, float_: bool) -> list[str]:
    argv = shlex.split(command)[1:]
    res = []
    for i, a in enumerate(argv):
        prev = argv[i - 1] if i else ""
        if prev == "--input":
            a = inp
        elif prev == "--reference-image":
            a = ref
        elif prev == "--transform":
            a = xfm
        elif prev == "--output":
            a = out
        elif prev == "--float":
            a = "1" if float_ else "0"
        res.append(a)
    return res


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    run = common.fmriprep_run(RUN)
    b, lib = ants_bin(), crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload is missing")
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    inp = e.input("input (fMRIPrep's T1w × brain mask)", run / INPUT)
    ref = e.input("reference (MNI152NLin2009cAsym, 2 mm)", Path(td.get(TPL + "T1w.nii.gz")))
    xfm = e.input("transform (fMRIPrep's T1w-to-MNI composite, .h5)", run / XFM)
    e.input("fMRIPrep's output", run / NODE / OUT)
    d = e.dir
    for prec, float_ in (("float", True), ("double", False)):
        for label, threads, cr in (
            (f"ants-{prec}-1", 1, False),
            (f"ants-{prec}-8", 8, False),
            (f"ants-{prec}-1-cr", 1, True),
        ):
            env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
            if cr:
                env["LD_PRELOAD"] = str(lib)
            e.run(
                label,
                [
                    str(b / "antsApplyTransforms"),
                    *_argv(
                        command, str(inp), str(ref), str(xfm), str(d / f"{label}.nii.gz"), float_
                    ),
                ],
                env=env,
                unset=common.CLEAN_ENV,
            )
        for threads in LX_THREADS:
            for mode in ("fix", "reproduce"):
                label = f"larmorx-{prec}-{threads}" + ("-reproduce" if mode == "reproduce" else "")
                e.run(
                    label,
                    fcache.larmorx_cli(
                        "ants",
                        "antsApplyTransforms",
                        *_argv(
                            command,
                            str(inp),
                            str(ref),
                            str(xfm),
                            str(d / f"{label}.nii.gz"),
                            float_,
                        ),
                    ),
                    env={
                        "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                        "LARMORX_UPSTREAM_DEFECTS": mode,
                    },
                    unset=common.CLEAN_ENV,
                )
    # the input on the reference grid without the transform (its own world coordinates)
    e.run(
        "unregistered",
        [
            str(b / "antsApplyTransforms"),
            *_argv(command, str(inp), str(ref), "identity", str(d / "unregistered.nii.gz"), True),
        ],
        env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"},
        unset=common.CLEAN_ENV,
    )
    e.finish(
        versions={"ants": ants_version()},
        command=command,
        threads={"ants": [1, 8], "larmorx": list(LX_THREADS), "fmriprep_run": 1},
        settings="fMRIPrep 25.2.6's anat2std_t1w node of ds000005 sub-01 (its command.txt and "
        "inputs), with --float 1 as fMRIPrep ran it and with --float 0; ANTs as built at 1 and 8 "
        "threads and with CORE-MATH; larmorx in both modes at 1 and 12 threads",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's T1w-to-MNI resampling (anat2std_t1w, .h5 composite, Lanczos) of ds000005 "
        "sub-01: fMRIPrep, ANTs 2.6.5 and larmorx, single and double precision",
        _compute,
    )


# ----------------------------------------------------------------------------------- export

RUNS = [
    (
        "larmorx-float",
        "larmorx-float-1-reproduce",
        "larmorx",
        "larmorx (port), --float 1",
        "1 thread, reproduce mode",
    ),
    (
        "larmorx-float-12",
        "larmorx-float-12-reproduce",
        "larmorx",
        "larmorx (port), --float 1",
        "12 threads, reproduce mode",
    ),
    (
        "larmorx-float-fix",
        "larmorx-float-1",
        "larmorx",
        "larmorx (port), --float 1",
        "1 thread, default mode",
    ),
    (
        "ants-float-cr",
        "ants-float-1-cr",
        "reference",
        "ANTs 2.6.5, --float 1, correctly rounded maths",
        "1 thread, CORE-MATH preloaded",
    ),
    (
        "ants-float",
        "ants-float-1",
        "original",
        "ANTs 2.6.5, --float 1, as built",
        "1 thread, glibc's maths (GCC 14.3 build)",
    ),
    ("ants-float-8", "ants-float-8", "original", "ANTs 2.6.5, --float 1", "8 threads, as built"),
    (
        "fmriprep",
        None,
        "fmriprep",
        "fMRIPrep 25.2.6, as it ran",
        "ANTs 2.6.2 in its image, --float 1, 1 thread",
    ),
    (
        "larmorx-double",
        "larmorx-double-1-reproduce",
        "larmorx",
        "larmorx (port), double",
        "1 thread, reproduce mode",
    ),
    (
        "larmorx-double-12",
        "larmorx-double-12-reproduce",
        "larmorx",
        "larmorx (port), double",
        "12 threads, reproduce mode",
    ),
    (
        "larmorx-double-fix",
        "larmorx-double-1",
        "larmorx",
        "larmorx (port), double",
        "1 thread, default mode",
    ),
    (
        "ants-double-cr",
        "ants-double-1-cr",
        "reference",
        "ANTs 2.6.5, double, correctly rounded maths",
        "1 thread, CORE-MATH preloaded",
    ),
    (
        "ants-double",
        "ants-double-1",
        "original",
        "ANTs 2.6.5, double, as built",
        "1 thread, glibc's maths",
    ),
    ("ants-double-8", "ants-double-8", "original", "ANTs 2.6.5, double", "8 threads, as built"),
]
OUTPUTS = [{"id": "warped", "label": "T1w in MNI space", "unit": ""}]


def _diffs() -> list[dict]:
    d = common.d
    out = []
    for p, P in (("float", "--float 1"), ("double", "double")):
        out += [
            d(
                f"lx-ants-{p}",
                f"larmorx-{p}",
                f"ants-{p}",
                "main",
                f"larmorx − ANTs 2.6.5, {P}, as built",
                common.ROLE_LX,
            ),
            d(
                f"lx-cr-{p}",
                f"larmorx-{p}",
                f"ants-{p}-cr",
                "main",
                f"larmorx − ANTs 2.6.5, {P}, correctly rounded maths",
                common.ROLE_LX,
            ),
        ]
    out += [
        d(
            "lx-fmriprep",
            "larmorx-float",
            "fmriprep",
            "fmriprep",
            "larmorx (--float 1) − fMRIPrep's own output",
            common.ROLE_FMRIPREP,
        ),
    ]
    for p, P in (("float", "--float 1"), ("double", "double")):
        out += [
            d(
                f"lx12-{p}",
                f"larmorx-{p}-12",
                f"larmorx-{p}",
                "check",
                f"larmorx {P}, 12 threads − 1 thread",
                common.ROLE_SELF,
            ),
            d(
                f"lx-fix-{p}",
                f"larmorx-{p}-fix",
                f"larmorx-{p}",
                "check",
                f"larmorx {P}, default − reproduce mode",
                common.ROLE_SELF,
            ),
        ]
    out += [
        d(
            "ants8-float",
            "ants-float-8",
            "ants-float",
            "variability",
            "ANTs 2.6.5 --float 1, 8 threads − 1 thread",
            common.ROLE_ORIG,
        ),
        d(
            "ants8-double",
            "ants-double-8",
            "ants-double",
            "variability",
            "ANTs 2.6.5 double, 8 threads − 1 thread",
            common.ROLE_ORIG,
        ),
        d(
            "fmriprep-ants",
            "fmriprep",
            "ants-float",
            "variability",
            "fMRIPrep's own output (ANTs 2.6.2) − ANTs 2.6.5, --float 1, 1 thread each",
            common.ROLE_ORIG,
        ),
        d(
            "cr-ants-float",
            "ants-float-cr",
            "ants-float",
            "variability",
            "ANTs --float 1: correctly rounded − glibc's maths",
            common.ROLE_ORIG,
        ),
        d(
            "float-double",
            "ants-float",
            "ants-double",
            "variability",
            "ANTs 2.6.5: --float 1 − double, as built",
            common.ROLE_ORIG,
        ),
    ]
    return out


def _verdict(x: dict, same: dict[str, bool]) -> None:
    i, links = x["id"], []
    if x["kind"] == "variability":
        x["verdict"] = None
        x["reason"] = {
            "ants8-float": "ANTs resamples each voxel on its own: its thread count changes nothing.",
            "ants8-double": "ANTs resamples each voxel on its own: its thread count changes "
            "nothing.",
            "fmriprep-ants": "ANTs' version and build play no part.",
            "cr-ants-float": "glibc's sin, cos (the windowed sinc's weights) and exp are not "
            "always correctly rounded.",
            "float-double": "Single precision, as fMRIPrep asks for, against double: the size of "
            "the precision fMRIPrep chose.",
        }[i]
        if not same[i] and i in ("ants8-float", "ants8-double", "fmriprep-ants"):
            x["reason"] = "These runs differ."
        if i == "cr-ants-float":
            links.append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
    elif same[i]:
        x["verdict"] = "bit-identical"
        x["reason"] = {
            "lx-cr-float": "With correctly rounded maths, ANTs computes exactly what larmorx "
            "computes, in single precision as ITK's float instantiation defines it.",
            "lx-cr-double": "With correctly rounded maths, ANTs computes exactly what larmorx "
            "computes.",
            "lx12-float": "The same bytes on any number of threads.",
            "lx12-double": "The same bytes on any number of threads.",
            "lx-fix-float": "No correction of the register applies to this command.",
            "lx-fix-double": "No correction of the register applies to this command.",
        }.get(i, "The same bytes.")
        if i == "lx-cr-float":
            links.append({"label": "ANTs' float", "href": "docs/findings/ants-float.md"})
    elif i.startswith("lx-ants-") and same[i.replace("lx-ants-", "lx-cr-")]:
        x["verdict"] = "identical-cr"
        x["reason"] = (
            "Only where glibc's sin, cos or exp is not correctly rounded (the Lanczos weights, "
            "the composite's matrices): with CORE-MATH preloaded, the same ANTs binary gives "
            "larmorx's output bit for bit."
        )
        links.append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
    elif i == "lx-fmriprep" and same["lx-cr-float"] and same["fmriprep-ants"]:
        x["verdict"] = "identical-cr"
        x["reason"] = (
            "fMRIPrep's output is ANTs as built (2.6.2 gives 2.6.5's bytes, reference rows); "
            "larmorx's is ANTs with correctly rounded maths."
        )
        links.append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
    else:
        x["verdict"] = "open"
        x["reason"] = "Not accounted for by the other runs of this example."
    x["links"] = links


def _example(ctx: core.Context) -> dict:
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = entry(ctx)
    info = e.info()
    run = common.fmriprep_run(RUN)
    ref = nib.load(str(td.get(TPL + "T1w.nii.gz")))
    brain = common.load(str(td.get(TPL + "desc-brain_mask.nii.gz"))) > 0
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(ref, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(
        exm.Panel(
            "template",
            "input",
            "MNI152NLin2009cAsym, 2 mm (reference, -r)",
            "the grid the T1w is resampled onto",
            {"all": common.load(str(td.get(TPL + "T1w.nii.gz")))},
        )
    )
    ex.add(
        exm.Panel(
            "input",
            "input",
            "ds000005 sub-01 T1w × brain mask (input, -i)",
            "on this grid without the transform (ANTs, Lanczos)",
            {"all": common.load(str(e.dir / "unregistered.nii.gz"))},
        )
    )
    for pid, label, role, text, sub in RUNS:
        path = run / NODE / OUT if label is None else e.dir / f"{label}.nii.gz"
        ex.add(exm.Panel(pid, role, text, sub, {"warped": common.load(str(path))}))
    diffs = _diffs()
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    for x in diffs:
        _verdict(x, same)
    by = {x["id"]: x for x in diffs}
    for i in ("lx-cr-float", "lx-cr-double", "fmriprep-ants", "ants8-float", "ants8-double"):
        if not same[i]:
            raise AssertionError(f"{TOOL}: the chain assumes {i} is bit-identical")
    labels = {p.id: (p.label, p.sub, p.role) for p in ex.panels}

    def node(pid, label=None, sub=None):
        lab, s, role = labels[pid]
        return {"label": label or lab, "sub": sub if sub is not None else s, "role": role}

    if not same["lx-ants-float"]:
        raise AssertionError(f"{TOOL}: the chain assumes larmorx --float 1 is ANTs' bytes")
    double_steps = [
        {"nodes": [node("ants-double"), node("ants-double-8")]},
        {
            "nodes": [
                node("ants-double-cr"),
                node("larmorx-double", "larmorx (port), double", "1 and 12 threads, both modes"),
            ]
        },
    ]
    if same["lx-ants-double"]:
        double_steps = [{"nodes": double_steps[0]["nodes"] + double_steps[1]["nodes"]}]
    chain = {
        "title": "Who gives which bytes: --float 1 (as fMRIPrep ran it), then double precision",
        "steps": [
            {
                "nodes": [
                    node("fmriprep"),
                    node("ants-float", sub="1 and 8 threads, glibc's or CORE-MATH's maths"),
                    node(
                        "larmorx-float", "larmorx (port), --float 1", "1 and 12 threads, both modes"
                    ),
                ]
            },
            *double_steps,
        ],
        "links": [
            {
                "cause": "double precision instead of --float 1",
                "max_abs": by["float-double"]["metrics"]["warped"]["max_abs"],
            },
            *(
                []
                if same["lx-ants-double"]
                else [
                    {
                        "cause": "correctly rounded maths instead of glibc's",
                        "max_abs": by["lx-ants-double"]["metrics"]["warped"]["max_abs"],
                    }
                ]
            ),
        ],
        "note": "Each box holds runs that give the same bytes. With --float 1, as fMRIPrep ran it, "
        "every run gives fMRIPrep's bytes: fMRIPrep's ANTs 2.6.2, the repository's ANTs 2.6.5 at "
        "one and eight threads, with glibc's or correctly rounded maths, and larmorx. In double "
        "precision glibc's maths moves a few voxels in their last bits"
        + (
            ", and larmorx computes ANTs' result with correctly rounded maths."
            if not same["lx-ants-double"]
            else "; here it does not."
        ),
    }
    at, _k = common.largest_in_slice(ex, "larmorx-double", "ants-double", "warped", "axial")
    default = {
        "output": "warped",
        "plane": "axial",
        "voxel": at,
        "panels": ["template", "input", "fmriprep", "larmorx-float"],
        "diffs": ["lx-fmriprep", "lx-ants-double", "float-double"],
        "compare": {"a": "template", "b": "larmorx-float"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        OUTPUTS,
        default=default,
        window_panel="larmorx-float",
        scale="log",
        caption="the crosshair where larmorx and ANTs as built differ most in double precision; "
        "the differences on one symmetric log scale over six decades below the largest.",
    )
    img = nib.load(str(run / INPUT))
    n_vox = int(np.prod(ref.shape))
    vers = info.get("versions", {})
    command = " ".join(info["command"].split()).replace(
        "/work/fmriprep_25_2_wf/sub_01_wf/ds_std_volumes_wf/", "…/"
    )
    parts = common.itk_parts(run / XFM)
    kinds = ", ".join(
        "a displacement field" if t.is_displacement_field else str(t.name)
        for t in parts
        if not t.is_composite
    )
    return {
        "id": "fmriprep-anat2std",
        "title": "fMRIPrep's T1w resampled into MNI space, by fMRIPrep, ANTs and larmorx",
        "summary": (
            "The node fMRIPrep 25.2.6 ran to write ds000005 sub-01's T1w in MNI152NLin2009cAsym "
            "at 2 mm: the masked T1w, the template and the composite transform of its outputs "
            "(the affine and the SyN displacement field of its template registration), Lanczos "
            "interpolation, `--float 1`; given to ANTs 2.6.5 and to larmorx, as fMRIPrep ran "
            "it and in double precision."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01's T1w as fMRIPrep preprocessed "
                f"and masked it ({common.DS000005['licence']}); {common.MNI_NOTICE}",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `ds_std_volumes_wf/anat2std_t1w`, one thread",
            },
            {"label": "Command", "text": f"`{command}`"},
            {"label": "Transform", "text": f"`{Path(XFM).name}`: {kinds}"},
            {
                "label": "Programs",
                "text": f"{vers.get('ants', '?')}; CORE-MATH preloaded where stated; larmorx at "
                f"commit `{info['larmorx_commit'][:12]}`",
            },
            {
                "label": "Grid",
                "text": f"input {' × '.join(map(str, img.shape))}; output "
                + common.describe_grid(ref, grid.shape),
            },
        ],
        **out,
        "chain": chain,
        "metrics_note": (
            f"Δ is A − B. Every number is computed in float64 over all {n_vox:,} voxels of the "
            f"{' × '.join(map(str, ref.shape))} outputs, not only the brain shown; the range is "
            "B's maximum minus its minimum. The rows *the original against itself* are ANTs' "
            "own variability on this input (its threads, its version, glibc's maths) and the "
            "size of single against double precision."
        ),
        "provenance": common.provenance(info, f"{TOOL}/{CASE}", [common.DS000005, common.MNI]),
    }


# ------------------------------------------------------------------------------------ cases


def _cases(ctx: core.Context, rec) -> list[dict]:
    from larmorx_validation.parity import ants_apply_transforms as suite
    from larmorx_validation.parity import ants_transform_output as outputs

    defs = {cid: (desc, s) for cid, _cat, desc, s in [*suite._scenarios(), *outputs.scenarios()]}
    defs.update({f"float/{cid}": v for cid, v in list(defs.items())})
    out = []
    for r in rec.results:
        desc, s = defs.get(r["case"], ("", None))
        v = common.outcome_verdict(r)
        rep = r.get("reported", {})
        if v is None:
            if r["case"].startswith("transform-output/"):
                h5 = {k: val for k, val in rep.items() if "ITK's strings" in k}
                vec = {k: val for k, val in rep.items() if "vectors bit for bit" in k}
                why = []
                if h5:
                    why.append(
                        "HDF5: every dataset and attribute bit for bit; the file's bytes are ITK's "
                        "given ITK's compressed chunks"
                        + (
                            ""
                            if all(int(x) == 0 for x in h5.values())
                            else f" ({max(int(x) for x in h5.values()):,} bytes differ with "
                            "larmorx's own deflate, which chooses other matches than zlib-ng)"
                        )
                    )
                if vec:
                    why.append(
                        "NIfTI field: header bytes and "
                        + (
                            "every vector bit for bit"
                            if all(int(x) == 0 for x in vec.values())
                            else "vectors that differ"
                        )
                    )
                if not why:
                    why.append("the file byte for byte (text or MATLAB format)")
                verdict = "bit-identical" if all(int(x) == 0 for x in vec.values()) else "open"
                v = (verdict, "; ".join(why))
            else:
                n = rep.get("bit-identical")
                n_cr = rep.get("bit-identical with CORE-MATH")
                if n is None:
                    v = ("bit-identical", "exit status and outputs compared")
                elif int(n) == 0:
                    v = ("bit-identical", "")
                elif n_cr is not None and int(n_cr) == 0:
                    v = (
                        "identical-cr",
                        f"{int(n):,} voxels differ from ANTs as built (glibc's sin, cos or exp); "
                        "bit-identical with CORE-MATH preloaded",
                    )
                else:
                    v = ("open", f"{int(n):,} voxels differ, within the record's thresholds")
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": desc,
                "command": _case_command(s) if s is not None else "",
                "results": {
                    "port": {
                        "outcome": r["status"],
                        "verdict": v[0],
                        "checks": f"{r['checks_passed']}/{r['checks_total']}",
                        "reason": common.clip(v[1]),
                    }
                },
                **(
                    {"example": "fmriprep-anat2std"}
                    if r["case"]
                    in ("fmriprep/T1w-to-MNI-lanczos", "float/fmriprep/T1w-to-MNI-lanczos")
                    else {}
                ),
            }
        )
    return out


def _case_command(s) -> str:
    def name(p: str) -> str:
        return core.short_path(p).split("/")[-1] if "/" in p else p

    if hasattr(s, "image"):
        args = ["-d", "3", "-i", name(s.image), "-r", name(s.reference)]
        for path, inv in s.transforms:
            p = name(path.split(":", 1)[-1]) if ":" in path else name(path)
            args += ["-t", f"[{p},1]" if inv else p]
        args += ["-n", s.interpolation, *s.extra, "-o", "out.nii.gz"]
        return common.command_text("antsApplyTransforms", args, lambda a: a)

    def sub(a: str) -> str:
        a = re.sub(r"\{t:([^}]+)\}", lambda m: m.group(1), a)
        a = re.sub(r"\{in:([^}]+)\}", lambda m: m.group(1).split("/")[-1], a)
        a = re.sub(r"\{gen:([^}]+)\}", lambda m: m.group(1), a)
        return a.replace("{out}/", "")

    return common.command_text("antsApplyTransforms", ["-d", "3", *s.args], sub)


def export(ctx: core.Context) -> dict:
    rec = records.load(ctx.repo, TOOL)
    cases = _cases(ctx, rec)
    bench = records.load_bench(ctx.repo, TOOL)
    from larmorx_validation.figures.ants_apply_transforms import SPEED_LABELS

    speed = common.speed(
        bench,
        [
            {
                "id": "ANTs 2.6.5|1 thread",
                "label": "ANTs (through ANTsPy), 1 thread",
                "mark": "original",
            },
            {
                "id": "ANTs 2.6.5|12 threads",
                "label": "ANTs (through ANTsPy), 12 threads",
                "mark": "original-open",
            },
            {"id": "larmorx (CLI)|1 thread", "label": "larmorx, 1 thread", "mark": "larmorx"},
            {
                "id": "larmorx (CLI)|12 threads",
                "label": "larmorx, 12 threads",
                "mark": "larmorx-open",
            },
        ],
        SPEED_LABELS,
        title="Seconds for the whole command (read, resample, write), median of repeated "
        "runs, log scale; the factor is larmorx's speed-up at one thread",
        base=("ANTs 2.6.5|1 thread", "larmorx (CLI)|1 thread"),
        note="From the benchmark report `docs/benchmarks/ants-apply-transforms.json` ({cpu}, "
        "{cpus} logical CPUs), 2026-10-10: ANTs 2.6.5 through ANTsPy, both in double precision "
        "(the report predates `--float 1`).",
        inputs=list(SPEED_LABELS),
    )
    from larmorx_validation.evidence import ants_apply_transforms_bspline as bspline

    e = entry(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "antsApplyTransforms",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source(),
            "benchmarks": bench.source() if bench else [],
            "cache": [fcache.provenance(e), fcache.provenance(bspline.entry(ctx))],
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
        "examples": [_example(ctx), bspline.example(ctx)],
    }


if __name__ == "__main__":
    from larmorx_validation.figures.core import REPO, default_cache

    print("done", entry(core.Context(repo=REPO, site=Path("."), cache=default_cache())).dir)
