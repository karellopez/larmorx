# SPDX-License-Identifier: Apache-2.0
"""The recorded heads' ``talairach.m3z`` read and written again (piece 32's ``gcw``) by the official
probe and the port, in both modes: each head as ``.m3z``, ``.m3d``, ``.mgz`` (formats 0 and 3),
``.nii.gz`` (0) and ``.nii`` (1). Run it inside ``scripts/orchestration/boxed.sh`` (16 GiB):

    boxed.sh 16 python3 -P validation/probes/fs-io/heads_gcw.py <worktree> <output folder>

It writes the case lines and both sides' outputs into the folder and prints the identical counts and
the first differing lines. (The ``.m3d`` outputs pass the official probe's 64 MiB file limit.)
"""

import glob
import subprocess
import sys
from pathlib import Path

wt = Path(sys.argv[1])
out = Path(sys.argv[2])
sys.path.insert(0, str(wt / "validation/probes/fs-io"))
import check  # noqa: E402

ws = check.workspace()
runs = ws / "oracles/freesurfer-8.2.0/runs"
heads = sorted(glob.glob(str(runs / "*-classic/subjects/*/mri/transforms/talairach.m3z")))
outputs = [
    ("w/o.m3z", 0),
    ("w/o.m3d", 0),
    ("w/o.mgz", 0),
    ("w/o.mgz", 3),
    ("w/o.nii.gz", 0),
    ("w/o.nii", 1),
]


def enc(s: str) -> str:
    return "x" + s.encode().hex()


lines = [f"gcw 0 0 {enc(h)} - {enc(n)} {f}" for h in heads for n, f in outputs]
cf = out / "heads.txt"
cf.write_text("\n".join(lines) + "\n")
exe = ws / "oracles/freesurfer-8.2.0-probes/fs-io/fsio"
port = wt / "crates-freesurfer/target/release/examples/fs_io_probe"
env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
run = subprocess.run([str(exe), str(cf)], env=env, capture_output=True, text=True, check=False)
off = run.stdout.splitlines()
(out / "heads-off.txt").write_text("\n".join(off) + "\n")
for mode in ("reproduce", "fix"):
    got = subprocess.run(
        [str(port), "run", str(cf), mode], capture_output=True, text=True, check=True
    ).stdout.splitlines()
    (out / f"heads-{mode}.txt").write_text("\n".join(got) + "\n")
    same = [check.matches(o, q.partition("\t")[0]) for o, q in zip(off, got, strict=True)]
    bad = [i for i, ok in enumerate(same) if not ok]
    print(mode, f"{sum(same)}/{len(lines)} identical", "different:", bad[:10])
    for i in bad[:2]:
        print(" OFF ", off[i][:400])
        print(" PORT", got[i][:400])
