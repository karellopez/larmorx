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
- **The fieldmap-less (SyN) estimation** (``syn_sdc_workflow``) of fMRIPrep's ds000228 estimator
  (``--use-syn-sdc``) on fMRIPrep's own inputs: LARMORX beside ANTs 2.6.5 at one thread with
  correctly rounded maths (its warp byte for byte), ANTs' own band for this call (glibc's maths
  at one thread and twice at four) and fMRIPrep's own field (its four threads) against that band.
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
from larmorx_validation.evidence.sdcflows_fieldmap import _NUMBER, _diff, _found, _want
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
#: The fieldmap-less (SyN) example: fMRIPrep's run and its estimator's working folder.
SYN = (
    "ds000228-sub-pixar008-noreconall-syn",
    "work/fmriprep_25_2_wf/sub_pixar008_wf/fmap_preproc_wf/wf_auto_00000",
)
#: Why ANTs' four-thread runs are never cached: they measure the original's own spread.
SYN_SPREAD = "the original's own spread: ANTs' registration at four threads, run to run"
#: The SyN example's runs: label, implementation, threads, maths, D13 mode, never-cached reason.
SYN_RUNS = [
    ("larmorx", "larmorx", 14, "", "reproduce", ""),
    ("larmorx-fix", "larmorx", 4, "", "fix", ""),
    ("ants-cr1", "ants", 1, "cr", "reproduce", ""),
    ("ants-g1", "ants", 1, "glibc", "reproduce", ""),
    ("ants-g4a", "ants", 4, "glibc", "reproduce", SYN_SPREAD),
    ("ants-g4b", "ants", 4, "glibc", "reproduce", SYN_SPREAD),
]
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
        for label, mode in (("larmorx-14", "reproduce"), ("larmorx-fix", "fix")):
            e.run(label, [_python(), "-P", "-m", RUN_MODULE, "app", run, str(e.dir / label),
                          "--threads", "14", "--defects", mode],
                  env=env, unset=UNSET, cwd=e.dir, timeout=7200)  # fmt: skip
        e.finish(
            versions=_versions(),
            settings="sdcflows 2.17.1's BIDS app (`sdcflows <bids> <out> participant "
            "--participant-label <sub> --nprocs 1 --omp-nthreads 1 --no-fmapless --notrack "
            "--pdb`) and LARMORX's port of it (`lx.sdcflows.run_app`, reproduce) as "
            "check_app.py --sections run runs them (every derivative compared there byte for "
            "byte: ALL OK); LARMORX's again at 14 threads, in its reproduce and its default "
            "mode",
        )

    return compute


def app_entry(ctx: core.Context, key: str) -> fcache.Entry:
    _k, run, _f, kind, _w = _app(key)
    return fcache.entry(
        ctx.figures_context(), TOOL, f"app-{key}",
        f"sdcflows' BIDS app on {run} ({kind}): sdcflows 2.17.1 and LARMORX at one thread, "
        "LARMORX at 14 threads in both modes",
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


def _syn_wf() -> Path:
    run, wf = SYN
    return common.fmriprep_run(run) / wf


def _compute_syn(e: fcache.Entry) -> None:
    from concurrent.futures import ThreadPoolExecutor

    wf = _syn_wf()
    e.input("fMRIPrep's run (RUN_INFO.txt)", common.fmriprep_run(SYN[0]) / "RUN_INFO.txt")
    e.input("the registration's command (syn/command.txt)", wf / "syn" / "command.txt")
    e.input("fMRIPrep's warp (syn/fmap_syn0Warp.nii.gz)", wf / "syn" / "fmap_syn0Warp.nii.gz")
    e.input("fMRIPrep's field (unwarp/clipped_field.nii.gz)",
            wf / "unwarp" / "clipped_field.nii.gz")  # fmt: skip
    env = {**_env(e), "LARMORX_PARITY_RUN_TIMEOUT": "14400"}

    def argv(label: str, impl: str, threads: int, maths: str, mode: str, never: str) -> list:
        a = [_python(), "-P", "-m", RUN_MODULE, "syn", str(wf), str(e.dir / label),
             "--implementation", impl, "--threads", str(threads), "--defects", mode]  # fmt: skip
        return a + (["--maths", maths] if maths else []) + (["--never", never] if never else [])

    # the six runs side by side (1 + 1 + 4 + 4 ANTs threads, 14 + 4 LARMORX threads)
    with ThreadPoolExecutor(len(SYN_RUNS)) as pool:
        list(pool.map(lambda r: e.run(r[0], argv(*r), env=env, unset=UNSET, cwd=e.dir,
                                      timeout=6 * 3600), SYN_RUNS))  # fmt: skip
    order = {r[0]: i for i, r in enumerate(SYN_RUNS)}
    e.runs.sort(key=lambda r: order.get(r.label, len(order)))
    e.run("fmriprep", argv("fmriprep", "fmriprep", 4, "", "reproduce", ""), env=env,
          unset=UNSET, cwd=e.dir)  # fmt: skip
    e.run("warps", [_python(), "-P", "-m", RUN_MODULE, "syn-warps", str(e.dir),
                    str(e.dir / "warps")], env=env, unset=UNSET, cwd=e.dir)  # fmt: skip
    e.finish(
        versions={
            **_versions(),
            "programs": "ANTs 2.6.5's antsRegistration (the GCC 14.3 build), through the oracle "
            "cache: one thread with CORE-MATH's correctly rounded functions preloaded, one thread "
            "and twice four threads with glibc's (never cached: the original's own spread); "
            "fMRIPrep 25.2.6's image holds ANTs 2.6.2.dev1 (its own build)",
        },
        node=SYN[1].removeprefix("work/"),
        settings="sdcflows' init_syn_sdc_wf as fMRIPrep 25.2.6 ran it for ds000228's estimator "
        "(--use-syn-sdc): LARMORX's lx.sdcflows.syn_sdc_workflow on fMRIPrep's own inputs and "
        "ANTS_RANDOM_SEED, fMRIPrep's image's numpy 2.2.6 for the clip (14 threads reproduce, 4 "
        "default); ANTs 2.6.5 on fMRIPrep's own registration command and inputs, then sdcflows' "
        "steps after it (extract_field, zooms_field, bs_filter, unwarp) by LARMORX's port in "
        "reproduce mode (= sdcflows' nodes on these inputs, check_syn_sdc.py --sections nodes); "
        "the same steps on fMRIPrep's own warp (its 4 threads); the newest environment",
    )


def syn_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(), TOOL, "syn-ds000228",
        "The fieldmap-less (SyN) estimation of fMRIPrep's ds000228 estimator: LARMORX, ANTs "
        "2.6.5 at one thread (correctly rounded maths, glibc's) and twice at four (glibc's), "
        "fMRIPrep's own warp",
        _compute_syn,
    )  # fmt: skip


def entries(ctx: core.Context) -> list[fcache.Entry]:
    return [
        *(app_entry(ctx, k) for k, *_ in APP),
        stage_entry(ctx),
        masker_entry(ctx),
        epi_entry(ctx),
        memory_entry(ctx),
        syn_entry(ctx),
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


def _named_text(named: list[str]) -> str:
    """The register entries a run's notices named, each with what it is (larmorx_core::defects)."""
    from larmorx._defects import defects

    what = {d.id: f"{d.tool}: {d.what}" for d in defects()}
    return "; ".join(f"{i} ({common.clip(what.get(i, ''), 170)})" for i in named)


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
            x["reason"] = (
                reasons[i] if same[i] else reasons[i + ":differ"] + " " + _differs(x, outputs) + "."
            )
            continue
        if same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = reasons[i]
        elif i.startswith("fix-") and named:
            x["verdict"] = "d13"
            x["reason"] = (
                f"The default mode corrects {_named_text(named)} and names "
                + ("it: " if len(named) == 1 else "them: ")
                + _differs(x, outputs)
                + "."
            )
            x["links"] += _link(links, named)
        elif explained and i in explained:
            x["verdict"], why = explained[i]
            x["reason"] = why + " " + _differs(x, outputs) + "."
        else:
            x["verdict"] = "open"
            x["reason"] = (
                "Not accounted for by the other runs of this example: " + _differs(x, outputs) + "."
            )


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
            {
                "label": "Data",
                "text": f"{_data('ds002603', 'sub-01')['dataset']}, sub-01 (CC0-1.0): "
                "the `acq-Mm2` magnitude as fMRIPrep's node was given it; shown inside fMRIPrep's "
                "brain mask, dilated by 2 voxels, so no face shows",
            },
            {"label": "Node", "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}`"},
            {
                "label": "Programs",
                "text": "sdcflows 2.17.1 in process (the newest environment); "
                f"LARMORX's `lx.sdcflows.brain_masker` at commit `{info['larmorx_commit'][:12]}`; "
                "numpy's OpenBLAS on its Haswell kernels, one thread",
            },
            {
                "label": "Time",
                "text": f"sdcflows {t['sdcflows']:.1f} s; LARMORX {t['larmorx']:.1f} s "
                f"at one thread, {t['larmorx-14']:.1f} s at 14 (informal: the benchmark below has "
                "the medians)",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
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


# ---------------------------------------------------------- epi_mask and SciPy's watershed

EPI_OUTPUTS = [{"id": "mask", "label": "Brain mask (epi_mask)", "unit": ""}]
EPI_PANELS = [
    ("sdcflows", "original", "sdcflows 2.17.1", "SciPy 1.18.1's watershed_ift: LARMORX's reference"),
    ("larmorx", "larmorx", "LARMORX (port)", "reproduce mode: SciPy's watershed"),
    ("larmorx-fix", "larmorx", "LARMORX (port)", "default mode: the corrected watershed (SCI-13, SCI-14)"),
]  # fmt: skip


def _epi_masks(e: fcache.Entry, run: str) -> dict[str, np.ndarray]:
    ds = run.split("-")[0]
    return {lab: _stored(e.dir / ds / lab / "mask.nii.gz") for lab, *_ in EPI_MASK_RUNS}


def _dice(a: np.ndarray, b: np.ndarray) -> float:
    a, b = a > 0, b > 0
    n = int(a.sum()) + int(b.sum())
    return 2.0 * int((a & b).sum()) / n if n else 0.0


def _example_epi(ctx: core.Context, links: dict) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.evidence.fsl_bet import window_of

    e = epi_entry(ctx)
    info = e.info()
    run = EPI_RUNS[0]
    ds = run.split("-")[0]
    sub = run.split("-")[1] + "-" + run.split("-")[2]
    ref, brain_path = _epi_ref(run)
    img = nib.load(str(ref))
    masks = _epi_masks(e, run)
    brain = _stored(brain_path)
    show = _show(np.logical_or.reduce([m > 0 for m in [*masks.values(), brain]]), 2)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    raw, rs, ri = common.stored(ref)
    ex.add(exm.Panel("ref", "input", f"{ds}: BOLD reference", "fMRIPrep's `desc-hmc_boldref`, "
                     "epi_mask's input", {"all": raw}, scale=rs, inter=ri))  # fmt: skip
    over = {"mask": {"on": "ref", "window": window_of(ex, "ref")}}
    ex.add(exm.Panel("brain", "input", f"{ds}: fMRIPrep's brain mask",
                     "`desc-brain_mask` (niworkflows' skull stripping), the independent "
                     "reference", {"all": brain}, overlay=over))  # fmt: skip
    for pid, role, label, sub_ in EPI_PANELS:
        ex.add(exm.Panel(pid, role, label, sub_, {"mask": masks[pid]}, overlay=over))
    d = common.d
    diffs = [
        d("lx-sdc", "larmorx", "sdcflows", "main", "LARMORX (reproduce) − sdcflows 2.17.1",
          common.ROLE_LX),
        d("fix-lx", "larmorx-fix", "larmorx", "check", "LARMORX: default − reproduce mode",
          common.ROLE_SELF),
    ]  # fmt: skip
    outputs = copy.deepcopy(EPI_OUTPUTS)
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    named = _outputs_json(e.dir / ds / "larmorx-fix").get("named", [])
    d_sdc, d_fix = _dice(masks["sdcflows"], brain), _dice(masks["larmorx-fix"], brain)
    n = {k: int((v > 0).sum()) for k, v in masks.items()}
    reasons = {
        "lx-sdc": "LARMORX's reproduce mode gives sdcflows' mask, every voxel: the non-flat "
        "`ball(3)` opening and `ball(2)` closing, numpy's percentiles, SciPy's watershed from "
        "the two markers with its defects, the `ball(2)` dilation.",
        "fix-lx": "No correction changes this reference's mask.",
    }
    _verdicts(diffs, outputs, same, named, links, reasons)
    for x in diffs:
        if x["id"] == "fix-lx" and x["verdict"] == "d13":
            x["reason"] = (
                f"The default corrects SciPy's watershed (SCI-13: neighbours across the image's "
                f"edges; SCI-14: an element left alone in its queue) and names it: the object's "
                f"region no longer stops early. sdcflows' mask holds {n['sdcflows']:,} voxels "
                f"(Dice {d_sdc:.3f} with fMRIPrep's brain mask), the corrected one "
                f"{n['larmorx-fix']:,} (Dice {d_fix:.3f}): rule A, measured on four references "
                "(the table below)."
            )
    at = _diff_view(ex, "larmorx-fix", "larmorx", "mask") or _peak_view(ex, "brain", "mask")
    default = {
        "output": "mask",
        "plane": "axial",
        "voxel": at,
        "panels": ["ref", "brain", "sdcflows", "larmorx-fix"],
        "diffs": ["lx-sdc", "fix-lx"],
        "compare": {"a": "larmorx-fix", "b": "larmorx"},
    }
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="sdcflows",
        caption="each mask over the BOLD reference; the crosshair where the corrected watershed "
        "changes the mask.",
    )  # fmt: skip
    data = _data(ds, sub)
    return {
        "id": f"epi-mask-{ds}",
        "title": f"SciPy's watershed corrected by default: sdcflows' epi_mask on {ds}'s BOLD "
        "reference",
        "summary": "sdcflows' `epi_mask` (`utils/epimanip.py`; no workflow of sdcflows or "
        "fMRIPrep calls it) masks an EPI by grey-level morphology and SciPy's `watershed_ift` "
        "from two markers, the object at the window's centre of mass and the background at one "
        "corner voxel. SciPy's watershed lets a region's front reach across the image's edges "
        "(SCI-13) and can leave an element alone in its queue (SCI-14): on this multi-echo "
        "BOLD reference the object's region stops early. LARMORX corrects both by default "
        "(rule A, measured against fMRIPrep's own brain masks) and gives SciPy's result in its "
        "reproduce mode.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {sub} ({data['licence']}): fMRIPrep "
                "25.2.6's BOLD reference of the run (`desc-hmc_boldref`) and its brain mask; "
                "shown inside the masks, dilated by 2 voxels",
            },
            {
                "label": "Masks",
                "text": f"sdcflows {n['sdcflows']:,} voxels (Dice {d_sdc:.3f} with "
                f"fMRIPrep's), LARMORX reproduce {n['larmorx']:,}, default {n['larmorx-fix']:,} "
                f"(Dice {d_fix:.3f}); fMRIPrep's mask {int((brain > 0).sum()):,}",
            },
            {
                "label": "Programs",
                "text": "sdcflows 2.17.1 in process (the newest environment); "
                f"LARMORX's `lx.sdcflows.epi_mask` at commit `{info['larmorx_commit'][:12]}`",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against sdcflows"],
            ["check", "LARMORX against itself: its default mode (D13)"],
        ],
        "metrics_note": f"Δ is A − B over all {int(np.prod(img.shape[:3])):,} voxels of each "
        "mask, not only those shown.",
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
    }


def _epi_table(ctx: core.Context) -> dict:
    e = epi_entry(ctx)
    rows = []
    for run in EPI_RUNS:
        ds = run.split("-")[0]
        masks = _epi_masks(e, run)
        brain = _stored(_epi_ref(run)[1])
        named = _outputs_json(e.dir / ds / "larmorx-fix").get("named", [])
        same = masks["larmorx"].tobytes() == masks["sdcflows"].tobytes()
        cells = [common.cell(int((brain > 0).sum()), "count")]
        for lab in ("sdcflows", "larmorx-fix"):
            m = masks[lab] > 0
            cells += [common.cell(int(m.sum()), "count"), common.cell(_dice(m, brain), "sig"),
                      common.cell(int((m & ~(brain > 0)).sum()), "count")]  # fmt: skip
        cells.append(common.cell("identical" if same else "differs"))
        cells.append(common.cell(", ".join(named) if named and
                                 masks["larmorx-fix"].tobytes() != masks["larmorx"].tobytes()
                                 else "–"))  # fmt: skip
        rows.append({"head": f"{ds} ({brain.shape[0]} × {brain.shape[1]} × "
                             f"{brain.shape[2]})", "cells": cells})  # fmt: skip
    return common.table(
        "epi-mask",
        "epi_mask on four BOLD references: SciPy's watershed and the corrected one, against "
        "fMRIPrep's brain masks",
        [
            "Reference",
            {"label": "fMRIPrep's mask", "num": True},
            {"label": "sdcflows: voxels", "num": True},
            {"label": "Dice", "num": True},
            {"label": "outside fMRIPrep's", "num": True},
            {"label": "default: voxels", "num": True},
            {"label": "Dice", "num": True},
            {"label": "outside fMRIPrep's", "num": True},
            "reproduce = sdcflows",
            "named by default",
        ],
        rows,
        note="The cache entry's runs (sdcflows 2.17.1 and LARMORX in both modes, the newest "
        "environment) on fMRIPrep 25.2.6's `desc-hmc_boldref` of four oracle runs; fMRIPrep's "
        "`desc-brain_mask` of each run is the independent reference (niworkflows' skull "
        "stripping; for ds005040's PEPOLAR run it is the corrected reference's). The corrected "
        "masks are better on three references and the same on the fourth, worse on none: rule "
        'A (docs/findings/sdcflows-estimation.md, "epi_mask and SciPy\'s watershed"). The '
        "default masks larger than fMRIPrep's by thousands of voxels are epi_mask's own "
        "`ball(2)` dilation around the brain; no sdcflows or fMRIPrep workflow calls epi_mask.",
    )


# ------------------------------------------------------------------------- the BIDS app

#: The app examples' panels of runs: id, role, label, sub.
APP_PANELS = [
    ("sdcflows", "original", "sdcflows 2.17.1, 1 thread",
     "its BIDS app; numpy 2.5.4, SciPy 1.18.1; FSL 6.0.7.23's topup, ANTs 2.6.5's N4 (one "
     "thread, correctly rounded maths), AFNI 26.3.05's 3dvolreg: LARMORX's reference"),
    ("larmorx", "larmorx", "LARMORX (port), 1 thread",
     "its BIDS app, reproduce mode; LARMORX's replicas and ports underneath"),
    ("larmorx-14", "larmorx", "LARMORX (port), 14 threads", "reproduce mode"),
    ("larmorx-fix", "larmorx", "LARMORX (port), 14 threads", "default mode (D13 corrections)"),
    ("fmriprep", "fmriprep", "fMRIPrep 25.2.6's own files",
     "sdcflows 2.15.1 in its image (numpy 2.2.6, SciPy 1.15.2); N4 at 8 threads, glibc's maths"),
    ("larmorx-fmriprep", "larmorx", "LARMORX (port), as fMRIPrep ran the stage",
     "sdcflows 2.15's wiring, fMRIPrep's image's numpy and SciPy, the replicas; reproduce mode"),
    ("sdcflows-4", "original", "sdcflows 2.17.1, 4 threads",
     "its own oracle run (--nprocs 4 --omp-nthreads 4, RUN_INFO.txt)"),
]  # fmt: skip
#: The derivatives' ``desc`` -> the example's output.
DESC = {
    "preproc_fieldmap": "fmap",
    "epi_fieldmap": "ref",
    "magnitude_fieldmap": "ref",
    "brain_mask": "mask",
    "coeff_fieldmap": "coeff",
}


def _app_outputs(kind: str) -> list[dict]:
    ref = "EPI reference (`desc-epi`)" if kind == "pepolar" else "Magnitude (`desc-magnitude`)"
    return [
        {"id": "fmap", "label": "Field map (Hz, `desc-preproc`)", "unit": "Hz"},
        {"id": "ref", "label": ref, "unit": ""},
        {"id": "probseg", "label": "Brain probability (the masker node)", "unit": ""},
        {"id": "mask", "label": "Brain mask (`desc-brain`)", "unit": ""},
    ]


def _app_dirs(ctx: core.Context, key: str) -> dict[str, tuple[Path, Path]]:
    """Each run's derivatives folder and working folder."""
    _k, run, frun, _kind, _w = _app(key)
    e, st = app_entry(ctx, key), stage_entry(ctx)
    sdc = _workspace() / "oracles" / "sdcflows-2.17.1" / run
    fm = common.fmriprep_run(frun)
    stage = st.dir / "stage" / "fmriprep" / frun
    return {
        "sdcflows": (e.dir / "app" / "run" / f"{run}-sdcflows" / "out",
                     e.dir / "app" / "run" / f"{run}-sdcflows" / "work"),
        "larmorx": (e.dir / "app" / "run" / f"{run}-larmorx" / "out",
                    e.dir / "app" / "run" / f"{run}-larmorx" / "work"),
        "larmorx-14": (e.dir / "larmorx-14" / "out", e.dir / "larmorx-14" / "work"),
        "larmorx-fix": (e.dir / "larmorx-fix" / "out", e.dir / "larmorx-fix" / "work"),
        "sdcflows-4": (sdc / "out", sdc / "work"),
        "fmriprep": (fm / "out", fm / "work"),
        "larmorx-fmriprep": (stage / "out", stage / "work"),
    }  # fmt: skip


def _derivs(out: Path) -> dict[str, Path]:
    found: dict[str, Path] = {}
    for p in sorted(out.rglob("*_desc-*.nii.gz")):
        if "figures" in p.parts or "fmap" not in p.parts:
            continue
        desc = p.name.split("_desc-", 1)[1].removesuffix(".nii.gz")
        if desc in DESC:
            found[DESC[desc]] = p
    return found


def _probseg(work: Path) -> Path:
    found = sorted(work.rglob("wf_auto_00000/**/brainextraction_wf/masker/clipped_probseg.nii.gz"))
    if len(found) != 1:
        raise FileNotFoundError(f"{work}: {len(found)} masker probabilities of wf_auto_00000")
    return found[0]


def _app_arrays(ctx: core.Context, key: str) -> tuple[dict, dict]:
    arrays, coeffs = {}, {}
    for pid, (out, work) in _app_dirs(ctx, key).items():
        f = _derivs(out)
        arrays[pid] = {k: _stored(f[k]) for k in ("fmap", "ref", "mask")}
        arrays[pid]["probseg"] = _stored(_probseg(work))
        coeffs[pid] = _stored(f["coeff"])
    return arrays, coeffs


def _app_diffs() -> list[dict]:
    d = common.d
    return [
        d("lx-sdc", "larmorx", "sdcflows", "main", "LARMORX − sdcflows 2.17.1 (both one thread)",
          common.ROLE_LX),
        d("lxf-fmriprep", "larmorx-fmriprep", "fmriprep", "fmriprep",
          "LARMORX as fMRIPrep ran the stage − fMRIPrep 25.2.6's own files", common.ROLE_FMRIPREP),
        d("lx14-lx1", "larmorx-14", "larmorx", "check",
          "LARMORX (reproduce): 14 threads − 1 thread", common.ROLE_SELF),
        d("fix-lx14", "larmorx-fix", "larmorx-14", "check",
          "LARMORX at 14 threads: default − reproduce mode", common.ROLE_SELF),
        d("sdc4-sdc1", "sdcflows-4", "sdcflows", "variability",
          "sdcflows: its own run at 4 threads − 1 thread", common.ROLE_ORIG),
        d("fmriprep-sdc", "fmriprep", "sdcflows", "variability",
          "fMRIPrep 25.2.6's own files − sdcflows 2.17.1 with the newest releases",
          common.ROLE_ORIG),
    ]  # fmt: skip


def _stage_lines(ctx: core.Context, frun: str) -> list[str]:
    log = (stage_entry(ctx).dir / "stage.log").read_text(encoding="utf-8")
    return [ln for ln in log.splitlines() if re.match(rf"INFO {re.escape(frun)}[ :]", ln)]


def _example_app(ctx: core.Context, key: str, links: dict) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.evidence.fsl_bet import window_of

    _k, run, frun, kind, what = _app(key)
    e = app_entry(ctx, key)
    info = e.info()
    dirs = _app_dirs(ctx, key)
    arrays, coeffs = _app_arrays(ctx, key)
    fpath = _derivs(dirs["sdcflows"][0])["fmap"]
    img = nib.load(str(fpath))
    show = _show(np.logical_or(arrays["sdcflows"]["mask"] > 0, arrays["fmriprep"]["mask"] > 0), 2)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    wf = dirs["sdcflows"][1] / "sdcflows_wf" / "wf_auto_00000"
    ds = key
    if kind == "pepolar":
        vols = sorted((wf / "select_volumes").glob("*_vol0.nii.gz"))
        first = None
        for v in vols:
            pe = re.search(r"_dir-([A-Z]+)_", v.name).group(1)  # type: ignore[union-attr]
            a, s_, i_ = common.stored(v)
            ex.add(exm.Panel(f"epi-{pe}", "input", f"{ds}: EPI, phase encoding {pe}",
                             "the first volume (SelectPEVolumes' output)", {"all": a}, scale=s_,
                             inter=i_))  # fmt: skip
            first = first or f"epi-{pe}"
    else:
        a, s_, i_ = common.stored(next((wf / "fmapmrg").glob("*.nii.gz")))
        ex.add(exm.Panel("fieldmap", "input", f"{ds}: the scanner's field map (Hz)",
                         "the BIDS `fieldmap` image as merged (fmapmrg)", {"all": a}, scale=s_,
                         inter=i_))  # fmt: skip
        a, s_, i_ = common.stored(next((wf / "magnitude_wf" / "magmrg").glob("*.nii.gz")))
        ex.add(exm.Panel("magnitude", "input", f"{ds}: magnitude", "the BIDS `magnitude` image "
                         "as merged (magmrg)", {"all": a}, scale=s_, inter=i_))  # fmt: skip
        first = "magnitude"
    over = {"mask": {"on": first, "window": window_of(ex, first)}}
    for pid, role, label, sub in APP_PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid], overlay=over))
    diffs = _app_diffs()
    outputs = copy.deepcopy(_app_outputs(kind))
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    named = _outputs_json(e.dir / "larmorx-fix").get("named", [])
    lines = _stage_lines(ctx, frun)
    n4 = [ln for ln in lines if "data different" in ln]
    reasons = {
        "lx-sdc": "Every value of the four outputs is sdcflows' (and every derivative file, "
        "byte for byte: the cache entry's check_app.py run), with LARMORX's replicas and ports "
        "underneath: "
        + (
            "FSL's topup, AFNI's 3dvolreg (niworkflows' RobustAverage), ANTs' N4, sdcflows' "
            "masks and the TOPUP coefficients' reorientation."
            if kind == "pepolar"
            else "ANTs' N4, sdcflows' masks and the B-spline fit of the field map in Hz."
        ),
        "lxf-fmriprep": "fMRIPrep's own derivatives hold the same values: LARMORX's field-map "
        "stage run as fMRIPrep ran it gives them.",
        "lx14-lx1": "The same bytes at 1 and 14 threads (every sum in sdcflows' order, whatever "
        "the thread count).",
        "fix-lx14": "No correction of the register changes this run's values: both modes give "
        "the same bytes.",
        "sdc4-sdc1": "The original against itself, a reference row: sdcflows' own run at four "
        "threads gives the same values.",
        "sdc4-sdc1:differ": "The original against itself, a reference row: sdcflows' own run "
        "of 2026-11-08 at four threads (`--nprocs 4 --omp-nthreads 4`; ANTs' N4 then sums in "
        "another order) gives other values than at one thread:",
        "fmriprep-sdc": "The version step, a reference row: fMRIPrep 25.2.6's files hold "
        "sdcflows 2.17.1's values.",
        "fmriprep-sdc:differ": "The version step, a reference row (the originals against each "
        "other): fMRIPrep 25.2.6 runs sdcflows 2.15.1"
        + ("'s PEPOLAR wiring" if kind == "pepolar" else "")
        + " with its image's numpy 2.2.6 and SciPy 1.15.2 and N4 at 8 threads with glibc's "
        "maths, sdcflows 2.17.1 here its own with the newest releases; LARMORX follows both "
        "(the first two rows):",
    }
    explained = {
        "lxf-fmriprep": (
            "within-variability",
            "Within the original's own variability: fMRIPrep's N4 node ran at 8 threads with "
            "glibc's exp and log, LARMORX's at one thread with correctly rounded maths (as "
            "ANTs' at one thread); ANTs 2.6.5 at the node's 8 threads with glibc gives "
            "fMRIPrep's N4 output, at one thread with correctly rounded maths LARMORX's (the "
            "cache entry's check_fmap_stage.py --sections fmriprep: every derivative identical, "
            "its data identical with only FSL's build string in `descrip`, or after such an N4 "
            f"node: {len(n4)} file{'' if len(n4) == 1 else 's'} of this run); the reference, its brain probability and "
            "mask follow from N4's output:",
        )
    }
    _verdicts(diffs, outputs, same, named, links, reasons, explained)
    at = _peak_view(ex, "sdcflows", "fmap")
    default = {
        "output": "fmap",
        "plane": "axial",
        "voxel": at,
        "panels": [first, "sdcflows", "larmorx", "fmriprep"],
        "diffs": ["lx-sdc", "lxf-fmriprep", "fix-lx14", "fmriprep-sdc"],
        "compare": {"a": "sdcflows", "b": "larmorx"},
    }
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="sdcflows",
        caption="the crosshair where the field is strongest in the slice; choose the output (the "
        "field map, the reference, its brain probability, the mask over the input) above the "
        "images.",
    )  # fmt: skip
    nc = {k: v.tobytes() == coeffs["larmorx"].tobytes() for k, v in coeffs.items()}
    ncf = coeffs["larmorx-fmriprep"].tobytes() == coeffs["fmriprep"].tobytes()
    cshape = " × ".join(str(x) for x in coeffs["sdcflows"].shape[:3])
    coeff_text = (
        f"the {cshape} coefficients (`desc-coeff`): LARMORX's "
        + ("equal sdcflows' at one thread" if nc["sdcflows"] else "differ from sdcflows'")
        + (", at 14 threads too" if nc["larmorx-14"] else "")
        + ("; the default mode's too" if nc["larmorx-fix"] else
           "; the default mode's differ (its corrections, below)")
        + "; as fMRIPrep ran the stage, "
        + ("fMRIPrep's own file, every value" if ncf else "not fMRIPrep's own file")
        + (" (TOPUP's own coefficients on the field map's grid, reoriented by "
           "TOPUPCoeffReorient)" if kind == "pepolar" else
           " (the B-spline fit's, 16 × 16 × 10 mm apart)")
    )  # fmt: skip
    sub = "sub-" + run.split("-sub-")[1].split("-")[0]
    data = _data(ds, sub)
    secs = {lab: r["seconds"] for lab in ("check-app", "larmorx-14", "larmorx-fix")
            for r in info["runs"] if r["label"] == lab}  # fmt: skip
    title = (
        f"PEPOLAR: sdcflows' BIDS app on {ds}'s AP/PA pair (TOPUP)"
        if kind == "pepolar"
        else f"A direct field map: sdcflows' BIDS app on {ds}'s field map in Hz"
    )
    return {
        "id": f"app-{key}",
        "title": title,
        "summary": f"sdcflows' BIDS app on {ds} ({what}): "
        + (
            "the two EPIs' first volumes regridded to one grid, merged and given to FSL's topup "
            "(its `b02b0` settings), the field and its coefficients reoriented back, the "
            "corrected reference averaged by niworkflows' RobustAverage (AFNI's 3dvolreg) and "
            "skull-stripped (IntensityClip, ANTs' N4, sdcflows' brain_masker)"
            if kind == "pepolar"
            else "the field map in Hz merged, the magnitude skull-stripped (IntensityClip, ANTs' "
            "N4, sdcflows' brain_masker), the field map's cubic B-spline fit inside the mask"
        )
        + ". sdcflows 2.17.1 and LARMORX run the whole app; fMRIPrep 25.2.6's own derivatives "
        "of the same data are compared with LARMORX's field-map stage run as fMRIPrep ran it, "
        "and sdcflows' own run at four threads is the original's own spread.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, {sub} ({data['licence']}): the "
                "field-map files of the dataset (with the derived sidecars of its view, RUN_INFO."
                "txt); shown inside the brain masks of sdcflows and fMRIPrep, dilated by 2 voxels, "
                "so no face shows",
            },
            {
                "label": "Command",
                "text": f"`sdcflows <bids> <out> participant --participant-label "
                f"{sub.removeprefix('sub-')} --nprocs 1 --omp-nthreads 1 --no-fmapless`; LARMORX: "
                "`larmorx sdcflows sdcflows` with the same arguments (`lx.sdcflows.run_app`)",
            },
            {"label": "Coefficients", "text": coeff_text},
            {
                "label": "Programs",
                "text": "sdcflows 2.17.1 in its BIDS app (the newest "
                "environment), its programs through the oracle cache; LARMORX at commit "
                f"`{info['larmorx_commit'][:12]}`; numpy's OpenBLAS on its Haswell kernels, one "
                "thread",
            },
            {
                "label": "Time",
                "text": f"both apps at one thread {secs.get('check-app', 0):.0f} s "
                f"(check_app.py, sdcflows' programs from the oracle cache); LARMORX at 14 threads "
                f"{secs.get('larmorx-14', 0):.0f} s (informal: the benchmark below has the medians)",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against sdcflows"],
            ["fmriprep", "LARMORX against fMRIPrep's own files"],
            ["check", "LARMORX against itself: threads and modes"],
            ["variability", "The originals against themselves: threads, versions"],
        ],
        "metrics_note": f"Δ is A − B over all {int(np.prod(img.shape[:3])):,} voxels of each "
        "output file, not only those shown (the field map's one volume as its 3-D image).",
        "provenance": common.provenance(
            info,
            f"{TOOL}/{e.case}",
            [data],
            [f"{TOOL}/stage-fmriprep (LARMORX's stage as fMRIPrep ran it)"],
        ),
    }


# ------------------------------------------------------------------ the fieldmap-less (SyN)

SYN_OUTPUTS = [
    {"id": "fmap", "label": "Field map (Hz, the unwarp node's `_field`)", "unit": "Hz"},
    {"id": "ref", "label": "Corrected EPI reference (`_sdc`)", "unit": ""},
]
#: The SyN example's panels of runs: id, role, label, sub.
SYN_PANELS = [
    ("larmorx", "larmorx", "LARMORX (port), 14 threads",
     "lx.sdcflows.syn_sdc_workflow on fMRIPrep's inputs, reproduce mode; its antsRegistration "
     "computes ANTs' one-thread result at any thread count"),
    ("larmorx-fix", "larmorx", "LARMORX (port), 4 threads", "default mode (D13 corrections)"),
    ("ants-cr1", "original", "ANTs 2.6.5, 1 thread, correctly rounded maths",
     "the GCC 14.3 build with CORE-MATH's functions preloaded, on fMRIPrep's command line; "
     "sdcflows' steps after it: LARMORX's reference"),
    ("ants-g1", "original", "ANTs 2.6.5, 1 thread, glibc's maths",
     "the same build with the C library's functions"),
    ("ants-g4a", "original", "ANTs 2.6.5, 4 threads, glibc's maths",
     "as fMRIPrep's node ran it (--omp-nthreads 4); the first run"),
    ("ants-g4b", "original", "ANTs 2.6.5, 4 threads, glibc's maths",
     "the same call again (never cached: the original's own spread)"),
    ("fmriprep", "fmriprep", "fMRIPrep 25.2.6's own files",
     "its image's ANTs 2.6.2.dev1 at 4 threads, glibc's maths"),
]  # fmt: skip


def _syn_diffs() -> list[dict]:
    d = common.d
    return [
        d("lx-ants", "larmorx", "ants-cr1", "main",
          "LARMORX (14 threads) − ANTs 2.6.5 at one thread, correctly rounded maths",
          common.ROLE_LX),
        d("lx-fmriprep", "larmorx", "fmriprep", "fmriprep",
          "LARMORX − fMRIPrep 25.2.6's own files", common.ROLE_FMRIPREP),
        d("fix-lx", "larmorx-fix", "larmorx", "check",
          "LARMORX: default mode (4 threads) − reproduce (14 threads)", common.ROLE_SELF),
        d("g1-cr1", "ants-g1", "ants-cr1", "variability",
          "ANTs 2.6.5 at one thread: glibc's maths − correctly rounded", common.ROLE_ORIG),
        d("g4-g1", "ants-g4a", "ants-g1", "variability",
          "ANTs 2.6.5 with glibc's maths: 4 threads − 1 thread", common.ROLE_ORIG),
        d("g4b-g4a", "ants-g4b", "ants-g4a", "variability",
          "ANTs 2.6.5 at 4 threads with glibc's maths: the second run − the first",
          common.ROLE_ORIG),
        d("fmriprep-g4", "fmriprep", "ants-g4a", "variability",
          "fMRIPrep's own files (ANTs 2.6.2.dev1) − ANTs 2.6.5, both at 4 threads with glibc's "
          "maths", common.ROLE_ORIG),
    ]  # fmt: skip


#: ANTs 2.6.5's runs of the call, as fMRIPrep's image runs ANTs (glibc's maths), and with
#: correctly rounded maths: the original's own band is the largest difference between two of them.
SYN_GLIBC = ("ants-g1", "ants-g4a", "ants-g4b")
SYN_ALL = ("ants-cr1", *SYN_GLIBC)
SYN_WORDS = {
    "ants-cr1": "its one-thread run with correctly rounded maths",
    "ants-g1": "its one-thread run",
    "ants-g4a": "its first four-thread run",
    "ants-g4b": "its second four-thread run",
}


def _band(arrays: dict, runs: tuple[str, ...]) -> tuple[float, float, str, str]:
    """The largest difference of the field (Hz, over every voxel) between two of ``runs``: its
    maximum, the root mean square of that pair, and the pair."""
    import itertools

    found = []
    for a, b in itertools.combinations(runs, 2):
        m = core.compare(arrays[a]["fmap"], arrays[b]["fmap"])
        found.append((m["max_abs"], m["rms"], a, b))
    return max(found)


def _syn_warps(e: fcache.Entry) -> dict[tuple[str, str], dict]:
    pairs = json.loads((e.dir / "warps" / "warps.json").read_text(encoding="utf-8"))
    return {(p["a"], p["b"]): p for p in pairs}


def _warp_pair(w: dict, a: str, b: str) -> dict:
    return w.get((a, b)) or w[(b, a)]


def _mm(p: dict) -> str:
    if p["identical"]:
        return f"byte for byte ({p['values']:,} values)"
    return (
        f"{p['differ']:,} of {p['values']:,} values differ, at most {p['max_abs_mm']:.3g} mm "
        f"apart (99th percentile {p['p99_mm']:.3g} mm)"
    )


def _example_syn(ctx: core.Context, links: dict) -> dict:
    import nibabel as nib

    from larmorx_validation.evidence import example as exm

    e = syn_entry(ctx)
    info = e.info()
    wf = _syn_wf()
    own = {"fmap": _stored(wf / "unwarp" / "clipped_field.nii.gz"),
           "ref": _stored(wf / "unwarp" / "clipped_sdc.nii.gz")}  # fmt: skip
    chain = _files(e.dir / "fmriprep")
    for k in ("fmap", "ref"):  # sdcflows' steps on fMRIPrep's own warp: its own files
        if _stored(chain[k]).tobytes() != own[k].tobytes():
            raise ValueError(f"the steps on fMRIPrep's warp do not give its {k}")
    own_coeff = _stored(wf / "bs_filter" / "fmap_syn0Warp_Hz_trans_coeff000.nii.gz")
    if _stored(chain["coeff"]).tobytes() != own_coeff.tobytes():
        raise ValueError("the steps on fMRIPrep's warp do not give its coefficients")
    fpath = wf / "unwarp" / "clipped_field.nii.gz"
    img = nib.load(str(fpath))
    pre = wf.parents[1] / "syn_preprocessing_auto_00000"
    mask_in = _stored(wf / "zooms_bmask" / "sub-pixar008_desc-brain_mask_trans_uint8_trans.nii.gz")
    epi_ref = (
        pre
        / "epi_reference_wf"
        / "clip_bg_noise"
        / "mapflow"
        / "_clip_bg_noise0"
        / "clipped.nii.gz"
    )
    epi_mask = _stored(pre / "epi_brain" / "clipped_mask.nii.gz")
    show = _show(np.logical_or(mask_in > 0, epi_mask > 0), 2)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    raw, rs, ri = common.stored(epi_ref)
    ex.add(exm.Panel("epi", "input", "ds000228: the EPI reference",
                     "the workflow's epi_ref (syn_preprocessing's clip_bg_noise), as fMRIPrep "
                     "gave it", {"all": raw}, scale=rs, inter=ri))  # fmt: skip
    arrays = {"fmriprep": own}
    for pid, *_ in SYN_PANELS:
        if pid != "fmriprep":
            f = _files(e.dir / pid)
            arrays[pid] = {o["id"]: _stored(f[o["id"]]) for o in SYN_OUTPUTS}
    for pid, role, label, sub in SYN_PANELS:
        ex.add(exm.Panel(pid, role, label, sub, arrays[pid]))
    diffs = _syn_diffs()
    outputs = copy.deepcopy(SYN_OUTPUTS)
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    named = _outputs_json(e.dir / "larmorx-fix").get("named", [])
    by = {x["id"]: x for x in diffs}
    fm = by["lx-fmriprep"]["metrics"]["fmap"]
    glibc, every = _band(arrays, SYN_GLIBC), _band(arrays, SYN_ALL)
    w = _syn_warps(e)
    reasons = {
        "lx-ants": "Every value of both outputs is ANTs 2.6.5's at one thread with correctly "
        "rounded maths followed by sdcflows' steps (the field in Hz from the displacements, its "
        "B-spline resampling onto the EPI grid, the B-spline fit 16 × 16 × 10 mm apart, the "
        "unwarping): LARMORX's warp is ANTs' "
        + _mm(_warp_pair(w, "larmorx", "ants-cr1"))
        + ", at 14 threads (its registration computes ANTs' one-thread sums whatever the thread "
        "count).",
        "fix-lx": "The same bytes in both modes, at 4 threads as at 14 (no correction of the "
        "register changes this call: SDC-16's axis is the phase encoding's world axis here "
        "too).",
        "g1-cr1": "The original against itself, a reference row: ANTs at one thread gives the "
        "same field with glibc's maths.",
        "g1-cr1:differ": "The original against itself, a reference row: glibc's exp and log are "
        "not always correctly rounded, and the registration's iterations grow those last bits "
        "(its warp: " + _mm(_warp_pair(w, "ants-g1", "ants-cr1")) + "):",
        "g4-g1": "The original against itself, a reference row: ANTs at four threads gives the "
        "one-thread field.",
        "g4-g1:differ": "The original against itself, a reference row: at four threads the "
        "Mattes metrics sum their joint histograms per work unit, in another order (its warp: "
        + _mm(_warp_pair(w, "ants-g4a", "ants-g1"))
        + "):",
        "g4b-g4a": "The original against itself, a reference row: ANTs at four threads repeats "
        "itself, every value (its warp too: " + _mm(_warp_pair(w, "ants-g4b", "ants-g4a")) + ").",
        "g4b-g4a:differ": "The original against itself, a reference row: ANTs at four threads "
        "does not repeat itself: above one thread its metrics' sums per work unit change from "
        "run to run (docs/findings/ants-registration.md; its warp: "
        + _mm(_warp_pair(w, "ants-g4b", "ants-g4a"))
        + "):",
        "fmriprep-g4": "The originals against each other, a reference row: fMRIPrep's image's "
        "ANTs gives the same field as ANTs 2.6.5 at the same threads and maths.",
        "fmriprep-g4:differ": "The originals against each other, a reference row: fMRIPrep's "
        "image holds ANTs 2.6.2.dev1 (its own build) where this machine's oracle is ANTs 2.6.5 "
        "built with GCC 14.3; both at four threads with glibc's maths (its warp: "
        + _mm(_warp_pair(w, "fmriprep", "ants-g4a"))
        + "):",
    }
    if fm["max_abs"] <= glibc[0]:
        band, which = glibc, "with glibc's maths, as fMRIPrep's image runs ANTs,"
    else:
        band, which = every, "with glibc's maths or correctly rounded ones"
    within = fm["max_abs"] <= band[0]
    explained = {
        "lx-fmriprep": (
            "within-variability" if within else "open",
            "fMRIPrep's registration ran its image's ANTs 2.6.2.dev1 at four threads with glibc's "
            "maths, LARMORX's gives ANTs 2.6.5's one-thread result with correctly rounded maths "
            "(the first row); sdcflows' steps after the registration give fMRIPrep's own files "
            "from fMRIPrep's own warp, every value (the cache entry's `fmriprep` run), so the "
            "difference is the registration's alone. ANTs 2.6.5's own runs of this call "
            f"{which} differ from each other by up to {band[0]:.3g} Hz (between "
            f"{SYN_WORDS[band[2]]} and {SYN_WORDS[band[3]]}: root mean square "
            f"{band[1]:.3g} Hz; the reference rows below), and fMRIPrep's field lies "
            + ("within that band" if within else "outside that band")
            + f" (at most {fm['max_abs']:.3g} Hz from LARMORX's, root mean square "
            f"{fm['rms']:.3g} Hz; its warp: " + _mm(_warp_pair(w, "larmorx", "fmriprep")) + "):",
        )
    }
    _verdicts(diffs, outputs, same, named, links, reasons, explained)
    at, _k = common.largest_in_slice(ex, "fmriprep", "larmorx", "fmap")
    default = {
        "output": "fmap",
        "plane": "axial",
        "voxel": at,
        "panels": ["epi", "larmorx", "ants-cr1", "fmriprep"],
        "diffs": ["lx-ants", "lx-fmriprep", "g4-g1", "fmriprep-g4"],
        "compare": {"a": "ants-cr1", "b": "larmorx"},
    }
    out = common.finish(
        ctx, ex, diffs, outputs, default=default, window_panel="ants-cr1",
        caption="the axial slice with the most brain, the crosshair where fMRIPrep's field "
        "differs most from LARMORX's in it; choose the output (the field map, the corrected "
        "reference) above the images.",
    )  # fmt: skip
    nc = _stored(_files(e.dir / "larmorx")["coeff"])
    cr = _stored(_files(e.dir / "ants-cr1")["coeff"])
    cshape = " × ".join(str(x) for x in nc.shape[:3])
    same_c = "equal" if nc.tobytes() == cr.tobytes() else "differ from"
    coeff_text = (
        f"the {cshape} coefficients: LARMORX's {same_c} ANTs' one-thread run's; sdcflows' steps "
        "on fMRIPrep's own warp give fMRIPrep's own coefficients, field and corrected reference, "
        "every value"
    )
    secs = {lab: _outputs_json(e.dir / lab).get("seconds", 0.0) for lab, *_ in SYN_RUNS}
    ants_s = {lab: _outputs_json(e.dir / lab).get("ants_seconds", 0.0)
              for lab, *_ in SYN_RUNS if lab.startswith("ants")}  # fmt: skip
    data = _data("ds000228", "sub-pixar008")
    return {
        "id": "syn-ds000228",
        "title": "Fieldmap-less (SyN): sdcflows' syn_sdc_workflow on fMRIPrep's ds000228 estimator",
        "summary": "sdcflows' fieldmap-less estimation (`init_syn_sdc_wf`, fMRIPrep's "
        "`--use-syn-sdc`) on the inputs fMRIPrep 25.2.6 gave it for ds000228's BOLD reference "
        "(a child of 3.5 years; the EPI 20.7° oblique): the reference clipped and registered "
        "with ANTs' SyN to the anatomical reference brought into its space (two Mattes metrics "
        "each, the image and its normalised Laplacian; the deformation restricted to the phase "
        "encoding), the displacements turned into Hz, fitted by sdcflows' B-splines and "
        "applied to the reference. LARMORX runs the whole workflow; ANTs 2.6.5 runs fMRIPrep's "
        "own registration command at one thread with correctly rounded maths (LARMORX's "
        "reference), and with glibc's maths at one thread and twice at four (the original's "
        "own band), each followed by sdcflows' steps; fMRIPrep's own field is placed against "
        "that band.",
        "facts": [
            {
                "label": "Data",
                "text": f"{data['dataset']}, sub-pixar008 ({data['licence']}): the BOLD "
                "reference and the anatomical reference in its space, as fMRIPrep's workflow "
                "was given them; shown inside the EPI's brain masks (fMRIPrep's and the "
                "anatomical one resampled), dilated by 2 voxels, so no face shows",
            },
            {
                "label": "Node",
                "text": f"fMRIPrep 25.2.6, `{info.get('node', '')}` (`--use-syn-sdc`)",
            },
            {
                "label": "Warp",
                "text": "LARMORX's registration against ANTs 2.6.5 at one thread with correctly "
                "rounded maths: "
                + _mm(_warp_pair(w, "larmorx", "ants-cr1"))
                + "; glibc's maths at one thread: "
                + _mm(_warp_pair(w, "ants-g1", "ants-cr1"))
                + "; four threads against one: "
                + _mm(_warp_pair(w, "ants-g4a", "ants-g1"))
                + "; four threads, run to run: "
                + _mm(_warp_pair(w, "ants-g4b", "ants-g4a"))
                + "; fMRIPrep's own warp against LARMORX's: "
                + _mm(_warp_pair(w, "larmorx", "fmriprep")),
            },
            {"label": "Coefficients", "text": coeff_text},
            {
                "label": "Programs",
                "text": "ANTs 2.6.5's antsRegistration (the GCC 14.3 build) through the oracle "
                "cache, the four-thread runs never cached; sdcflows' steps after it by LARMORX's "
                "port (= sdcflows' nodes on these inputs, the records' `syn-sdc` case); LARMORX "
                f"at commit `{info['larmorx_commit'][:12]}`, fMRIPrep's image's numpy 2.2.6 "
                "for the clip",
            },
            {
                "label": "Time",
                "text": f"LARMORX's whole workflow {secs['larmorx']:.0f} s at 14 threads; "
                f"ANTs' registration {ants_s['ants-cr1']:.0f} s and {ants_s['ants-g1']:.0f} s "
                f"at one thread, {ants_s['ants-g4a']:.0f} s and {ants_s['ants-g4b']:.0f} s at "
                "four (the six runs side by side; informal)",
            },
            {"label": "Grid", "text": common.describe_grid(img, grid.shape)},
        ],
        **out,
        "chain": None,
        "groups": [
            ["main", "LARMORX against ANTs 2.6.5 (one thread, correctly rounded maths)"],
            ["fmriprep", "LARMORX against fMRIPrep's own files"],
            ["check", "LARMORX against itself: modes and threads"],
            ["variability", "ANTs against itself: maths, threads, runs, builds"],
        ],
        "metrics_note": f"Δ is A − B over all {int(np.prod(img.shape[:3])):,} voxels of each "
        "output file, not only those shown (the field extrapolated over the whole grid).",
        "provenance": common.provenance(info, f"{TOOL}/{e.case}", [data]),
    }


# ------------------------------------------------------------------------------ the tables


def _records(ctx: core.Context) -> list[tuple[records.Record, str, str, str]]:
    return [(records.load(ctx.repo, n), suite, tier, env) for n, suite, tier, env in RECORDS]


STEP_PREFIX = "the originals' own step"


def _step_lines(r: dict) -> dict[str, str]:
    """A case's step comparisons: name (without the releases) -> identical/changed/varying."""
    out = {}
    for k, v in r.get("measured", {}).items():
        if k.startswith(STEP_PREFIX):
            out[k.split(": ", 2)[-1]] = v
    return out


def _records_table(ctx: core.Context, recs) -> dict:
    rows = []
    for rec, suite, tier, env in recs:
        s = rec.summary
        lines = sum(r["checks_total"] for r in rec.results)
        step = [v for r in rec.results for v in _step_lines(r).values()]
        rows.append(
            {
                "head": f"[{rec.name}](repo:docs/validation/{rec.name}.md)",
                "cells": [
                    common.cell(suite),
                    common.cell(tier),
                    common.cell(env),
                    common.cell(len(rec.results), "count"),
                    common.cell(s.get("pass", 0), "count"),
                    common.cell(lines, "count"),
                    common.cell(f"{len(step):,} / {step.count('changed'):,}" if step else "–"),
                ],
                "verdict": "bit-identical" if s.get("pass", 0) == len(rec.results) else "open",
            }
        )
    return common.table(
        "records",
        "The records of the part: every case passes",
        [
            "Record",
            "Suite",
            "Tier",
            "Environment",
            {"label": "Cases", "num": True},
            {"label": "Pass", "num": True},
            {"label": "Checks", "num": True},
            "The originals' step: compared / changed",
        ],
        rows,
        note="Each record runs the investigation's check scripts (some of their sections) as "
        "separate processes of the environment's interpreter, from a clean commit: one check per "
        "summary line a script prints (its cases, all identical), its final line and its exit "
        "status. newest: numpy 2.5.4 (OpenBLAS 0.3.34), SciPy 1.18.1 (oracles/python-newest, "
        "LARMORX's reference); shared: numpy 2.3.5 (OpenBLAS 0.3.30), SciPy 1.15.3 (the shared "
        ".venv). The full tier holds the whole workflows and the BIDS app's whole runs "
        "(minutes to an hour each). The originals' step: each comparison of the library scripts "
        "with the originals' outputs digested in both environments; every change has a known "
        "reason (the table below).",
    )


def _step_table(ctx: core.Context, recs) -> dict:
    from larmorx_validation.parity import sdcflows_scripts as ss

    rows = []
    total = changed = 0
    for rec, suite, _tier, _env in recs:
        for r in rec.results:
            lines = _step_lines(r)
            if not lines:
                continue
            total += len(lines)
            ch = [k for k, v in lines.items() if v == "changed"]
            changed += len(ch)
            if not ch:
                continue
            pats = ss.REASONS.get(f"{suite}/{r['case']}", {})
            keys = sorted({next((v for p, v in pats.items() if re.search(p, k)), "?") for k in ch})
            rows.append(
                {
                    "head": f"`{suite}/{r['case']}`",
                    "cells": [
                        common.cell(len(lines), "count"),
                        common.cell(len(ch), "count"),
                        common.cell(", ".join(keys)),
                    ],
                }
            )
    rows.insert(
        0,
        {
            "head": "every case",
            "cells": [common.cell(total, "count"), common.cell(changed, "count"), common.cell("–")],
        },
    )
    reasons = "; ".join(f"{k}: {v.split(';')[0].split(':')[0]}" for k, v in ss.CHANGES.items()
                        if k not in ("SCI-03",))  # fmt: skip
    return common.table(
        "step",
        "The originals' own step: numpy 2.3.5 / SciPy 1.15.3 → numpy 2.5.4 / SciPy 1.18.1",
        [
            "Case",
            {"label": "Comparisons", "num": True},
            {"label": "Changed", "num": True},
            "Reasons",
        ],
        rows,
        note="The records' step (python_versions): each comparison of a library script whose "
        "originals' outputs are digested, in the shared releases (two runs: none varies) and in "
        "the newest. Every change has a known reason, LARMORX following each release: "
        + reasons
        + " (sdcflows_scripts.CHANGES, with their sources). The workflow scripts carry no step: "
        "their originals (sdcflows 2.17.1, niworkflows, nipype) are the same in both "
        "environments, and the libraries' step under them is the library scripts'.",
    )


def _app_table(ctx: core.Context, recs) -> dict:
    from larmorx_validation.parity.sdcflows_scripts import APP_RUNS

    by = {(rec.name, r["case"]): r for rec, *_ in recs for r in rec.results}
    bench = _bench(ctx)
    times = {r["dataset"]: r for a in bench["app"] for r in a["rows"]}
    rows = []
    for short, run, what in APP_RUNS:
        cells = [common.cell(what)]
        for name in ("sdcflows-estimation-full", "sdcflows-estimation-app-shared"):
            r = by.get((name, f"app-run-{short}"))
            cells.append(common.cell(f"{r['checks_passed']}/{r['checks_total']}" if r else "–"))
        t = times.get(run)
        cells += [common.cell(t["sdcflows"], "sig") if t else common.cell("–"),
                  common.cell(t["larmorx_1"], "sig") if t else common.cell("–")]  # fmt: skip
        ok = all(by.get((n, f"app-run-{short}"), {}).get("status") == "pass"
                 for n in ("sdcflows-estimation-full", "sdcflows-estimation-app-shared"))  # fmt: skip
        rows.append({"head": f"`{run}`", "cells": cells,
                     "verdict": "bit-identical" if ok else "open"})  # fmt: skip
    return common.table(
        "app-runs",
        "sdcflows' BIDS app run whole on its eight oracle datasets",
        [
            "sdcflows' run",
            "Estimators",
            "Checks, newest",
            "Checks, shared",
            {"label": "sdcflows (s)", "num": True},
            {"label": "LARMORX (s)", "num": True},
        ],
        rows,
        note="The records' cases `app-run-*` (check_app.py --sections run): sdcflows 2.17.1's own "
        "BIDS app and LARMORX's (`larmorx sdcflows sdcflows`, reproduce) at one thread on the "
        "datasets of sdcflows' own oracle runs, with `--no-fmapless`: the exit statuses, the "
        "command-line logger's lines, the list of derivative files and every file byte for byte "
        "(NIfTI decompressed, the JSON sidecars), the logs folder and the settings file, in "
        "both environments. The times: the benchmark (docs/benchmarks/sdcflows-estimation.md; "
        "the originals' programs run fresh, LARMORX at one thread), where measured.",
    )


def _stage_table(ctx: core.Context) -> dict:
    """fMRIPrep's own derivatives against LARMORX's stage as fMRIPrep ran it: per run, the
    files identical, those whose data are and only ``descrip`` differs, those after N4."""
    rows = []
    tot = [0, 0, 0, 0]
    for frun in STAGE_RUNS:
        lines = _stage_lines(ctx, frun)
        files = [ln for ln in lines if ".nii.gz:" in ln]
        same = sum(ln.endswith(": identical") for ln in files)
        descrip = sum("data identical; descrip" in ln for ln in files)
        n4 = sum("data different" in ln for ln in files)
        secs = next((re.search(r"\) (\d+) s$", ln).group(1) for ln in lines  # type: ignore[union-attr]
                     if "estimate_fieldmaps" in ln), "?")  # fmt: skip
        for i, v in enumerate((len(files), same, descrip, n4)):
            tot[i] += v
        rows.append(
            {
                "head": f"`{frun}`",
                "cells": [
                    common.cell(len(files), "count"),
                    common.cell(same, "count"),
                    common.cell(descrip, "count"),
                    common.cell(n4, "count"),
                    common.cell(f"{secs} s"),
                ],
                "verdict": "bit-identical" if n4 == 0 else "within-variability",
            }
        )
    rows.insert(0, {"head": "every run", "cells": [common.cell(v, "count") for v in tot]
                    + [common.cell("–")]})  # fmt: skip
    return common.table(
        "stage",
        "fMRIPrep's own field-map derivatives, against LARMORX's stage run as fMRIPrep ran it",
        [
            "fMRIPrep's run",
            {"label": "NIfTI files", "num": True},
            {"label": "identical", "num": True},
            {"label": "data identical, FSL's build string in descrip", "num": True},
            {"label": "after an N4 node", "num": True},
            "LARMORX's stage",
        ],
        rows,
        note="The cache entry `stage-fmriprep` (check_fmap_stage.py --sections fmriprep, ALL OK): "
        "larmorprepx's estimate_fieldmaps with fMRIPrep 25.2.6's options, sdcflows 2.15's "
        "PEPOLAR wiring, its image's numpy 2.2.6 and SciPy 1.15.2, LAPACK 3.12.0 in the data "
        "sinks and the replicas, against the run's own derivatives in `out/`. A file is "
        "identical (data and header, decompressed), or its data are and its header differs "
        "only by FSL's build string in `descrip` (fMRIPrep's image's FSL 2501.4, the replicas' "
        "FSL 6.0.7.23 2601.0: PRELUDE's and TOPUP's outputs and what copies their headers), or "
        "it follows an N4 node that ANTs 2.6.5 reproduces at the node's 8 threads with glibc's "
        "maths (fMRIPrep's file) and at one thread with correctly rounded maths (LARMORX's): at "
        "most one stored value apart in a few hundred voxels (the magnitude and EPI references, "
        "and once a brain mask by 3 voxels). Every difference is explained; the field maps and "
        "the coefficients are fMRIPrep's in every run.",
    )


def _memory_table(ctx: core.Context) -> dict:
    e = memory_entry(ctx)
    peaks = e.info()["peaks_mib"]
    labels = {
        "masker": "sdcflows' brain_masker, ds002603's magnitude (128 × 128 × 128 padded)",
        "clip": "niworkflows' IntensityClip, ds002603's merged magnitude",
        "ravg": "niworkflows' RobustAverage's own steps, ds004958's PEPOLAR series",
        "recon": "fMRIPrep's ReconstructFieldmap, ds005454 onto its T1w grid",
    }
    rows = []
    for name, p in peaks.items():
        o = p["original"]
        worst = max(p["larmorx-reproduce"], p["larmorx-fix"])
        rows.append(
            {
                "head": labels.get(name, name),
                "cells": [
                    common.cell(round(o), "count"),
                    common.cell(round(p["larmorx-reproduce"]), "count"),
                    common.cell(round(p["larmorx-fix"]), "count"),
                    common.cell(worst / o, "sig"),
                ],
            }
        )
    return common.table(
        "memory",
        "Peak memory (MiB): the memory rule's calls of the part, one thread",
        [
            "Call",
            {"label": "The original", "num": True},
            {"label": "LARMORX, reproduce", "num": True},
            {"label": "LARMORX, default", "num": True},
            {"label": "LARMORX ÷ the original", "num": True},
        ],
        rows,
        note="Each call in a process of its own (validation/investigations/sdcflows-fieldmap-b/"
        "memory_call.py, the calls of tests/parity/test_memory_peak.py), its peak resident "
        "memory from /usr/bin/time -v, in the newest environment at one thread. The rule: at "
        "most 10 % above the original's peak plus 8 MiB. sdcflows' brain_masker holds the "
        "random walker's Laplacian several times (SciPy's sparse matrices, scikit-image's "
        "copies), LARMORX once; fMRIPrep's ReconstructFieldmap holds nitransforms' sampling "
        "grids in float64.",
    )


def _bench(ctx: core.Context) -> dict:
    return json.loads((ctx.repo / BENCH).read_text(encoding="utf-8"))


def _speed(ctx: core.Context) -> dict:
    b = _bench(ctx)
    series = [
        {"id": "original|1", "label": "the original, 1 thread", "mark": "original"},
        {"id": "larmorx|1", "label": "LARMORX, 1 thread", "mark": "larmorx"},
        {"id": "larmorx|14", "label": "LARMORX, 14 threads", "mark": "larmorx-open"},
    ]
    names = {
        "masker-ds002603": "brain_masker: ds002603's magnitude",
        "clip-ds002603": "IntensityClip: ds002603's magnitude",
        "ravg-ds004958": "RobustAverage's own steps: ds004958's PEPOLAR series",
        "recon-ds005454": "ReconstructFieldmap: ds005454 onto its T1w",
    }
    rows = []
    for r in b["calls"]["rows"]:
        sec = {"original|1": r["original"], "larmorx|1": r["larmorx_1"],
               "larmorx|14": r["larmorx_14"]}  # fmt: skip
        rows.append({"label": names.get(r["call"], r["call"]), "input": r["call"], "seconds": sec,
                     "speedup": r["original"] / r["larmorx_1"]})  # fmt: skip
    for a in b["app"]:
        for r in a["rows"]:
            sec = {"original|1": r["sdcflows"], "larmorx|1": r["larmorx_1"]}
            rows.append({"label": f"the BIDS app: {r['dataset']}", "input": r["dataset"],
                         "seconds": sec, "speedup": r["sdcflows"] / r["larmorx_1"]})  # fmt: skip
    nan = b["masker_nan"]["relay28_build"]["rows"][0]
    rows.append({"label": "random walker that does not converge (random testing's case)",
                 "input": nan["case"],
                 "seconds": {"original|1": nan["original"], "larmorx|1": nan["larmorx_1"]},
                 "speedup": nan["original"] / nan["larmorx_1"]})  # fmt: skip
    axis, ticks = common.ticks_log([v for row in rows for v in row["seconds"].values()], " s")
    m = b["calls"]["machine"]
    return {
        "title": "Seconds per call (median of 3), log scale; the factor is LARMORX's speed-up "
        "over the original at one thread",
        "axis": axis,
        "ticks": ticks,
        "series": series,
        "rows": rows,
        "note": f"From the benchmark report `{BENCH}` (commit {m.get('commit', '?')}, "
        f"{m.get('cpus', '?')} logical CPUs, the newest environment, OPENBLAS_NUM_THREADS=1 as "
        "fMRIPrep's nodes; every output byte-identical). The originals have no thread option on "
        "these calls. The BIDS app: one run each (minutes), sdcflows' programs run fresh (FSL's "
        "topup, ANTs' N4, AFNI's 3dvolreg), LARMORX's replicas and ports at one thread. The "
        "random walker that does not converge: a NaN system from its first iteration (an image "
        "whose grey closing is 0 everywhere), SciPy's conjugate gradients run to their limit; "
        "LARMORX stops with SciPy's solution once nothing can change it (210 s before that "
        "change).",
    }


def trial(ctx: core.Context) -> tuple[list[dict], dict]:
    """Random testing's two trials of the part and its triage: two tables."""
    import tomllib

    out_rows, prov = [], []
    head = ["", *(f"{t} ({'AVX-512 off' if 'noavx' in t else 'numpy default'})" for t in TRIALS)]
    data = []
    for t in TRIALS:
        path = common.oracles() / "difftest" / t / "summary.json"
        raw = path.read_bytes()
        data.append(json.loads(raw))
        prov.append({"trial": f"oracles/difftest/{t}/summary.json",
                     "sha256": hashlib.sha256(raw).hexdigest()})  # fmt: skip

    def row(label, fn):
        out_rows.append({"head": label, "cells": [fn(s) for s in data]})

    row(
        "Random cases, each compared with the original",
        lambda s: common.cell(int(s["cases"]), "count"),
    )
    for tool in TRIAL_TOOLS:

        def cell(s, tool=tool):
            t = s["tools"].get(tool, {})
            im = t.get("implementations", {}).get("port", {})
            text = f"{t.get('cases', 0):,} cases: {im.get('agree', 0):,} agree"
            for k in ("differ", "skipped", "timeout", "notice"):
                if im.get(k):
                    text += f", {im[k]:,} {k}"
            return common.cell(text + f"; {t.get('options_covered', 0)} options drawn")

        row(tool, cell)
    row("length (minutes)", lambda s: common.cell(round(s["elapsed_s"] / 60, 1), "sig"))
    row("seed, LARMORX commit", lambda s: common.cell(f"{s['seed']}, {s['larmorx_commit']}"))
    table = common.table(
        "trial",
        "Random testing: the part's two trials",
        head,
        out_rows,
        note="Differential random testing (docs/validation/difftest.md): random inputs and "
        "options for SciPy's ndimage and scikit-image (filters, morphology, labels, the random "
        "walker, Otsu), numpy's statistics, niworkflows' helpers with nipype's command lines, "
        "and sdcflows' estimation steps, masks, readout times and derivatives; the originals "
        "in-process, LARMORX in its reproduce mode compared output by output, its default mode "
        "checked by D13's rule. `skipped`: calls the descriptor does not compare (an input the "
        "original refuses in a way it cannot run twice, or a program not drawn); `timeout`: "
        "sdcflows' brain_masker cases whose random walker does not converge, slower than the "
        "case's limit in the original (the benchmark's last row): replayed, every one agrees. "
        "The second trial turns numpy's AVX-512 kernels off (NPY_DISABLE_CPU_FEATURES).",
    )
    tri = tomllib.loads(
        (ctx.repo / "validation" / "difftest" / "triage.toml").read_text(encoding="utf-8")
    )
    trows = []
    for d in tri.get("difference", []):
        if d.get("tool") not in TRIAL_TOOLS:
            continue
        trows.append(
            {
                "head": d.get("status", ""),
                "cells": [
                    common.cell(d.get("tool", "")),
                    common.cell(common.clip(d.get("explanation", ""), 420)),
                    common.cell(d.get("defect", "–")),
                ],
                "sub": d.get("link", ""),
            }
        )
    triage = common.table(
        "triage",
        "Random testing: every difference of the part's runs, triaged",
        ["Status", "Tool", "What it was", "Register"],
        trows,
        note="validation/difftest/triage.toml (the field maps' part B, 2026-11-09): each "
        "difference of the short runs before the trials and of the trials themselves, a "
        "LARMORX mistake fixed with a permanent case, a defect of the original registered by "
        "the rules A to D, or a slow original.",
    )
    return [table, triage], {"trials": prov}


def _defect_rows(ctx: core.Context) -> list[tuple[str, ...]]:
    """The findings' table of part B's register entries: (id, original, class, defect,
    fMRIPrep's calls, LARMORX)."""
    text = (ctx.repo / FINDINGS).read_text(encoding="utf-8")
    pat = r"^\| ((?:SDC|PIPE|SCI|SKI)-\d+) \| (.*?) \| (\S+) \| (.*?) \| (.*?) \| (.*?) \|\s*$"
    return [tuple(g.strip() for g in m.groups()) for m in re.finditer(pat, text, re.M)]


def _corrections_table(ctx: core.Context, links: dict) -> dict:
    from larmorx._defects import defects

    switch = {d.id: d for d in defects()}
    rows = []
    for i, tool, cls, what, users, today in _defect_rows(ctx):
        sw = switch.get(i)
        rows.append(
            {
                "head": f"[{i}](repo:{links.get(i, FINDINGS)})",
                "cells": [
                    common.cell(tool.replace("`", "")),
                    common.cell(cls),
                    common.cell(what.replace("`", "")),
                    common.cell(users.replace("`", "")),
                    common.cell(today.replace("`", "")),
                    common.cell("yes" if sw else "–"),
                ],
            }
        )
    return common.table(
        "corrections",
        "The originals' defects found by the part (SDC-12 to SDC-46, PIPE-18 to PIPE-40, "
        "SCI-06 to SCI-32, SKI-01 to SKI-04)",
        [
            "Entry",
            "Original",
            "Class",
            "What the original does",
            "fMRIPrep's calls",
            "What LARMORX does",
            "Switch",
        ],
        rows,
        note="Each decided by rules A to D (A: corrected by default; B: a clear error; C: kept "
        "and documented; D: measured first), with `upstream_defects='reproduce'` "
        "(LARMORX_UPSTREAM_DEFECTS=reproduce) giving the original's behaviour where LARMORX has "
        "a switch (larmorx_core::defects, the Switch column); crashes, reads out of bounds and "
        "nondeterminism are never reproduced. The class is the register's (a: crash or undefined "
        "behaviour; b: nondeterminism; c: a wrong result; e: a command-line quirk; f: a message; "
        'c?: questionable). The findings: docs/findings/sdcflows-estimation.md (its table "Part '
        "B's register entries at a glance\") and docs/findings/scipy-ndimage-filters.md.",
    )


# --------------------------------------------------------------------------- the cases


def _cases(ctx: core.Context, recs) -> list[dict]:
    from larmorx_validation.parity.sdcflows_scripts import SUITES

    out = []
    for rec, suite, _tier, env in recs:
        for r in rec.results:
            spec = SUITES.get(suite, {}).get(r["case"])
            cid = r["case"] if env == "newest" else f"{env}/{r['case']}"
            v = common.outcome_verdict(r)
            if v is None:
                step = _step_lines(r)
                ch = sum(x == "changed" for x in step.values())
                v = (
                    "bit-identical",
                    f"Every one of its {r['checks_total']} checks: the outputs bit-identical to "
                    "the originals' (or fMRIPrep's own files), their messages, warnings and exit "
                    "statuses too"
                    + (f"; the originals' step: {len(step)} comparisons, {ch} changed with a "
                       "known reason" if step else "")
                    + ".",
                )  # fmt: skip
            command = ""
            if spec:
                command = "python -P validation/investigations/sdcflows-fieldmap-b/" + " ".join(
                    [spec.script, *spec.args]
                )
            out.append(
                {
                    "id": cid,
                    "category": r["category"],
                    "description": (spec.description if spec else "")
                    + ("" if env == "newest" else f" (the {env} environment)"),
                    "command": command,
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


# -------------------------------------------------------------------------------- export


def _provenance(e: fcache.Entry) -> dict:
    """An entry's provenance without its runs' temporary folder (``TMPDIR``, made for the
    computation and gone)."""
    out = fcache.provenance(e)
    for c in out["commands"]:
        c["env"].pop("TMPDIR", None)
    return out


def export(ctx: core.Context) -> dict:
    recs = _records(ctx)
    links = _findings_links(ctx.repo)
    ents = entries(ctx)
    cases = _cases(ctx, recs)
    examples = [
        _example_app(ctx, "ds006736", links),
        _example_app(ctx, "ds005752", links),
        _example_syn(ctx, links),
        _example_masker(ctx, links),
        _example_epi(ctx, links),
    ]
    for ex in examples:
        _check_default(ex)
    trial_tables, tprov = trial(ctx)
    tables = [
        _records_table(ctx, recs),
        _app_table(ctx, recs),
        _stage_table(ctx),
        _epi_table(ctx),
        _corrections_table(ctx, links),
        _step_table(ctx, recs),
        _memory_table(ctx),
        *trial_tables,
    ]
    bench = ctx.repo / BENCH
    doc = {
        "schema": 1,
        "tool": TOOL,
        "title": "sdcflows' field-map estimation: PEPOLAR, direct and fieldmap-less, the masks "
        "and the BIDS app",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": [x for rec, *_ in recs for x in rec.source()],
            "benchmarks": [
                {
                    "path": BENCH,
                    "sha256": hashlib.sha256(bench.read_bytes()).hexdigest(),
                    "larmorx_commit": _bench(ctx)["calls"]["machine"].get("commit"),
                }
            ],
            "cache": [_provenance(x) for x in ents],
            "difftest": tprov,
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cases,
                "port",
                "LARMORX (port), against sdcflows 2.17.1, niworkflows, nipype, SciPy, "
                "scikit-image, numpy and fMRIPrep 25.2.6's own files",
                "port",
                "docs/validation/sdcflows-estimation.md",
                "validated",
            )
        ],
        "cases": cases,
        "cases_total": len(cases),
        "speed": _speed(ctx),
        "tables": tables,
        "examples": examples,
    }
    return present(doc)


# ---------------------------------------------------------------------- the presentation

TRIAL_DATE = TRIALS[0][:10]


def _page(program: str = "", software: str = "sdcflows", version: str = "2.17.1"):
    from larmorx_validation.evidence import present as present_

    return present_.Page(software, program, version, part="estimation", texts=TEXTS)


#: The page's own rewordings (applied before the guide's words). In this page's texts LARMORX
#: names the port (the replica) but where it names the project's replicas underneath, its
#: reference setup or its switch; the replicas of the libraries are replicas; the comparisons
#: of the originals with themselves by what they are; the part's own words by what they mean.
TEXTS = [
    (r"\bLARMORX's replicas and ports\b", "LARMORX's replicas"),
    (r"\bthe replicas and ports\b", "the replicas"),
    (r"\bLARMORX's port of it\b", "LARMORX's replica of it"),
    (r"\bby LARMORX's port\b", "by the replica"),
    (r"\ba LARMORX mistake\b", "a mistake of the replica's"),
    # the rows named by what they compare: the guide's order moves them
    (r"\bLARMORX follows both \(the first two rows\)", "the replica follows both (its rows "
     "against the original and against fMRIPrep's output)"),
    (r"\bLARMORX follows both \(the row above\)", "the replica follows both (its row against "
     "fMRIPrep's output)"),
    (r"\(the first row\)", "(its row against the original)"),
    (r"; the reference rows below\)", "; the original's rows against itself)"),
    (r"\bLARMORX reproduce ([\d,]+), default ([\d,]+)", r"the replica in `reproduce` \1, in `fix` \2"),
    (r"\bits own oracle run\b", "its own run"),
    (r"\bsdcflows' own oracle runs\b", "sdcflows' own runs"),
    (r"\b(\d+) of (\d+) oracle references\b", r"\1 of \2 references from fMRIPrep's runs"),
    (r"\bwhere this machine's oracle is ANTs 2\.6\.5 built with\b", "where the repository's build "
     "is ANTs 2.6.5, made with"),
    (r"\bLARMORX in both modes\b", "the replica in `fix` and in `reproduce`"),
    (r"^seed, LARMORX commit$", "seed, the replica's commit"),
    (r"\bThe default masks larger\b", "The masks of `fix` larger"),
    (r"\bThe second trial\b", "The second run"),
    (r"\bin the trial\b", "in the run"),
    (r"(?<![\w`/-])LARMORX(?!'s replicas|'s reference|'s replica of|_| has a switch)", "larmorx"),
    (r"\blarmorx's reference\b", "LARMORX's reference"),
    (r"\bthe default mode's differ\b", "those of `fix`, the default, differ"),
    (r"^The original against itself, a reference row: ", "For reference, the original against "
     "itself: "),
    (r"^The originals against each other, a reference row: ", "For reference, the originals "
     "against each other: "),
    (r"^The version step, a reference row \(the originals against each other\): ",
     "For reference, the step between the originals' versions: "),
    (r"^The libraries' step, a reference row \(the original against itself\): ",
     "For reference, the originals' own step between releases of the libraries: "),
    (r"\bin both modes \(no correction of the register applies to this call\)",
     "in `fix` and in `reproduce` (no correction by default applies to this call)"),
    (r"\bits default mode checked by D13's rule\b", "its default, `fix`, checked by the rule of "
     "the corrections by default"),
    (r"\bD13's rule: clamped\b", "the rule of the corrections by default: clamped"),
    (r"^the part's contract: ", "the per-program contract for these families: "),
    (r"\bevery public function of the part's families\b", "every public function"),
    (r"\(the field maps' part B, (\d{4}-\d\d-\d\d)\)", r"(\1)"),
    (r"\bthe part's runs\b", "the runs"),
    (r"\bthe short runs before the trials and of the trials themselves\b", "the short runs before "
     "the two runs and of the two runs themselves"),
    (r"^Differential random testing \(", "The replica's random testing ("),
]  # fmt: skip

_SAME_STEP = r"\b(?:reference row|the originals' own step)\b"


def _ids(ex: dict, i: str) -> list[str]:
    """The register's entries a comparison's reason names, in its order."""
    return list(dict.fromkeys(re.findall(r"\b[A-Z]+-\d+\b", _diff(ex, i)["reason"] or "")))


def _and(xs: list[str]) -> str:
    return xs[0] if len(xs) == 1 else ", ".join(xs[:-1]) + " and " + xs[-1]


def _shows_app(ex: dict) -> tuple[str, list[str]]:
    """sdcflows' BIDS app whole: every output and file sdcflows', fMRIPrep's own derivatives
    within the original's variability, the corrections by default."""
    _want(ex, lx_sdc="bit-identical", lxf_fmriprep="within-variability", lx14_lx1="bit-identical",
          fix_lx14="d13", sdc4_sdc1=None, fmriprep_sdc=None)  # fmt: skip
    _found(ex, "lx-sdc", r"\(and every derivative file, byte for byte")
    nodes = _found(ex, "lxf-fmriprep", r"fMRIPrep's N4 node ran at (\d+) threads with glibc's "
                   r"exp and log, LARMORX's at one thread with correctly rounded maths")  # fmt: skip
    fixed = _ids(ex, "fix-lx14")
    n = _NUMBER[len(ex["outputs"])]
    return (
        f"In `reproduce` the replica's BIDS app gives every value of sdcflows 2.17.1's {n} "
        "outputs and every derivative file byte for byte, at 1 and 14 threads alike, with "
        "LARMORX's replicas of the programs underneath. Run as fMRIPrep ran the stage, it holds "
        "fMRIPrep's own derivatives within the original's own variability: fMRIPrep's N4 ran at "
        f"{nodes.group(1)} threads with glibc's maths, the replica's at one thread with correctly "
        f"rounded maths. In `fix`, its default, it corrects {_and(fixed)}."
    ), ["lx-sdc"]


def _shows_syn(ex: dict) -> tuple[str, list[str]]:
    """The fieldmap-less estimation: ANTs' one-thread registration with correctly rounded maths
    byte for byte, fMRIPrep's own field within ANTs' own spread."""
    _want(ex, lx_ants="bit-identical", lx_fmriprep="within-variability", fix_lx="d13",
          g1_cr1=None, g4_g1=None, g4b_g4a=None, fmriprep_g4=None)  # fmt: skip
    warp = _found(ex, "lx-ants", r"warp is ANTs' byte for byte \(([\d,]+) values\), at (\d+) "
                  r"threads").groups()  # fmt: skip
    band, near = _found(ex, "lx-fmriprep", r"differ from each other by up to ([\d.]+) Hz .*?"
                        r"\(at most ([\d.]+) Hz from LARMORX's").groups()  # fmt: skip
    fixed = _ids(ex, "fix-lx")
    return (
        "In `reproduce` the replica gives every value of ANTs 2.6.5's registration at one thread "
        "with correctly rounded maths, followed by sdcflows' steps, at "
        f"{warp[1]} threads: its warp is ANTs' byte for byte ({warp[0]} values). fMRIPrep's own "
        "field, made by its image's ANTs 2.6.2.dev1 at four threads with glibc's maths, lies "
        f"within ANTs' own spread on this call: ANTs' runs differ from each other by up to {band} "
        f"Hz, and fMRIPrep's field is at most {near} Hz from the replica's. In `fix`, its "
        f"default, the replica corrects {_and(fixed)}."
    ), ["lx-ants"]


def _shows_masker(ex: dict) -> tuple[str, list[str]]:
    _want(ex, lx_sdc="bit-identical", lxf_fmriprep="bit-identical", lx14_lx1="bit-identical",
          fmriprep_sdc=None)  # fmt: skip
    _found(ex, "lx14-lx1", r"no correction of the register applies to this call")
    libs = _found(ex, "lxf-fmriprep", r"With (numpy [\d.]+)'s sums and (SciPy [\d.]+)'s").groups()
    n = _NUMBER[len(ex["outputs"])]
    return (
        f"In `reproduce` the replica gives every value of sdcflows 2.17.1's {n} outputs, at 1 and "
        "14 threads and in `fix` alike, and with fMRIPrep's libraries "
        f"({libs[0]}, {libs[1]}) it writes fMRIPrep's own masker files, every value. No "
        "correction by default changes this call."
    ), ["lx-sdc"]


def _shows_epi(ex: dict) -> tuple[str, list[str]]:
    _want(ex, lx_sdc="bit-identical", fix_lx="d13")
    m = _found(ex, "fix-lx", r"sdcflows' mask holds ([\d,]+) voxels \(Dice ([\d.]+) with "
               r"fMRIPrep's brain mask\), the corrected one ([\d,]+) \(Dice ([\d.]+)\)")  # fmt: skip
    a, da, b, db = m.groups()
    fixed = _ids(ex, "fix-lx")
    return (
        "In `reproduce` the replica gives sdcflows 2.17.1's mask, every voxel. In `fix`, its "
        f"default, it corrects SciPy's watershed ({_and(fixed)}): the object's region no longer "
        f"stops early, and the mask grows from {a} to {b} voxels, its Dice coefficient with "
        f"fMRIPrep's brain mask from {da} to {db}."
    ), ["lx-sdc"]


#: "What this shows" of each worked example (the writing guide, section 4), from its own
#: verdicts and reasons; beside it the replica's verdict against the original.
SHOWS = {
    "app-ds006736": _shows_app,
    "app-ds005752": _shows_app,
    "syn-ds000228": _shows_syn,
    "masker-ds002603": _shows_masker,
    "epi-mask-ds003592": _shows_epi,
}
#: Each example's program (the BIDS app's examples: the page's sdcflows; the fieldmap-less
#: estimation's original is ANTs' registration, sdcflows' steps after it).
PAGES = {
    "syn-ds000228": ("antsRegistration", "ANTs", "2.6.5"),
    "masker-ds002603": ("brain_masker", "sdcflows", "2.17.1"),
    "epi-mask-ds003592": ("epi_mask", "sdcflows", "2.17.1"),
}
#: Comparisons the shapes name otherwise, per example.
DIFFS = {
    "app-ds006736": {
        "fmriprep-sdc": ("fMRIPrep's output − sdcflows 2.17.1, 1 thread",
                         "the original against itself: versions 2.15.1 and 2.17.1, with their "
                         "libraries"),
    },
    "app-ds005752": {
        "fmriprep-sdc": ("fMRIPrep's output − sdcflows 2.17.1, 1 thread",
                         "the original against itself: versions 2.15.1 and 2.17.1, with their "
                         "libraries"),
    },
    "syn-ds000228": {
        "lx-ants": ("replica (14 threads) − ANTs antsRegistration 2.6.5, 1 thread, correctly "
                    "rounded maths", "the replica against the original: correctly rounded maths"),
        "g4b-g4a": ("ANTs antsRegistration 2.6.5, 4 threads, glibc's maths: the second run − the "
                    "first", "the original against itself: two runs of the same command"),
        "fmriprep-g4": ("fMRIPrep's output (ANTs 2.6.2.dev1) − ANTs antsRegistration 2.6.5, both "
                        "at 4 threads with glibc's maths",
                        "the original against itself: fMRIPrep's build and the repository's"),
    },
    "masker-ds002603": {
        "lx14-lx1": ("replica: 14 threads, fix − 1 thread, reproduce",
                     "the replica against itself: 1 and 14 threads, fix and reproduce"),
        "fmriprep-sdc": ("fMRIPrep's output − sdcflows brain_masker 2.17.1, the newest releases",
                         "the original against itself: fMRIPrep's libraries and the newest "
                         "releases"),
    },
}  # fmt: skip
#: The record's threshold, in each example's metrics note (the records' own words).
THRESHOLD = (
    "The records' threshold: in `reproduce`, every output bit-identical to the originals' (or "
    "fMRIPrep's own files), their messages, warnings and exit statuses too."
)
#: The case categories (the records' check scripts), as the page names them.
CATEGORIES = {
    "check_app": "sdcflows' BIDS app",
    "check_skimage": "scikit-image and the brain masker",
    "check_pepolar": "PEPOLAR",
    "check_epi_mask": "epi_mask and the watershed",
    "check_match_histogram": "histogram matching",
    "check_syn_preprocessing": "the fieldmap-less preprocessing",
    "check_syn_sdc": "the fieldmap-less estimation",
    "check_estimators": "the estimators of a BIDS dataset",
    "check_derivatives": "the derivatives",
    "check_ndimage": "SciPy's ndimage",
    "check_numpy": "numpy",
    "check_scipy_inv": "SciPy's inv",
    "check_regrid": "regridding and Gaussian filters",
    "check_merge": "niworkflows' IntraModalMerge",
    "check_masker_steps": "the brain masker's steps",
    "check_workflows": "the field-map workflows",
    "relay22/audit_contract": "the per-program contract (these families)",
    "check_qwarp_steps": "3dQwarp's Python steps",
    "check_fmap_stage": "fMRIPrep's field-map stage",
    "check_unwarp": "the unwarping",
    "check_bold_fmap": "fMRIPrep's BOLD field-map nodes",
    "check_bold_steps": "fMRIPrep's BOLD field-map steps",
    "check_bold_stage": "fMRIPrep's BOLD field-map stages",
    "check_nipype": "nipype's command lines",
}
#: The originals the page compares with, in titles.
#: The originals the records ran: niworkflows 1.15.3, the environments' release (its Reference line in
#: docs/validation/niworkflows-helpers.md); the replica is translated from 1.15.4, whose code is 1.15.3's
#: on these paths.
ORIGINALS = "sdcflows 2.17.1, niworkflows 1.15.3 (1.15.4's code on these paths), SciPy, scikit-image and numpy"
#: The tables' titles by the guide's pattern.
TABLE_TITLES = {
    "records": "The replica's records against the originals: " + ORIGINALS + " (every case passes)",
    "app-runs": "sdcflows' BIDS app whole on the eight datasets of its own runs, the replica "
    "against the original: sdcflows 2.17.1",
    "stage": "fMRIPrep's field-map derivatives, the replica's stage run as fMRIPrep ran it against "
    "fMRIPrep's output",
    "epi-mask": "epi_mask on four BOLD references, the original and the replica's fix against "
    "fMRIPrep's brain masks: sdcflows `epi_mask` 2.17.1",
    "corrections": "Corrections by default, the replica: the originals' defects (SDC-12 to SDC-46, "
    "PIPE-18 to PIPE-40, SCI-06 to SCI-32, SKI-01 to SKI-04)",
    "step": "Changes, the originals against themselves: numpy 2.3.5 / SciPy 1.15.3 and numpy "
    "2.5.4 / SciPy 1.18.1",
    "memory": "Peak memory, the replica against the originals (MiB, the memory rule's calls, one "
    "thread)",
    "trial": "Random testing, the replica against the originals: " + ORIGINALS + " (the two runs "
    "of {date})",
    "triage": "The replica's differences from the originals in the runs of {date}, triaged",
}
#: The tables' columns: each run as the panels name it.
COLUMNS = {
    "Tier": "Comparison",
    "LARMORX (s)": "replica (s)",
    "LARMORX's stage": "the replica's stage",
    "default: voxels": "fix: voxels",
    "named by default": "named by fix",
    "What LARMORX does": "What the replica does",
    "The original": "the original",
    "LARMORX, reproduce": "replica, reproduce",
    "LARMORX, default": "replica, fix",
    "LARMORX ÷ the original": "replica ÷ the original",
}
#: The speed chart's series, as the page names them.
SPEED_LABELS = {
    "the original, 1 thread": "the original, 1 thread",
    "LARMORX, 1 thread": "replica, 1 thread",
    "LARMORX, 14 threads": "replica, 14 threads",
}
#: The record's implementation: its id (the case explorer's) and port.
PORTS = {"port": "replica"}


def _labels(ex: dict) -> None:
    """The labels' old forms that the shapes read in lower case ("LARMORX (port), 1 thread",
    "LARMORX − sdcflows 2.17.1"), fMRIPrep's files by its output (its version is in the panel's
    second line), the replicas underneath by their name."""
    for p in ex["panels"]:
        if p.get("sub"):
            p["sub"] = re.sub(r"\bLARMORX's replicas and ports\b", "LARMORX's replicas", p["sub"])
            p["sub"] = re.sub(r"\bthe replicas and ports\b", "the replicas", p["sub"])
    for d in ex["diffs"]:
        d["label"] = re.sub(r"(?<![\w`/-])LARMORX\b", "larmorx", d["label"])
        d["label"] = re.sub(
            r"\bfMRIPrep \d[\d.]*'s own (files?)\b", r"fMRIPrep's own \1", d["label"]
        )


def present(doc: dict) -> dict:
    """sdcflows' PEPOLAR, direct and fieldmap-less estimation in the writing guide's words and
    order: texts, ports and order only."""
    from larmorx_validation.evidence import present as present_

    page = _page()
    for ex in doc["examples"]:
        line, verdicts = SHOWS[ex["id"]](ex)
        program, software, version = PAGES.get(ex["id"], ("", "sdcflows", "2.17.1"))
        own = _page(program, software, version)
        _labels(ex)
        present_.example(
            own, ex, shows=line, verdicts=verdicts, diffs=DIFFS.get(ex["id"]), split=True
        )
        note = ex.get("metrics_note") or ""
        if THRESHOLD in note:
            raise ValueError(f"{ex['id']}: the threshold is in its note already")
        ex["metrics_note"] = f"{note} {THRESHOLD}".strip()
    for im in doc["implementations"]:
        present_.implementation(page, im, "standard comparison", port=PORTS[im["id"]])
        im["title"] = f"Agreement, replica against the originals: {ORIGINALS}"
        im["label"] = f"replica against the originals: {ORIGINALS} (standard comparison)"
    present_.speed(
        page,
        doc.get("speed"),
        "Seconds per call (the median of three runs, one run for the BIDS app) on a log scale; "
        "the factor is the original's time divided by the replica's, both on one thread.",
        labels=SPEED_LABELS,
    )
    if doc.get("speed"):
        doc["speed"]["title"] = f"Speed, the replica against the originals: {ORIGINALS}"
    present_.finish(page, doc, CATEGORIES, ports=PORTS)
    for t in doc.get("tables", []):
        for c in t["columns"]:
            c["label"] = COLUMNS.get(c["label"], c["label"])
        if t["id"] in TABLE_TITLES:
            t["title"] = TABLE_TITLES[t["id"]].format(date=TRIAL_DATE)
    return doc


if __name__ == "__main__":  # compute the cache entries only
    from larmorx_validation.figures.core import REPO, default_cache

    c = core.Context(repo=REPO, site=Path("."), cache=default_cache())
    for x in entries(c):
        print("done", x.dir, flush=True)
