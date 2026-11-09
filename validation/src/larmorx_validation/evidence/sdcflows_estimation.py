# SPDX-License-Identifier: Apache-2.0
"""The evidence of the field maps' part B: the port of sdcflows 2.17.1's field-map estimation
(PEPOLAR, direct and fieldmap-less field maps, the masks, the derivatives, the BIDS app), of
niworkflows' and nipype's pieces under it, of fMRIPrep's own field-map steps and of the SciPy,
scikit-image and numpy routines they call, against sdcflows itself and fMRIPrep 25.2.6's own
files.

- **sdcflows' BIDS app** on two of sdcflows' own oracle runs (a PEPOLAR pair, ds006736; a
  direct field map, ds005752): sdcflows 2.17.1 at one thread (its programs through the oracle
  cache: FSL 6.0.7.23's topup, ANTs 2.6.5's N4 at one thread with correctly rounded maths,
  AFNI 26.3.05's 3dvolreg) against LARMORX's port of the app at one thread (reproduce) and 14
  (default), as the field maps' ``check_app.py --sections run`` runs them; sdcflows' own run at
  four threads (the original's own spread); fMRIPrep 25.2.6's own derivatives of the same data
  beside LARMORX's field-map stage run as fMRIPrep ran it (sdcflows 2.15's wiring, its image's
  numpy and SciPy, the replicas: ``check_fmap_stage.py --sections fmriprep``, every difference
  explained there).
- **The brain mask of a magnitude** (``brain_masker``, sdcflows' ``BrainExtraction``) on
  fMRIPrep's ds002603 call, its probabilities and mask, fMRIPrep's own files beside.
- **SciPy's watershed corrected by default** (SCI-13, SCI-14) in sdcflows' ``epi_mask`` on four
  BOLD references, against fMRIPrep's own brain masks.
- The records' cases (three suites), the originals' own step, the defects of the part, random
  testing's trials, the memory peaks (the memory rule's calls) and the benchmark.

Every real image is shown inside the brain mask of the step that made it (or of fMRIPrep's run),
dilated by two voxels: no face can show.

    python -m larmorx_validation.evidence.sdcflows_estimation   (computes the cache entries)
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

TOOL = "sdcflows-estimation"
#: The records of the part: name, the suite, its tier and environment.
RECORDS = [
    ("sdcflows-estimation", "sdcflows-estimation", "standard", "newest"),
    ("sdcflows-estimation-full", "sdcflows-estimation", "full", "newest"),
    ("sdcflows-estimation-app-shared", "sdcflows-estimation", "full", "shared"),
    ("ndimage-skimage", "ndimage-skimage", "standard", "newest"),
    ("ndimage-skimage-full", "ndimage-skimage", "full", "newest"),
    ("niworkflows-helpers", "niworkflows-helpers", "standard", "newest"),
]
FINDINGS = "docs/findings/sdcflows-estimation.md"
FINDINGS_SCIPY = "docs/findings/scipy-ndimage-filters.md"
BENCH = "docs/benchmarks/sdcflows-estimation.json"
#: Random testing's trials of the part (``<workspace>/oracles/difftest/<trial>/summary.json``):
#: the trial with numpy's default (AVX-512) kernels, and one with them off.
TRIALS = ("2026-11-09-fieldmaps-b", "2026-11-09-fieldmaps-b-noavx512")
TRIAL_TOOLS = ("sdcflows-estimation", "ndimage-skimage", "niworkflows-helpers")
RUN_MODULE = "larmorx_validation.evidence.sdcflows_estimation_run"
#: The app examples: key, sdcflows' own run, fMRIPrep's run of the same data, kind, what it is.
APP = [
    ("ds006736", "ds006736-sub-004", "ds006736-sub-004-noreconall", "pepolar",
     "a spin-echo PEPOLAR pair (one AP and one PA volume) at 3 T"),
    ("ds005752", "ds005752-sub-ON01016-direct", "ds005752-sub-ON01016-noreconall-direct",
     "direct", "a direct field map in Hz with its magnitude (Siemens, 3 T)"),
]  # fmt: skip
#: fMRIPrep's runs whose field-map stage LARMORX runs as fMRIPrep ran it (relay 14's five).
STAGE_RUNS = (
    "ds005752-sub-ON01016-noreconall-direct",
    "ds001600-sub-1-noreconall",
    "ds005040-sub-002-noreconall",
    "ds002422-sub-02-noreconall",
    "ds006736-sub-004-noreconall",
)
#: The brain-mask example: fMRIPrep's run, its magnitude's brain-extraction workflow.
MASKER = (
    "ds002603-sub-01-noreconall",
    "work/fmriprep_25_2_wf/sub_01_wf/fmap_preproc_wf/wf_auto_00000/magnitude_wf/brainextraction_wf",
)
#: ``epi_mask``'s BOLD references (``measure_watershed_masks.py``'s four), the first shown.
EPI_RUNS = (
    "ds003592-sub-280-noreconall",
    "ds000228-sub-pixar008-noreconall",
    "ds005040-sub-002-noreconall",
    "ds004958-sub-fsm64ps-noreconall",
)
#: The memory rule's calls (``tests/parity/test_memory_peak.py``, ``FIELDMAP_B_CASES``).
MEMORY_CALLS = ("masker", "clip", "ravg", "recon")
ENV = {"OPENBLAS_CORETYPE": "Haswell", "OPENBLAS_NUM_THREADS": "1", "OMP_NUM_THREADS": "1"}
UNSET = ("LARMORX_UPSTREAM_DEFECTS", "PYTHONPATH")


# --------------------------------------------------------------------------- the runs


def _python(name: str = "newest") -> str:
    from larmorx_validation.parity import python_versions as pv

    p = pv.prefix(name)
    if p is None:
        raise FileNotFoundError(f"the {name} Python environment ({pv.ENVS[name]})")
    return str(p / "bin" / "python")


def _scripts() -> Path:
    from larmorx_validation.parity.sdcflows_scripts import SCRIPTS

    return SCRIPTS


def _workspace() -> Path:
    from larmorx_validation.parity import python_versions as pv

    ws = pv.workspace()
    if ws is None:
        raise FileNotFoundError("the workspace (a folder holding oracles/)")
    return ws


def _env(e: fcache.Entry, nc: bool = False) -> dict[str, str]:
    """The check scripts' environment, as the parity suites give it (``sdcflows_scripts``)."""
    import tempfile

    repo = e.ctx.repo
    # a short folder: sdcflows' multiprocessing manager's socket lives there (AF_UNIX paths
    # are at most 107 bytes)
    tmp = Path(tempfile.mkdtemp(prefix="lxse-"))
    env = {
        **ENV,
        "PYTHONPATH": f"{repo / 'python'}:{repo / 'validation' / 'src'}",
        "LARMORX_WORKSPACE": str(_workspace()),
        "TMPDIR": str(tmp),
        "PYTHONUNBUFFERED": "1",
    }
    if nc:
        from larmorx_validation.parity.sdcflows_scripts import _nc_binary

        b = _nc_binary()
        if b is None:
            raise FileNotFoundError("the FSL replicas' program (larmorx-nc) is not built")
        env["LARMORX_NC_BIN"] = str(b)
    return env


def _versions() -> dict[str, str]:
    from larmorx_validation.parity import python_versions as pv

    return {
        "newest": "sdcflows 2.17.1, niworkflows 1.15.3, nipype 1.12.0, scikit-image 0.26.0, "
        f"nibabel 5.4.2, {pv.RELEASES[pv.NEWEST]} (oracles/python-newest: larmorx's reference)",
        "programs": "FSL 6.0.7.23's topup, ANTs 2.6.5's N4BiasFieldCorrection (the GCC 14.3 "
        "build; one thread, correctly rounded maths), AFNI 26.3.05's 3dvolreg, through the "
        "oracle cache; LARMORX's replicas and ports",
    }


def _app(key: str) -> tuple[str, str, str, str, str]:
    return next(x for x in APP if x[0] == key)


def _compute_app(key: str):
    def compute(e: fcache.Entry) -> None:
        _k, run, frun, _kind, _what = _app(key)
        ws = _workspace()
        sdc = ws / "oracles" / "sdcflows-2.17.1" / run
        e.input("sdcflows' run (RUN_INFO.txt)", sdc / "RUN_INFO.txt")
        e.input("fMRIPrep's run (RUN_INFO.txt)", common.fmriprep_run(frun) / "RUN_INFO.txt")
        env = _env(e, nc=True)
        e.run("check-app", [_python(), "-P", str(_scripts() / "check_app.py"), "--sections",
                            "run", "--datasets", run, "--scratch", str(e.dir / "app")],
              env=env, unset=UNSET, cwd=e.dir, timeout=7200)  # fmt: skip
        log = (e.dir / "check-app.log").read_text(encoding="utf-8")
        if "ALL OK" not in log:
            raise RuntimeError(f"check_app.py on {run} did not end with ALL OK")
        e.run("larmorx-14", [_python(), "-P", "-m", RUN_MODULE, "app", run,
                             str(e.dir / "larmorx-14"), "--threads", "14", "--defects", "fix"],
              env=env, unset=UNSET, cwd=e.dir, timeout=7200)  # fmt: skip
        e.finish(
            versions=_versions(),
            settings="sdcflows 2.17.1's BIDS app (`sdcflows <bids> <out> participant "
            "--participant-label <sub> --nprocs 1 --omp-nthreads 1 --no-fmapless --notrack "
            "--pdb`) and LARMORX's port of it (`lx.sdcflows.run_app`, reproduce) as "
            "check_app.py --sections run runs them (every derivative compared there byte for "
            "byte: ALL OK); LARMORX's again at 14 threads in its default mode",
        )

    return compute


def app_entry(ctx: core.Context, key: str) -> fcache.Entry:
    _k, run, _f, kind, _w = _app(key)
    return fcache.entry(
        ctx.figures_context(), TOOL, f"app-{key}",
        f"sdcflows' BIDS app on {run} ({kind}): sdcflows 2.17.1 and LARMORX at one thread, "
        "LARMORX at 14 threads (default)",
        _compute_app(key),
    )  # fmt: skip


def _compute_stage(e: fcache.Entry) -> None:
    for run in STAGE_RUNS:
        e.input(f"fMRIPrep's run {run} (RUN_INFO.txt)", common.fmriprep_run(run) / "RUN_INFO.txt")
    e.run("stage", [_python(), "-P", str(_scripts() / "check_fmap_stage.py"), "--sections",
                    "fmriprep", "--datasets", *STAGE_RUNS, "--scratch", str(e.dir / "stage")],
          env=_env(e, nc=True), unset=UNSET, cwd=e.dir, timeout=7200)  # fmt: skip
    log = (e.dir / "stage.log").read_text(encoding="utf-8")
    if "ALL OK" not in log:
        raise RuntimeError("check_fmap_stage.py --sections fmriprep did not end with ALL OK")
    e.finish(
        versions=_versions(),
        settings="LARMORX's field-map estimation stage (larmorprepx's estimate_fieldmaps) as "
        "fMRIPrep 25.2.6 ran it: sdcflows 2.15's PEPOLAR wiring, its image's numpy 2.2.6 and "
        "SciPy 1.15.2, LAPACK 3.12.0 in the data sinks, the replicas, reproduce; against "
        "fMRIPrep's own derivatives, every difference explained (check_fmap_stage.py "
        "--sections fmriprep: FSL's build string in descrip, or an N4 node that ANTs reproduces "
        "at the node's thread count with glibc's maths)",
    )


def stage_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "stage-fmriprep",
        "LARMORX's field-map stage as fMRIPrep 25.2.6 ran it on five oracle runs, against "
        "fMRIPrep's own derivatives",
        _compute_stage,
    )  # fmt: skip


#: The brain-mask example's runs: label, implementation, threads, D13 mode, libraries.
MASKER_RUNS = [
    ("sdcflows", "sdcflows", 1, "reproduce", "environment"),
    ("larmorx", "larmorx", 1, "reproduce", "environment"),
    ("larmorx-14", "larmorx", 14, "fix", "environment"),
    ("larmorx-fmriprep", "larmorx", 1, "reproduce", "fmriprep"),
]


def _masker_input() -> Path:
    run, wf = MASKER
    return common.fmriprep_run(run) / wf / "clipper_post" / "clipped.nii.gz"


def _compute_masker(e: fcache.Entry) -> None:
    src = e.input("the clipped, N4-corrected magnitude (masker's in_file)", _masker_input())
    env = _env(e)
    for label, impl, threads, mode, libs in MASKER_RUNS:
        e.run(label, [_python(), "-P", "-m", RUN_MODULE, "masker", str(src), str(e.dir / label),
                      "--implementation", impl, "--threads", str(threads), "--defects", mode,
                      "--libraries", libs],
              env=env, unset=UNSET, cwd=e.dir)  # fmt: skip
    e.finish(
        versions=_versions(),
        node=f"{MASKER[1].removeprefix('work/')}/masker",
        settings="sdcflows 2.17.1's brain_masker (BrainExtraction's) on fMRIPrep's own input "
        "of its masker node; LARMORX's lx.sdcflows.brain_masker at 1 thread (reproduce), 14 "
        "(default), and at 1 with fMRIPrep's image's numpy 2.2.6 and SciPy 1.15.2 (reproduce); "
        "the newest environment, OPENBLAS_CORETYPE=Haswell, OPENBLAS_NUM_THREADS=1",
    )


def masker_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "masker-ds002603",
        "brain_masker on fMRIPrep's ds002603 magnitude: sdcflows 2.17.1 and LARMORX (1 and 14 "
        "threads, both modes, fMRIPrep's libraries)",
        _compute_masker,
    )  # fmt: skip


def _epi_ref(run: str) -> tuple[Path, Path]:
    out = common.fmriprep_run(run) / "out"
    ref = sorted(out.rglob("*_desc-hmc_boldref.nii.gz"))[0]
    return ref, Path(str(ref).replace("_desc-hmc_boldref", "_desc-brain_mask"))


#: ``epi_mask``'s runs: label, implementation, D13 mode.
EPI_MASK_RUNS = [("sdcflows", "sdcflows", "reproduce"), ("larmorx", "larmorx", "reproduce"),
                 ("larmorx-fix", "larmorx", "fix")]  # fmt: skip


def _compute_epi(e: fcache.Entry) -> None:
    env = _env(e)
    for run in EPI_RUNS:
        ref, brain = _epi_ref(run)
        e.input(f"{run}: the BOLD reference (desc-hmc_boldref)", ref)
        e.input(f"{run}: fMRIPrep's brain mask (desc-brain_mask)", brain)
        ds = run.split("-")[0]
        for label, impl, mode in EPI_MASK_RUNS:
            e.run(f"{ds}-{label}", [_python(), "-P", "-m", RUN_MODULE, "epi-mask", str(ref),
                                    str(e.dir / ds / label), "--implementation", impl,
                                    "--defects", mode],
                  env=env, unset=UNSET, cwd=e.dir)  # fmt: skip
    e.finish(
        versions=_versions(),
        settings="sdcflows 2.17.1's epi_mask and LARMORX's lx.sdcflows.epi_mask in both modes "
        "on fMRIPrep 25.2.6's BOLD references (desc-hmc_boldref) of four oracle runs; the newest "
        "environment",
    )


def epi_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "epi-mask",
        "epi_mask on four BOLD references: sdcflows 2.17.1 and LARMORX in both modes",
        _compute_epi,
    )  # fmt: skip


def _memory_inputs(call: str) -> list[str]:
    """The memory rule's inputs (``test_memory_peak._fieldmap_b_inputs``)."""
    runs = common.oracles() / "fmriprep-25.2.6"
    mag = runs / MASKER[0] / MASKER[1]
    data = Path.home() / ".cache" / "larmorx-testdata" / "data" / "openneuro"
    if call == "masker":
        return [str(mag / "clipper_post" / "clipped.nii.gz")]
    if call == "clip":
        merged = mag.parent / "magmrg" / "sub-01_acq-Mm2_magnitude1_merged.nii.gz"
        return [str(merged), "35.0", "99.98"]
    if call == "ravg":
        return [
            str(data / "ds004958" / "sub-fsm64ps" / "fmap" / "sub-fsm64ps_acq-se_dir-AP_epi.nii.gz")
        ]
    r54 = runs / "ds005454-sub-16-noreconall"
    fmap, func = r54 / "out" / "sub-16" / "fmap", r54 / "out" / "sub-16" / "func"
    return [
        str(fmap / "sub-16_fmapid-gre16_desc-coeff_fieldmap.nii.gz"),
        str(fmap / "sub-16_fmapid-gre16_desc-magnitude_fieldmap.nii.gz"),
        str(r54 / "work" / "fmriprep_25_2_wf" / "sub_16_wf" / "bold_task_rest_wf" / "bold_anat_wf"
            / "gen_ref" / "sub-16_desc-preproc_T1w_reference.nii.gz"),
        str(func / "sub-16_task-rest_from-boldref_to-gre16_mode-image_desc-fmap_xfm.txt"),
        str(func / "sub-16_task-rest_from-boldref_to-T1w_mode-image_desc-coreg_xfm.txt"),
        "--inverse", "true,false",
    ]  # fmt: skip


def _compute_memory(e: fcache.Entry) -> None:
    script = _scripts() / "memory_call.py"
    repo = e.ctx.repo
    peaks: dict[str, dict] = {}
    for call in MEMORY_CALLS:
        inputs = _memory_inputs(call)
        for p in inputs:
            if Path(p).is_file():
                e.input(f"{call}: {Path(p).name}", Path(p))
        peaks[call] = {}
        for impl, mode in (("original", None), ("larmorx", "reproduce"), ("larmorx", "fix")):
            label = f"{call}-{impl}" + (f"-{mode}" if mode else "")
            cwd = e.dir / label
            cwd.mkdir(exist_ok=True)
            argv = ["/usr/bin/time", "-v", _python(), "-P", str(script), impl, call, *inputs]
            env = dict(ENV)
            if impl == "larmorx":
                argv += ["--defects", mode]
                env["PYTHONPATH"] = str(repo / "python")
            e.run(label, argv, env=env, unset=UNSET, cwd=cwd)
            log = (e.dir / f"{label}.log").read_text(encoding="utf-8")
            kb = int(re.search(r"Maximum resident set size \(kbytes\): (\d+)", log).group(1))  # type: ignore[union-attr]
            peaks[call][impl + (f"-{mode}" if mode else "")] = kb / 1024.0
            for f in cwd.iterdir():  # the outputs are not kept (the memory test compares them)
                if f.is_file():
                    f.unlink()
    e.finish(
        versions=_versions(),
        peaks_mib=peaks,
        settings="The memory rule's calls of part B (tests/parity/test_memory_peak.py, "
        "FIELDMAP_B_CASES): validation/investigations/sdcflows-fieldmap-b/memory_call.py, one "
        "call per process, its peak resident memory from /usr/bin/time -v; the originals and "
        "LARMORX (one thread, both modes) in the newest environment",
    )


def memory_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "memory",
        "The memory rule's four calls of part B: the originals' and LARMORX's peak resident "
        "memory",
        _compute_memory,
    )  # fmt: skip


def entries(ctx: core.Context) -> list[fcache.Entry]:
    return [
        *(app_entry(ctx, k) for k, *_ in APP),
        stage_entry(ctx),
        masker_entry(ctx),
        epi_entry(ctx),
        memory_entry(ctx),
    ]




# ------------------------------------------------------------------------- shared pieces


def _stored(path: Path) -> np.ndarray:
    """A file's values as stored; a 4-D image of one volume (sdcflows' field maps) as 3-D."""
    import nibabel as nib

    a = np.asanyarray(nib.load(str(path)).dataobj)
    return a[..., 0] if a.ndim == 4 and a.shape[3] == 1 else a


def _outputs_json(d: Path) -> dict:
    return json.loads((d / "outputs.json").read_text(encoding="utf-8"))


def _files(d: Path) -> dict[str, Path]:
    return {k: d / v for k, v in _outputs_json(d)["files"].items()}


def _data(ds: str, subject: str) -> dict:
    """An example's credit facts from larmorx-testdata's catalog (``credits.attach`` checks
    the licence again)."""
    from larmorx_validation.evidence import credits

    c = credits.credit(f"openneuro-{ds}")
    return {"dataset": f"OpenNeuro {ds}", "subject": subject, "licence": c["licence"]}


def _show(mask: np.ndarray, iterations: int = 2) -> np.ndarray:
    from scipy import ndimage

    return ndimage.binary_dilation(mask > 0, iterations=iterations)


def _findings_links(repo: Path) -> dict[str, str]:
    """``SDC-xx``, ``PIPE-xx``, ``SCI-xx``, ``SKI-xx`` -> the at-a-glance table of the findings
    (its section), or the section that names the entry first."""
    out: dict[str, str] = {}
    head = "Part B's register entries at a glance (relay 29)"
    anchor = f"{FINDINGS}#{core.github_anchor(head)}"
    for line in (repo / FINDINGS).read_text(encoding="utf-8").splitlines():
        m = re.match(r"\| ((?:SDC|PIPE|SCI|SKI)-\d+) \|", line)
        if m:
            out[m.group(1)] = anchor
    return out


def _link(links: dict[str, str], ids) -> list[dict]:
    return [{"label": i, "href": links[i]} for i in ids if i in links]


def _differs(x: dict, outputs: list[dict]) -> str:
    """What differs in a comparison, output by output (values that differ, the largest)."""
    labels = {o["id"]: o["label"] for o in outputs}
    parts = []
    for oid, m in x["metrics"].items():
        if m["identical"]:
            continue
        parts.append(
            f"{labels.get(oid, oid)}: {m['differ']:,} of {m['voxels']:,} values differ, at most "
            f"{m['max_abs']:.3g} apart"
        )
    return "; ".join(parts)


def _peak_view(ex, panel: str, output: str, inner: bool = True) -> list[int]:
    """The default view: the axial slice with the most of the shown region, the crosshair
    where ``panel``'s ``output`` is largest in magnitude inside it (its edge eroded)."""
    from scipy import ndimage

    v = np.abs(np.asarray(ex.values(ex.key(panel, output)), dtype=np.float64))
    k = int(np.argmax(ex.m.sum(axis=(0, 1))))
    region = ndimage.binary_erosion(ex.m, iterations=2) if inner else ex.m
    region = region if region[..., k].any() else ex.m
    sl = np.where(region[..., k], v[..., k], -1.0)
    i, j = np.unravel_index(int(np.argmax(sl)), sl.shape)
    return [int(i), int(j), k]


def _diff_view(ex, a: str, b: str, output: str) -> list[int] | None:
    """The voxel where ``a`` and ``b`` differ most in ``output`` (shown region), if they do."""
    d = np.abs(
        np.asarray(ex.values(ex.key(a, output)), dtype=np.float64)
        - np.asarray(ex.values(ex.key(b, output)), dtype=np.float64)
    )
    d = np.where(ex.m, d, 0.0)
    if not d.any():
        return None
    return [int(v) for v in np.unravel_index(int(np.argmax(d)), d.shape)]


def _check_default(ex: dict) -> None:
    """The viewer draws the default's panels and comparisons: every panel must exist and every
    comparison must have images (``display``: both sides panels), or the viewer cannot draw."""
    panels = {p["id"] for p in ex["panels"]}
    drawn = {x["id"] for x in ex["diffs"] if x.get("display")}
    d = ex["default"]
    bad = [p for p in d["panels"] if p not in panels] + [x for x in d["diffs"] if x not in drawn]
    bad += [v for v in (d["compare"]["a"], d["compare"]["b"]) if v not in panels]
    if bad:
        raise ValueError(f"{ex['id']}: the default view names what the viewer cannot draw: {bad}")


def _verdicts(diffs: list[dict], outputs: list[dict], same: dict, named: list[str],
              links: dict, reasons: dict[str, str],
              explained: dict[str, tuple[str, str]] | None = None) -> None:  # fmt: skip
    """Each comparison's verdict from its metrics: bit-identical (``reasons``), a correction the
    default names (d13), a difference ``explained`` names with its verdict and reason, the
    original's own rows (no verdict: their reason), else open."""
    for x in diffs:
        i = x["id"]
        x["links"] = []
        if x["kind"] == "variability":
            x["verdict"] = None
            x["reason"] = reasons[i] if same[i] else reasons[i + ":differ"] + " " + _differs(
                x, outputs
            ) + "."
            continue
        if same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = reasons[i]
        elif i.startswith("lx14") and named:
            x["verdict"] = "d13"
            x["reason"] = f"The default mode corrects {', '.join(named)} and names it: " + _differs(
                x, outputs
            ) + "."
            x["links"] += _link(links, named)
        elif explained and i in explained:
            x["verdict"], why = explained[i]
            x["reason"] = why + " " + _differs(x, outputs) + "."
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example: " + _differs(
                x, outputs
            ) + "."


# ------------------------------------------------------------------- the brain mask (masker)

MASKER_OUTPUTS = [
    {"id": "probseg", "label": "Brain probability (random walker)", "unit": ""},
    {"id": "mask", "label": "Brain mask", "unit": ""},
    {"id": "brain", "label": "Masked magnitude", "unit": ""},
]
#: The masker example's panels of runs: id, role, label, sub.
MASKER_PANELS = [
    ("sdcflows", "original", "sdcflows 2.17.1",
     "numpy 2.5.4, SciPy 1.18.1, scikit-image 0.26.0: LARMORX's reference"),
    ("larmorx", "larmorx", "LARMORX (port), 1 thread", "reproduce mode"),
    ("larmorx-14", "larmorx", "LARMORX (port), 14 threads", "default mode (D13 corrections)"),
    ("fmriprep", "fmriprep", "fMRIPrep 25.2.6's own files",
     "sdcflows 2.15.1 in its image: numpy 2.2.6, SciPy 1.15.2"),
    ("larmorx-fmriprep", "larmorx", "LARMORX (port), fMRIPrep's libraries",
     "numpy 2.2.6's sums, SciPy 1.15.2's order; reproduce mode"),
]  # fmt: skip


def _masker_diffs() -> list[dict]:
    d = common.d
    return [
        d("lx-sdc", "larmorx", "sdcflows", "main", "LARMORX − sdcflows 2.17.1", common.ROLE_LX),
        d("lxf-fmriprep", "larmorx-fmriprep", "fmriprep", "fmriprep",
          "LARMORX with fMRIPrep's libraries − fMRIPrep 25.2.6's own files", common.ROLE_FMRIPREP),
        d("lx14-lx1", "larmorx-14", "larmorx", "check",
          "LARMORX: 14 threads, default − 1 thread, reproduce", common.ROLE_SELF),
        d("fmriprep-sdc", "fmriprep", "sdcflows", "variability",
          "fMRIPrep's own files − sdcflows 2.17.1 with the newest releases", common.ROLE_ORIG),
    ]  # fmt: skip


def _example_masker(ctx: core.Context, links: dict) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.evidence.fsl_bet import window_of

    e = masker_entry(ctx)
    info = e.info()
    run, wf = MASKER
    node = common.fmriprep_run(run) / wf / "masker"
    src = _masker_input()
    img = nib.load(str(src))
    own = {o["id"]: _stored(node / f"clipped_{'brainmasked' if o['id'] == 'brain' else o['id']}"
                                    ".nii.gz") for o in MASKER_OUTPUTS}  # fmt: skip
    show = _show(own["mask"], 2)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    raw, rs, ri = common.stored(src)
    ex.add(exm.Panel("clipped", "input", "ds002603: magnitude, clipped and N4-corrected",
                     "the masker node's input (niworkflows' IntensityClip, ANTs' N4, "
                     "IntensityClip again)", {"all": raw}, scale=rs, inter=ri))  # fmt: skip
    over = {"mask": {"on": "clipped", "window": window_of(ex, "clipped")}}
    arrays = {"fmriprep": own}
    for pid, *_ in MASKER_PANELS:
        if pid != "fmriprep":
            f = _files(e.dir / pid)
            arrays[pid] = {o["id"]: _stored(f[o["id"]]) for o in MASKER_OUTPUTS}
    for pid, role, label, sub in MASKER_PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    diffs = _masker_diffs()
    outputs = copy.deepcopy(MASKER_OUTPUTS)
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    named = _outputs_json(e.dir / "larmorx-14").get("named", [])
    reasons = {
        "lx-sdc": "Every value of the three outputs is sdcflows': the grey closing and median, "
        "Otsu's threshold, the binary opening and the largest component (SciPy's ndimage, "
        "LARMORX's port of it), scikit-image's random walker (its Laplacian in SciPy 1.18's "
        "order, its conjugate gradients with OpenBLAS's dot products), the probabilities cropped "
        "back, the mask at 0.5.",
        "lxf-fmriprep": "With numpy 2.2.6's sums and SciPy 1.15.2's Laplacian order (fMRIPrep's "
        "image), LARMORX writes fMRIPrep's own masker files, every value.",
        "lx14-lx1": "The same bytes at 1 and 14 threads and in both modes (no correction of the "
        "register applies to this call).",
        "fmriprep-sdc": "The libraries' step, a reference row (the original against itself): "
        "fMRIPrep's image (numpy 2.2.6, SciPy 1.15.2) gives the same values as the newest "
        "releases.",
        "fmriprep-sdc:differ": "The libraries' step, a reference row (the original against "
        "itself): fMRIPrep's image runs numpy 2.2.6 (its sums in chunks) and SciPy 1.15.2 (the "
        "random walker's Laplacian in another row order), so its probabilities differ in their "
        "last bits from sdcflows' with numpy 2.5.4 and SciPy 1.18.1; LARMORX follows both "
        "(the row above).",
    }
    _verdicts(diffs, outputs, same, named, links, reasons)
    at = _peak_view(ex, "sdcflows", "probseg")
    near = _diff_view(ex, "fmriprep", "sdcflows", "probseg")
    default = {
        "output": "probseg",
        "plane": "axial",
        "voxel": near or at,
        "panels": ["clipped", "sdcflows", "larmorx", "fmriprep"],
        "diffs": ["lx-sdc", "lxf-fmriprep", "fmriprep-sdc"],
        "compare": {"a": "sdcflows", "b": "larmorx"},
    }
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="sdcflows",
        caption="the crosshair where fMRIPrep's probabilities differ most from sdcflows' with "
        "the newest releases; choose the output (the probability, the mask over the input, the "
        "masked magnitude) above the images.",
    )  # fmt: skip
    n_vox = int(np.prod(img.shape[:3]))
    t = {lab: _outputs_json(e.dir / lab)["seconds"] for lab, *_ in MASKER_RUNS}
    return {
        "id": "masker-ds002603",
        "title": "The magnitude's brain mask: sdcflows' brain_masker on fMRIPrep's ds002603 call",
        "summary": "sdcflows' `brain_masker` (its `BrainExtraction` interface, the `masker` node of "
        "every magnitude's and PEPOLAR reference's brain extraction) on the input fMRIPrep "
        "25.2.6 gave it for ds002603's phase-difference field map: a grey closing and median of "
        "the inverted image, Otsu's threshold, a binary opening and the largest component, then "
        "scikit-image's random walker between the eroded mask and the padded image's border; "
        "its probability, its mask at 0.5 and the masked magnitude. sdcflows 2.17.1 and LARMORX "
        "compute every output; fMRIPrep's own files are compared with LARMORX run on its "
        "image's libraries.",
        "facts": [
            {"label": "Data", "text": f"{_data('ds002603', 'sub-01')['dataset']}, sub-01 (CC0-1.0): "
             "the `acq-Mm2` magnitude as fMRIPrep's node was given it; shown inside fMRIPrep's "
             "brain mask, dilated by 2 voxels, so no face shows"},
            {"label": "Node", "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}`"},
            {"label": "Programs", "text": "sdcflows 2.17.1 in process (the newest environment); "
             f"LARMORX's `lx.sdcflows.brain_masker` at commit `{info['larmorx_commit'][:12]}`; "
             "numpy's OpenBLAS on its Haswell kernels, one thread"},
            {"label": "Time", "text": f"sdcflows {t['sdcflows']:.1f} s; LARMORX {t['larmorx']:.1f} s "
             f"at one thread, {t['larmorx-14']:.1f} s at 14 (informal: the benchmark below has "
             "the medians)"},
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],  # fmt: skip
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against sdcflows"],
            ["fmriprep", "LARMORX against fMRIPrep's own files"],
            ["check", "LARMORX against itself: threads and modes"],
            ["variability", "sdcflows against itself: the libraries' step"],
        ],
        "metrics_note": f"Δ is A − B over all {n_vox:,} voxels of each output file, not only "
        "those shown.",
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [_data("ds002603", "sub-01")]),
    }
