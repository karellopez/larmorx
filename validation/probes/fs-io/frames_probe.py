# SPDX-License-Identifier: Apache-2.0
"""Write MGH files whose header asks for many frames of a 1x1x1 uchar volume; run a command on
each; report its exit status, peak memory (children's maxrss) and the last line of its output."""

import os
import resource
import struct
import subprocess
import sys
import time

out = sys.argv[1]
cmd = sys.argv[2:]
os.makedirs(out, exist_ok=True)
for nf, with_data in [
    (2_000_000, True),
    (12_000_000, True),
    (20_000_000, True),
    (449_657_401, False),
]:
    p = os.path.join(out, f"f{nf}.mgh")
    with open(p, "wb") as f:
        h = struct.pack(">iiiiiiih", 1, 1, 1, 1, nf, 0, 1, 1) + struct.pack(
            ">15f", 1, 1, 1, -1, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0
        )
        f.write(h + b"\0" * (284 - len(h)))
        if with_data:
            f.write(b"\x07" * nf)
    t = time.time()
    r = subprocess.run([*cmd, p], capture_output=True, timeout=300)
    ru = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
    tail = (r.stdout + r.stderr).decode(errors="replace").strip().splitlines()[-3:]
    print(
        nf,
        "exit",
        r.returncode,
        "maxrss_so_far_MB",
        ru // 1024,
        "s %.1f" % (time.time() - t),
        tail,
        flush=True,
    )
