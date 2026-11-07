# SPDX-License-Identifier: Apache-2.0
"""Wall time of fMRIPrep 25.2.6's seven prelude calls: FSL 6.0.7.23 (through ``oracle_cache.run``
with a reason never to store it: a timed run is the one kind the cache cannot serve), the replica
(``larmorx-nc fsl prelude``, a black box) and the clean-room original (``larmorx fsl prelude``) at
the thread counts given, each output checked bit for bit against FSL's (the data and the header
but ``descrip``). Usage::

    boxed.sh 8 python -P bench_calls.py <scratch dir> --larmorx <larmorx program>
        --replica <larmorx-nc program> [--threads 1 --threads 14] [--repeat 3] [--json out.json]

One line per call and program (the best of ``--repeat`` runs; FSL once), then the JSON.
"""

from __future__ import annotations

import argparse
import json
import platform
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from larmorx_validation import oracle_cache
from larmorx_validation.parity import fsl_prelude as fp
from larmorx_validation.parity import fsl_versions


def _timed(argv: list[str], cwd: Path, env: dict[str, str], repeat: int) -> tuple[int, float]:
    best = float("inf")
    code = 0
    for _ in range(repeat):
        t0 = time.perf_counter()
        code = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, check=False).returncode
        best = min(best, time.perf_counter() - t0)
    return code, best


def _same(a: Path, b: Path) -> bool:
    ha, da = fp._parts(a)
    hb, db = fp._parts(b)
    return ha == hb and da == db


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("scratch", type=Path)
    ap.add_argument("--larmorx", required=True)
    ap.add_argument("--replica", required=True)
    ap.add_argument("--threads", type=int, action="append")
    ap.add_argument("--repeat", type=int, default=3)
    ap.add_argument("--json", type=Path)
    a = ap.parse_args()
    threads = a.threads or [1]
    a.scratch.mkdir(parents=True, exist_ok=True)
    base = Path(tempfile.mkdtemp(prefix="lx-prelude-bench_", dir=a.scratch))
    home = base / "home"
    home.mkdir()
    rows = []
    for name, _node, words in fp.fmriprep_calls():
        env = fp._base_env(home, {})
        d = base / name.replace("/", "_")
        (d / "fsl").mkdir(parents=True)
        r = oracle_cache.run(
            [str(fsl_versions.program("prelude")), *words],
            env=env,
            cwd=d / "fsl",
            outputs=[d / "fsl"],
            policy=oracle_cache.Policy(threads=1, family="fsl", never="a timed run (benchmark)"),
            runner="fsl-prelude-bench",
            timeout=900,
        )
        row = {"call": name, "fsl_s": round(r.seconds, 4), "fsl_exit": r.returncode}
        ref = d / "fsl" / "out.nii.gz"
        progs = [("replica", [a.replica], 1)] + [(f"original-{t}", [a.larmorx], t) for t in threads]
        for label, cmd, t in progs:
            out = d / label
            out.mkdir()
            e = {**env, "OMP_NUM_THREADS": str(t)}
            if label != "replica":
                e["LARMORX_IMPLEMENTATION"] = "original"
            code, s = _timed([*cmd, "fsl", "prelude", *words], out, e, a.repeat)
            row[f"{label}_s"] = round(s, 4)
            row[f"{label}_same"] = code == 0 and _same(ref, out / "out.nii.gz")
        rows.append(row)
        print(
            f"{name}: FSL {row['fsl_s']:.3f} s; "
            + "; ".join(
                f"{k[:-2]} {v:.3f} s{'' if row[k[:-2] + '_same'] else ' (DIFFERS)'}"
                for k, v in row.items()
                if k.endswith("_s") and k != "fsl_s"
            ),
            flush=True,
        )
    doc = {
        "machine": platform.processor() or platform.machine(),
        "python": sys.version.split()[0],
        "repeat": a.repeat,
        "threads": threads,
        "rows": rows,
    }
    if a.json:
        a.json.write_text(json.dumps(doc, indent=1), encoding="utf-8")
    return 0 if all(v for r in rows for k, v in r.items() if k.endswith("_same")) else 1


if __name__ == "__main__":
    sys.exit(main())
