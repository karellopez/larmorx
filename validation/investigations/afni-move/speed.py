# SPDX-License-Identifier: Apache-2.0
"""The speed of the AFNI replicas (in-process since 2026-10-29) and of the clean-room originals
on the standard tiers' real cases: each case's command line through larmorx's entry point
(``larmorx.cli.run``, as the parity suites run it) with ``implementation="replica"`` and
``"original"``, larmorx's default mode, at each thread count, the median wall-clock time of
``--repeats`` runs (after one untimed run). Prints a Markdown table.

Usage (from the checkout, with the built ``_core`` in ``python/larmorx``)::

    PYTHONPATH=python:validation/src python -P validation/investigations/afni-move/speed.py \\
        [--threads 1,8] [--repeats 3]
"""

from __future__ import annotations

import argparse
import contextlib
import io
import os
import statistics
import tempfile
import time
from pathlib import Path


def tshift_cases() -> list[tuple[str, str, object]]:
    import larmorx_testdata as td
    from larmorx_validation.parity import afni_tshift as at

    out = []
    for case in at.suite("replica").cases("standard"):
        if case.category != "real":
            continue
        s = case.payload
        image = (
            at._generated(s.image.removeprefix("gen:"))
            if s.image.startswith("gen:")
            else td.get(s.image)
        )

        def argv(tmp: Path, s=s, image=image) -> list[str]:
            return ["afni", "3dTshift", *at._args(s, tmp), "-prefix", "out.nii.gz", str(image)]

        out.append(("3dTshift", case.id, argv))
    return out


def mask_cases() -> list[tuple[str, str, object]]:
    from larmorx_validation.parity import afni_automask, afni_unifize
    from larmorx_validation.parity import afni_replica as ar

    out = []
    for module, program in ((afni_automask, "3dAutomask"), (afni_unifize, "3dUnifize")):
        spec = module.spec("replica")
        for case in ar.cases_of(spec, "standard"):
            if case.category != "real":
                continue
            s = case.payload
            paths = [str(spec.inputs(name)) for name in s.inputs]

            def argv(tmp: Path, s=s, paths=paths, program=program) -> list[str]:
                return ["afni", program, *(a.format(*paths) for a in s.args)]

            out.append((program, case.id, argv))
    return out


def run_once(argv, implementation: str, threads: int) -> float:
    from larmorx.cli import run

    with tempfile.TemporaryDirectory(prefix="lx-afni-speed-") as tmp:
        cwd = os.getcwd()
        os.environ["OMP_NUM_THREADS"] = str(threads)
        os.environ.pop("LARMORX_UPSTREAM_DEFECTS", None)
        try:
            os.chdir(tmp)
            args = argv(Path(tmp))
            t0 = time.perf_counter()
            with (
                contextlib.redirect_stdout(io.StringIO()),
                contextlib.redirect_stderr(io.StringIO()),
            ):
                code = run(["larmorx", *args], implementation=implementation)
            seconds = time.perf_counter() - t0
        finally:
            os.chdir(cwd)
            os.environ.pop("OMP_NUM_THREADS", None)
    if code != 0:
        raise RuntimeError(f"{implementation} {args}: exit {code}")
    return seconds


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("--threads", default="1,8")
    p.add_argument("--repeats", type=int, default=3)
    a = p.parse_args()
    threads = [int(t) for t in a.threads.split(",")]
    cases = [*tshift_cases(), *mask_cases()]
    print("| Program | Case | Threads | Replica (s) | Original (s) | Original / replica |")
    print("|---|---|---|---|---|---|")
    totals: dict[tuple[str, int], list[float]] = {}
    for program, case_id, argv in cases:
        for t in threads:
            times = {}
            for implementation in ("replica", "original"):
                run_once(argv, implementation, t)
                times[implementation] = statistics.median(
                    run_once(argv, implementation, t) for _ in range(a.repeats)
                )
            ratio = times["original"] / times["replica"]
            totals.setdefault((program, t), []).append(ratio)
            print(
                f"| {program} | `{case_id}` | {t} | {times['replica']:.3f} | "
                f"{times['original']:.3f} | {ratio:.2f} |",
                flush=True,
            )
    print()
    print("| Program | Threads | Cases | Median original / replica |")
    print("|---|---|---|---|")
    for (program, t), ratios in totals.items():
        print(f"| {program} | {t} | {len(ratios)} | {statistics.median(ratios):.2f} |")
    print("\nSPEED-DONE")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
