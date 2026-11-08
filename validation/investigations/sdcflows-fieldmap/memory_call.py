# SPDX-License-Identifier: Apache-2.0
"""One call of sdcflows' field-map estimation, by sdcflows or by larmorx, in a process of its
own: the calls ``tests/parity/test_memory_peak.py`` measures (``test_sdcflows_*``), each
process's peak resident memory from ``/usr/bin/time -v``.

    python -P memory_call.py sdcflows|larmorx fit <fieldmap> <mask> [--threads N] [--env TEXT]
    python -P memory_call.py sdcflows|larmorx apply <bold> <coeff> <fmap2data.mat> <ro_time> \\
        <pe_dir> [--threads N] [--env TEXT] [--defects fix|reproduce]

``fit``: ``BSplineApprox`` as fMRIPrep's ``bs_filter`` calls it (fMRIPrep's spacing, median,
ridge alpha 1e-4, extrapolation, zooms_min 1) against ``lx.sdcflows.bspline_approx``; ``apply``:
``ApplyCoeffsField`` (the field of the coefficients on the series, through the registration's
inverse, unwarped with its Jacobian) against ``lx.sdcflows.apply_coeffs_field``. The outputs go
into the working directory, with sdcflows' file names; ``--threads``: sdcflows' ``num_threads``
and larmorx's ``n_threads`` (BSplineApprox has no thread option); ``--env``: the releases the
caller expects (``releases()``, part of the oracle cache's key, since the cache keys the
interpreter but not its packages): the call refuses (exit status 3) when they are not the
running ones; ``--defects``: larmorx's ``upstream_defects`` (D13). Only the implementation
measured is imported.
"""

from __future__ import annotations

import argparse
import os
import sys


def releases() -> str:
    """The releases a sdcflows call runs with, and numpy's BLAS kernels."""
    import importlib.metadata as md

    names = ("sdcflows", "scikit-learn", "scipy", "numpy", "nibabel", "nitransforms", "nipype")
    out = [f"{n} {md.version(n)}" for n in names]
    return ", ".join(out) + f"; OPENBLAS_CORETYPE={os.environ.get('OPENBLAS_CORETYPE', '')}"


def _quiet() -> None:
    """nipype's log records off (their time stamps would make every run's output differ); set
    after nipype's import, which configures its loggers."""
    import logging

    for name in ("nipype", "nipype.interface", "nipype.workflow", "nipype.utils"):
        logging.getLogger(name).setLevel(logging.ERROR)


def _sdcflows(args: argparse.Namespace) -> None:
    if args.call == "fit":
        from sdcflows.interfaces.bspline import BSplineApprox

        _quiet()
        BSplineApprox(
            in_data=args.inputs[0],
            in_mask=args.inputs[1],
            bs_spacing=[(16.0, 16.0, 10.0)],
            ridge_alpha=1e-4,
            recenter="median",
            extrapolate=True,
            zooms_min=1.0,
        ).run(cwd=os.getcwd())
    else:
        from sdcflows.interfaces.bspline import ApplyCoeffsField

        _quiet()
        bold, coeff, xfm, ro, pe = args.inputs
        ApplyCoeffsField(
            in_data=bold,
            in_coeff=[coeff],
            fmap2data_xfm=xfm,
            ro_time=[float(ro)],
            pe_dir=[pe],
            num_threads=args.threads,
        ).run(cwd=os.getcwd())


def _larmorx(args: argparse.Namespace) -> None:
    import larmorx as lx

    if args.call == "fit":
        lx.sdcflows.bspline_approx(
            args.inputs[0],
            args.inputs[1],
            out_dir=os.getcwd(),
            n_threads=args.threads,
            upstream_defects=args.defects,
        )
    else:
        bold, coeff, xfm, ro, pe = args.inputs
        lx.sdcflows.apply_coeffs_field(
            bold,
            [coeff],
            [float(ro)],
            [pe],
            fmap2data_xfm=xfm,
            out_dir=os.getcwd(),
            n_threads=args.threads,
            upstream_defects=args.defects,
        )


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("implementation", choices=("sdcflows", "larmorx"))
    p.add_argument("call", choices=("fit", "apply"))
    p.add_argument("inputs", nargs="+")
    p.add_argument("--threads", type=int, default=1)
    p.add_argument("--env", default=None)
    p.add_argument("--defects", choices=("fix", "reproduce"), default="fix")
    args = p.parse_args()
    if args.implementation == "sdcflows":
        if args.env is not None and args.env != releases():
            print(f"releases {releases()!r}, not {args.env!r}", file=sys.stderr)
            return 3
        _sdcflows(args)
    else:
        _larmorx(args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
