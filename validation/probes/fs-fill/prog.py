# SPDX-License-Identifier: Apache-2.0
"""mri_fill as a whole program: FreeSurfer 8.2.0's official binary against the port, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

``--golden``: the official program (``$FREESURFER_HOME/bin/mri_fill``, run by name through the
oracle-output cache at one thread) on the golden cases' inputs (written by the port's example
``fs_fill_golden``: 32^3 volumes), hashed as ``tests/mri_fill_golden.rs`` hashes the
port's run (standard output and standard error, masked as the test masks them, and each output's
digest); prints the value to record. Each case runs in a copy of the inputs of its own; every
file of the copy is an input of the cache's key. A case with a diagnostic setting (an option
starting ``-w`` or ``-diag``, or a ``DIAG`` variable) is refused: the official program's licence
check prints the licence file from a diagnostic level of 1.

    python -P validation/probes/fs-fill/prog.py --golden
"""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

POLICY = oracle_cache.Policy(
    threads=1, family="freesurfer", formats="freesurfer", volatile=(r"filling took \S+ minutes",)
)
#: Never a diagnostic setting on the official program.
DIAG_ARG = re.compile(r"^(--?diag|-[wW])", re.I)
MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"


def status(code: int) -> int:
    """An exit status as a shell reports it: a process ended by signal ``n`` as ``128 + n``."""
    return 128 - code if code < 0 else code


def build(jobs: int, *what: str) -> None:
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", *what, "-j", str(jobs), "--manifest-path", str(MANIFEST)],
        check=True,
        env=env,
    )


def golden(ws: Path, env: dict[str, str], jobs: int) -> int:
    """The golden cases (``tests/mri_fill_golden.rs``): the official program on the port's inputs,
    hashed as the test hashes the port's run; prints the value to record."""
    build(jobs, "-p", "larmorx-freesurfer-volumes", "--example", "fs_fill_golden")
    ex = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_fill_golden"
    root = ws / "oracles" / "inputs" / "fs-fill-prog" / "golden"
    if root.exists():
        shutil.rmtree(root)
    (root / "in").mkdir(parents=True)
    subprocess.run([ex, "inputs", root / "in"], check=True)
    lines = subprocess.run([ex, "cases"], check=True, capture_output=True, text=True).stdout
    h = hashlib.sha256()
    for line in lines.splitlines():
        k, args, outs, extra = line.split("\t")
        if any(DIAG_ARG.match(w) for w in args.split()) or "DIAG" in extra:
            raise SystemExit(f"case {k}: a diagnostic setting on the official program")
        d = root / f"case-{k}"
        shutil.copytree(root / "in", d)
        given = sorted(p for p in d.rglob("*") if p.is_file())
        case_env = dict(env)
        case_env.update(dict(x.split("=", 1) for x in extra.split()))
        done = oracle_cache.run(
            ["mri_fill", *args.split()],
            env=case_env,
            cwd=d,
            outputs=[d],
            policy=POLICY,
            timeout=600,
            runner="fs-fill-prog-golden",
            inputs=given,
        )
        (root / f"stdout-{k}.txt").write_text(done.stdout, encoding="utf-8")
        (root / f"stderr-{k}.txt").write_text(done.stderr, encoding="utf-8")
        text = subprocess.run(
            [ex, "mask", root / f"stdout-{k}.txt", root / f"stderr-{k}.txt"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.rstrip("\n")
        h.update(f"case {k} {status(done.returncode)}\n{text}\n".encode())
        for name in outs.split():
            dig = subprocess.run(
                [ex, "digest", d / name], check=True, capture_output=True, text=True
            )
            h.update(f"{name} {dig.stdout.strip()}\n".encode())
        print(f"case {k}: exit {status(done.returncode)}, {outs}", flush=True)
    print(f"golden OFFICIAL_SHA256 {h.hexdigest()}")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--golden", action="store_true", help="the golden cases' official value")
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    if not a.golden:
        raise SystemExit(
            "the whole program's cases are the open probe's: "
            "validation/investigations/freesurfer-mri-fill-probe/compare.py; here use --golden"
        )
    exe = ws / FS_HOME / "bin" / "mri_fill"
    genv = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
            "OMP_NUM_THREADS": "1", "USER": "probe", "FS_DISABLE_LANG": "1"}  # fmt: skip
    genv.update(freesurfer_env())
    return golden(ws, genv, a.jobs)


if __name__ == "__main__":
    raise SystemExit(main())
