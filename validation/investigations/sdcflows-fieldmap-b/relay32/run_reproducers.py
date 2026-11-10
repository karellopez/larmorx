# SPDX-License-Identifier: Apache-2.0
"""Run the seven upstream reports' reproducers (relay 32, `docs/upstream-reports/`) with one interpreter, each in a
process of its own (SCI-21's past-the-end writes may crash or hang a process: those cases run
``--repeats`` times, each outcome counted), and print every outcome.

    python -P run_reproducers.py <python> [--repeats 10] [--timeout 120] [--malloc-check]
        [--path <folder>] [--only PIPE-32,SDC-45,...]

Run it boxed (``scripts/orchestration/boxed.sh 8 ...``) in both kept environments.
``--malloc-check`` runs SCI-21's past-the-end cases with glibc's checking allocator
(``libc_malloc_debug.so.0``, ``MALLOC_CHECK_=3``); ``--path`` puts a folder before the
environment's packages (e.g. a copy of pybids' master, ``src/``), without writing bytecode.
"""

from __future__ import annotations

import argparse
import collections
import os
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
SCRIPTS = [
    ("PIPE-32", "pipe32_shared_identifier.py", []),
    ("PIPE-32", "pipe32_shared_identifier.py", ["02"]),
    ("PIPE-32", "pipe32_automatic_names.py", []),
    ("SDC-45", "sdc45_fieldmapfile_entities.py", []),
    ("PIPE-36", "pipe36_extract_entities.py", []),
    ("PIPE-37", "pipe37_pybids_parent_folders.py", []),
    ("SCI-21", "sci21_rank_filter_output_type.py", ["float64"]),
    ("SCI-21", "sci21_rank_filter_output_type.py", ["int64"]),
    ("SCI-21", "sci21_rank_filter_output_type.py", ["float32"]),
    ("SCI-21", "sci21_rank_filter_output_type.py", ["int16"]),
    ("SCI-21", "sci21_related.py", []),
    ("SCI-25", "sci25_erosion_float_output.py", []),
    ("SCI-29", "sci29_label_bool_output.py", []),
]


MALLOC_DEBUG = Path("/lib/x86_64-linux-gnu/libc_malloc_debug.so.0")


def run(python: str, script: str, args: list[str], timeout: float, tmp: str, extra: dict) -> str:
    env = {k: v for k, v in os.environ.items() if not k.startswith("PYTHON")}
    env.update(TMPDIR=tmp, OMP_NUM_THREADS="1", OPENBLAS_NUM_THREADS="1", **extra)
    flag = "-P" if "PYTHONPATH" in extra else "-I"
    try:
        p = subprocess.run(
            [python, flag, str(HERE / script), *args],
            capture_output=True, text=True, timeout=timeout, env=env, cwd=tmp,
        )  # fmt: skip
    except subprocess.TimeoutExpired:
        return "hung (killed at the time limit)"
    lines = [ln for ln in p.stdout.splitlines() if ln.strip()]
    err = [ln for ln in p.stderr.splitlines() if ln.strip()]
    tail = err[-1] if err else ""
    status = "exit 0" if p.returncode == 0 else f"exit {p.returncode}"
    return "\n    ".join([status, *lines] + ([f"stderr: {tail}"] if p.returncode else []))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("python")
    ap.add_argument("--repeats", type=int, default=10)
    ap.add_argument("--timeout", type=float, default=120)
    ap.add_argument("--malloc-check", action="store_true")
    ap.add_argument("--path")
    ap.add_argument("--only", default="")
    a = ap.parse_args()
    extra = {"PYTHONPATH": a.path, "PYTHONDONTWRITEBYTECODE": "1"} if a.path else {}
    only = set(filter(None, a.only.split(",")))
    out = subprocess.run(
        [a.python, "-P" if a.path else "-I", "-c", "import scipy, numpy, bids, sdcflows, fmriprep; "
         "print('scipy', scipy.__version__, 'numpy', numpy.__version__, 'pybids', bids.__version__, "
         "'sdcflows', sdcflows.__version__, 'fmriprep', fmriprep.__version__)"],
        capture_output=True, text=True, env={**os.environ, **extra},
    )  # fmt: skip
    print(out.stdout.strip() or out.stderr.strip())
    with tempfile.TemporaryDirectory() as tmp:
        for ident, script, args in SCRIPTS:
            if only and ident not in only:
                continue
            past_end = ident == "SCI-21" and args[:1] in (["float32"], ["int16"])
            n = a.repeats if past_end else 1
            modes = [("", {})]
            if past_end and a.malloc_check and MALLOC_DEBUG.exists():
                modes.append((" (glibc's checking allocator)",
                              {"LD_PRELOAD": str(MALLOC_DEBUG), "MALLOC_CHECK_": "3"}))  # fmt: skip
            for label, more in modes:
                env = {**extra, **more}
                seen = collections.Counter(
                    run(a.python, script, args, a.timeout, tmp, env) for _ in range(n)
                )
                for outcome, count in seen.items():
                    print(f"{ident} {script} {' '.join(args)}{label} [{count}/{n}]:\n    {outcome}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
