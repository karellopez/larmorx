# SPDX-License-Identifier: Apache-2.0
"""mri_normalize as a whole program: FreeSurfer 8.2.0's official binary against the port, case by case.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica; the clean room's own comparison
is the open probe ``validation/investigations/freesurfer-mri-normalize-probe/compare.py``, whose
inputs, cases and comparison this runs with the port as the candidate).

The port (``larmorx-freesurfer --upstream-defects reproduce|fix freesurfer mri_normalize``, built
here) against the unit oracle's ``bin/mri_normalize`` (byte-identical to the official program, run
by name as recon-all runs it, through the oracle-output cache at one thread) on the open probe's 82
command lines; ``reproduce`` must give FreeSurfer's on every case but those whose reproduce run
notes a defect of the closed probes' ``SKIP`` lists or the open probe's ``NOT_COMPARABLE``
(counted); ``fix`` may differ only with a notice.

    python -P validation/probes/fs-normalize/prog.py --seed 1 [--only 3,7] [--json out.json]
    python -P validation/probes/fs-normalize/prog.py --golden

``--golden``: the official program on the golden cases' inputs (written by the port's example
``fs_normalize_golden``), hashed as ``tests/mri_normalize_golden.rs`` hashes the port's run.
"""

from __future__ import annotations

import argparse
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
sys.path.insert(0, str(HERE))


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


import check  # noqa: E402
from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

compare = _load(
    "freesurfer_mri_normalize_compare",
    REPO / "validation" / "investigations" / "freesurfer-mri-normalize-probe" / "compare.py",
)
check_filter = _load("check_filter", HERE.parent / "fs-filter" / "check.py")

# the cases' reproduce notices that make a case not comparable (counted, not compared): the fs-filter
# and fs-normalize probes' SKIP (FreeSurfer reads memory it does not own, crashes, or seeds from the
# clock) and the open probe's list
SKIP = frozenset(check_filter.SKIP) | frozenset(check.SKIP) | compare.NOT_COMPARABLE


def port_binary(jobs: int) -> Path:
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-cli", "-j", str(jobs),
         "--manifest-path", str(REPO / "crates-freesurfer" / "Cargo.toml")],
        check=True,
        env=env,
    )  # fmt: skip
    return REPO / "crates-freesurfer" / "target" / "release" / "larmorx-freesurfer"


def golden_line(line: str) -> str:
    """``mask_line`` of the golden test (``tests/mri_normalize_golden/common.rs``): the timing
    line's numbers, and a tissue level read from a histogram bin below 0 (FS-597: memory contents
    in FreeSurfer) as ``?`` without its padding."""
    if line.startswith("3D bias adjustment took "):
        return "3D bias adjustment took M minutes and S seconds."
    w = line.split(" ")
    keep: list[str | None] = list(w)
    for i in range(1, len(w)):
        if w[i] != "at" or w[i - 1] not in ("peak", "valley"):
            continue
        j = next((j for j in range(i + 1, len(w)) if w[j]), None)
        if j is None:
            continue
        if j + 1 < len(w) and w[j + 1].startswith("(-"):
            for k in range(i + 1, j):
                keep[k] = None
            keep[j] = "?"
    return " ".join(x for x in keep if x is not None)


def golden(ws: Path, env: dict[str, str], jobs: int) -> int:
    """The golden cases (``tests/mri_normalize_golden.rs``): the official program on the port's
    32^3 inputs, hashed as the test hashes the port's run; prints the value to record."""
    import hashlib

    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-volumes", "--example",
         "fs_normalize_golden", "-j", str(jobs), "--manifest-path",
         str(REPO / "crates-freesurfer" / "Cargo.toml")],
        check=True,
        env={k: v for k, v in os.environ.items() if k != "FSLDIR"},
    )  # fmt: skip
    ex = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_normalize_golden"
    root = ws / "oracles" / "inputs" / "fs-normalize-prog" / "golden"
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
            ["mri_normalize", *args.split()],
            env=env,
            cwd=d,
            outputs=[d],
            policy=compare.POLICY,
            timeout=600,
            runner="fs-normalize-prog-golden",
        )
        text = "\n".join(golden_line(x) for x in done.stdout.splitlines())
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
    port = port_binary(a.jobs)
    candidate = [str(port), "--upstream-defects", "{mode}", "freesurfer", "mri_normalize"]
    summary = compare.run(a.seed, candidate, only=a.only, show=a.show, skip=SKIP, tag="port")
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    v = summary["verdicts"]
    return 1 if v.get("different", 0) + v.get("fix-unexplained", 0) else 0


if __name__ == "__main__":
    raise SystemExit(main())
