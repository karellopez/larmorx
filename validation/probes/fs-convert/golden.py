# SPDX-License-Identifier: Apache-2.0
"""mri_convert's golden cases: the official program's value on the port's inputs.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica's example).

Writes the 32^3 input and the files beside it with the port's example ``fs_convert_golden``, runs
FreeSurfer 8.2.0's official ``mri_convert`` (``$FREESURFER_HOME/bin``, by name) on each case through
the oracle-output cache at one thread, and hashes exit status, standard output (masked as the test
masks it) and the output's digest (computed by the example) as
``crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_convert_golden.rs`` hashes the port's run:
the value it records as ``OFFICIAL_SHA256``.

    python -P validation/probes/fs-convert/golden.py
"""

from __future__ import annotations

import argparse
import hashlib
import os
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

MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
EXAMPLE = REPO / "crates-freesurfer" / "target" / "release" / "examples" / "fs_convert_golden"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--jobs", type=int, default=6)
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_convert"
    if not exe.is_file():
        raise SystemExit(f"no {exe}")
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        [
            "cargo",
            "build",
            "--release",
            "-p",
            "larmorx-freesurfer-volumes",
            "--example",
            "fs_convert_golden",
            "-j",
            str(a.jobs),
            "--manifest-path",
            str(MANIFEST),
        ],
        check=True,
        env=env,
    )
    root = ws / "oracles" / "inputs" / "fs-convert" / "golden"
    if root.exists():
        shutil.rmtree(root)
    (root / "in").mkdir(parents=True)
    subprocess.run([EXAMPLE, "inputs", root / "in"], check=True)
    lines = subprocess.run([EXAMPLE, "cases"], check=True, capture_output=True, text=True).stdout
    fenv = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
            "OMP_NUM_THREADS": "1", "USER": "probe", **freesurfer_env()}  # fmt: skip
    # FS-1053: the conformed geometry's validity flag is memory contents
    policy = oracle_cache.Policy(
        threads=1, family="freesurfer", volatile=(r"valid   : -?[0-9]+",), formats="freesurfer"
    )
    h = hashlib.sha256()
    for line in lines.splitlines():
        k, args, out = line.split("\t")
        d = root / f"case-{k}"
        shutil.copytree(root / "in", d)
        done = oracle_cache.run(
            ["mri_convert", *args.split()],
            env=fenv,
            cwd=d,
            outputs=[d],
            policy=policy,
            timeout=600,
            runner="fs-convert-golden",
        )
        (d / "stdout.txt").write_text(done.stdout, encoding="utf-8")
        text = subprocess.run(
            [EXAMPLE, "mask", d / "stdout.txt"], check=True, capture_output=True, text=True
        ).stdout.rstrip("\n")
        h.update(f"case {k} {done.returncode & 0xFF}\n{text}\n".encode())
        dig = subprocess.run(
            [EXAMPLE, "digest", d / out], check=True, capture_output=True, text=True
        )
        h.update(f"{out} {dig.stdout.strip()}\n".encode())
        print(f"case {k}: exit {done.returncode}, {out}", flush=True)
    print(f"golden OFFICIAL_SHA256 {h.hexdigest()}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
