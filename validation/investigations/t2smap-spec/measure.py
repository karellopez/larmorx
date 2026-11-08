# SPDX-License-Identifier: Apache-2.0
"""Runs behind ``specs/t2smap.md`` §17.3 and §18: the model against tedana, and tedana against
itself (its own spread), checked by ``check.py``.

Written from the spec and black-box runs only (a clean-room implementer may read and run it)::

    python -I measure.py <work dir> --tedana <launcher dir> [--fmriprep <oracle runs dir>]
        [--jobs N] [--only NAME,...]

``<launcher dir>`` holds ``launcher-newest/t2smap`` and ``launcher-shared/t2smap`` (programs
that start tedana 26.0.3 in the newest and the shared library environments). The cases:
``blackbox.py``'s cases that tedana completes (its inputs are made again under the work dir), and,
with ``--fmriprep``, fMRIPrep 25.2.6's four multi-echo calls (``<run>/work/.../t2smap_node/
command.txt``, its paths mapped onto the run's ``work`` directory) and ds003592's with
``loglin``, and with ``--verbose`` (the variance maps on real data). The runs of each case: tedana in the reference setup (``ref``), tedana in the shared
environment (``shared``), with ``--n-threads 4`` and the BLAS threads free (``threads4``), with
numpy's AVX-512 kernels on (``avx512``), the model (``model``) and its ``fast`` variant
(``model-fast``); each non-reference run is checked against ``ref``. The report goes to
standard output and to ``<work dir>/report.json``.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import blackbox  # noqa: E402

FMRIPREP_CALLS = {
    "ds003592-curvefit": "ds003592-sub-280-noreconall/work/fmriprep_25_2_wf/sub_280_ses_1_wf/"
    "bold_ses_1_task_rest_echo_1_wf/bold_native_wf/bold_t2smap_wf/t2smap_node",
    "ds000210-cuedSGT-curvefit": "ds000210-sub-02-noreconall/work/fmriprep_25_2_wf/sub_02_wf/"
    "bold_task_cuedSGT_run_01_echo_1_wf/bold_native_wf/bold_t2smap_wf/t2smap_node",
    "ds000210-rest-curvefit": "ds000210-sub-02-noreconall/work/fmriprep_25_2_wf/sub_02_wf/"
    "bold_task_rest_run_01_echo_1_wf/bold_native_wf/bold_t2smap_wf/t2smap_node",
    "ds000258-rest-curvefit": "ds000258-sub-21262-noreconall/work/fmriprep_25_2_wf/sub_21262_wf/"
    "bold_task_rest_echo_1_wf/bold_native_wf/bold_t2smap_wf/t2smap_node",
}
SPREAD_CASES = {"loglin", "curvefit", "ts-curvefit-verbose", "paid", "exclude", "int16", "no-mask"}
RUNS = ("ref", "shared", "threads4", "avx512", "model", "model-fast")


def fmriprep_args(runs: Path, node: str) -> list[str]:
    text = (runs / node / "command.txt").read_text(encoding="utf-8").split()
    work = str(runs / node.split("/")[0] / "work")
    return [w.replace("/work", work, 1) if w.startswith("/work/") else w for w in text[1:]]


def environment(run: str) -> dict[str, str]:
    env = blackbox.environment()
    if run == "threads4":
        env.pop("OPENBLAS_NUM_THREADS", None)
    if run == "avx512":
        env.pop("NPY_DISABLE_CPU_FEATURES", None)
    return env


def command(run: str, args: list[str], tedana: Path) -> list[str]:
    if run.startswith("model"):
        variant = ["--variant", "fast"] if run == "model-fast" else []
        return [sys.executable, "-I", str(HERE / "model.py"), *variant, *args]
    launcher = tedana / ("launcher-shared" if run == "shared" else "launcher-newest") / "t2smap"
    extra = ["--n-threads", "4"] if run == "threads4" else []
    return [str(launcher), *args, *extra]


def run_one(case_dir: Path, run: str, args: list[str], tedana: Path) -> dict:
    d = case_dir / run
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    t0 = time.time()
    proc = subprocess.run(
        command(run, [*args, "--out-dir", "out"], tedana),
        cwd=d,
        env=environment(run),
        capture_output=True,
        text=True,
        check=False,
        timeout=3600,
    )
    (d / "stderr.txt").write_text(proc.stderr, encoding="utf-8")
    return {"exit": proc.returncode, "seconds": round(time.time() - t0, 1)}


def check_one(case_dir: Path, run: str) -> dict:
    proc = subprocess.run(
        [
            sys.executable,
            "-I",
            str(HERE / "check.py"),
            str(case_dir / "ref" / "out"),
            str(case_dir / run / "out"),
            "--cwd",
            str(case_dir / "ref"),
            "--json",
            str(case_dir / f"check-{run}.json"),
            "--details",
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=3600,
    )
    return {
        "check_exit": proc.returncode,
        "lines": proc.stdout.splitlines(),
        "err": proc.stderr[-2000:],
    }


def print_uses(work: Path, report: dict) -> None:
    """The largest use of each allowance (|difference| / allowance) per output and run kind,
    over every case checked; the (b)/(c) line gives the share of the fit voxels."""
    table: dict[str, dict[str, float]] = {}
    for name, runs in report.items():
        for run in runs:
            path = work / name / f"check-{run}.json"
            if run == "ref" or not path.exists():
                continue
            for r in json.loads(path.read_text(encoding="utf-8")):
                val = r["use"]
                if r["name"].startswith("curvefit (b)/(c)"):
                    val = sum(r["by"].values()) / max(r["n"], 1)
                row = table.setdefault(r["name"], {})
                row[run] = max(row.get(run, 0.0), val)
    kinds = [k for k in RUNS[1:] if any(k in row for row in table.values())]
    print("largest use per output (the (b)/(c) share as a fraction of the fit voxels)")
    print("output".ljust(28) + "".join(k.rjust(12) for k in kinds))
    for out, row in table.items():
        print(out.ljust(28) + "".join(f"{row.get(k, float('nan')):12.3g}" for k in kinds))


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("work", type=Path)
    p.add_argument("--tedana", type=Path, required=True)
    p.add_argument("--fmriprep", type=Path, default=None)
    p.add_argument("--jobs", type=int, default=4)
    p.add_argument("--only", default=None)
    p.add_argument("--check-only", action="store_true", help="check the runs already made")
    a = p.parse_args(argv)
    work = a.work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    inputs = work / "inputs"
    if not inputs.exists():
        blackbox.make_inputs(inputs)
    cases: dict[str, tuple[list[str], tuple[str, ...]]] = {}
    for name, args, _ in blackbox.CASES:
        if name in blackbox.NO_OUT_DIR:
            continue
        full = [w.replace("<I>", str(inputs)) for w in args]
        runs = RUNS if name in SPREAD_CASES else ("ref", "model", "model-fast")
        cases[f"bb/{name}"] = (full, runs)
    if a.fmriprep:
        for name, node in FMRIPREP_CALLS.items():
            cases[f"fmriprep/{name}"] = (fmriprep_args(a.fmriprep, node), RUNS)
        args = fmriprep_args(a.fmriprep, FMRIPREP_CALLS["ds003592-curvefit"])
        cases["fmriprep/ds003592-curvefit-verbose"] = ([*args, "--verbose"], RUNS)
        args[args.index("curvefit")] = "loglin"
        cases["fmriprep/ds003592-loglin"] = (args, RUNS)
    if a.only:
        keep = set(a.only.split(","))
        cases = {k: v for k, v in cases.items() if k.split("/")[1] in keep}

    report: dict[str, dict] = {name: {} for name in cases}
    if a.check_only:
        old = json.loads((work / "report.json").read_text(encoding="utf-8"))
        report = {
            k: {r: {"exit": v["exit"], "seconds": v["seconds"]} for r, v in old[k].items()}
            for k in cases
            if k in old
        }
    with ThreadPoolExecutor(a.jobs) as pool:
        # tedana's reference run first: the other runs only where it completes
        refs = (
            {}
            if a.check_only
            else {pool.submit(run_one, work / n, "ref", cases[n][0], a.tedana): n for n in cases}
        )
        for f, name in refs.items():
            report[name]["ref"] = f.result()
        futures = (
            {}
            if a.check_only
            else {
                pool.submit(run_one, work / name, run, args, a.tedana): (name, run)
                for name, (args, runs) in cases.items()
                if report[name]["ref"]["exit"] == 0
                for run in runs[1:]
            }
        )
        for f, (name, run) in futures.items():
            report[name][run] = f.result()
        checks = {
            pool.submit(check_one, work / name, run): (name, run)
            for name, runs in report.items()
            for run, r in runs.items()
            if run != "ref" and r["exit"] == 0
        }
        for f, (name, run) in checks.items():
            report[name][run].update(f.result())
    (work / "report.json").write_text(json.dumps(report, indent=1), encoding="utf-8")
    bad = 0
    for name, runs in report.items():
        ref = runs["ref"]
        print(f"== {name}: tedana exit {ref['exit']}, {ref['seconds']} s")
        for run, r in runs.items():
            if run == "ref":
                continue
            status = r.get("check_exit")
            print(f"   {run}: exit {r['exit']}, {r['seconds']} s, check {status}")
            for line in r.get("lines", []):
                if line.startswith("    ") or (
                    not line.startswith("PASS") or "identical, 0 failed, use 0" not in line
                ):
                    print(f"      {line}")
            if r.get("err"):
                print("      " + r["err"].strip().replace("\n", "\n      "))
            bad += status not in (0, None) or r["exit"] != 0
    print_uses(work, report)
    print(f"MEASURE-END {bad} run(s) not passing")
    return 0


if __name__ == "__main__":
    os.environ.setdefault("PYTHONHASHSEED", "0")
    sys.exit(main(sys.argv[1:]))
