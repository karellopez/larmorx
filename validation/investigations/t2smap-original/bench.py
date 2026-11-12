# SPDX-License-Identifier: Apache-2.0
"""Wall times of fMRIPrep's multi-echo ``t2smap`` calls (``docs/benchmarks/tedana-t2smap.md``, the
clean-room original's columns): tedana 26.0.3 once (through the oracle-output cache with a reason
never to store it: a timed run), the replica (``larmorx-gpl``, a black box) by default and the
clean-room original (its own binding, ``larmorx._core.tedana_t2smap_cleanroom``, as a program:
``python -c``) in both modes, each at every thread count, the best of ``--repeat`` runs::

    python -P bench.py <work> [--replica <program>] [--threads 1,8] [--repeat 2] [--runs ...] \\
        [--json <file>]

The inputs are fMRIPrep's (``fmriprep_calls.py``); the calls are its four curve-fit calls and
ds003592's again with ``--fittype loglin``. Each run writes into a folder of its own
(``--out-dir out``). The original's runs at every thread count are checked to write the same
files, value for value (decompressed), as its run at one thread in the same mode.
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import fmriprep_calls as fc  # noqa: E402

#: the calls: (name, fMRIPrep's run in fmriprep_calls.CALLS, the fit type)
CALLS = [
    ("ds000210-sub-02-cuedSGT-curvefit", "ds000210-cuedSGT", "curvefit"),
    ("ds000210-sub-02-rest-curvefit", "ds000210-rest", "curvefit"),
    ("ds000258-sub-21262-rest-curvefit", "ds000258", "curvefit"),
    ("ds003592-sub-280-rest-curvefit", "ds003592", "curvefit"),
    ("ds003592-sub-280-rest-loglin", "ds003592", "loglin"),
]

#: the clean-room original as a program: tedana's arguments, this environment, this directory
ORIGINAL = (
    "import os, sys\n"
    "from larmorx import _core\n"
    "code, out, err = _core.tedana_t2smap_cleanroom(sys.argv[1:], list(os.environ.items()), "
    "os.getcwd())\n"
    "sys.stdout.write(out)\n"
    "sys.stderr.write(err)\n"
    "sys.exit(code)\n"
)


def timed(cmd: list[str], cwd: Path, env: dict) -> tuple[int, float, str]:
    """``cmd`` run in a new folder ``cwd``: its exit status, wall time and standard error."""
    if cwd.exists():
        shutil.rmtree(cwd)
    cwd.mkdir(parents=True)
    t0 = time.perf_counter()
    proc = subprocess.run(cmd, cwd=cwd, env=env, capture_output=True, text=True, check=False)
    return proc.returncode, time.perf_counter() - t0, proc.stderr


def contents(out: Path) -> dict[str, bytes]:
    """Every file of an output folder, decompressed (gzip headers aside), but the two that hold
    the command (``t2smap_call.sh`` and ``dataset_description.json``: ``--n-threads`` itself)."""
    found = {}
    for p in sorted(out.rglob("*")):
        if p.is_file() and not p.name.endswith(("t2smap_call.sh", "dataset_description.json")):
            raw = p.read_bytes()
            found[p.relative_to(out).as_posix()] = (
                gzip.decompress(raw) if p.suffix == ".gz" else raw
            )
    return found


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("work", type=Path)
    p.add_argument("--replica", default=None, help="the replica's program (larmorx-gpl)")
    p.add_argument("--threads", default="1,8")
    p.add_argument("--repeat", type=int, default=2)
    p.add_argument("--runs", default=",".join(c[0] for c in CALLS))
    p.add_argument("--json", type=Path, default=None)
    a = p.parse_args(argv)
    from larmorx_validation import oracle_cache

    work = a.work.resolve()
    work.mkdir(parents=True, exist_ok=True)
    env = fc.blackbox.environment()
    threads = [int(t) for t in a.threads.split(",")]
    wanted = a.runs.split(",")
    results: dict[str, dict] = {}
    for name, call, fittype in CALLS:
        if name not in wanted:
            continue
        run, task, tes = fc.CALLS[call]
        echoes, mask = fc.inputs(run, task)
        args = ["-d", *map(str, echoes), "-e", *tes, "--mask", str(mask), "--fittype", fittype]
        row: dict = {}
        case = work / name
        t_dir = case / "tedana"
        if t_dir.exists():
            shutil.rmtree(t_dir)
        t_dir.mkdir(parents=True)
        res = oracle_cache.run(
            [str(fc.LAUNCHER), *args, "--out-dir", "out"],
            env=env,
            cwd=t_dir,
            outputs=[t_dir / "out"],
            policy=oracle_cache.Policy(threads=1, family="python", never="a timed run"),
            timeout=3600,
            runner="t2smap-original-bench",
        )
        assert res.returncode == 0, res.stderr_bytes[-2000:]
        row["tedana"] = round(res.seconds, 2)
        print(f"{name}: tedana {res.seconds:.1f} s", flush=True)
        for n in threads:
            words = [*args, "--n-threads", str(n), "--out-dir", "out"]
            if a.replica:
                best = []
                for r in range(a.repeat):
                    code, s, err = timed([a.replica, "tedana", "t2smap", *words], case / f"replica-{n}-{r}", env)  # fmt: skip
                    assert code == 0, err[-2000:]
                    best.append(s)
                row[f"replica default ({n})"] = round(min(best), 2)
            for mode in ("reproduce", "fix"):
                menv = {**env, "LARMORX_UPSTREAM_DEFECTS": mode}
                best = []
                for r in range(a.repeat):
                    sub = case / f"original-{mode}-{n}-{r}"
                    code, s, err = timed([sys.executable, "-P", "-c", ORIGINAL, *words], sub, menv)
                    assert code == 0, err[-2000:]
                    best.append(s)
                row[f"original {mode} ({n})"] = round(min(best), 2)
                one = case / f"original-{mode}-{threads[0]}-0" / "out"
                same = contents(one) == contents(case / f"original-{mode}-{n}-0" / "out")
                row[f"original {mode} ({n}) identical to ({threads[0]})"] = same
            print(f"  {n} thread(s): {json.dumps({k: v for k, v in row.items() if f'({n})' in k})}", flush=True)  # fmt: skip
        results[name] = row
        for d in case.iterdir():  # keep the timings, not the images
            if d.is_dir():
                shutil.rmtree(d)
    document = {
        "machine": {"system": platform.system(), "machine": platform.machine(), "cpus": os.cpu_count()},
        "threads": threads,
        "repeat": a.repeat,
        "calls": results,
    }  # fmt: skip
    text = json.dumps(document, indent=1)
    if a.json:
        a.json.write_text(text + "\n", encoding="utf-8")
    print(text)
    print("BENCH-END")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
