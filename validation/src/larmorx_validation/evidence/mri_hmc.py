# SPDX-License-Identifier: Apache-2.0
"""Head-motion correction's evidence (``lx.mri.hmc``: the clean-room original, experimental,
and the bit-exact replica of ``larmorx-nc``): fMRIPrep's mcflirt command on a real BOLD run, by
FSL 6.0.7's mcflirt (twice: it is deterministic; and on four slightly perturbed copies of its
input: its own variability), by larmorx's clean-room original (12 and 1 threads,
``reproduce`` and default modes) and by the replica (``reproduce`` and default modes, 3 and 1
threads); fMRIPrep's own matrices; the six parameters and the framewise displacement over the
run; both records' cases (the original's classified against mcflirt's variability band, the
replica's bit for bit); speed.

mcflirt runs as a black box (FSL 6.0.7, ``FSLDIR`` and ``FSLOUTPUTTYPE`` set for its call
only), and the replica as a separate program (``larmorx-nc``, FSL Licence); nothing here reads
FSL's source or imports the replica.
"""

from __future__ import annotations

import json
import os
import re
from pathlib import Path

import numpy as np

from larmorx_validation.evidence import common, core, fsl_step
from larmorx_validation.figures import cache as fcache
from larmorx_validation.figures import records

TOOL = "mri-hmc"
CASE = "fmriprep-ds000005-run-01"
#: The replica's runs of the same command, a cache entry of their own (the entry above, with
#: mcflirt's runs, stays as it was computed).
CASE_REPLICA = "fmriprep-ds000005-run-01-replica"
#: The replica's records: the full tier against FSL 6.0.7.17 (every case the site showed before
#: the version update) and the standard tier against FSL 6.0.7.23 (``python -m
#: larmorx_validation parity fsl-mcflirt-replica``; its full tier waits for the final validation
#: phase).
REPLICA_RECORD = "fsl-mcflirt-replica-full"
REPLICA_RECORD_NEW = "fsl-mcflirt-replica"
#: FSL 6.0.7.23's mcflirt and the replica's run compared with it, an entry of their own
#: (``fsl_step``: the entries above stay as they were computed, with FSL 6.0.7.17).
CASE_NEW = fsl_step.case(CASE)
#: The replica's runs: label, D13 mode, threads (at most 3: the machine is shared).
REPLICA_RUNS = (
    ("replica", "reproduce", "3"),
    ("replica-1", "reproduce", "1"),
    ("replica-fix", "fix", "3"),
)
RUN = "ds000005-sub-01-noreconall"
BOLD = "openneuro/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz"
NODE = (
    "work/fmriprep_25_2_wf/sub_01_wf/bold_task_mixedgamblestask_run_01_wf/bold_fit_wf/"
    "bold_hmc_wf/mcflirt"
)
REF = "out/sub-01/func/sub-01_task-mixedgamblestask_run-01_desc-hmc_boldref.nii.gz"
MASK = "out/sub-01/func/sub-01_task-mixedgamblestask_run-01_desc-brain_mask.nii.gz"
FMRIPREP_MATS = "sub-01_task-mixedgamblestask_run-01_bold_mcf.nii.gz.mat"
FSL = Path.home() / "fsl"
MCFLIRT = FSL / "share" / "fsl" / "bin" / "mcflirt"
PERTURB = ("noise", "noise2", "noise3", "reverse")
#: volumes of the series shown in the viewer (a subset keeps the page light)
N_SHOWN = 6


def _argv(command: str, inp: str, out: str, ref: str) -> list[str]:
    args = command.split()[1:]
    res = []
    for i, a in enumerate(args):
        prev = args[i - 1] if i else ""
        res.append(
            inp if prev == "-in" else out if prev == "-out" else ref if prev == "-reffile" else a
        )
    return res


def _fsl_env() -> dict[str, str]:
    return {"FSLDIR": str(FSL), "FSLOUTPUTTYPE": "NIFTI_GZ"}


def _compute(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.mri_hmc_band import _perturb

    run = common.fmriprep_run(RUN)
    if not MCFLIRT.is_file():
        raise FileNotFoundError(f"FSL 6.0.7's mcflirt ({MCFLIRT})")
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    bold = e.input("BOLD", Path(td.get(BOLD)))
    ref = e.input("HMC reference (fMRIPrep's desc-hmc_boldref)", run / REF)
    d = e.dir
    unset = (*common.CLEAN_ENV, "FSLDIR", "FSLOUTPUTTYPE", "LARMORX_UPSTREAM_DEFECTS")
    for label in ("mcflirt", "mcflirt-2"):
        (d / label).mkdir(exist_ok=True)
        e.run(
            label,
            [str(MCFLIRT), *_argv(command, str(bold), str(d / label / "out_mcf.nii.gz"), str(ref))],
            env=_fsl_env(),
            unset=unset,
        )
    for kind in PERTURB:
        out = d / f"mcflirt-{kind}"
        out.mkdir(exist_ok=True)
        if kind == "reverse":
            series = _reversed(bold, out)
        else:
            series, _ = _perturb(kind, bold, ref, out)
        e.run(
            f"mcflirt-{kind}",
            [str(MCFLIRT), *_argv(command, str(series), str(out / "out_mcf.nii.gz"), str(ref))],
            env=_fsl_env(),
            unset=unset,
        )
        series.unlink()  # a perturbed copy of the input, made again from the seed if needed
    for label, mode, threads in (
        ("larmorx", "reproduce", None),
        ("larmorx-1", "reproduce", "1"),
        ("larmorx-fix", "fix", None),
    ):
        (d / label).mkdir(exist_ok=True)
        env = {"LARMORX_UPSTREAM_DEFECTS": mode, "FSLOUTPUTTYPE": "NIFTI_GZ"}
        if threads:
            env["OMP_NUM_THREADS"] = threads
        e.run(
            label,
            fcache.larmorx_cli(
                "mri",
                "hmc",
                *_argv(command, str(bold), str(d / label / "out_mcf.nii.gz"), str(ref)),
            ),
            env=env,
            unset=unset,
        )
    version = (FSL / "etc" / "fslversion").read_text(encoding="utf-8").split()[0]
    e.finish(
        versions={"fsl": f"FSL {version} (mcflirt)"},
        command=command,
        threads={"mcflirt": 1, "larmorx": ["all logical CPUs", 1]},
        seed="the noise perturbations: larmorx_validation.parity.mri_hmc_band.SEED",
        settings="fMRIPrep 25.2.6's mcflirt node of ds000005 sub-01 run 1 (its command.txt and "
        "its HMC reference); mcflirt twice and on four perturbed copies of the series (uniform "
        "noise of ±0.5, three realisations; the volumes in reverse order); larmorx in reproduce "
        "mode (all CPUs and one) and in its default mode",
    )


def _reversed(bold: Path, out: Path) -> Path:
    """The series with its volumes in reverse order, stored as the input is (its type, scale
    factor and header): only the order changes, so mcflirt's output keeps the input's type,
    as for the unperturbed run, and the realigned images compare value for value."""
    import nibabel as nib

    img = nib.load(str(bold))
    raw = np.asanyarray(img.dataobj.get_unscaled())[..., ::-1]
    hdr = img.header.copy()
    new = nib.Nifti1Image(np.ascontiguousarray(raw), img.affine, hdr)
    new.header.set_slope_inter(img.dataobj.slope, img.dataobj.inter)
    path = out / "series.nii.gz"
    new.to_filename(path)
    back = nib.load(str(path))
    assert np.array_equal(np.asanyarray(back.dataobj), np.asanyarray(img.dataobj)[..., ::-1])
    return path


def _compute_replica(e: fcache.Entry) -> None:
    import larmorx_testdata as td

    from larmorx_validation.parity.fsl_mcflirt_replica import replica_binary, replica_version

    run = common.fmriprep_run(RUN)
    exe = replica_binary()
    if exe is None:
        raise FileNotFoundError("the larmorx-nc replica (LARMORX_NC_BIN, crates-nc)")
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    bold = e.input("BOLD", Path(td.get(BOLD)))
    ref = e.input("HMC reference (fMRIPrep's desc-hmc_boldref)", run / REF)
    unset = (*common.CLEAN_ENV, "FSLDIR", "FSLOUTPUTTYPE", "LARMORX_UPSTREAM_DEFECTS")
    for label, mode, threads in REPLICA_RUNS:
        (e.dir / label).mkdir(exist_ok=True)
        e.run(
            label,
            [
                str(exe),
                "fsl",
                "mcflirt",
                *_argv(command, str(bold), str(e.dir / label / "out_mcf.nii.gz"), str(ref)),
            ],
            env={
                "LARMORX_UPSTREAM_DEFECTS": mode,
                "FSLOUTPUTTYPE": "NIFTI_GZ",
                "OMP_NUM_THREADS": threads,
            },
            unset=unset,
        )
    e.finish(
        versions={"larmorx-nc": replica_version()},
        command=command,
        threads={"larmorx-nc": [3, 1]},
        settings="fMRIPrep 25.2.6's mcflirt node of ds000005 sub-01 run 1 (its command.txt and "
        "its HMC reference), by the larmorx-nc replica in reproduce mode (3 threads and 1) and "
        "in its default mode",
    )


def replica_entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE_REPLICA,
        "fMRIPrep's head-motion correction of ds000005 sub-01 run 1 by the larmorx-nc replica",
        _compute_replica,
    )


def _compute_new(e: fcache.Entry) -> None:
    """FSL 6.0.7.23's mcflirt and the replica (reproduce mode) on fMRIPrep's command, as the
    FSL 6.0.7.17 entry ran it."""
    import larmorx_testdata as td

    from larmorx_validation.parity import fsl_versions
    from larmorx_validation.parity.fsl_mcflirt_replica import replica_binary, replica_version

    run = common.fmriprep_run(RUN)
    fsl = fsl_versions.fsldir(fsl_step.NEW)
    mcflirt = fsl / "bin" / "mcflirt"
    exe = replica_binary()
    if not mcflirt.is_file() or exe is None:
        raise FileNotFoundError(f"{fsl_step.NEW_LABEL}'s mcflirt ({mcflirt}) or the replica")
    command = (run / NODE / "command.txt").read_text(encoding="utf-8").strip()
    bold = e.input("BOLD", Path(td.get(BOLD)))
    ref = e.input("HMC reference (fMRIPrep's desc-hmc_boldref)", run / REF)
    unset = (*common.CLEAN_ENV, "FSLDIR", "FSLOUTPUTTYPE", "LARMORX_UPSTREAM_DEFECTS")
    for label, argv0, env in (
        (
            "mcflirt23",
            [str(mcflirt)],
            {"FSLDIR": str(fsl), "FSLOUTPUTTYPE": "NIFTI_GZ", **fsl_versions.openblas_env()},
        ),
        (
            "replica23",
            [str(exe), "fsl", "mcflirt"],
            {
                "LARMORX_UPSTREAM_DEFECTS": "reproduce",
                "FSLOUTPUTTYPE": "NIFTI_GZ",
                "OMP_NUM_THREADS": "3",
            },
        ),
    ):
        (e.dir / label).mkdir(exist_ok=True)
        out = str(e.dir / label / "out_mcf.nii.gz")
        e.run(label, [*argv0, *_argv(command, str(bold), out, str(ref))], env=env, unset=unset)
    e.finish(
        versions={
            "fsl": f"FSL {fsl_versions.version_of(fsl)} (mcflirt)",
            "larmorx-nc": replica_version(),
        },
        command=command,
        threads={"mcflirt": 1, "larmorx-nc": 3},
        settings=f"fMRIPrep 25.2.6's mcflirt node of ds000005 sub-01 run 1 (its command.txt and "
        f"its HMC reference), as the FSL 6.0.7.17 entry ran it: {fsl_step.NEW_LABEL}'s mcflirt "
        "(~/fsl-6.0.7.23, never on the PATH) and the larmorx-nc replica in reproduce mode, 3 "
        "threads",
    )


def entry_new(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE_NEW,
        f"fMRIPrep's head-motion correction of ds000005 sub-01 run 1: {fsl_step.NEW_LABEL}'s "
        "mcflirt and the larmorx-nc replica",
        _compute_new,
    )


def entry(ctx: core.Context) -> fcache.Entry:
    return fcache.entry(
        ctx.figures_context(),
        TOOL,
        CASE,
        "fMRIPrep's head-motion correction of ds000005 sub-01 run 1: FSL 6.0.7's mcflirt (and "
        "its perturbation band) and larmorx",
        _compute,
    )


# ----------------------------------------------------------------------------------- export


def _params(mats: list, centre: np.ndarray) -> np.ndarray:
    from larmorx_validation.parity.mri_hmc import decompose

    return np.array([decompose(m, centre) for m in mats])


def _mats(path: Path, reverse: bool = False) -> list:
    from larmorx_validation.parity.mri_hmc import _read_mats

    mats, _ = _read_mats(path)
    return mats[::-1] if reverse else mats


def _compare_mats(a: list, b: list, centre: np.ndarray) -> dict:
    from larmorx_validation.parity.mri_hmc import framewise_displacement, rms_deviation

    devs = np.array([rms_deviation(x, y, centre) for x, y in zip(a, b, strict=True)])
    pa, pb = _params(a, centre), _params(b, centre)
    fa, fb = framewise_displacement(pa), framewise_displacement(pb)
    return {
        "devs": devs,
        "rms_median": float(np.median(devs)),
        "rms_p95": float(np.percentile(devs, 95)),
        "rms_max": float(devs.max()),
        "mm_median": float(np.median(np.abs(pa[:, 3:] - pb[:, 3:]))),
        "deg_median": float(np.median(np.degrees(np.abs(pa[:, :3] - pb[:, :3])))),
        "fd_r": float(np.corrcoef(fa, fb)[0, 1]) if fa.std() > 0 and fb.std() > 0 else 1.0,
        "identical_text": None,
    }


def _example(ctx: core.Context, reg: dict) -> dict:
    import larmorx_testdata as td
    import nibabel as nib
    from scipy import ndimage

    from larmorx_validation.evidence import example as exm
    from larmorx_validation.parity.mri_hmc import _fov_centre, framewise_displacement

    e = entry(ctx)
    info = e.info()
    re_ = replica_entry(ctx)
    rinfo = re_.info()
    rd = re_.dir
    ne = entry_new(ctx)
    ninfo = ne.info()
    nd = ne.dir
    run = common.fmriprep_run(RUN)
    d = e.dir
    ref_path = run / REF
    centre = _fov_centre(ref_path)
    mats = {
        "mcflirt": _mats(d / "mcflirt" / "out_mcf.nii.gz.mat"),
        "mcflirt-2": _mats(d / "mcflirt-2" / "out_mcf.nii.gz.mat"),
        "larmorx": _mats(d / "larmorx" / "out_mcf.nii.gz.mat"),
        "larmorx-1": _mats(d / "larmorx-1" / "out_mcf.nii.gz.mat"),
        "larmorx-fix": _mats(d / "larmorx-fix" / "out_mcf.nii.gz.mat"),
        "fmriprep": _mats(run / NODE / FMRIPREP_MATS),
        **{k: _mats(rd / k / "out_mcf.nii.gz.mat") for k, _, _ in REPLICA_RUNS},
        **{k: _mats(nd / k / "out_mcf.nii.gz.mat") for k in ("mcflirt23", "replica23")},
        **{
            f"mcflirt-{k}": _mats(d / f"mcflirt-{k}" / "out_mcf.nii.gz.mat", reverse=k == "reverse")
            for k in PERTURB
        },
    }
    texts = {
        k: [p.read_text() for p in sorted((d / k / "out_mcf.nii.gz.mat").glob("MAT_*"))]
        for k in ("mcflirt", "mcflirt-2", "larmorx", "larmorx-1", "larmorx-fix")
    }
    texts["fmriprep"] = [p.read_text() for p in sorted((run / NODE / FMRIPREP_MATS).glob("MAT_*"))]
    for k, _, _ in REPLICA_RUNS:
        texts[k] = [p.read_text() for p in sorted((rd / k / "out_mcf.nii.gz.mat").glob("MAT_*"))]
    for k in ("mcflirt23", "replica23"):
        texts[k] = [p.read_text() for p in sorted((nd / k / "out_mcf.nii.gz.mat").glob("MAT_*"))]
    cmp = {
        "larmorx": _compare_mats(mats["larmorx"], mats["mcflirt"], centre),
        "larmorx-fix": _compare_mats(mats["larmorx-fix"], mats["mcflirt"], centre),
        "fix-repro": _compare_mats(mats["larmorx-fix"], mats["larmorx"], centre),
        **{k: _compare_mats(mats[f"mcflirt-{k}"], mats["mcflirt"], centre) for k in PERTURB},
        "fmriprep": _compare_mats(mats["fmriprep"], mats["mcflirt"], centre),
        "mcflirt-2": _compare_mats(mats["mcflirt-2"], mats["mcflirt"], centre),
        "larmorx-1": _compare_mats(mats["larmorx-1"], mats["larmorx"], centre),
        "replica": _compare_mats(mats["replica"], mats["mcflirt"], centre),
        "replica-fix": _compare_mats(mats["replica-fix"], mats["mcflirt"], centre),
        "replica-fix-repro": _compare_mats(mats["replica-fix"], mats["replica"], centre),
        "replica-1": _compare_mats(mats["replica-1"], mats["replica"], centre),
        "lx-replica-fix": _compare_mats(mats["larmorx-fix"], mats["replica-fix"], centre),
        "replica23": _compare_mats(mats["replica23"], mats["mcflirt23"], centre),
        "mcflirt23": _compare_mats(mats["mcflirt23"], mats["mcflirt"], centre),
        # The clean-room original against FSL 6.0.7.23, its reference since 2026-10-29: its run
        # of the entry above (fMRIPrep's command meets none of the alignment's changes).
        "larmorx23": _compare_mats(mats["larmorx"], mats["mcflirt23"], centre),
    }
    same_text = {k: texts[k] == texts["mcflirt"] for k in ("mcflirt-2", "fmriprep", "larmorx")}
    same_text["larmorx-1"] = texts["larmorx-1"] == texts["larmorx"]
    same_text["larmorx-fix"] = texts["larmorx-fix"] == texts["larmorx"]
    n_ident = sum(a == b for a, b in zip(texts["larmorx"], texts["mcflirt"], strict=True))
    n_ident_replica = sum(a == b for a, b in zip(texts["replica"], texts["mcflirt"], strict=True))
    n_ident_fix = sum(
        a == b for a, b in zip(texts["replica-fix"], texts["larmorx-fix"], strict=True)
    )
    n_ident_23 = sum(a == b for a, b in zip(texts["replica23"], texts["mcflirt23"], strict=True))
    n_ident_step = sum(a == b for a, b in zip(texts["mcflirt23"], texts["mcflirt"], strict=True))
    n_ident_lx23 = sum(a == b for a, b in zip(texts["larmorx"], texts["mcflirt23"], strict=True))
    band = {
        k: max(cmp[p][k] for p in PERTURB)
        for k in ("rms_median", "rms_p95", "rms_max", "mm_median", "deg_median")
    }
    band["fd_r"] = min(cmp[p]["fd_r"] for p in PERTURB)
    band_devs = np.max([cmp[p]["devs"] for p in PERTURB], axis=0)
    lx = cmp["larmorx"]
    within = (
        lx["rms_p95"] <= band["rms_p95"]
        and lx["mm_median"] <= band["mm_median"]
        and lx["deg_median"] <= band["deg_median"]
        and lx["fd_r"] >= band["fd_r"]
    )

    # the viewer: a few volumes of the series (the largest deviations, the largest motion,
    # the ends), on the BOLD grid inside fMRIPrep's BOLD brain mask
    from larmorx_validation.parity.mri_hmc import rms_deviation

    motion = np.array([rms_deviation(m, np.eye(4), centre) for m in mats["mcflirt"]])
    n = len(mats["mcflirt"])
    pick = [
        int(np.argmax(lx["devs"])),
        int(np.argmax(motion)),
        int(np.argmax(cmp["fix-repro"]["devs"])),
        0,
        n - 1,
        int(np.argmax(band_devs)),
    ]
    shown: list[int] = []
    for t in pick:
        if t not in shown:
            shown.append(t)
    for t in np.argsort(-lx["devs"]):
        if len(shown) >= N_SHOWN:
            break
        if int(t) not in shown:
            shown.append(int(t))
    shown = sorted(shown[:N_SHOWN])
    img = nib.load(str(td.get(BOLD)))
    brain = common.load(str(run / MASK)) > 0
    show = ndimage.binary_dilation(brain, iterations=1)
    grid = core.Grid.around(img, show)
    ex = exm.Example(ctx, grid, show)
    raw, slope, inter = common.load_raw(td.get(BOLD))

    def vols(a: np.ndarray) -> np.ndarray:
        return np.ascontiguousarray(a[..., shown])

    ex.add(
        exm.Panel(
            "bold",
            "input",
            "ds000005 sub-01 BOLD, run 1",
            f"int16, {n} volumes; {len(shown)} shown",
            {"all": vols(raw)},
            scale=slope if slope != 1.0 or inter != 0.0 else None,
            inter=inter,
        )
    )
    series = {}
    for pid, label, role, text, sub in [
        (
            "larmorx",
            "larmorx",
            "larmorx",
            "larmorx clean-room original",
            "reproduce mode, all CPUs",
        ),
        ("larmorx-1", "larmorx-1", "larmorx", "larmorx clean-room original", "reproduce, 1 thread"),
        ("larmorx-fix", "larmorx-fix", "larmorx", "larmorx clean-room original", "default mode"),
        ("mcflirt", "mcflirt", "original", "mcflirt, FSL 6.0.7.17", "fMRIPrep's command"),
        ("mcflirt-2", "mcflirt-2", "original", "mcflirt, FSL 6.0.7.17", "the same command again"),
        (
            "mcflirt-reverse",
            "mcflirt-reverse",
            "original",
            "mcflirt, FSL 6.0.7.17",
            "the volumes in reverse order (put back)",
        ),
    ]:
        a = common.load(str(d / label / "out_mcf.nii.gz"))
        if label == "mcflirt-reverse":
            a = a[..., ::-1]
        series[pid] = vols(a)
        ex.add(exm.Panel(pid, role, text, sub, {"series": series[pid]}))
    for pid, sub in (
        ("replica", "reproduce mode, 3 threads"),
        ("replica-1", "reproduce mode, 1 thread"),
        ("replica-fix", "default mode, 3 threads"),
    ):
        series[pid] = vols(common.load(str(rd / pid / "out_mcf.nii.gz")))
        ex.add(exm.Panel(pid, "larmorx", "larmorx-nc replica", sub, {"series": series[pid]}))
    for pid, role, text, sub in (
        ("mcflirt23", "original", f"mcflirt, {fsl_step.NEW_LABEL}",
         f"fMRIPrep's command; {fsl_step.REFERENCE}"),
        ("replica23", "larmorx", "larmorx-nc replica",
         f"reproduce mode, 3 threads, following {fsl_step.NEW_LABEL}"),
    ):  # fmt: skip
        series[pid] = vols(common.load(str(nd / pid / "out_mcf.nii.gz")))
        ex.add(exm.Panel(pid, role, text, sub, {"series": series[pid]}))
    dd = common.d
    diffs = [
        dd(
            "lx-mcflirt",
            "larmorx",
            "mcflirt",
            "main",
            "clean-room original − mcflirt",
            common.ROLE_LX,
        ),
        dd(
            "fix-lx",
            "larmorx-fix",
            "larmorx",
            "main",
            "clean-room original: default − reproduce mode",
            "larmorx's corrections",
        ),
        dd(
            "lx1-lx",
            "larmorx-1",
            "larmorx",
            "check",
            "clean-room original, 1 thread − all CPUs",
            common.ROLE_SELF,
        ),
        dd(
            "mc2-mc",
            "mcflirt-2",
            "mcflirt",
            "variability",
            "mcflirt: second run − first run",
            common.ROLE_ORIG,
        ),
        dd(
            "rev-mc",
            "mcflirt-reverse",
            "mcflirt",
            "variability",
            "mcflirt: reversed input − unperturbed",
            common.ROLE_ORIG,
        ),
        dd(
            "replica-mcflirt",
            "replica",
            "mcflirt",
            "main",
            f"replica − mcflirt {fsl_step.OLD}",
            common.ROLE_LX,
        ),
        dd(
            "replica-fix-replica",
            "replica-fix",
            "replica",
            "main",
            "replica: default − reproduce mode",
            "larmorx's corrections",
        ),
        dd(
            "replica1-replica",
            "replica-1",
            "replica",
            "check",
            "replica, 1 thread − 3 threads",
            common.ROLE_SELF,
        ),
        dd(
            "lxfix-replicafix",
            "larmorx-fix",
            "replica-fix",
            "main",
            "clean-room original − replica, both in their default mode",
            "the two implementations of the corrections",
        ),
        fsl_step.replica_diff("replica23", "mcflirt23"),
        fsl_step.step_diff("mcflirt23", "mcflirt"),
        # The clean-room original against FSL 6.0.7.23 (2026-10-29), last: the earlier
        # comparisons keep their places.
        dd(
            "lx-mcflirt23",
            "larmorx",
            "mcflirt23",
            "main",
            f"clean-room original − mcflirt {fsl_step.NEW}",
            "larmorx against the original (both implementations' reference)",
        ),
    ]
    outputs = [{"id": "series", "label": "Realigned series", "unit": ""}]
    common.compare_all(ex, diffs, outputs)
    same = common.identical(diffs)
    fix_names = sorted(set(re.findall(r"FSL-\d+", (d / "larmorx-fix.log").read_text())))
    replica_fix_names = sorted(set(re.findall(r"FSL-\d+", (rd / "replica-fix.log").read_text())))
    rv = rinfo.get("versions", {}).get("larmorx-nc", "larmorx-nc")

    for x in diffs:
        i, links = x["id"], []
        if x["kind"] == "variability" and i != "mcflirt23-mcflirt":
            x["verdict"] = None
            x["reason"] = {
                "mc2-mc": "mcflirt is deterministic: the same command gives the same bytes."
                if same[i]
                else "mcflirt's two runs differ.",
                "rev-mc": "mcflirt's own variability: the same volumes registered in another "
                "order end at other estimates (no change of the data).",
            }[i]
        elif i == "lx-mcflirt":
            x["verdict"] = (
                "within-variability"
                if within and not same[i]
                else ("bit-identical" if same[i] else "open")
            )
            x["reason"] = (
                f"The matrices: 95th-percentile RMS deviation {lx['rms_p95']:.3g} mm, against "
                f"mcflirt's own {band['rms_p95']:.3g} mm under the perturbations (the chart "
                "below); the images then differ by interpolating at slightly other positions. "
                "A clean-room implementation, so not bit-identical by design."
                if within
                else f"The matrices deviate more than mcflirt's own variability "
                f"({lx['rms_p95']:.3g} mm against {band['rms_p95']:.3g} mm at the 95th "
                "percentile)."
            )
        elif i == "replica-mcflirt":
            x["verdict"] = "bit-identical" if same[i] else "open"
            x["reason"] = (
                f"The replica gives mcflirt's bytes: the realigned series and {n_ident_replica} of "
                f"{n} matrices identical as text ({rv} at the time, a translation of FSL "
                "6.0.7.17's source with glibc 2.35's float functions)."
                if same[i]
                else "The replica should give mcflirt's bytes here; it does not (see the record)."
            )
            links += [{"label": "replica record", "href": f"docs/validation/{REPLICA_RECORD}.md"}]
        elif i == "replica23-mcflirt23":
            x["verdict"] = "bit-identical" if same[i] else "open"
            x["reason"] = fsl_step.replica_reason(
                same[i],
                f" The realigned series and {n_ident_23} of {n} matrices identical as text "
                f"({ninfo.get('versions', {}).get('larmorx-nc', 'larmorx-nc')}, following FSL "
                f"{fsl_step.NEW}'s source).",
            )
            links += [
                {"label": "replica record", "href": f"docs/validation/{REPLICA_RECORD_NEW}.md"}
            ]
        elif i == "lx-mcflirt23":
            l23 = cmp["larmorx23"]
            ok = (
                l23["rms_p95"] <= band["rms_p95"]
                and l23["mm_median"] <= band["mm_median"]
                and l23["deg_median"] <= band["deg_median"]
                and l23["fd_r"] >= band["fd_r"]
            )
            x["verdict"] = "bit-identical" if same[i] else ("within-variability" if ok else "open")
            x["reason"] = (
                f"FSL {fsl_step.NEW}'s mcflirt is the clean-room original's reference since "
                "2026-10-29 (its record is against it). On fMRIPrep's command the original's "
                f"matrices against it: 95th-percentile RMS deviation {l23['rms_p95']:.3g} mm, "
                f"against mcflirt's own {band['rms_p95']:.3g} mm under the perturbations; "
                f"{n_ident_lx23} of {n} matrices identical as text. The original's run is the "
                "one above: fMRIPrep's command meets none of the changes of the alignment "
                "(FSL-59's final interpolations, the zstd and bzip2 files, FSL-60's names, the "
                "histogram costs)."
            )
            links += [{"label": "record", "href": f"docs/validation/{TOOL}.md"}]
        elif i == "mcflirt23-mcflirt":
            x["verdict"] = None
            x["reason"] = (
                fsl_step.step_reason(same[i], "fMRIPrep's command", None)
                + f" {n_ident_step} of {n} matrices identical as text."
            )
        elif i == "replica-fix-replica":
            x["verdict"] = "d13" if not same[i] else "bit-identical"
            x["reason"] = (
                "The same corrections of mcflirt's defects as the clean-room original's, by "
                "default with a warning: "
                + (", ".join(replica_fix_names) or "none met")
                + f"; they move the matrices by up to {cmp['replica-fix-repro']['rms_max']:.3g} mm "
                "(RMS deviation). Reproduce mode gives mcflirt's bytes."
            )
            links += [
                {"label": n_, "href": reg[n_]["link"]} for n_ in replica_fix_names if n_ in reg
            ]
        elif i == "lxfix-replicafix":
            lr = cmp["lx-replica-fix"]
            ok = lr["rms_p95"] <= band["rms_p95"]
            x["verdict"] = "within-variability" if ok else "open"
            x["reason"] = (
                f"Both implementations correct the same defects; their matrices differ as the "
                f"original's differ from mcflirt's: 95th-percentile RMS deviation "
                f"{lr['rms_p95']:.3g} mm against mcflirt's own {band['rms_p95']:.3g} mm "
                f"({n_ident_fix} of {n} matrices identical as text)."
            )
        elif i == "fix-lx":
            x["verdict"] = "d13" if not same[i] else "bit-identical"
            x["reason"] = (
                "Corrections of mcflirt's deterministic defects, by default with a warning: "
                + ", ".join(fix_names)
                + f"; they move the matrices by up to {cmp['fix-repro']['rms_max']:.3g} mm "
                "(RMS deviation). Reproduce mode gives mcflirt's behaviour."
            )
            links += [{"label": n_, "href": reg[n_]["link"]} for n_ in fix_names if n_ in reg]
        elif same[i]:
            x["verdict"] = "bit-identical"
            x["reason"] = "The same bytes on any number of threads."
        else:
            x["verdict"] = "open"
            x["reason"] = "Not accounted for by the other runs of this example."
        x["links"] = links
    t0 = shown.index(int(np.argmax(lx["devs"]))) if int(np.argmax(lx["devs"])) in shown else 0
    at, _k = common.largest_in_slice(ex, "larmorx", "mcflirt", "series", "axial", t=t0)
    default = {
        "output": "series",
        "plane": "axial",
        "voxel": at,
        "t": t0,
        "panels": ["bold", "mcflirt", "replica", "larmorx", "larmorx-fix"],
        "diffs": ["replica-mcflirt", "lx-mcflirt", "rev-mc", "fix-lx"],
        "compare": {"a": "mcflirt", "b": "larmorx"},
    }
    out = common.finish(
        ctx,
        ex,
        diffs,
        outputs,
        default=default,
        window_panel="mcflirt",
        caption=f"file volume {shown[t0]}, where larmorx's matrix deviates most from mcflirt's; "
        "the differences on one symmetric scale.",
    )
    out["grid"]["t_index"] = shown

    # the parameters over the run
    pm, pl, _pf = (_params(mats[k], centre) for k in ("mcflirt", "larmorx", "larmorx-fix"))
    names = [
        "Rotation x (°)",
        "Rotation y (°)",
        "Rotation z (°)",
        "Translation x (mm)",
        "Translation y (mm)",
        "Translation z (mm)",
    ]

    def unit(j: int, a: np.ndarray) -> list:
        v = np.degrees(a[:, j]) if j < 3 else a[:, j]
        return [round(float(x), 6) for x in v]

    panels = [
        {
            "label": names[j],
            "series": [
                {"label": "mcflirt", "mark": "original", "y": unit(j, pm)},
                {"label": "larmorx", "mark": "larmorx", "y": unit(j, pl)},
            ],
        }
        for j in range(6)
    ]
    fdm, fdl = framewise_displacement(pm), framewise_displacement(pl)
    panels.append(
        {
            "label": "Framewise displacement (Power, mm)",
            "series": [
                {"label": "mcflirt", "mark": "original", "y": [round(float(v), 6) for v in fdm]},
                {"label": "larmorx", "mark": "larmorx", "y": [round(float(v), 6) for v in fdl]},
            ],
        }
    )
    panels.append(
        {
            "label": "Deviation from mcflirt's matrix (RMS over an 80 mm sphere, mm)",
            "series": [
                {
                    "label": "larmorx, reproduce",
                    "mark": "larmorx",
                    "y": [round(float(v), 6) for v in lx["devs"]],
                },
                {
                    "label": "larmorx-nc replica, reproduce",
                    "mark": "replica",
                    "y": [round(float(v), 6) for v in cmp["replica"]["devs"]],
                },
            ],
            "bands": [
                {"mark": "band", "lo": [0.0] * n, "hi": [round(float(v), 6) for v in band_devs]}
            ],
        }
    )
    panels.append(
        {
            "label": "larmorx default mode − reproduce mode (RMS deviation, mm)",
            "series": [
                {
                    "label": "default − reproduce",
                    "mark": "larmorx",
                    "y": [round(float(v), 6) for v in cmp["fix-repro"]["devs"]],
                },
                {
                    "label": "replica: default − reproduce",
                    "mark": "replica",
                    "y": [round(float(v), 6) for v in cmp["replica-fix-repro"]["devs"]],
                },
            ],
        }
    )

    def row(label: str, c: dict, cls: str = "", sub: str = "") -> dict:
        return {
            "head": label,
            **({"sub": sub} if sub else {}),
            **({"class": cls} if cls else {}),
            "cells": [
                common.cell(c["rms_median"]),
                common.cell(c["rms_p95"]),
                common.cell(c["rms_max"]),
                common.cell(c["mm_median"]),
                common.cell(c["deg_median"]),
                common.cell(c["fd_r"], "sig4"),
            ],
        }

    stats = common.table(
        "hmc-stats",
        "The matrices compared, volume by volume (RMS deviation over an 80 mm sphere; "
        "parameter differences about the reference's centre; FD's correlation)",
        [
            "Comparison",
            {"label": "median (mm)", "num": True},
            {"label": "95th pct (mm)", "num": True},
            {"label": "largest (mm)", "num": True},
            {"label": "median Δ translation (mm)", "num": True},
            {"label": "median Δ rotation (°)", "num": True},
            {"label": "FD r", "num": True},
        ],
        [
            row(
                "larmorx (reproduce) − mcflirt",
                lx,
                sub=f"{n_ident} of {n} matrices identical as text",
            ),
            row("larmorx (default) − mcflirt", cmp["larmorx-fix"]),
            row(
                "larmorx: default − reproduce",
                cmp["fix-repro"],
                sub=", ".join(fix_names) + " corrected",
            ),
            row("mcflirt: noise − unperturbed", cmp["noise"], "is-sep is-yardstick"),
            row("mcflirt: noise2 − unperturbed", cmp["noise2"], "is-yardstick"),
            row("mcflirt: noise3 − unperturbed", cmp["noise3"], "is-yardstick"),
            row("mcflirt: reversed − unperturbed", cmp["reverse"], "is-yardstick"),
            row(
                "fMRIPrep's own matrices − mcflirt 6.0.7.17",
                cmp["fmriprep"],
                "is-sep",
                sub="identical as text" if same_text["fmriprep"] else "fMRIPrep's FSL in its image",
            ),
            row(
                "mcflirt: second run − first",
                cmp["mcflirt-2"],
                sub="identical as text" if same_text["mcflirt-2"] else "",
            ),
            row(
                "replica (reproduce) − mcflirt",
                cmp["replica"],
                "is-sep",
                sub=f"{n_ident_replica} of {n} matrices identical as text",
            ),
            row("replica (default) − mcflirt", cmp["replica-fix"]),
            row(
                "replica: default − reproduce",
                cmp["replica-fix-repro"],
                sub=", ".join(replica_fix_names) + " corrected",
            ),
            row("replica: 1 thread − 3 threads", cmp["replica-1"]),
            row(
                "clean-room original − replica (both default)",
                cmp["lx-replica-fix"],
                sub=f"{n_ident_fix} of {n} matrices identical as text",
            ),
            row(
                f"replica (reproduce) − mcflirt {fsl_step.NEW}",
                cmp["replica23"],
                "is-sep",
                sub=f"{n_ident_23} of {n} matrices identical as text",
            ),
            row(
                f"mcflirt {fsl_step.NEW} − {fsl_step.OLD}",
                cmp["mcflirt23"],
                "is-yardstick",
                sub=f"the version step; {n_ident_step} of {n} matrices identical as text",
            ),
            row(
                f"clean-room original (reproduce) − mcflirt {fsl_step.NEW}",
                cmp["larmorx23"],
                "is-sep",
                sub=f"its reference since 2026-10-29; {n_ident_lx23} of {n} matrices identical "
                "as text",
            ),
        ],
        note="The record's thresholds: 95th percentile ≤ 0.1 mm, median parameter differences ≤ "
        "0.05 mm and 0.05°, FD r ≥ 0.99 where FD reaches 0.5 mm, each widened to mcflirt's own "
        "variability where that is larger. The shaded rows are mcflirt's own variability: the "
        "worst of them is the band of the chart above.",
    )
    lines = {
        "params": {
            "title": f"The six parameters and the framewise displacement over the {n} volumes",
            "x_label": "volume",
            "x": list(range(n)),
            "key": [
                {"label": "mcflirt, FSL 6.0.7.17", "mark": "original"},
                {"label": "larmorx clean-room original (reproduce)", "mark": "larmorx"},
                {"label": "larmorx-nc replica (reproduce: mcflirt's bytes)", "mark": "replica"},
                {
                    "label": "mcflirt's own variability (worst of its four perturbations)",
                    "mark": "band",
                },
            ],
            "panels": panels,
            "note": "Parameters as the record decomposes the matrices (about the centre of the "
            "reference's field of view); FD as Power et al. (50 mm). Move the pointer over a "
            "chart to read every value at one volume.",
            "stats": stats,
        }
    }
    fsl = info.get("versions", {}).get("fsl", "FSL 6.0.7")
    command = " ".join(info["command"].split()).replace(
        "/work/fmriprep_25_2_wf/sub_01_wf/bold_task_mixedgamblestask_run_01_wf/bold_fit_wf/"
        "bold_hmc_wf/mcflirt/",
        "…/",
    )
    n_vox = int(np.prod(img.shape))
    return {
        "id": "fmriprep-hmc",
        "title": "fMRIPrep's head-motion estimation of a real BOLD run, by mcflirt and larmorx",
        "summary": (
            "The mcflirt node fMRIPrep 25.2.6 ran on OpenNeuro ds000005 sub-01's first BOLD run "
            "(240 volumes, its own HMC reference): its command given to FSL 6.0.7.17's mcflirt, "
            "to the bit-exact replica (larmorx-nc) and to larmorx's clean-room original, and to "
            "FSL 6.0.7.23's mcflirt with the replica compared with it (the replica's reference "
            "since the version update of 2026-10-29). fMRIPrep "
            "keeps only the matrices; the realigned "
            f"series are shown at {len(shown)} of the {n} volumes (where larmorx deviates most, "
            "the largest motion, the ends)."
        ),
        "facts": [
            {
                "label": "Data",
                "text": f"{common.DS000005['dataset']}, sub-01, task mixedgamblestask run 1 "
                f"({common.DS000005['licence']}); shown inside fMRIPrep's BOLD brain mask, "
                "dilated by one voxel",
            },
            {"label": "Node", "text": "fMRIPrep 25.2.6, `bold_fit_wf/bold_hmc_wf/mcflirt`"},
            {"label": "Command", "text": f"`{command}`"},
            {
                "label": "Programs",
                "text": f"{fsl}, run as a black box; larmorx's clean-room original "
                f"(`lx.mri.hmc`, experimental) at commit `{info['larmorx_commit'][:12]}`; the "
                f'replica ({rv}, `lx.mri.hmc(implementation="replica")`) at commit '
                f"`{(rinfo.get('larmorx_commit') or '')[:12]}`",
            },
            {
                "label": "Volumes",
                "text": "shown: file volumes " + ", ".join(map(str, shown)),
            },
            fsl_step.fact(
                same["mcflirt23-mcflirt"],
                "fMRIPrep's command",
                f"{ninfo.get('versions', {}).get('fsl', fsl_step.NEW_LABEL)} and "
                f"{ninfo.get('versions', {}).get('larmorx-nc', 'larmorx-nc')} at commit "
                f"`{(ninfo.get('larmorx_commit') or '')[:12]}`. The case explorer below shows the "
                "replica's full-tier record (against FSL 6.0.7.17) and its standard record "
                "(against FSL 6.0.7.23) side by side; the clean-room original's record runs "
                "against 6.0.7.23's recorded runs since its alignment (2026-10-29).",
            ),
        ],
        **out,
        "lines": lines,
        "metrics_note": (
            f"Δ is A − B over the {len(shown)} volumes shown ({n_vox // img.shape[3] * len(shown):,} "
            "values), not only the brain; the matrices are compared over every volume in the "
            "chart and its table."
        ),
        "provenance": [
            *common.provenance(info, f"{TOOL}/{CASE}", [common.DS000005]),
            # the replica's runs (the same data: its first line, "Data: ...", left out)
            *common.provenance(rinfo, f"{TOOL}/{CASE_REPLICA}", [common.DS000005])[1:],
            *common.provenance(ninfo, f"{TOOL}/{CASE_NEW}", [common.DS000005])[1:],
        ],
    }


# ------------------------------------------------------------------------------------ cases


def _value(v) -> list[float]:
    return [float(x) for x in re.findall(r"[-+]?\d+(?:\.\d+)?(?:e[-+]?\d+)?", str(v))]


def _case_verdict(r: dict, band_for) -> tuple[str, str]:
    v = common.outcome_verdict(r)
    if v is not None:
        return v
    m = r.get("measured", {})
    p95 = next((x for k, x in m.items() if "RMS deviation, 95th percentile" in k), None)
    med = next(
        (x for k, x in m.items() if "median parameter difference" in k and ".mat" in k), None
    )
    fd = m.get("FD (Power, 50 mm) correlation")
    ident = next((x for k, x in r.get("reported", {}).items() if "identical as text" in k), None)
    base = r["case"].removesuffix("-reports")
    for suffix in ("-repeat2", "-repeat3"):
        base = base.removesuffix(suffix)
    band = band_for(base)
    if p95 is None:
        par = next((x for k, x in m.items() if k.endswith(".par: median difference")), None)
        what = f"median .par difference {par} (mm / °)" if par else "files and messages compared"
        return (
            "open",
            f"Within the record's thresholds ({what}); no matrices to set against mcflirt's own variability.",
        )
    p95v = _value(p95)[0]
    medv = _value(med) if med else [0.0, 0.0]
    text = (
        f"95th-percentile RMS deviation {p95v:.3g} mm, median Δ {medv[0]:.3g} mm / {medv[1]:.3g}°"
    )
    if ident:
        text += f"; {ident} matrices identical as text"
    if band is None:
        return (
            "open",
            f"{text}: within the record's thresholds (0.1 mm; 0.05 mm / 0.05°); mcflirt's own "
            "variability is not measured for this run, so the difference is not yet set "
            "against it.",
        )
    fdv = _value(fd)[0] if fd is not None and "reported" not in str(fd) else None
    # the record prints its values to four decimals: half a unit of the last is the doubt
    tol = 0.5e-4
    ok = (
        p95v <= band["rms_p95"] + tol
        and medv[0] <= band["param_mm_median"] + tol
        and medv[1] <= band["param_deg_median"] + tol
        and (fdv is None or fdv >= band["fd_r"] - tol)
    )
    btext = (
        f"mcflirt's own: {band['rms_p95']:.3g} mm, {band['param_mm_median']:.3g} mm / "
        f"{band['param_deg_median']:.3g}°"
        + (f", FD r {band['fd_r']:.4g}" if fdv is not None else "")
    )
    if ok:
        return "within-variability", f"{text}" + (
            f", FD r {fdv:.4g}" if fdv is not None else ""
        ) + f"; {btext}"
    return (
        "open",
        f"{text}"
        + (f", FD r {fdv:.4g}" if fdv is not None else "")
        + f": above mcflirt's own variability ({btext}), within the record's thresholds",
    )


def _cases(ctx: core.Context, rec) -> list[dict]:
    from larmorx_validation.parity.mri_hmc_band import band_for

    descs = {}
    for t in rec.tables:
        if "Case" in t.columns and "What it tests" in t.columns:
            for r in t.rows:
                descs[records.strip_code(r["Case"])] = r["What it tests"]
    out = []
    root = None
    try:
        from larmorx_validation.parity.mri_hmc import oracle_dir

        root = oracle_dir()
    except ImportError:
        pass
    for r in rec.results:
        v = _case_verdict(r, band_for)
        cmd = ""
        if root is not None and (root / "runs" / r["case"] / "run.json").is_file():
            runj = json.loads((root / "runs" / r["case"] / "run.json").read_text(encoding="utf-8"))
            if runj.get("steps"):
                cmd = " ".join(
                    core.short_path(a).split("/")[-1] if "/" in a else a
                    for a in ["mcflirt", *runj["steps"][0]["argv"][1:]]
                )
        out.append(
            {
                "id": r["case"],
                "category": r["category"],
                "description": descs.get(r["case"], ""),
                "command": cmd,
                "results": {
                    "original": {
                        "outcome": r["status"],
                        "verdict": v[0],
                        "checks": f"{r['checks_passed']}/{r['checks_total']}",
                        "reason": common.clip(v[1], 400),
                    }
                },
            }
        )
    return out


def _replica_cases(
    ctx: core.Context,
    cases: list[dict],
    record: str = REPLICA_RECORD,
    impl: str = "replica",
    version: str = fsl_step.OLD,
) -> bool:
    """A replica record (``docs/validation/<record>.json``, against FSL ``version``) in the
    case explorer, as ``impl``'s results: its replay of a recorded mcflirt run joins that run's
    case; its edge cases and fMRIPrep's calls are cases of their own (joined by the next record).
    False when the record is not there."""
    path = ctx.repo / "docs" / "validation" / f"{record}.json"
    if not path.is_file():
        return False
    rec = json.loads(path.read_text(encoding="utf-8"))
    by_id = {c["id"]: c for c in cases}
    md = ctx.repo / "docs" / "validation" / f"{record}.md"
    descs: dict[str, str] = {}
    if md.is_file():
        for line in md.read_text(encoding="utf-8").splitlines():
            cells = [c.strip() for c in line.strip().strip("|").split("|")]
            if len(cells) >= 3 and cells[0].startswith("`") and cells[0].endswith("`"):
                descs.setdefault(cells[0].strip("`"), cells[2] if len(cells) > 2 else "")
    for r in rec["results"]:
        kind, _, name = r["case"].partition("/")
        fix = next(
            (
                str(c.get("detail", ""))
                for c in r.get("failed_checks", [])
                if c.get("name") == "D13 fix pass"
            ),
            "",
        )
        files = (r.get("measured") or {}).get("every file bit-identical")
        if r["status"] == "pass":
            verdict = "bit-identical"
            reason = (
                "Every file mcflirt writes"
                + (f" ({files})" if files else "")
                + f", its exit status and both streams identical to mcflirt's (FSL {version} as "
                "built), with the replica in reproduce mode."
            )
        elif r["status"] == "both-error":
            verdict = "both-reject"
            reason = common.clip(
                "mcflirt and the replica fail alike, with the same messages and status: "
                + r["reason"],
                400,
            )
        elif r["status"] == "expected-divergence":
            verdict = "divergence"
            reason = common.clip(r["reason"], 400)
        elif r["status"] == "skipped":
            continue
        else:
            verdict = "open"
            reason = common.clip(
                "; ".join(f"{c['name']}: {c.get('detail', '')}" for c in r.get("failed_checks", []))
                or r["reason"],
                400,
            )
        if fix:
            reason += " " + fix
        result = {
            "outcome": r["status"],
            "verdict": verdict,
            "checks": f"{r['checks_passed']}/{r['checks_total']}",
            "reason": common.clip(reason, 400),
        }
        if kind == "recorded" and name in by_id:
            by_id[name]["results"][impl] = result
            continue
        if kind != "recorded" and r["case"] in by_id:
            by_id[r["case"]]["results"][impl] = result
            continue
        # A recorded run the original's record does not have (its full tier) keeps the
        # original's names: its run name and category.
        category = {
            "edge": "replica: edge cases",
            "fmriprep": "replica: fMRIPrep's calls",
        }.get(kind, r["category"].removeprefix("recorded: "))
        c = {
            "id": name if kind == "recorded" else r["case"],
            "category": category,
            "description": descs.get(r["case"], ""),
            "command": "",
            "results": {impl: result},
        }
        cases.append(c)
        by_id[c["id"]] = c
    return True


def _open_table(cases: list[dict]) -> dict:
    """The open cases by why they are open."""
    kinds = [
        ("fail", "misses a threshold of the record", "the record's failure"),
        (
            "above",
            "above mcflirt's own variability in at least one estimate, within the record's "
            "thresholds",
            "mcflirt's band is measured for these runs",
        ),
        (
            "noband",
            "within the record's thresholds; mcflirt's variability not measured for the run",
            "options and synthetic series: their band is not measured yet",
        ),
        (
            "nomats",
            "within the record's thresholds; no matrices to set against it",
            "runs that write no matrices, or whose motion only one parameter can show",
        ),
    ]
    count = dict.fromkeys([k for k, *_ in kinds], 0)
    for c in cases:
        if "original" not in c["results"]:
            continue
        r = c["results"]["original"]
        if r["verdict"] != "open":
            continue
        why = r["reason"]
        k = (
            "fail"
            if r["outcome"] == "fail"
            else "above"
            if "above mcflirt" in why
            else "nomats"
            if "no matrices" in why
            else "noband"
        )
        count[k] += 1
    return common.table(
        "open",
        "The open cases, by why they are open",
        ["Why", {"label": "cases", "num": True}, "What would close them"],
        [
            {"head": label, "cells": [common.cell(count[k], "count"), {"text": close}]}
            for k, label, close in kinds
        ],
        note="From the case explorer below: each case's reason gives its numbers.",
    )


def _replica_source(ctx: core.Context) -> list:
    """The replica's records, as the provenance names a record (none before they are made)."""
    out = []
    for name in (REPLICA_RECORD, REPLICA_RECORD_NEW):
        if (ctx.repo / "docs" / "validation" / f"{name}.json").is_file():
            out += records.load(ctx.repo, name).source()
    return out


def export(ctx: core.Context) -> dict:
    reg = core.register(ctx.repo)
    rec = records.load(ctx.repo, TOOL)
    cases = _cases(ctx, rec)
    open_table = _open_table(cases)
    has_replica = _replica_cases(ctx, cases)
    has_replica_new = _replica_cases(ctx, cases, REPLICA_RECORD_NEW, "replica-23", fsl_step.NEW)
    bench = records.load_bench(ctx.repo, TOOL)
    from larmorx_validation.figures.mri_hmc import SPEED_LABELS

    series = [
        {
            "id": "mcflirt (FSL 6.0.7)|1 thread",
            "label": "mcflirt (single-threaded)",
            "mark": "original",
        },
        {"id": "larmorx (CLI)|1 thread", "label": "larmorx, 1 thread", "mark": "larmorx"},
        {"id": "larmorx (CLI)|12 threads", "label": "larmorx, 12 threads", "mark": "larmorx-open"},
    ]
    speed = common.speed(
        bench,
        series,
        SPEED_LABELS,
        title="Seconds for the whole command, median of repeated runs, log scale; the factor is "
        "the clean-room original's speed-up at one thread",
        base=(series[0]["id"], series[1]["id"]),
        note="From the benchmark report `docs/benchmarks/mri-hmc.json` ({cpu}, {cpus} logical "
        "CPUs).",
    )
    e = entry(ctx)
    return {
        "schema": 1,
        "tool": TOOL,
        "title": "Head-motion correction",
        "provenance": {
            "exporter": core.tree_hash(ctx.repo, "validation/src/larmorx_validation/evidence"),
            "records": rec.source() + _replica_source(ctx),
            "benchmarks": bench.source() if bench else [],
            "cache": [
                fcache.provenance(e),
                fcache.provenance(replica_entry(ctx)),
                fcache.provenance(entry_new(ctx)),
            ],
            "command": f"python -m larmorx_validation evidence {TOOL} --site <site>",
        },
        "implementations": [
            common.implementation(
                cases,
                "original",
                f"larmorx clean-room original, against FSL {fsl_step.NEW} (standard tier)",
                "original",
                f"docs/validation/{TOOL}.md",
                "experimental",
            ),
            *(
                [
                    common.implementation(
                        cases,
                        "replica",
                        "larmorx-nc replica, against FSL 6.0.7.17 (full tier)",
                        "replica",
                        f"docs/validation/{REPLICA_RECORD}.md",
                        "validated",
                        name=REPLICA_RECORD,
                    )
                ]
                if has_replica
                else []
            ),
            *(
                [
                    common.implementation(
                        cases,
                        "replica-23",
                        "larmorx-nc replica, against FSL 6.0.7.23 (standard tier)",
                        "replica, 6.0.7.23",
                        f"docs/validation/{REPLICA_RECORD_NEW}.md",
                        "validated",
                        name=REPLICA_RECORD_NEW,
                    )
                ]
                if has_replica_new
                else []
            ),
        ],
        "cases": cases,
        "tables": [open_table],
        "cases_total": len(cases),
        "speed": speed,
        "examples": [_example(ctx, reg)],
    }


if __name__ == "__main__":
    from larmorx_validation.figures.core import REPO, default_cache

    os.environ.setdefault("PYTHONUNBUFFERED", "1")
    print("done", entry(core.Context(repo=REPO, site=Path("."), cache=default_cache())).dir)
