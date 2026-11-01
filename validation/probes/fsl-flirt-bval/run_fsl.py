# SPDX-License-Identifier: Apache-2.0
"""Run an FSL 6.0.7.23 program (or the replica, with --replica[=mode], or under gdb with --gdb)
in a directory with the parity suite's environment.

python run_fsl.py <cwd> [--gdb] [--replica[=fix|reproduce]] <program> <args...>
"""

import os
import subprocess
import sys
from pathlib import Path

FSL = os.environ.get("LARMORX_FSL_623", str(Path.home() / "fsl-6.0.7.23"))  # not $FSLDIR
NC = os.environ.get(
    "LARMORX_NC_BIN",
    str(Path(__file__).resolve().parents[3] / "crates-nc/target/release/larmorx-nc"),
)
cwd = sys.argv[1]
args = sys.argv[2:]
gdb = False
replica = None
while args and args[0].startswith("--"):
    a = args.pop(0)
    if a == "--gdb":
        gdb = True
    elif a.startswith("--replica"):
        replica = a.split("=", 1)[1] if "=" in a else "reproduce"
env = {
    "HOME": os.environ.get("HOME", "/tmp"),
    "PATH": "/usr/bin:/bin",
    "FSLDIR": FSL,
    "FSLOUTPUTTYPE": os.environ.get("FSLOUTPUTTYPE", "NIFTI_GZ"),
    "OPENBLAS_CORETYPE": "Haswell",
}
if replica:
    env["LARMORX_UPSTREAM_DEFECTS"] = replica
    argv = [NC, "fsl", *args]
else:
    argv = [f"{FSL}/bin/{args[0]}", *args[1:]]
if gdb:
    argv = ["gdb", "-batch", "-ex", "run", "-ex", "bt 40", "--args", *argv]
p = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True, errors="replace")
sys.stdout.write(p.stdout if os.environ.get("FULL") else p.stdout[-20000:])
sys.stderr.write(p.stderr[-8000:])
print(f"[exit {p.returncode}]")
