# SPDX-License-Identifier: Apache-2.0
"""antsApplyTransforms's second worked example: a B-spline transform applied by ANTs 2.6.5 and
by larmorx. The transforms come from ANTs itself: sMRIPrep's template registration of ds000005
sub-01 (its own command, every third voxel of its images, as the antsRegistration record runs
it) with a ``BSpline`` stage (a cubic B-spline transform, knot spacing 104 mm) in place of its
SyN stage. Not 26 mm (antsRegistrationSyN.sh's B-spline spacing): the mesh doubles at each level,
to about a million parameters at the last one, and ANTs computes every parameter's Jacobian at
every voxel; it had not finished after almost four hours (larmorx: 38 s), see
``docs/findings/ants-bspline.md``. fMRIPrep's masked T1w (1 mm) is then resampled onto the 2 mm
template through the registration's own composite (``transformComposite.h5``, as sMRIPrep's
options write it: the collapsed affine, then the B-spline transform, both in float), with
Lanczos interpolation, as fMRIPrep resamples the T1w into MNI space, in single (``--float 1``)
and double precision: ANTs as built (1 and 8 threads) and with correctly rounded maths, larmorx
at 1 and 12 threads in both modes. The affine alone (for the panel that shows what the B-spline
transform adds) is taken out of the composite by ANTs' ``CompositeTransformUtil``."""

from __future__ import annotations

from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core
from larmorx_validation.figures import cache as fcache

TOOL = "ants-apply-transforms"
CASE = "bspline104-ds000005-t1w"
RUN = "ds000005-sub-01-noreconall"
WF = "work/fmriprep_25_2_wf/sub_01_wf/ds_std_volumes_wf"
INPUT = f"{WF}/mask_anat/sub-01_desc-preproc_T1w_masked.nii.gz"
TPL = "templateflow/tpl-MNI152NLin2009cAsym/tpl-MNI152NLin2009cAsym_res-02_"
STAGE = "BSpline[0.1,104]"
LX_THREADS = (1, 12)


def _registration_args(out: Path, tmp: Path) -> list[str]:
    from larmorx_validation.evidence.ants_registration import SPEC
    from larmorx_validation.parity.ants_registration import SMALL, _fmriprep_args, _replace_syn

    args = _fmriprep_args(SPEC, out, tmp=tmp, stand_ins=True, small=SMALL)
    return _replace_syn(args, STAGE)


def _apply_args(inp: str, ref: str, transforms: list[str], out: str, float_: bool):
    args = ["-d", "3", "--float", "1" if float_ else "0", "-i", inp, "-r", ref]
    args += ["-n", "LanczosWindowedSinc"]
    for t in transforms:
        args += ["-t", t]
    return [*args, "-o", out]


def _logged_run(e: fcache.Entry, label: str, argv: list[str]) -> bool:
    """Take the run ``label`` from its log when it already ran with these arguments and
    succeeded (an interrupted computation of this entry resumed: the registration takes ANTs
    over an hour); ``False`` when it has to run."""
    import ast
    import shlex

    log = e.dir / f"{label}.log"
    if not log.is_file():
        return False
    head = log.read_text(encoding="utf-8").split("\n", 3)
    if len(head) < 3 or not head[0].startswith("$ ") or shlex.split(head[0][2:]) != argv:
        return False
    env = ast.literal_eval(head[1].removeprefix("# env: "))
    status, _, secs = head[2].removeprefix("# exit ").partition(", ")
    if status != "0":
        return False
    e.runs.append(fcache.Run(label, argv, env, 0, float(secs.removesuffix(" s"))))
    e.ctx.log(f"  {e.tool}/{e.case}: {label} (taken from its log)")
    return True


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload
    from larmorx_validation.parity.ants_registration import SEED

    run = common.fmriprep_run(RUN)
    b, lib = ants_bin(), crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload is missing")
    d = e.dir
    # The transforms: ANTs' own registration with a BSpline stage (4 threads: an input here).
    reg = d / "registration"
    reg.mkdir(exist_ok=True)
    tmp = d / "inputs"
    tmp.mkdir(exist_ok=True)
    argv = [str(b / "antsRegistration"), *_registration_args(reg, tmp)]
    composite = reg / "transformComposite.h5"
    if not (composite.is_file() and _logged_run(e, "registration", argv)):
        e.run(
            "registration",
            argv,
            env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "4", "ANTS_RANDOM_SEED": SEED},
            unset=common.CLEAN_ENV,
        )
    comp = e.input("the registration's composite (affine, then B-spline; float)", composite)
    # The affine alone, out of the composite by ANTs' own tool (00_..._AffineTransform.mat).
    e.run(
        "disassemble",
        [str(b / "CompositeTransformUtil"), "--disassemble", str(composite), str(reg / "part")],
        unset=common.CLEAN_ENV,
    )
    aff = e.input("affine (the composite's first transform)", reg / "part_00_AffineTransform.mat")
    inp = e.input("input (fMRIPrep's T1w × brain mask)", run / INPUT)
    ref = e.input("reference (MNI152NLin2009cAsym, 2 mm)", Path(td.get(TPL + "T1w.nii.gz")))
    for prec, float_ in (("float", True), ("double", False)):
        for label, threads, cr in (
            (f"ants-{prec}-1", 1, False),
            (f"ants-{prec}-8", 8, False),
            (f"ants-{prec}-1-cr", 1, True),
        ):
            env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
            if cr:
                env["LD_PRELOAD"] = str(lib)
            out = str(d / f"{label}.nii.gz")
            e.run(
                label,
                [
                    str(b / "antsApplyTransforms"),
                    *_apply_args(str(inp), str(ref), [str(comp)], out, float_),
                ],
                env=env,
                unset=common.CLEAN_ENV,
            )
        for threads in LX_THREADS:
            for mode in ("fix", "reproduce"):
                label = f"larmorx-{prec}-{threads}" + ("-reproduce" if mode == "reproduce" else "")
                out = str(d / f"{label}.nii.gz")
                e.run(
                    label,
                    fcache.larmorx_cli(
                        "ants",
                        "antsApplyTransforms",
                        *_apply_args(str(inp), str(ref), [str(comp)], out, float_),
                    ),
                    env={
                        "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                        "LARMORX_UPSTREAM_DEFECTS": mode,
                    },
                    unset=common.CLEAN_ENV,
                )
    # The input through the affine alone: what the B-spline transform adds.
    e.run(
        "affine-only",
        [
            str(b / "antsApplyTransforms"),
            "-d",
            "3",
            "--float",
            "1",
            "-i",
            str(inp),
            "-r",
            str(ref),
            "-n",
            "LanczosWindowedSinc",
            "-t",
            str(aff),
            "-o",
            str(d / "affine-only.nii.gz"),
        ],
        env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"},
        unset=common.CLEAN_ENV,
    )
    e.finish(
        versions={"ants": ants_version()},
        command="antsApplyTransforms -d 3 --float 1 -i <T1w> -r <MNI 2 mm> -n LanczosWindowedSinc "
        "-t transformComposite.h5 -o <out>",
        threads={"ants": [1, 8], "larmorx": list(LX_THREADS), "registration": 4},
        seed=f"ANTS_RANDOM_SEED={SEED}",
        settings=f"the composite transform (affine, then B-spline) of sMRIPrep's template "
        f"registration of {RUN} with {STAGE} for its SyN stage (ANTs 2.6.5, every third voxel), "
        "applied to the 1 mm T1w onto the 2 mm template, --float 1 and --float 0",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "a B-spline transform (ANTs' BSpline stage on ds000005 sub-01) and its affine applied to "
        "the T1w: ANTs 2.6.5 and larmorx, single and double precision",
        _compute,
    )


RUNS = []
for _p, _P in (("float", "--float 1"), ("double", "double")):
    RUNS += [
        (f"larmorx-{_p}", f"larmorx-{_p}-1-reproduce", "larmorx", f"larmorx (port), {_P}", "1 thread, reproduce mode"),
        (f"larmorx-{_p}-12", f"larmorx-{_p}-12-reproduce", "larmorx", f"larmorx (port), {_P}", "12 threads, reproduce mode"),
        (f"larmorx-{_p}-fix", f"larmorx-{_p}-1", "larmorx", f"larmorx (port), {_P}", "1 thread, default mode"),
        (f"ants-{_p}-cr", f"ants-{_p}-1-cr", "reference", f"ANTs 2.6.5, {_P}, correctly rounded maths", "1 thread, CORE-MATH preloaded"),
        (f"ants-{_p}", f"ants-{_p}-1", "original", f"ANTs 2.6.5, {_P}, as built", "1 thread, glibc's maths (GCC 14.3 build)"),
        (f"ants-{_p}-8", f"ants-{_p}-8", "original", f"ANTs 2.6.5, {_P}", "8 threads, as built"),
    ]  # fmt: skip
OUTPUTS = [{"id": "warped", "label": "T1w in MNI space", "unit": ""}]


def _diffs() -> list[dict]:
    d = common.d
    out = []
    for p, P in (("float", "--float 1"), ("double", "double")):
        out += [
            d(f"lx-ants-{p}", f"larmorx-{p}", f"ants-{p}", "main", f"larmorx − ANTs 2.6.5, {P}, as built", common.ROLE_LX),
            d(f"lx-cr-{p}", f"larmorx-{p}", f"ants-{p}-cr", "main", f"larmorx − ANTs 2.6.5, {P}, correctly rounded maths", common.ROLE_LX),
            d(f"lx12-{p}", f"larmorx-{p}-12", f"larmorx-{p}", "check", f"larmorx {P}, 12 threads − 1 thread", common.ROLE_SELF),
            d(f"lx-fix-{p}", f"larmorx-{p}-fix", f"larmorx-{p}", "check", f"larmorx {P}, default − reproduce mode", common.ROLE_SELF),
            d(f"ants8-{p}", f"ants-{p}-8", f"ants-{p}", "variability", f"ANTs 2.6.5 {P}, 8 threads − 1 thread", common.ROLE_ORIG),
            d(f"cr-ants-{p}", f"ants-{p}-cr", f"ants-{p}", "variability", f"ANTs {P}: correctly rounded − glibc's maths", common.ROLE_ORIG),
        ]  # fmt: skip
    out.append(
        d(
            "float-double",
            "ants-float",
            "ants-double",
            "variability",
            "ANTs 2.6.5: --float 1 − double, as built",
            common.ROLE_ORIG,
        )
    )
    return out


def _verdict(x: dict, same: dict[str, bool]) -> None:
    i, links = x["id"], []
    if x["kind"] == "variability":
        x["verdict"] = None
        if i.startswith("ants8-"):
            x["reason"] = (
                "ANTs resamples each voxel on its own: its thread count changes nothing."
                if same[i]
                else "These runs differ."
            )
        elif i.startswith("cr-ants-"):
            x["reason"] = (
                "glibc's sin and cos (the windowed sinc's weights) are not always correctly rounded."
                if not same[i]
                else "glibc's maths plays no part here."
            )
            links.append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
        else:
            x["reason"] = "Single precision against double: the size of the precision chosen."
    elif same[i]:
        x["verdict"] = "bit-identical"
        x["reason"] = {
            "lx-cr-float": "With correctly rounded maths, ANTs computes exactly what larmorx "
            "computes, the B-spline transform in single precision as ITK's float instantiation "
            "defines it.",
            "lx-cr-double": "With correctly rounded maths, ANTs computes exactly what larmorx "
            "computes.",
            "lx-ants-float": "The same bytes as ANTs as built.",
            "lx-ants-double": "The same bytes as ANTs as built.",
        }.get(i, "The same bytes." if not i.startswith("lx-fix") else "No correction of the "
              "register applies to this command.")  # fmt: skip
    elif i.startswith("lx-ants-") and same[i.replace("lx-ants-", "lx-cr-")]:
        x["verdict"] = "identical-cr"
        x["reason"] = (
            "Only where glibc's sin or cos is not correctly rounded (the Lanczos weights): with "
            "CORE-MATH preloaded, the same ANTs binary gives larmorx's output bit for bit."
        )
        links.append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
    else:
        x["verdict"] = "open"
        x["reason"] = "Not accounted for by the other runs of this example."
    x["links"] = links


def example(ctx: core.Context) -> dict:
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
            "affine-only",
            "input",
            "the T1w through the affine alone",
            "ANTs, --float 1, Lanczos: what the B-spline transform then moves",
            {"all": common.load(str(e.dir / "affine-only.nii.gz"))},
        )
    )
    for pid, label, role, text, sub in RUNS:
        ex.add(
            exm.Panel(pid, role, text, sub, {"warped": common.load(str(e.dir / f"{label}.nii.gz"))})
        )
    diffs = _diffs()
    common.compare_all(ex, diffs, OUTPUTS)
    same = common.identical(diffs)
    for x in diffs:
        _verdict(x, same)
    for i in ("lx-cr-float", "lx-cr-double", "lx12-float", "lx12-double"):
        if not same[i]:
            raise AssertionError(f"{TOOL}/{CASE}: the chain assumes {i} is bit-identical")
    by = {x["id"]: x for x in diffs}
    labels = {p.id: (p.label, p.sub, p.role) for p in ex.panels}

    def node(pid, label=None, sub=None):
        lab, s, role = labels[pid]
        return {"label": label or lab, "sub": sub if sub is not None else s, "role": role}

    def steps(p: str) -> list[dict]:
        a = [node(f"ants-{p}", sub="1 and 8 threads, as built")]
        b = [node(f"ants-{p}-cr"), node(f"larmorx-{p}", sub="1 and 12 threads, both modes")]
        return [{"nodes": a + b}] if same[f"lx-ants-{p}"] else [{"nodes": a}, {"nodes": b}]

    chain = {
        "title": "Who gives which bytes: --float 1, then double precision",
        "steps": [*steps("float"), *steps("double")],
        "links": [
            {
                "cause": "double precision instead of --float 1",
                "max_abs": common.max_abs(by["float-double"]),
            },
            *(
                []
                if same["lx-ants-double"]
                else [
                    {
                        "cause": "correctly rounded maths instead of glibc's (double)",
                        "max_abs": common.max_abs(by["lx-ants-double"]),
                    }
                ]
            ),
        ],
        "note": "Each box holds runs that give the same bytes. larmorx applies ANTs' B-spline "
        "transform as ANTs does, in single and in double precision, on any number of threads.",
    }
    at, _k = common.largest_in_slice(ex, "larmorx-double", "ants-double", "warped", "axial")
    default = {
        "output": "warped",
        "plane": "axial",
        "voxel": at,
        "panels": ["template", "affine-only", "ants-float", "larmorx-float"],
        "diffs": ["lx-ants-float", "lx-ants-double", "float-double"],
        "compare": {"a": "affine-only", "b": "larmorx-float"},
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
        "Swipe compares the T1w through the affine alone with it through the B-spline transform "
        "too; the differences on one symmetric log scale over six decades below the largest.",
    )
    img = nib.load(str(run / INPUT))
    n_vox = int(np.prod(ref.shape))
    vers = info.get("versions", {})
    parts = common.itk_parts(e.dir / "registration" / "transformComposite.h5")
    a, b = (next(t for t in parts if t.name.startswith(k)) for k in ("Affine", "BSpline"))
    mesh = " × ".join(str(int(v) - 3) for v in b.fixed_parameters[:3])
    coef = np.abs(np.asarray(b.parameters, float))
    return {
        "id": "bspline-transform",
        "title": "A B-spline transform applied by ANTs and larmorx",
        "summary": (
            "The B-spline transform ANTs 2.6.5 found for ds000005 sub-01's T1w (sMRIPrep's "
            f"template registration with {STAGE} for its SyN stage, on every third voxel; "
            "a knot spacing of 104 mm, because with antsRegistrationSyN.sh's 26 mm ANTs had not "
            "finished after almost four hours), "
            "applied with the registration's affine (its composite, as antsRegistration writes "
            "it with sMRIPrep's options) to the 1 mm T1w onto the 2 mm template, "
            "Lanczos, as fMRIPrep resamples the T1w into MNI space; given to ANTs 2.6.5 and to "
            "larmorx, in single and double precision. Before A5 part 3 larmorx refused "
            "B-spline transform files."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01's T1w as fMRIPrep preprocessed "
                f"and masked it ({common.DS000005['licence']}); {common.MNI_NOTICE}",
            },
            {
                "label": "Transforms",
                "text": f"`transformComposite.h5` (ANTs' HDF5 file): an `{a.name}` (the Rigid "
                f"and Affine stages, collapsed), then a `{b.name}` of mesh {mesh} (control "
                f"points every {', '.join(f'{v:.4g}' for v in b.fixed_parameters[6:9])} mm, "
                f"{b.parameters.size} coefficients up to {coef.max():.3g} mm)",
            },
            {"label": "Command", "text": f"`{info['command']}`"},
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
            "own variability on this input and the size of single against double precision."
        ),
        "provenance": common.provenance(info, f"{TOOL}/{CASE}", [common.DS000005, common.MNI]),
    }


if __name__ == "__main__":
    from larmorx_validation.figures.core import REPO, default_cache

    print("done", entry(core.Context(repo=REPO, site=Path("."), cache=default_cache())).dir)
