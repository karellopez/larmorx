# SPDX-License-Identifier: Apache-2.0
"""The resident memory over time of FSL's flirt and of the replica on one call, sampled every
millisecond from ``/proc/<pid>/status`` (the largest value of each ``BIN`` seconds, 0.05 by
default), with the lines the programs print (``-v``) placed in time: where each program's peak
is reached.

Usage: python -P rsstrace.py <work dir> <flirt arguments; @name copies the suite's input name>
(``LARMORX_NC_BIN`` the replica's program; ``LARMORX_FSL_623`` FSL 6.0.7.23, ``~/fsl-6.0.7.23`` by default;
``LARMORX_FLIRT_INPUTS`` the parity suite's derived inputs, ``<workspace>/oracles/inputs/
fsl-flirt-replica`` by default).
"""

import os
import re
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

FSL = os.environ.get("LARMORX_FSL_623", str(Path.home() / "fsl-6.0.7.23"))  # not $FSLDIR
NC = os.environ["LARMORX_NC_BIN"]
INPUTS = Path(
    os.environ.get(
        "LARMORX_FLIRT_INPUTS",
        Path(__file__).resolve().parents[4] / "oracles/inputs/fsl-flirt-replica",
    )
)
work = Path(sys.argv[1])
args = sys.argv[2:]
env = {
    "HOME": os.environ.get("HOME", "/tmp"),
    "PATH": "/usr/bin:/bin",
    "FSLDIR": FSL,
    "FSLOUTPUTTYPE": "NIFTI_GZ",
    "OPENBLAS_CORETYPE": "Haswell",
    "OMP_NUM_THREADS": "1",
    "LARMORX_UPSTREAM_DEFECTS": "reproduce",
}
BIN = float(os.environ.get("BIN", "0.05"))
for who in ("fsl", "rep"):
    d = work / who
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    argv = []
    for a in args:
        if a.startswith("@"):
            shutil.copy(INPUTS / a[1:], d / a[1:])
            argv.append(a[1:])
        else:
            argv.append(a)
    cmd = [f"{FSL}/bin/flirt", *argv] if who == "fsl" else [NC, "fsl", "flirt", *argv]
    t0 = time.perf_counter()
    p = subprocess.Popen(
        cmd, cwd=d, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True
    )
    lines = []

    def rd(p=p, lines=lines, t0=t0):
        for line in p.stdout:
            lines.append((time.perf_counter() - t0, line.rstrip()[:70]))

    th = threading.Thread(target=rd)
    th.start()
    samples = []
    while p.poll() is None:
        try:
            s = Path(f"/proc/{p.pid}/status").read_text()
            m = re.search(r"VmRSS:\s+(\d+)", s)
            if m:
                samples.append((time.perf_counter() - t0, int(m.group(1)) // 1024))
        except Exception:
            pass
        time.sleep(0.001)
    th.join()
    print(
        f"== {who} exit {p.returncode} peak {max(s for _, s in samples)} MiB, "
        f"{len(samples)} samples, {samples[-1][0]:.2f}s"
    )
    events = sorted(
        [(t, "L", v) for t, v in lines] + [(t, "S", s) for t, s in samples], key=lambda e: e[0]
    )
    last_bin = -1
    binmax = 0
    for t, k, v in events:
        b = int(t / BIN)
        if k == "S":
            if b != last_bin:
                if last_bin >= 0:
                    print(f"  {last_bin * BIN:6.2f}s {binmax:5d} MiB")
                last_bin = b
                binmax = v
            else:
                binmax = max(binmax, v)
        else:
            print(f"  {t:6.2f}s  | {v}")
    print(f"  {last_bin * BIN:6.2f}s {binmax:5d} MiB")
