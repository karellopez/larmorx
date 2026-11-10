# SPDX-License-Identifier: Apache-2.0
"""One run of the sdcflows-estimation evidence, in the Python environment that runs this
module (``oracles/python-newest``: numpy 2.5.4, SciPy 1.18.1, larmorx's reference), as
``sdcflows_fieldmap_run`` runs part A's steps.

Usage::

    python -m larmorx_validation.evidence.sdcflows_estimation_run <task> <arg> <out_dir> \\
        [--threads N] [--defects fix|reproduce] \\
        [--implementation sdcflows|larmorx|ants|fmriprep] [--libraries environment|fmriprep] \\
        [--maths cr|glibc] [--never <reason>]

Tasks:

- ``app``: LARMORX's port of sdcflows' BIDS app (``lx.sdcflows.run_app``) on the dataset and
  participant of sdcflows' own run ``<arg>`` (``oracles/sdcflows-2.17.1/<arg>/RUN_INFO.txt``),
  as the field maps' ``check_app.py --sections run`` runs it, but at ``--threads`` (its
  ``--nprocs`` and ``--omp-nthreads``) and in the ``--defects`` mode: ``<out_dir>/out`` and
  ``<out_dir>/work``.
- ``epi-mask``: sdcflows 2.17.1's ``epi_mask`` (``--implementation sdcflows``) or
  ``lx.sdcflows.epi_mask`` in the ``--defects`` mode (``larmorx``) of the image ``<arg>``:
  ``<out_dir>/mask.nii.gz``.
- ``masker``: sdcflows 2.17.1's ``brain_masker`` (``BrainExtraction``'s, ``--implementation
  sdcflows``) or ``lx.sdcflows.brain_masker`` at ``--threads`` in the ``--defects`` mode, with
  the running environment's numpy and SciPy or, with ``--libraries fmriprep``, fMRIPrep 25.2.6's
  image's (numpy 2.2.6, SciPy 1.15.2), of the image ``<arg>``: ``clipped_mask.nii.gz``,
  ``clipped_probseg.nii.gz`` and ``clipped_brainmasked.nii.gz`` in ``<out_dir>``.
- ``syn``: the fieldmap-less estimation of fMRIPrep's estimator ``<arg>`` (its working folder
  ``.../fmap_preproc_wf/wf_<estimator>``), fed fMRIPrep's own inputs and seed, as the field
  maps' ``check_syn_sdc.py`` feeds them: ``--implementation larmorx`` runs
  ``lx.sdcflows.syn_sdc_workflow`` at ``--threads`` in the ``--defects`` mode with fMRIPrep's
  image's numpy (2.2.6) for the clip; ``--implementation ants`` runs ANTs 2.6.5's
  ``antsRegistration`` (the GCC 14.3 oracle, through the oracle cache) on fMRIPrep's own command
  line at ``--threads`` with ``--maths`` (``cr``: CORE-MATH's correctly rounded functions
  preloaded; ``glibc``: the C library's, as fMRIPrep's image runs ANTs), never cached with
  ``--never``'s reason, then the steps after the registration (``extract_field``,
  ``zooms_field``, ``bs_filter``, ``unwarp``) by LARMORX's port in reproduce mode, fed
  fMRIPrep's node inputs (= sdcflows' nodes on these inputs: ``check_syn_sdc.py --sections
  nodes``); ``--implementation fmriprep`` runs those steps on fMRIPrep's own warp (its 4
  threads): their outputs must be fMRIPrep's own files. Files ``fmap`` (Hz), ``ref`` (the
  corrected reference), ``coeff`` and ``warp``.
- ``syn-warps``: the warps of the ``syn`` runs in ``<arg>`` (an entry's folder) compared pairwise
  (``<out_dir>/warps.json``: per pair whether the bytes are equal, the values that differ and
  the largest difference in mm).

``<out_dir>/outputs.json`` holds the exit status, the register entries LARMORX's notices name,
the seconds and the releases.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
import warnings
from pathlib import Path

HERE = Path(__file__).resolve()
#: The field maps' part B investigation (its check scripts' helpers are imported from there).
SCRIPTS = HERE.parents[3] / "investigations" / "sdcflows-fieldmap-b"


def releases() -> dict[str, str]:
    import importlib.metadata as md

    names = ("sdcflows", "niworkflows", "nipype", "scipy", "numpy", "nibabel", "scikit-image")
    out = {n: md.version(n) for n in names}
    out["OPENBLAS_CORETYPE"] = os.environ.get("OPENBLAS_CORETYPE", "")
    return out


def _app(run: str, out: Path, threads: int, mode: str) -> dict:
    sys.path.insert(0, str(SCRIPTS))
    import check_app as ca
    import check_derivatives as cd
    import check_pepolar as cp

    info = cd.run_info(ca.SDC / run)
    out.mkdir(parents=True, exist_ok=True)
    argv = [info["root"], str(out / "out"), "participant", "--participant-label", info["sub"],
            "-w", str(out / "work"), "--nprocs", str(threads), "--omp-nthreads", str(threads),
            "--no-fmapless", "--notrack", "--pdb"]  # fmt: skip
    env = {
        "LARMORX_IMPLEMENTATION": "replica",
        "FSLDIR": cp.fsldir(),
        "LARMORX_SDCFLOWS_LIBRARIES": ca._libraries(),
    }
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        r = ca.run_mine(argv, out, mode, env)
    (out / "stdout.txt").write_text(r["out"], encoding="utf-8")
    (out / "stderr.txt").write_text(r["err"], encoding="utf-8")
    named = sorted({w.message.entry for w in caught if hasattr(w.message, "entry")})
    return {"code": r["code"], "named": named}


def _epi_mask(image: str, out: Path, impl: str, mode: str) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    old = os.getcwd()
    os.chdir(out)
    try:
        with warnings.catch_warnings(record=True) as caught:
            warnings.simplefilter("always")
            if impl == "sdcflows":
                from sdcflows.utils.epimanip import epi_mask

                epi_mask(image)
            else:
                import numpy as np

                import larmorx.sdcflows as ls

                ls.epi_mask(image, "mask.nii.gz", numpy_version=np.__version__,
                            upstream_defects=mode)  # fmt: skip
    finally:
        os.chdir(old)
    named = sorted({w.message.entry for w in caught if hasattr(w.message, "entry")})
    return {"code": 0, "files": {"mask": "mask.nii.gz"}, "named": named}


def _masker(image: str, out: Path, impl: str, mode: str, threads: int, libraries: str) -> dict:
    out.mkdir(parents=True, exist_ok=True)
    target = str(out / "clipped_mask.nii.gz")
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        if impl == "sdcflows":
            from sdcflows.utils.tools import brain_masker

            brain, probseg, mask = brain_masker(image, target)
        else:
            import numpy as np
            import scipy

            import larmorx.sdcflows as ls

            versions = (
                {"numpy_version": "2.2.6", "scipy_version": "1.15.2"}
                if libraries == "fmriprep"
                else {"numpy_version": np.__version__, "scipy_version": scipy.__version__}
            )
            r = ls.brain_masker(image, target, upstream_defects=mode, n_threads=threads,
                                **versions)  # fmt: skip
            brain, probseg, mask = r.out_brain, r.out_probseg, r.out_mask
    named = sorted({w.message.entry for w in caught if hasattr(w.message, "entry")})
    files = {k: Path(v).name for k, v in (("brain", brain), ("probseg", probseg), ("mask", mask))}
    return {"code": 0, "files": files, "named": named}


def _syn_chain(wf: Path, warp: Path, out: Path, threads: int) -> dict[str, str]:
    """sdcflows' steps after the registration (LARMORX's port, reproduce), fed fMRIPrep's node
    inputs and the registration's warp ``warp``."""
    import check_syn_preprocessing as csp
    import check_syn_sdc as css

    from larmorx import sdcflows as sd
    from larmorx.sdcflows.syn import _apply_transforms

    msg = f"FixHeaderApplyTransforms (niworkflows v{css.NIWF_IMAGE})"
    n = wf / "extract_field"
    ex = sd.displacements_field_to_fieldmap(
        str(warp), csp.hosts(n, "epi"), csp.value(n, "ro_time"), csp.value(n, "pe_dir"),
        out_dir=out / "extract_field",
    )  # fmt: skip
    n = wf / "zooms_field"
    cwd = out / "zooms_field"
    cwd.mkdir(parents=True, exist_ok=True)
    field, _cmd = _apply_transforms(
        str(cwd), str(ex.out_file), csp.hosts(n, "reference_image"), ["identity"], None,
        "BSpline", msg, "reproduce", threads, args="-u float",
    )  # fmt: skip
    n = wf / "bs_filter"
    fit = sd.bspline_approx(
        field, None, bs_spacing=csp.value(n, "bs_spacing"), recenter=False, extrapolate=True,
        out_dir=out / "bs_filter", n_threads=threads, upstream_defects="reproduce",
    )  # fmt: skip
    n = wf / "unwarp"
    coeffs = [str(p) for p in fit.out_coeff]
    res = sd.apply_coeffs_field(
        csp.hosts(n, "in_data"), coeffs, csp.value(n, "ro_time"), csp.value(n, "pe_dir"),
        jacobian=False, out_dir=out / "unwarp", n_threads=threads, upstream_defects="reproduce",
    )  # fmt: skip
    return {"fmap": str(res.out_field), "ref": str(res.out_corrected), "coeff": coeffs[0]}


def _syn(wf_dir: str, out: Path, impl: str, mode: str, threads: int, maths: str,
         never: str) -> dict:  # fmt: skip
    sys.path.insert(0, str(SCRIPTS))
    import check_bold_fmap as cbf
    import check_syn_sdc as css

    wf = Path(wf_dir)
    seed = cbf.ants_seed(wf / "syn")
    out.mkdir(parents=True, exist_ok=True)
    extra: dict = {"seed": seed}
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        if impl == "larmorx":
            from larmorx import sdcflows as sd

            res = sd.syn_sdc_workflow(
                **css.wf_inputs(wf), out_dir=out, name=wf.name, omp_nthreads=threads,
                ants_random_seed=seed, niworkflows_version=css.NIWF_IMAGE,
                numpy_version=css.IMAGE_NUMPY, upstream_defects=mode, n_threads=threads,
            )  # fmt: skip
            files = {"fmap": str(res.fmap), "ref": str(res.fmap_ref),
                     "coeff": str(res.fmap_coeff[0]), "warp": str(res.files["syn"][0])}  # fmt: skip
        elif impl == "ants":
            from larmorx_validation.parity.ants_programs import _run_ants

            argv, _seed = css.syn_argv(wf)
            syn = out / "syn"
            syn.mkdir(parents=True, exist_ok=True)
            here = os.getcwd()
            os.environ["ANTS_RANDOM_SEED"] = str(seed)
            t0 = time.perf_counter()
            try:
                os.chdir(syn)
                code, _o, err = _run_ants("antsRegistration", argv, threads=threads,
                                          correctly_rounded=maths == "cr", never=never)  # fmt: skip
            finally:
                os.chdir(here)
                os.environ.pop("ANTS_RANDOM_SEED", None)
            extra["ants_seconds"] = round(time.perf_counter() - t0, 2)
            if code != 0:
                return {"code": code, "stderr": err[-2000:], **extra}
            warp = syn / "fmap_syn0Warp.nii.gz"
            files = {**_syn_chain(wf, warp, out, threads), "warp": str(warp)}
        else:
            warp = wf / "syn" / "fmap_syn0Warp.nii.gz"
            files = {**_syn_chain(wf, warp, out, threads), "warp": str(warp)}
    named = sorted({w.message.entry for w in caught if hasattr(w.message, "entry")})
    rel = {k: os.path.relpath(v, out) if Path(v).is_relative_to(out) else v
           for k, v in files.items()}  # fmt: skip
    return {"code": 0, "files": rel, "named": named, **extra}


def _syn_warps(entry: Path, out: Path) -> dict:
    """The ``syn`` runs' warps (``<entry>/<run>/outputs.json``'s ``warp``) compared pairwise."""
    import itertools

    import nibabel as nib
    import numpy as np

    warps = {}
    for f in sorted(entry.glob("*/outputs.json")):
        rec = json.loads(f.read_text(encoding="utf-8"))
        w = rec.get("files", {}).get("warp")
        if w:
            p = Path(w) if Path(w).is_absolute() else f.parent / w
            warps[f.parent.name] = np.asanyarray(nib.load(str(p)).dataobj)
    pairs = []
    for a, b in itertools.combinations(sorted(warps), 2):
        x, y = warps[a], warps[b]
        d = np.abs(x.astype(np.float64) - y.astype(np.float64))
        pairs.append({"a": a, "b": b, "identical": bool(x.tobytes() == y.tobytes()),
                      "differ": int(np.count_nonzero(d)), "values": int(d.size),
                      "max_abs_mm": float(d.max()), "p99_mm": float(np.percentile(d, 99)),
                      "rms_mm": float(np.sqrt(np.mean(d * d)))})  # fmt: skip
    out.mkdir(parents=True, exist_ok=True)
    (out / "warps.json").write_text(json.dumps(pairs, indent=1) + "\n", encoding="utf-8")
    return {"code": 0, "files": {"warps": "warps.json"}, "named": []}


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("task", choices=("app", "epi-mask", "masker", "syn", "syn-warps"))
    ap.add_argument("arg")
    ap.add_argument("out", type=Path)
    ap.add_argument("--threads", type=int, default=1)
    ap.add_argument("--defects", choices=("fix", "reproduce"), default="reproduce")
    ap.add_argument("--implementation", choices=("sdcflows", "larmorx", "ants", "fmriprep"),
                    default="larmorx")  # fmt: skip
    ap.add_argument("--maths", choices=("cr", "glibc"), default="cr")
    ap.add_argument("--never", default="")
    ap.add_argument("--libraries", choices=("environment", "fmriprep"), default="environment")
    a = ap.parse_args()
    t0 = time.perf_counter()
    if a.task == "app":
        rec = _app(a.arg, a.out, a.threads, a.defects)
    elif a.task == "masker":
        rec = _masker(a.arg, a.out, a.implementation, a.defects, a.threads, a.libraries)
    elif a.task == "syn":
        rec = _syn(a.arg, a.out, a.implementation, a.defects, a.threads, a.maths, a.never)
    elif a.task == "syn-warps":
        rec = _syn_warps(Path(a.arg), a.out)
    else:
        rec = _epi_mask(a.arg, a.out, a.implementation, a.defects)
    rec["seconds"] = round(time.perf_counter() - t0, 2)
    rec["releases"] = releases()
    (a.out / "outputs.json").write_text(json.dumps(rec, indent=1) + "\n", encoding="utf-8")
    return 0 if rec["code"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
