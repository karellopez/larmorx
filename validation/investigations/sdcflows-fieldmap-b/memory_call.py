# SPDX-License-Identifier: Apache-2.0
"""One call of the field maps' part B, by the original (sdcflows, niworkflows, fMRIPrep) or by
larmorx, in a process of its own: the calls ``tests/parity/test_memory_peak.py`` measures
(``test_fieldmap_steps_need_no_more_memory_than_the_originals``), each process's peak resident
memory from ``/usr/bin/time -v``.

    python -P memory_call.py original|larmorx masker <clipped.nii.gz> [--threads N] [--env TEXT]
    python -P memory_call.py original|larmorx clip <in_file> <p_min> <p_max> [...]
    python -P memory_call.py original|larmorx ravg <in_file> [...]
    python -P memory_call.py original|larmorx recon <coeff> <fmap_ref> <target> <xfm>... \\
        --inverse <true|false>,... [...]

``masker``: sdcflows' ``brain_masker`` (``BrainExtraction``'s) against
``lx.sdcflows.brain_masker``; ``clip``: niworkflows' ``IntensityClip`` (``_advanced_clip``)
against ``lx.niworkflows.intensity_clip``; ``ravg``: niworkflows' ``RobustAverage`` without
head-motion correction (``mc_method=None``: its own steps, the median and the drift; the
3dvolreg it runs otherwise is a program of its own, measured by the 3dvolreg calls) against
``lx.niworkflows.robust_average``; ``recon``: fMRIPrep's ``ReconstructFieldmap`` against
``lx.transforms.reconstruct_fieldmap_node``. The outputs go into the working directory, with the
originals' file names. larmorx follows the running environment's numpy and SciPy (the kept two,
``python_versions``). ``--threads``: larmorx's ``n_threads`` (the originals here have no thread
option); ``--env``: the releases the caller expects (:func:`releases`, part of the oracle cache's
key, since the cache keys the interpreter but not its packages): the call refuses (exit status
3) when they are not the running ones; ``--defects``: larmorx's ``upstream_defects`` (D13). Only
the implementation measured is imported.
"""

from __future__ import annotations

import argparse
import os
import sys


def releases() -> str:
    """The releases an original's call runs with, and numpy's BLAS kernels."""
    import importlib.metadata as md

    names = ("sdcflows", "niworkflows", "fmriprep", "scikit-image", "scipy", "numpy")
    names += ("nibabel", "nitransforms", "nipype")
    out = [f"{n} {md.version(n)}" for n in names]
    return ", ".join(out) + f"; OPENBLAS_CORETYPE={os.environ.get('OPENBLAS_CORETYPE', '')}"


def _versions() -> tuple[str, str]:
    """numpy's and SciPy's releases larmorx follows: the running ones (a kept environment)."""
    import numpy
    import scipy

    return numpy.__version__, scipy.__version__


def _quiet() -> None:
    """nipype's log records off (their time stamps would make every run's output differ); set
    after nipype's import, which configures its loggers."""
    import logging

    for name in ("nipype", "nipype.interface", "nipype.workflow", "nipype.utils"):
        logging.getLogger(name).setLevel(logging.ERROR)


def _inverse(text: str) -> list[bool]:
    return [w.strip().lower() == "true" for w in text.split(",") if w.strip()]


def _original(args: argparse.Namespace) -> None:
    cwd = os.getcwd()
    if args.call == "masker":
        from sdcflows.utils.tools import brain_masker

        brain_masker(args.inputs[0], os.path.join(cwd, "clipped_mask.nii.gz"))
    elif args.call == "clip":
        from niworkflows.interfaces.nibabel import _advanced_clip

        src, p_min, p_max = args.inputs
        _advanced_clip(src, p_min=float(p_min), p_max=float(p_max), newpath=cwd)
    elif args.call == "ravg":
        from niworkflows.interfaces.images import RobustAverage

        _quiet()
        RobustAverage(in_file=args.inputs[0], mc_method=None).run(cwd=cwd)
    else:
        from fmriprep.interfaces.resampling import ReconstructFieldmap

        _quiet()
        coeff, fmap_ref, target, *xfms = args.inputs
        ReconstructFieldmap(
            in_coeffs=[coeff],
            fmap_ref_file=fmap_ref,
            target_ref_file=target,
            transforms=xfms,
            inverse=_inverse(args.inverse),
        ).run(cwd=cwd)


def _larmorx(args: argparse.Namespace) -> None:
    import larmorx as lx

    cwd = os.getcwd()
    np_v, sp_v = _versions()
    common = {"upstream_defects": args.defects, "n_threads": args.threads}
    if args.call == "masker":
        lx.sdcflows.brain_masker(
            args.inputs[0],
            os.path.join(cwd, "clipped_mask.nii.gz"),
            numpy_version=np_v,
            scipy_version=sp_v,
            **common,
        )
    elif args.call == "clip":
        src, p_min, p_max = args.inputs
        lx.niworkflows.intensity_clip(
            src, float(p_min), float(p_max), out_dir=cwd, numpy_version=np_v, **common
        )
    elif args.call == "ravg":
        lx.niworkflows.robust_average(
            args.inputs[0], mc_method=None, out_dir=cwd, numpy_version=np_v, **common
        )
    else:
        coeff, fmap_ref, target, *xfms = args.inputs
        lx.transforms.reconstruct_fieldmap_node(
            [coeff], target, fmap_ref, xfms, _inverse(args.inverse), out_dir=cwd, **common
        )


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("implementation", choices=("original", "larmorx"))
    p.add_argument("call", choices=("masker", "clip", "ravg", "recon"))
    p.add_argument("inputs", nargs="+")
    p.add_argument("--inverse", default="")
    p.add_argument("--threads", type=int, default=1)
    p.add_argument("--env", default=None)
    p.add_argument("--defects", choices=("fix", "reproduce"), default="fix")
    args = p.parse_args()
    if args.implementation == "original":
        if args.env is not None and args.env != releases():
            print(f"releases {releases()!r}, not {args.env!r}", file=sys.stderr)
            return 3
        _original(args)
    else:
        _larmorx(args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
