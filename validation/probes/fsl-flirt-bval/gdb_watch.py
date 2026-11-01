# SPDX-License-Identifier: Apache-2.0
"""Run FSL's flirt under gdb with a hardware watchpoint on a heap word; print a backtrace at every
write. python gdb_watch.py <cwd> <hex address> <flirt args...>"""

import os
import subprocess
import sys
from pathlib import Path

FSL = os.environ.get("FSLDIR", str(Path.home() / "fsl-6.0.7.23"))
cwd, addr, *args = sys.argv[1:]
env = {
    "HOME": os.environ.get("HOME", "/tmp"),
    "PATH": "/usr/bin:/bin",
    "FSLDIR": FSL,
    "FSLOUTPUTTYPE": "NIFTI_GZ",
    "OPENBLAS_CORETYPE": "Haswell",
    "OMP_NUM_THREADS": "1",
}
script = f"""
set pagination off
set confirm off
set breakpoint pending on
break main
run
watch -l *(long*){addr}
commands
bt 12
continue
end
continue
bt 20
"""
sp = os.path.join(cwd, "watch.gdb")
Path(sp).write_text(script, encoding="utf-8")
p = subprocess.run(
    ["gdb", "-batch", "-x", sp, "--args", f"{FSL}/bin/flirt", *args],
    cwd=cwd,
    env=env,
    capture_output=True,
    text=True,
    errors="replace",
    timeout=600,
)
out = "\n".join(ln for ln in p.stdout.splitlines() if not ln.startswith(("[New Thread", "[Thread")))
print(out[-30000:])
print(p.stderr[-3000:])
