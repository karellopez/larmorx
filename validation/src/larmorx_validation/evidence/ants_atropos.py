# SPDX-License-Identifier: Apache-2.0
"""Atropos' evidence: sMRIPrep's segmentation of a real T1w (the brain extraction's
``atropos_wf/01_atropos``), by fMRIPrep, by ANTs 2.6.5 at 1, 2, 4 and 8 threads, with correctly
rounded maths, three times with the clock's seed, by fMRIPrep's own ANTs at one thread, and by
larmorx (1 and 6 threads, default and ``reproduce`` modes, and with ``--use-random-seed 1``);
an antsCorticalThickness-style segmentation of the same T1w with the OASIS30ANTs tissue priors;
the record's cases.

The worked examples use OpenNeuro ds000005 sub-01 (PDDL-1.0), the inputs fMRIPrep 25.2.6 gave
Atropos in its oracle run (the N4-corrected T1w and the brain mask), and the OASIS30ANTs
priors from TemplateFlow (CC-BY 4.0) brought into the T1w's space with the transform fMRIPrep's
brain extraction computed."""

from __future__ import annotations

import re
import subprocess
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import ants_atropos_pv, common, core
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "ants-atropos"
RUN = "ds000005-sub-01-noreconall"
WF = "work/fmriprep_25_2_wf/sub_01_wf/anat_fit_wf/brain_extraction_wf"
NODE = WF + "/atropos_wf/01_atropos"
MASK = "out/sub-01/anat/sub-01_desc-brain_mask.nii.gz"
#: ANTs' thread counts (1 is the reference; fMRIPrep ran 8)
THREADS = (1, 2, 4, 8)
#: larmorx's thread counts (6: the machine was shared when the entry was computed)
LX_THREADS = (1, 6)
#: ANTs runs with the clock's seed (``--use-random-seed 1``)
SEEDS = ("ants-seed-1", "ants-seed-2", "ants-seed-3")
OASIS = {
    "dataset": "OASIS30ANTs (Avants et al., MICCAI 2016), from TemplateFlow",
    "licence": "CC-BY 4.0",
}
OUTPUTS = (
    {"id": "labels", "label": "Classes (1 CSF, 2 GM, 3 WM)", "unit": ""},
    {"id": "post1", "label": "Posterior: CSF", "unit": ""},
    {"id": "post2", "label": "Posterior: grey matter", "unit": ""},
    {"id": "post3", "label": "Posterior: white matter", "unit": ""},
)


def _outputs(ids=None) -> list[dict]:
    """A worked example's own copy of the outputs (``finish`` writes each output's default
    windows into it, so two examples must not share them)."""
    return [dict(o) for o in OUTPUTS if ids is None or o["id"] in ids]


FILES = {"labels": "{}.nii.gz", "post1": "{}-post01.nii.gz", "post2": "{}-post02.nii.gz",
         "post3": "{}-post03.nii.gz"}  # fmt: skip


def _words(run: Path, node: str) -> list[str]:
    return (run / node / "command.txt").read_text(encoding="utf-8").split()


def _argv(words: list[str], root: str, out: str, seed: str | None = None) -> list[str]:
    """The node's Atropos arguments with ``/work/`` at ``root`` and the outputs at ``out``
    (``out.nii.gz``, ``out-post%02d.nii.gz``); ``seed`` replaces ``--use-random-seed``."""
    args = [w.replace("/work/", f"{root}/") for w in words[1:]]
    i = args.index("--output")
    args[i + 1] = f"[{out}.nii.gz,{out}-post%02d.nii.gz]"
    if seed is not None:
        args[args.index("--use-random-seed") + 1] = seed
    return args


def _compute(e: fcache.Entry) -> None:
    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    ora = common.oracles()
    run = common.fmriprep_run(RUN)
    b, lib, sif = ants_bin(), crmath_preload(), ora / common.FMRIPREP_IMAGE
    if b is None or lib is None or not sif.is_file():
        raise FileNotFoundError("the ANTs oracle, the CORE-MATH preload or fMRIPrep's image")
    words = _words(run, NODE)
    work = str(run / "work")
    args = _argv(words, work, "x")
    e.input("N4-corrected T1w", Path(args[args.index("--intensity-image") + 1]))
    e.input("brain mask", Path(args[args.index("--mask-image") + 1]))
    for f in sorted((run / NODE).glob("*.nii.gz")):
        e.input(f"fMRIPrep's {f.name}", f)
    d = e.dir
    runs = [(f"ants-{t}", t, False, None) for t in THREADS] + [("ants-1-cr", 1, True, None)]
    runs += [(label, 1, False, "1") for label in SEEDS]
    for label, threads, cr, seed in runs:
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads)}
        if cr:
            env["LD_PRELOAD"] = str(lib)
        e.run(label, [str(b / "Atropos"), *_argv(words, work, str(d / label), seed)], env=env,
              unset=common.CLEAN_ENV)  # fmt: skip
    home = d / ".home"
    home.mkdir(exist_ok=True)
    cmd = ["apptainer", "exec", "--cleanenv", "--home", f"{home}:/home/fmriprep"]
    for k, v in {"NSLOTS": "1", "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}.items():
        cmd += ["--env", f"{k}={v}"]
    cmd += ["-B", f"{work}:/work:ro", "-B", f"{d}:/out", str(sif), "Atropos"]
    e.run("fmriprep-image-1", [*cmd, *_argv(words, "/work", "/out/fmriprep-image-1")],
          unset=common.CLEAN_ENV)  # fmt: skip
    for threads in LX_THREADS:
        for mode in ("fix", "reproduce"):
            label = f"larmorx-{threads}" + ("-reproduce" if mode == "reproduce" else "")
            e.run(
                label,
                fcache.larmorx_cli("ants", "Atropos", *_argv(words, work, str(d / label))),
                env={
                    "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                    "LARMORX_UPSTREAM_DEFECTS": mode,
                },
                unset=common.CLEAN_ENV,
            )
    e.run(
        "larmorx-seed",
        fcache.larmorx_cli("ants", "Atropos", *_argv(words, work, str(d / "larmorx-seed"), "1")),
        env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "6", "LARMORX_UPSTREAM_DEFECTS": "reproduce"},
        unset=common.CLEAN_ENV,
    )
    image_version = subprocess.run(
        # Atropos has no --version; the image's antsRegistration is the same ANTs build
        ["apptainer", "exec", "--cleanenv", str(sif), "antsRegistration", "--version"],
        capture_output=True,
        text=True,
        encoding="utf-8",
    ).stdout.strip()
    e.finish(
        versions={
            "ants": ants_version(),
            "ants_in_fmriprep_image": " ".join(image_version.split()),
        },
        command=" ".join(words),
        threads={"ants": list(THREADS), "larmorx": list(LX_THREADS), "fmriprep_run": 8},
        settings="fMRIPrep 25.2.6's Atropos call in ds000005 sub-01's brain extraction "
        "(atropos_wf/01_atropos), its arguments from command.txt; ANTs 2.6.5 at 1, 2, 4 and 8 "
        "threads, with CORE-MATH at 1, three times with --use-random-seed 1 (the clock's seed); "
        "fMRIPrep's own ANTs at 1 thread; larmorx in both modes at 1 and 6 threads, and with "
        "--use-random-seed 1 (seed 121212)",
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        "fmriprep-ds000005-atropos",
        "fMRIPrep's Atropos of ds000005 sub-01's T1w: fMRIPrep, ANTs 2.6.5 at 1-8 threads, "
        "CORE-MATH, the clock's seed, fMRIPrep's ANTs at 1 thread, and larmorx",
        _compute,
    )


#: the antsCorticalThickness-style segmentation's options
PRIORS_ARGS = ("-k", "Gaussian", "-m", "[0.1,1x1x1]", "-c", "[5,0]", "-g", "1",
               "-p", "Socrates[1]", "-r", "0", "-e", "0")  # fmt: skip


def _compute_priors(e: fcache.Entry) -> None:
    from larmorx_validation.parity import ants_atropos as suite
    from larmorx_validation.parity.ants_programs import ants_bin, ants_version, crmath_preload

    run = common.fmriprep_run(RUN)
    b, lib = ants_bin(), crmath_preload()
    if b is None or lib is None:
        raise FileNotFoundError("the ANTs oracle or the CORE-MATH preload")
    d = e.dir
    words = _words(run, WF + "/map_brainmask")
    work = str(run / "work")
    warp = [w.replace("/work/", f"{work}/") for w in words[1:]]
    for c, label in enumerate(suite.OASIS_CLASSES[3], 1):
        template = e.input(f"OASIS30ANTs {label} prior", suite._oasis_prior(label))
        args = list(warp)
        args[args.index("--input") + 1] = str(template)
        args[args.index("--output") + 1] = str(d / f"prior{c:02d}.nii.gz")
        e.run(f"prior{c:02d}", [str(b / "antsApplyTransforms"), *args],
              env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "6"}, unset=common.CLEAN_ENV)  # fmt: skip
    node = _argv(_words(run, NODE), work, "x")
    image = node[node.index("--intensity-image") + 1]
    mask = node[node.index("--mask-image") + 1]
    e.input("N4-corrected T1w", Path(image))
    e.input("brain mask", Path(mask))

    def argv(out: str) -> list[str]:
        return ["-d", "3", "-a", image, "-x", mask, "-i",
                f"PriorProbabilityImages[3,{d}/prior%02d.nii.gz,0.25]", *PRIORS_ARGS,
                "-o", f"[{out}.nii.gz,{out}-post%02d.nii.gz]"]  # fmt: skip

    for label, cr in (("ants-1", False), ("ants-1-cr", True)):
        env = {"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": "1"}
        if cr:
            env["LD_PRELOAD"] = str(lib)
        e.run(label, [str(b / "Atropos"), *argv(str(d / label))], env=env, unset=common.CLEAN_ENV)
    for label, threads, mode in (
        ("larmorx-1-reproduce", 1, "reproduce"),
        ("larmorx-6-reproduce", 6, "reproduce"),
        ("larmorx-1", 1, "fix"),
    ):
        e.run(label, fcache.larmorx_cli("ants", "Atropos", *argv(str(d / label))),
              env={"ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": str(threads),
                   "LARMORX_UPSTREAM_DEFECTS": mode}, unset=common.CLEAN_ENV)  # fmt: skip
    e.finish(
        versions={"ants": ants_version()},
        command="Atropos " + " ".join(argv("out")),
        threads={"ants": [1], "larmorx": [1, 6]},
        settings="antsCorticalThickness-style Atropos (PriorProbabilityImages weight 0.25, MRF "
        "0.1, five iterations, Socrates, the fixed seed) on the inputs of fMRIPrep's Atropos "
        "call in ds000005 sub-01, with OASIS30ANTs' CSF, cortical grey matter and white matter "
        "priors brought into the T1w's space by fMRIPrep's map_brainmask command (ANTs "
        "antsApplyTransforms, Gaussian interpolation); ANTs 2.6.5 at 1 thread, with CORE-MATH; "
        "larmorx in both modes",
    )


def priors_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        "ds000005-oasis-priors",
        "An antsCorticalThickness-style Atropos of ds000005 sub-01's T1w with the OASIS30ANTs "
        "tissue priors: ANTs 2.6.5, CORE-MATH, larmorx",
        _compute_priors,
    )


# ----------------------------------------------------------------------------------- export


def _load_run(dirpath: Path, label: str) -> dict[str, np.ndarray]:
    return {o: common.load(str(dirpath / f.format(label))) for o, f in FILES.items()}


def _fmriprep_outputs(run: Path) -> dict[str, np.ndarray]:
    node = run / NODE
    seg = sorted(node.glob("*_labeled.nii.gz"))[0]
    out = {"labels": common.load(str(seg))}
    for k in (1, 2, 3):
        out[f"post{k}"] = common.load(str(node / f"POSTERIOR_{k:02d}.nii.gz"))
    return out


#: panel id, cache label (None: fMRIPrep's own), role, label, sub-label
RUNS = [
    ("larmorx", "larmorx-1-reproduce", "larmorx", "larmorx (port), 1 thread", "reproduce mode"),
    ("larmorx-6", "larmorx-6-reproduce", "larmorx", "larmorx (port), 6 threads", "reproduce"),
    ("larmorx-fix", "larmorx-1", "larmorx", "larmorx (port), 1 thread",
     "default mode: ITK-50 corrected"),
    ("larmorx-6-fix", "larmorx-6", "larmorx", "larmorx (port), 6 threads", "default mode"),
    ("ants-1-cr", "ants-1-cr", "reference", "ANTs 2.6.5, 1 thread, correctly rounded maths",
     "the same binary, CORE-MATH preloaded"),
    ("ants-1", "ants-1", "original", "ANTs 2.6.5, 1 thread, as built",
     "the repository's oracle build (GCC 14.3), glibc's maths"),
    ("image-1", "fmriprep-image-1", "original", "ANTs (fMRIPrep's image), 1 thread",
     "conda-forge's build, glibc's maths"),
    ("ants-8", "ants-8", "original", "ANTs 2.6.5, 8 threads", "as built (fMRIPrep ran 8)"),
    ("ants-seed-1", "ants-seed-1", "original", "ANTs 2.6.5, --use-random-seed 1",
     "the clock's seed (one of three runs)"),
    ("fmriprep", None, "fmriprep", "fMRIPrep 25.2.6, as it ran", "ANTs in its image, 8 threads"),
]  # fmt: skip


def _diffs() -> list[dict]:
    d = common.d
    return [
        d("lx-ants1", "larmorx", "ants-1", "main", "larmorx − ANTs 2.6.5, 1 thread, as built",
          common.ROLE_LX),
        d("lx-cr", "larmorx", "ants-1-cr", "main",
          "larmorx − ANTs 2.6.5, 1 thread, correctly rounded maths", common.ROLE_LX),
        d("lx-fmriprep", "larmorx", "fmriprep", "fmriprep", "larmorx − fMRIPrep's own output",
          common.ROLE_FMRIPREP),
        d("fix-lx", "larmorx-fix", "larmorx", "main",
          "larmorx default mode − reproduce mode (ITK-50)", "larmorx's correction"),
        d("lxseed-seed1", "larmorx-seed", "ants-seed-1", "main",
          "larmorx, seed 121212 − ANTs, the clock's seed (run 1)", common.ROLE_LX),
        d("lx6-lx1", "larmorx-6", "larmorx", "check", "larmorx, 6 threads − 1 thread",
          common.ROLE_SELF),
        d("fix6-fix1", "larmorx-6-fix", "larmorx-fix", "check",
          "larmorx default mode, 6 threads − 1 thread", common.ROLE_SELF),
        *[d(f"ants{t}-ants1", f"ants-{t}", "ants-1", "variability",
            f"ANTs 2.6.5, {t} threads − 1 thread", common.ROLE_ORIG) for t in THREADS[1:]],
        d("seed2-seed1", "ants-seed-2", "ants-seed-1", "variability",
          "ANTs 2.6.5, the clock's seed: run 2 − run 1", common.ROLE_ORIG),
        d("seed3-seed1", "ants-seed-3", "ants-seed-1", "variability",
          "ANTs 2.6.5, the clock's seed: run 3 − run 1", common.ROLE_ORIG),
        d("seed1-ants1", "ants-seed-1", "ants-1", "variability",
          "ANTs 2.6.5: the clock's seed − sMRIPrep's fixed seed", common.ROLE_ORIG),
        d("image1-ants1", "image-1", "ants-1", "variability",
          "ANTs in fMRIPrep's image − ANTs 2.6.5, 1 thread each, as built", common.ROLE_ORIG),
        d("fmriprep-ants8", "fmriprep", "ants-8", "variability",
          "fMRIPrep's own output − ANTs 2.6.5, 8 threads", common.ROLE_ORIG),
    ]  # fmt: skip


def _seed_band(by: dict) -> dict[str, float]:
    """ANTs' own spread with the clock's seed, per output: the most labels that differ
    between two of its runs (in voxels) and the largest posterior difference."""
    out = {}
    for o in ("labels", "post1", "post2", "post3"):
        ms = [by[i]["metrics"][o] for i in ("seed2-seed1", "seed3-seed1") if o in by[i]["metrics"]]
        out[o] = max((m["differ"] if o == "labels" else m["max_abs"]) for m in ms)
    return out


def _verdict(x: dict, same: dict[str, bool], by: dict, reg: dict) -> None:
    i, links = x["id"], []
    if x["kind"] == "variability":
        x["verdict"] = None
        if same[i]:
            x["reason"] = {
                "image1-ants1": "ANTs' version and build play no part at one thread.",
                "fmriprep-ants8": "fMRIPrep's own run gives the repository's ANTs result bit "
                "for bit.",
            }.get(i, "The same bytes: Atropos' result does not depend on ANTs' thread count.")
        elif i.startswith("seed"):
            x["reason"] = (
                "Atropos seeds its generator from the clock unless --use-random-seed 0 (sMRIPrep "
                "passes 0): the ICM visiting order and the random tie-breaks change from run "
                "to run."
            )
        elif i == "image1-ants1":
            x["reason"] = "The two ANTs builds differ at one thread."
        else:
            x["reason"] = "ANTs' result moves with its thread count."
    elif i == "fix-lx":
        x["verdict"] = "d13" if not same[i] else "bit-identical"
        x["reason"] = (
            "ITK-50, corrected by default with a warning: ITK's k-means leaves out the point "
            "each divided k-d tree node is split at, so the starting centres miss those points; "
            "larmorx counts every point. Reproduce mode gives ANTs' result."
        )
        links.append({"label": "ITK-50", "href": reg["ITK-50"]["link"]})
    elif i == "lxseed-seed1":
        band = _seed_band(by)
        m = x["metrics"]
        inside = m["labels"]["differ"] <= band["labels"] and all(
            m[o]["max_abs"] <= band[o] for o in ("post1", "post2", "post3")
        )
        x["verdict"] = "within-variability" if inside else "open"
        x["reason"] = (
            "With --use-random-seed 1 ANTs draws its seed from the clock and larmorx uses "
            f"121212: another draw. larmorx's labels differ from ANTs' run 1 in "
            f"{m['labels']['differ']:,} voxels, ANTs' own runs from each other in up to "
            f"{int(band['labels']):,}; the posteriors by up to "
            f"{max(m[o]['max_abs'] for o in ('post1', 'post2', 'post3')):.3g}, ANTs' own by up "
            f"to {max(band[o] for o in ('post1', 'post2', 'post3')):.3g}."
        )
    elif same[i]:
        x["verdict"] = "bit-identical"
        x["reason"] = {
            "lx-cr": "With correctly rounded maths, ANTs computes exactly what larmorx computes.",
            "lx6-lx1": "larmorx's result does not depend on its thread count.",
            "fix6-fix1": "The same bytes on any number of threads.",
        }.get(i, "The same bytes.")
    elif i in ("lx-ants1", "lx-fmriprep") and same["lx-cr"]:
        x["verdict"] = "identical-cr"
        x["reason"] = (
            "The classified images are identical; a few posteriors differ in their last bits "
            "where glibc's expf and logf are not correctly rounded for a value the run meets: "
            "with CORE-MATH preloaded, the same ANTs binary gives larmorx's output bit for bit."
            + (
                " fMRIPrep's own output is ANTs 2.6.5's at 8 threads, which is its result at "
                "one thread."
                if i == "lx-fmriprep"
                else ""
            )
        )
        links.append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
    else:
        x["verdict"] = "open"
        x["reason"] = "Not accounted for by the other runs of this example."
    x["links"] = links


def _example(ctx: core.Context, reg: dict) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = entry(ctx)
    info = e.info()
    run = common.fmriprep_run(RUN)
    words = _words(run, NODE)
    args = _argv(words, str(run / "work"), "x")
    t1_path = args[args.index("--intensity-image") + 1]
    img = nib.load(t1_path)
    brain = common.load(str(run / MASK)) > 0
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("t1w", "input", "ds000005 sub-01 T1w, N4-corrected",
                     "Atropos' input in fMRIPrep's brain extraction (float32)",
                     {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    full: dict[str, dict] = {}
    for pid, label, role, text, sub in RUNS:
        arr = _fmriprep_outputs(run) if pid == "fmriprep" else _load_run(e.dir, label)
        full[pid] = arr
        ex.add(exm.Panel(pid, role, text, sub, arr))
    for t in THREADS:
        full.setdefault(f"ants-{t}", _load_run(e.dir, f"ants-{t}"))
    # metrics only (the page's size): the other clock-seeded runs and larmorx's with -r 1
    for label in ("ants-seed-2", "ants-seed-3", "larmorx-seed"):
        full[label] = _load_run(e.dir, label)
    diffs = _diffs()
    outputs = _outputs()
    common.compare_all(ex, diffs, outputs, full=full)
    same = common.identical(diffs)
    by = {x["id"]: x for x in diffs}
    for x in diffs:
        _verdict(x, same, by, reg)
    for i in ("lx-cr", "lx6-lx1", "fix6-fix1"):
        if not same[i]:
            raise AssertionError(f"{TOOL}: the chain assumes {i} is bit-identical")
    labels = {p.id: (p.label, p.sub, p.role) for p in ex.panels}

    def node(pid, label=None, sub=None):
        lab, s, role = labels[pid]
        return {"label": label or lab, "sub": sub if sub is not None else s, "role": role}

    def mx(i: str) -> float:
        return common.max_abs(by[i])

    chain = {
        "title": "What separates fMRIPrep's output from larmorx's",
        "steps": [
            {"nodes": [node("fmriprep")]},
            {
                "nodes": [
                    node("ants-8"),
                    node(
                        "ants-1",
                        sub="1, 2, 4 and 8 threads: the same bytes"
                        if all(same[f"ants{t}-ants1"] for t in THREADS[1:])
                        else None,
                    ),
                ]
            },
            {
                "nodes": [
                    node("ants-1-cr"),
                    node("larmorx", "larmorx (port), reproduce mode", "1 and 6 threads"),
                ]
            },
        ],
        "links": [
            {
                "cause": "fMRIPrep's run (its ANTs build, 8 threads)",
                "max_abs": mx("fmriprep-ants8"),
            },
            {"cause": "correctly rounded maths instead of glibc's", "max_abs": mx("lx-ants1")},
        ],
        "note": "Each box holds runs that give the same bytes. Atropos' result does not depend "
        "on ANTs' thread count, and fMRIPrep's own run is the repository's ANTs result; larmorx "
        "differs from it only where glibc's maths is not correctly rounded, in the last bits of "
        "a few posteriors. Its default mode then corrects ITK-50 (the metrics below), a change "
        "smaller than ANTs' own spread when it seeds from the clock.",
    }
    at, _k = common.largest_in_slice(ex, "ants-seed-1", "ants-1", "post2", "axial")
    default = {
        "output": "post2",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1w", "ants-1", "larmorx", "fmriprep"],
        "diffs": ["lx-ants1", "fix-lx", "seed1-ants1"],
        "compare": {"a": "ants-seed-1", "b": "ants-1"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        outputs,
        default=default,
        window_panel="larmorx",
        scale="log",
        caption="the crosshair where ANTs with the clock's seed differs most from its fixed-seed "
        "run in it; "
        "the differences on one symmetric log scale over six decades below the largest.",
    )
    vers = info.get("versions", {})
    m = re.search(r"Version:\s*(\S+)", vers.get("ants_in_fmriprep_image", ""))
    image_ants = f"ANTs {m.group(1)} (conda-forge)" if m else "ANTs (version not recorded)"
    for p in out["panels"]:
        if p["id"] == "image-1":
            p["label"] = f"{image_ants.split(' (')[0]} (fMRIPrep's image), 1 thread"
    fix = by["fix-lx"]["metrics"]
    band = _seed_band(by)
    n_vox = int(np.prod(img.shape))
    return {
        "id": "fmriprep-atropos",
        "title": "sMRIPrep's segmentation of a real T1w, by fMRIPrep, ANTs and larmorx",
        "summary": (
            "fMRIPrep 25.2.6's brain extraction on OpenNeuro ds000005 sub-01 segments the "
            "N4-corrected T1w into three classes with Atropos; its inputs and arguments given to "
            "the repository's ANTs 2.6.5 at 1 to 8 threads, with correctly rounded maths, with "
            "the clock's seed, to fMRIPrep's own ANTs at one thread, and to larmorx in both "
            "modes."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01's T1w "
                f"({common.DS000005['licence']}); shown inside fMRIPrep's brain mask, dilated by 2 "
                "voxels",
            },
            {
                "label": "Node",
                "text": "fMRIPrep 25.2.6, `anat_fit_wf/brain_extraction_wf/"
                "atropos_wf/01_atropos`, run with 8 threads",
            },
            {"label": "Command", "text": "`" + core.short_path(info["command"]) + "`"},
            {
                "label": "Programs",
                "text": f"{vers.get('ants', '?')}; {image_ants} in fMRIPrep's "
                "image; CORE-MATH preloaded where stated; larmorx at commit "
                f"`{info['larmorx_commit'][:12]}`",
            },
            {
                "label": "ITK-50",
                "text": f"larmorx's default mode changes "
                f"{fix['labels']['differ']:,} of {fix['labels']['voxels']:,} labels and the "
                f"posteriors by up to {max(fix[o]['max_abs'] for o in ('post1', 'post2', 'post3')):.3g}; "
                f"ANTs' own runs with the clock's seed differ in up to {int(band['labels']):,} "
                "labels",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": chain,
        "metrics_note": (
            f"Δ is A − B. Every number is computed in float64 over all {n_vox:,} voxels of the "
            f"{' × '.join(map(str, img.shape))} outputs, not only the brain shown; the range is "
            "B's maximum minus its minimum. The rows *the original against itself* are ANTs' "
            "own variability on this input: its thread counts, its builds and its runs with the "
            "clock's seed."
        ),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [common.DS000005]),
    }


#: the priors example shows the classified image and the grey matter's posterior (its other
#: posteriors are in the record: bit-identical with correctly rounded maths)
OUTPUTS_PRIORS = ("labels", "post2")
PRIOR_RUNS = [
    ("larmorx", "larmorx-1-reproduce", "larmorx", "larmorx (port), 1 thread", "reproduce mode"),
    ("larmorx-6", "larmorx-6-reproduce", "larmorx", "larmorx (port), 6 threads", "reproduce"),
    ("larmorx-fix", "larmorx-1", "larmorx", "larmorx (port), 1 thread", "default mode"),
    ("ants-1-cr", "ants-1-cr", "reference", "ANTs 2.6.5, 1 thread, correctly rounded maths",
     "the same binary, CORE-MATH preloaded"),
    ("ants-1", "ants-1", "original", "ANTs 2.6.5, 1 thread, as built",
     "the repository's oracle build (GCC 14.3), glibc's maths"),
]  # fmt: skip


def _priors_example(ctx: core.Context) -> dict:
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm

    e = priors_entry(ctx)
    info = e.info()
    run = common.fmriprep_run(RUN)
    args = _argv(_words(run, NODE), str(run / "work"), "x")
    img = nib.load(args[args.index("--intensity-image") + 1])
    brain = common.load(str(run / MASK)) > 0
    show = ndimage.binary_dilation(brain, iterations=2)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    ex.add(exm.Panel("t1w", "input", "ds000005 sub-01 T1w, N4-corrected", "Atropos' input",
                     {"all": np.asanyarray(img.dataobj)}))  # fmt: skip
    names = {1: "CSF", 2: "cortical grey matter", 3: "white matter"}
    for c in (2,):
        ex.add(exm.Panel(f"prior{c}", "input", f"OASIS30ANTs prior: {names[c]}",
                         "in the T1w's space (fMRIPrep's transform, Gaussian interpolation)",
                         {"all": common.load(str(e.dir / f"prior{c:02d}.nii.gz"))}))  # fmt: skip
    outputs = _outputs(OUTPUTS_PRIORS)
    shown = {o["id"] for o in outputs}
    for pid, label, role, text, sub in PRIOR_RUNS:
        arr = {k: v for k, v in _load_run(e.dir, label).items() if k in shown}
        ex.add(exm.Panel(pid, role, text, sub, arr))
    d = common.d
    diffs = [
        d("lx-ants1", "larmorx", "ants-1", "main", "larmorx − ANTs 2.6.5, 1 thread, as built",
          common.ROLE_LX),
        d("lx-cr", "larmorx", "ants-1-cr", "main",
          "larmorx − ANTs 2.6.5, 1 thread, correctly rounded maths", common.ROLE_LX),
        d("fix-lx", "larmorx-fix", "larmorx", "main", "larmorx default mode − reproduce mode",
          "larmorx's correction"),
        d("lx6-lx1", "larmorx-6", "larmorx", "check", "larmorx, 6 threads − 1 thread",
          common.ROLE_SELF),
    ]  # fmt: skip
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = {
                "lx-cr": "With correctly rounded maths, ANTs computes exactly what larmorx "
                "computes.",
                "fix-lx": "No defect larmorx corrects is met: the default mode gives ANTs' result.",
                "lx6-lx1": "larmorx's result does not depend on its thread count.",
            }.get(i, "The same bytes.")
        elif i == "lx-ants1" and same["lx-cr"]:
            x["verdict"] = "identical-cr"
            x["reason"] = (
                "The classified images are identical; some posteriors differ in their last bits "
                "where glibc's expf and logf are not correctly rounded for a value the run "
                "meets; with CORE-MATH preloaded the outputs are bit-identical."
            )
            x["links"].append({"label": "platform maths", "href": "docs/findings/platform-math.md"})
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example."
    for i in ("lx-cr", "lx6-lx1"):
        if not same[i]:
            raise AssertionError(f"{TOOL}: the priors example assumes {i} is bit-identical")
    at, _k = common.largest_in_slice(ex, "larmorx", "ants-1", "post2", "axial")
    default = {
        "output": "post2",
        "plane": "axial",
        "voxel": at,
        "panels": ["t1w", "prior2", "ants-1", "larmorx"],
        "diffs": ["lx-ants1"],
        "compare": {"a": "ants-1", "b": "larmorx"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        outputs,
        default=default,
        window_panel="larmorx",
        input_windows={"prior2": "zero"},
        scale="log",
        caption="the crosshair where larmorx and ANTs as built differ most in it.",
    )
    n_vox = int(np.prod(img.shape))
    return {
        "id": "oasis-priors",
        "title": "A segmentation with tissue priors, by ANTs and larmorx",
        "summary": (
            "The same T1w segmented as antsCorticalThickness segments one: OASIS30ANTs' CSF, "
            "cortical grey matter and white matter priors, brought into the T1w's space with "
            "the transform fMRIPrep's brain extraction computed, weigh 0.25 against the "
            "likelihoods; MRF 0.1, five iterations. Shown: the classified image and the grey "
            "matter's posterior, beside its prior."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01's T1w "
                f"({common.DS000005['licence']}); {OASIS['dataset']} ({OASIS['licence']})",
            },
            {"label": "Command", "text": "`" + core.short_path(info["command"]) + "`"},
            {
                "label": "Programs",
                "text": f"{info.get('versions', {}).get('ants', '?')}; "
                f"CORE-MATH preloaded where stated; larmorx at commit `{info['larmorx_commit'][:12]}`",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "metrics_note": (
            f"Δ is A − B. Every number is computed in float64 over all {n_vox:,} voxels of the "
            "outputs, not only the brain shown."
        ),
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [common.DS000005, OASIS]),
    }


# ------------------------------------------------------------------------------------ cases


def _cases(rec) -> list[dict]:
    from larmorx_validation.parity.ants_atropos import all_cases

    defs = {c.id: c for c in all_cases()}
    cols = ("Case", "Output", "Difference from ANTs as built")
    t = rec.table(*cols) if rec.has_table(*cols) else None
    rows: dict[str, list[dict]] = {}
    for r in t.rows if t is not None else []:
        rows.setdefault(records.strip_code(r["Case"]), []).append(r)
    examples = {
        "real/ds000005-sub-01/smriprep": "fmriprep-atropos",
        "real/ds000005-sub-01/priors-3": "oasis-priors",
    }
    out = []
    for r in rec.results:
        c = defs.get(r["case"])
        v = common.outcome_verdict(r)
        undefined = [k for k, val in r.get("reported", {}).items() if "(not required)" in k and val]
        if v is None and undefined:
            v = (
                "divergence",
                "ANTs weighs part of its samples with memory it never wrote (ANTS-107): its "
                "values depend on the heap and change with the output paths' length; larmorx "
                "weighs them 0. Everything else (exit status, headers, types, shapes) is compared.",
            )
        elif v is None and r["case"] in rows:
            v = (
                "identical-cr",
                "; ".join(
                    f"{x['Output']}: as built, {x['Difference from ANTs as built']}"
                    for x in rows[r["case"]]
                )
                + "; bit-identical with correctly rounded maths",
            )
        elif v is None:
            v = ("bit-identical", "")
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": c.description if c else "",
                "command": _command(c) if c else "",
                "results": {
                    "port": {
                        "outcome": r["status"],
                        "verdict": v[0],
                        "checks": f"{r['checks_passed']}/{r['checks_total']}",
                        "reason": common.clip(v[1], 400),
                    }
                },
                **({"example": examples[r["case"]]} if r["case"] in examples else {}),
            }
        )
    return out


def _command(c) -> str:
    if len(c.args) == 1 and c.args[0].startswith("{fmriprep:"):
        return "Atropos <fMRIPrep's command line: " + c.args[0][10:-1] + "/command.txt>"

    def sub(a: str) -> str:
        a = re.sub(r"\{gen:([^}]+)\}", lambda m: f"<generated: {m.group(1)}>", a)
        a = re.sub(r"\{series:([^}]+)\}", lambda m: f"<generated: {m.group(1)}%02d>", a)
        a = re.sub(r"\{missing:([^}]+)\}", lambda m: m.group(1), a)
        a = re.sub(r"\{node-image:([^}]+)\}", lambda m: f"<{m.group(1)}: N4-corrected T1w>", a)
        a = re.sub(r"\{node-mask:([^}]+)\}", lambda m: f"<{m.group(1)}: brain mask>", a)
        a = re.sub(r"\{priors:([^:}]+):(\d)\}", lambda m: "<OASIS30ANTs priors%02d>", a)
        return a.replace("{out}/", "")

    return common.command_text("Atropos", c.args, sub)


def export(ctx: core.Context) -> dict:
    reg = core.register(ctx.repo)
    rec = records.load(ctx.repo, TOOL)
    cases = _cases(rec)
    from larmorx_validation.evidence import ants_panel

    prec = ants_panel.panel_record(ctx.repo, TOOL)
    if prec is not None:
        cases += ants_panel.panel_cases_tidy(_cases(prec))
    inventory = ants_panel.inventory_table(prec, "panel-nodes", "Atropos")
    e, p = entry(ctx), priors_entry(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "Atropos",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source() + (prec.source() if prec is not None else []),
            "benchmarks": [],
            "cache": [
                fcache.provenance(e),
                fcache.provenance(p),
                ants_atropos_pv.provenance(ctx),
                *(fcache.provenance(x) for x in ants_panel.entries(ctx, TOOL)),
            ],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cases, "port", "larmorx (port)", "port", f"docs/validation/{TOOL}.md", "validated"
            )
        ],
        "cases": cases,
        "cases_total": len(cases),
        "examples": [
            _example(ctx, reg),
            _priors_example(ctx),
            ants_atropos_pv.example(ctx),
            *ants_panel.examples(ctx, TOOL),
        ],
        "tables": [ants_atropos_pv.table(ctx), *([inventory] if inventory else [])],
    }


if __name__ == "__main__":  # compute the cache entries only
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    print("done", entry(c).dir, priors_entry(c).dir)
