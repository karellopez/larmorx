# SPDX-License-Identifier: Apache-2.0
"""One run of the sdcflows-estimation evidence, in the Python environment that runs this
module (``oracles/python-newest``: numpy 2.5.4, SciPy 1.18.1, larmorx's reference), as
``sdcflows_fieldmap_run`` runs part A's steps.

Usage::

    python -m larmorx_validation.evidence.sdcflows_estimation_run <task> <arg> <out_dir> \\
        [--threads N] [--defects fix|reproduce] [--implementation sdcflows|larmorx] \\
        [--libraries environment|fmriprep]

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


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("task", choices=("app", "epi-mask", "masker"))
    ap.add_argument("arg")
    ap.add_argument("out", type=Path)
    ap.add_argument("--threads", type=int, default=1)
    ap.add_argument("--defects", choices=("fix", "reproduce"), default="reproduce")
    ap.add_argument("--implementation", choices=("sdcflows", "larmorx"), default="larmorx")
    ap.add_argument("--libraries", choices=("environment", "fmriprep"), default="environment")
    a = ap.parse_args()
    t0 = time.perf_counter()
    if a.task == "app":
        rec = _app(a.arg, a.out, a.threads, a.defects)
    elif a.task == "masker":
        rec = _masker(a.arg, a.out, a.implementation, a.defects, a.threads, a.libraries)
    else:
        rec = _epi_mask(a.arg, a.out, a.implementation, a.defects)
    rec["seconds"] = round(time.perf_counter() - t0, 2)
    rec["releases"] = releases()
    (a.out / "outputs.json").write_text(json.dumps(rec, indent=1) + "\n", encoding="utf-8")
    return 0 if rec["code"] == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
