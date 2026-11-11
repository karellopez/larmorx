# SPDX-License-Identifier: Apache-2.0
"""Black-box reports of ``t2smap`` programs on the spec's probe cases, for the clean-room original.

Written from ``specs/t2smap.md`` and runs of the programs only (the clean-room part of
2026-11-11): it reuses the spec's probe (``validation/investigations/t2smap-spec/blackbox.py``:
its inputs, cases, environment and report form) and adds cases for questions the spec leaves to
black-box runs, then writes one report per program::

    python -P compare.py <work dir> <name> <program word>...

``<name>`` names the report (``<work>/<name>.txt``) and the program's own work folder
(``<work>/<name>/``). tedana's program (the launcher's path ends in ``t2smap``) runs through the
oracle-output cache (``oracle_cache.run``, the family ``python``); every other program runs
directly. With ``--check <reference name>`` each case that both programs completed is judged by
the spec's ``check.py`` (§17.3) against that reference's output folder, and the verdicts are
appended to the report. ``--only a,b`` runs those cases only; ``--env NAME=VALUE`` adds a
variable (``LARMORX_UPSTREAM_DEFECTS=reproduce``). Then ``diff <work>/<a>.txt <work>/<b>.txt``.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np

HERE = Path(__file__).resolve().parent
SPEC = HERE.parent / "t2smap-spec"
sys.path.insert(0, str(SPEC))
import blackbox  # noqa: E402  (the spec's probe: inputs, cases, report form)

E3 = blackbox.E3
TES = blackbox.TES
MASK = blackbox.MASK


def extra_inputs(d: Path) -> None:
    """Inputs of the added cases (seeded numpy, plain arithmetic, as the probe's)."""
    ech = blackbox.series()
    nan2 = ech[1].copy()
    nan2[7, 6, 3, 2] = np.nan  # a count-1 voxel whose echo-2 sample is NaN (group 2's fit)
    blackbox.save(nan2, d / "nan-e2.nii.gz")
    inf1 = ech[0].copy()
    inf1[7, 6, 3, 2] = np.inf  # an infinite first-echo mean (the dropout percentile)
    blackbox.save(inf1, d / "inf-e1.nii.gz")
    blackbox.save(-ech[1], d / "neg-e2.nii.gz")  # echo 2 bad everywhere: every count 1
    for i, e in enumerate(ech, 1):
        blackbox.save(e[..., :1], d / f"one-e{i}.nii.gz")
        c = e.copy()
        c[7, 6, 3, :] = c[7, 6, 3, 0]  # a voxel constant over time (PAID's zero weights)
        blackbox.save(c, d / f"const-e{i}.nii.gz")
    _, _, head = blackbox.phantom()
    one = np.zeros(head.shape, dtype=np.uint8)
    one[7, 6, 3] = 1
    blackbox.save(one, d / "mask-one.nii.gz", dtype=np.uint8)
    mask = np.asanyarray(nib.load(str(d / "mask.nii.gz")).dataobj)
    blackbox.save(mask[..., None], d / "mask-4d.nii.gz", dtype=np.uint8)
    blackbox.save(mask[:, :, :-1], d / "mask-grid.nii.gz", dtype=np.uint8)
    # voxels with one good echo among the others: half a slab with echo 2 rising (the decay
    # test fails: count 1, the two-echo fit's start has T2* < 0) and six voxels whose echoes 2
    # and 3 fall to a twentieth and a hundredth of echo 1 (count 1 by the dropout test, too few
    # to move its thresholds: the two-echo fit converges)
    mixed = ech[1].copy()
    mixed[0::2, :, 3, :] = ech[0][0::2, :, 3, :] * 1.2
    mixed[6:9, 6:8, 4, :] = ech[0][6:9, 6:8, 4, :] * 0.05
    blackbox.save(mixed, d / "mixed4-e2.nii.gz")
    mixed3 = ech[2].copy()
    mixed3[6:9, 6:8, 4, :] = ech[0][6:9, 6:8, 4, :] * 0.01
    blackbox.save(mixed3, d / "mixed4-e3.nii.gz")


def files(names: list[str]) -> list[str]:
    return ["-d", *(f"<I>/{n}" for n in names)]


EXTRA: list[tuple[str, list[str], int]] = [
    ("x-nan-echo2", [*files(["e1.nii.gz", "nan-e2.nii.gz", "e3.nii.gz"]), *TES, *MASK], 1),
    ("x-inf-echo1", [*files(["inf-e1.nii.gz", "e2.nii.gz", "e3.nii.gz"]), *TES, *MASK], 1),
    ("x-cap-zero", [*files(["e1.nii.gz", "neg-e2.nii.gz", "e3.nii.gz"]), *TES, *MASK], 1),
    ("x-one-volume", [*files([f"one-e{i}.nii.gz" for i in (1, 2, 3)]), *TES, *MASK], 1),
    (
        "x-paid-one-volume",
        [*files([f"one-e{i}.nii.gz" for i in (1, 2, 3)]), *TES, *MASK, "--combmode", "paid"],
        1,
    ),
    (
        "x-paid-constant",
        [*files([f"const-e{i}.nii.gz" for i in (1, 2, 3)]), *TES, *MASK, "--combmode", "paid"],
        1,
    ),
    ("x-single-voxel", [*E3, *TES, "--mask", "<I>/mask-one.nii.gz"], 1),
    ("x-mask-4d", [*E3, *TES, "--mask", "<I>/mask-4d.nii.gz"], 1),
    ("x-mask-grid", [*E3, *TES, "--mask", "<I>/mask-grid.nii.gz"], 1),
    ("x-n-independent-negative", [*E3, *TES, *MASK, "--n-independent-echos", "-1"], 1),
    (
        "x-dummy-negative",
        [*E3, *TES, *MASK, "--dummy-scans", "-1", "--exclude", "0"],
        1,
    ),
    ("x-te-single-inf", ["-d", "<I>/e1.nii.gz", "-e", "inf", *MASK], 1),
    ("x-debug-ts", [*E3, *TES, *MASK, "--fitmode", "ts", "--debug"], 1),
    ("x-help-40", ["-h"], 1),
    # the curve fit (relay 2b): failures of voxels with one good echo (TED-08), the failure
    # counts' messages, the variances, the per-volume fits, one volume, an infinite sample, two
    # echoes per volume (m = 2: SciPy's warning), the used volumes, no mask, PAID per volume
    (
        "x-curvefit-decay",
        [*E3, *TES, *MASK, "--masktype", "decay", "--fittype", "curvefit", "--debug", "--verbose"],
        1,
    ),
    (
        "x-ts-curvefit-decay",
        [*E3, *TES, *MASK, "--masktype", "decay", "--fittype", "curvefit", "--fitmode", "ts"]
        + ["--debug", "--verbose"],
        1,
    ),
    (
        "x-one-volume-curvefit",
        [*files([f"one-e{i}.nii.gz" for i in (1, 2, 3)]), *TES, *MASK]
        + ["--fittype", "curvefit", "--verbose", "--debug"],
        1,
    ),
    (
        "x-inf-sample-curvefit",
        [*files(["e1.nii.gz", "inf-e2.nii.gz", "e3.nii.gz"]), *TES, *MASK]
        + ["--fittype", "curvefit", "--debug", "--verbose"],
        1,
    ),
    (
        "x-two-echoes-ts-curvefit",
        [*files(["e1.nii.gz", "e2.nii.gz"]), "-e", "0.012", "0.028", *MASK]
        + ["--fittype", "curvefit", "--fitmode", "ts", "--verbose"],
        1,
    ),
    (
        "x-curvefit-used-volumes",
        [*E3, *TES, *MASK, "--fittype", "curvefit", "--dummy-scans", "1", "--exclude", "2"]
        + ["--debug"],
        1,
    ),
    ("x-curvefit-no-mask", [*E3, *TES, "--fittype", "curvefit", "--debug"], 1),
    (
        "x-curvefit-one-echo-voxels",
        [*files(["e1.nii.gz", "mixed4-e2.nii.gz", "mixed4-e3.nii.gz"]), *TES, *MASK]
        + ["--masktype", "dropout", "decay", "--fittype", "curvefit", "--verbose", "--debug"],
        1,
    ),
    (
        "x-ts-curvefit-one-echo-voxels",
        [*files(["e1.nii.gz", "mixed4-e2.nii.gz", "mixed4-e3.nii.gz"]), *TES, *MASK]
        + ["--masktype", "dropout", "decay", "--fittype", "curvefit", "--fitmode", "ts"]
        + ["--verbose", "--debug"],
        1,
    ),
    (
        "x-cap-zero-curvefit",
        [*files(["e1.nii.gz", "neg-e2.nii.gz", "e3.nii.gz"]), *TES, *MASK]
        + ["--fittype", "curvefit", "--verbose", "--debug"],
        1,
    ),
    (
        "x-ts-curvefit-paid",
        [*E3, *TES, *MASK, "--fittype", "curvefit", "--fitmode", "ts", "--combmode", "paid"],
        1,
    ),
]
NO_OUT_DIR = blackbox.NO_OUT_DIR | {"x-help-40"}
CASE_ENV = {"x-help-40": {"COLUMNS": "40"}}


def run_case(program: list[str], full: list[str], case: Path, env: dict, never: str):
    if program[0].endswith("t2smap"):
        from larmorx_validation import oracle_cache

        policy = oracle_cache.Policy(threads=1, family="python", never=never)
        res = oracle_cache.run(
            [*program, *full],
            env=env,
            cwd=case,
            outputs=[case / "out"],
            policy=policy,
            timeout=900,
            runner="t2smap-original-compare",
        )
        return (
            res.returncode,
            res.stdout_bytes.decode("utf-8", "replace"),
            res.stderr_bytes.decode("utf-8", "replace"),
        )
    proc = subprocess.run(
        [*program, *full], cwd=case, env=env, capture_output=True, text=True, timeout=900
    )
    return proc.returncode, proc.stdout, proc.stderr


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("work", type=Path)
    p.add_argument("name")
    p.add_argument("program", nargs="+")
    p.add_argument("--check", default=None)
    p.add_argument("--only", default=None)
    p.add_argument("--env", action="append", default=[])
    a = p.parse_args(argv)
    root = a.work.resolve() / a.name
    if root.exists():
        import shutil

        shutil.rmtree(root)
    root.mkdir(parents=True)
    # the inputs are shared by every report of the work folder (the same paths in every call
    # file); made once, seeded, so every program reads the same bytes
    inputs = a.work.resolve() / "inputs"
    if not (inputs / "mixed4-e3.nii.gz").exists():
        if inputs.exists():
            import shutil

            shutil.rmtree(inputs)
        blackbox.make_inputs(inputs)
        extra_inputs(inputs)
    env = blackbox.environment()
    for kv in a.env:
        k, _, v = kv.partition("=")
        env[k] = v
    only = set(a.only.split(",")) if a.only else None
    lines: list[str] = []
    done: list[str] = []
    for name, args, runs in [*blackbox.CASES, *EXTRA]:
        if only and name not in only:
            continue
        case = root / name
        case.mkdir()
        full = [w.replace("<I>", str(inputs)) for w in args]
        if name not in NO_OUT_DIR:
            full = [*full, "--out-dir", "out"]
        lines.append(f"== {name}: {' '.join(args)}")
        cenv = {**env, **CASE_ENV.get(name, {})}
        for run in range(runs):
            never = "" if run == 0 else "a second run depends on the first run's outputs"
            code, out, err = run_case(a.program, full, case, cenv, never)
            lines.append(f"-- run {run + 1}: exit {code}")
            lines += [
                f"  out| {s}" for s in blackbox.normalise(out.replace(str(inputs), "<I>"), root)
            ]
            lines += [
                f"  err| {s}" for s in blackbox.normalise(err.replace(str(inputs), "<I>"), root)
            ]
        lines += [s.replace(str(inputs), "<I>") for s in blackbox.describe_dir(case / "out", root)]
        if (case / "out" / "t2smap_call.sh").exists():
            done.append(name)
    if a.check:
        ref_root = a.work.resolve() / a.check
        for name in done:
            ref = ref_root / name / "out"
            if (
                not (ref / "desc-optcom_bold.nii.gz").exists()
                and not (ref / "ts_OC.nii.gz").exists()
            ):
                continue
            proc = subprocess.run(
                [
                    sys.executable,
                    "-I",
                    str(SPEC / "check.py"),
                    str(ref),
                    str(root / name / "out"),
                    "--cwd",
                    str(ref_root / name),
                    "--details",
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            lines.append(f"== check {name}: exit {proc.returncode}")
            lines += [f"  {s}" for s in proc.stdout.splitlines()]
            lines += [f"  err| {s}" for s in proc.stderr.splitlines()[-5:]]
    (a.work.resolve() / f"{a.name}.txt").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"COMPARE-END {a.name}: {len(done)} cases with outputs")
    return 0


if __name__ == "__main__":
    os.environ.setdefault("PYTHONHASHSEED", "0")
    sys.exit(main(sys.argv[1:]))
