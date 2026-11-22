# SPDX-License-Identifier: Apache-2.0
"""mri_segstats as a whole program: FreeSurfer 8.2.0's official binary against the port, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica; the clean room's own comparison
is the open probe ``validation/investigations/freesurfer-mri-segstats-probe/compare.py``, whose
inputs, cases and comparison this runs with the port as the candidate).

The port (``larmorx-freesurfer --upstream-defects reproduce|fix freesurfer mri_segstats``, built
here) against the official ``$FREESURFER_HOME/bin/mri_segstats`` (run by name as the scripts run
it, through the oracle-output cache at one thread) on the open probe's command lines;
``reproduce`` must give FreeSurfer's on every case but those whose reproduce run notes a defect of
the open probe's ``NOT_COMPARABLE`` (counted); ``fix`` may differ only with a notice.

    python -P validation/probes/fs-segstats/prog.py --seed 1 [--only 3,7] [--json out.json]
    python -P validation/probes/fs-segstats/prog.py --golden

``--golden``: the official program on the golden cases' inputs (written by the port's example
``fs_segstats_golden``), hashed as ``tests/mri_segstats_golden.rs`` hashes the port's run.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

compare = _load(
    "freesurfer_mri_segstats_compare",
    REPO / "validation" / "investigations" / "freesurfer-mri-segstats-probe" / "compare.py",
)

#: The cases' reproduce notices that make a case not comparable (counted, not compared).
SKIP = compare.NOT_COMPARABLE

MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"


def build(jobs: int, *what: str) -> None:
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", *what, "-j", str(jobs), "--manifest-path", str(MANIFEST)],
        check=True,
        env=env,
    )


def golden(ws: Path, env: dict[str, str], jobs: int) -> int:
    """The golden cases (``tests/mri_segstats_golden.rs``): the official program on the port's
    32^3 inputs, hashed as the test hashes the port's run; prints the value to record."""
    build(jobs, "-p", "larmorx-freesurfer-volumes", "--example", "fs_segstats_golden")
    ex = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_segstats_golden"
    root = ws / "oracles" / "inputs" / "fs-segstats-prog" / "golden"
    if root.exists():
        shutil.rmtree(root)
    (root / "in").mkdir(parents=True)
    subprocess.run([ex, "inputs", root / "in"], check=True)
    lines = subprocess.run([ex, "cases"], check=True, capture_output=True, text=True).stdout
    h = hashlib.sha256()
    for line in lines.splitlines():
        k, args, outs = line.split("\t")
        d = root / f"case-{k}"
        shutil.copytree(root / "in", d)
        done = oracle_cache.run(
            ["mri_segstats", *args.split()],
            env=env,
            cwd=d,
            outputs=[d],
            policy=compare.POLICY,
            timeout=600,
            runner="fs-segstats-prog-golden",
        )
        (d / "stdout.txt").write_text(done.stdout, encoding="utf-8")
        text = subprocess.run(
            [ex, "mask", d / "stdout.txt"], check=True, capture_output=True, text=True
        ).stdout.rstrip("\n")
        h.update(f"case {k} {done.returncode & 0xFF}\n{text}\n".encode())
        for name in outs.split():
            dig = subprocess.run(
                [ex, "digest", d / name], check=True, capture_output=True, text=True
            )
            h.update(f"{name} {dig.stdout.strip()}\n".encode())
        print(f"case {k}: exit {done.returncode}, {outs}", flush=True)
    print(f"golden OFFICIAL_SHA256 {h.hexdigest()}")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--seed", type=int, default=1)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--only", help="comma-separated case numbers")
    ap.add_argument("--json", type=Path)
    ap.add_argument("--show", type=int, default=6)
    ap.add_argument("--golden", action="store_true", help="the golden cases' official value")
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    if a.golden:
        exe = compare.official()
        genv = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
                "OMP_NUM_THREADS": "1", "USER": "probe"}  # fmt: skip
        genv.update(freesurfer_env())
        return golden(ws, genv, a.jobs)
    build(a.jobs, "-p", "larmorx-freesurfer-cli")
    port = REPO / "crates-freesurfer" / "target" / "release" / "larmorx-freesurfer"
    candidate = [str(port), "--upstream-defects", "{mode}", "freesurfer", "mri_segstats"]
    summary = compare.run(a.seed, candidate, only=a.only, show=a.show, skip=SKIP, tag="port")
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    v = summary["verdicts"]
    return 1 if v.get("different", 0) + v.get("fix-unexplained", 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
