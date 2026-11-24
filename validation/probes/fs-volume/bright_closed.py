# SPDX-License-Identifier: Apache-2.0
"""Track E's relay 22 cross-check of specs/freesurfer-volume.md §59's FS-1233 text (2026-11-24): the
open model (models.py bright: FreeSurfer's labels of the byte volumes, written by value into the
T1's clone) against the replica's default, run through this closed probe's port side (the replica's
example) on the same inputs converted to its case format (every voxel painted). Compares the voxels
and the count line. Result on 2026-11-24: 92 lines with FS-1233, 92 as the model, 0 differ.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica and reads this probe's format).
Run it in a memory box (scripts/orchestration/boxed.sh).

    python -P bright_closed.py <worktree> <probe> <replica example> <scratch>
"""

import importlib.util
import struct
import subprocess
import sys
from pathlib import Path

wt, probe, replica, scratch = (Path(a) for a in sys.argv[1:5])
spec = importlib.util.spec_from_file_location(
    "models", wt / "validation/investigations/freesurfer-volume-probe/models.py"
)
M = importlib.util.module_from_spec(spec)
sys.modules["models"] = M
spec.loader.exec_module(M)
C = M.C


def closed_vol(w: list[str]) -> str:
    (x, y, z, _f), vals = M.vol_values(" ".join(w))
    t = int(w[3])
    words = [int(v) for v in w[6][1:].split(",")]
    paints = []
    for i, v in enumerate(vals):
        if t == 3:
            bits = words[i % len(words)]
            if bits == 0:
                continue
            h = f"{bits:08x}"
        else:
            if v == 0:
                continue
            h = struct.pack(">f", v).hex()
        paints.append(f"{i % x} {i // x % y} {i // (x * y)} 0 {h}")
    head = f"{x} {y} {z} {t} 1 0 0 1 {x} {y} {z} " + " ".join(w[7:26]) + " a " + " ".join(w[26:41])
    return f"{head} 1 0 0 1 00000000 {len(paints)} " + " ".join(paints)


gen = C.Gen20(7)
runner = C.Runner(probe.resolve(), scratch, True)
rows = []
for group in ("vbright", "vrules24"):
    lines = [ln for b in getattr(gen, group)() for ln in b.lines if ln.startswith("fbnwm ")]
    extra, plan = [], []
    for line in lines:
        parts = M.bright_parts(line)
        if parts is None or (parts[0][3] == "0" and parts[1][3] == "0"):
            continue
        t1, wm = parts
        t1b = " ".join(t1) + " 0" if t1[3] == "0" else M.as_bytes(t1, M.t1_byte)
        wmb = " ".join(wm) + " 0" if wm[3] == "0" else M.as_bytes(wm, M.wm_byte)
        plan.append((line, t1, wm, len(extra)))
        extra.append(f"fbnwm {t1b} {wmb}")
        extra.append(f"clone {' '.join(t1)} new")
    eb = C.Batch(group, f"bright-{group}-bytes", extra)
    eplain, _ = runner.probe_runs(eb, C.write_batch(scratch, eb))
    for line, t1, wm, k in plan:
        lres, lso, _ = C.decode(eplain[k])
        labels = bytes.fromhex(lres.decode().split(" ")[-1][1:])
        t1type = int(t1[3])
        if t1type == 0:
            vox = labels
        else:
            cres = C.decode(eplain[k + 1])[0].decode("latin-1").split(" ")
            vox = bytearray(bytes.fromhex(cres[-1][1:]))
            size = struct.calcsize(M.LABFMT[t1type])
            for j, v in enumerate(labels):
                if v:
                    vox[j * size : (j + 1) * size] = struct.pack(M.LABFMT[t1type], v)
        rows.append((group, line, f"fbnwm {closed_vol(t1)} {closed_vol(wm)}", bytes(vox), lso))

cases = scratch / "closed-cases.txt"
cases.write_text("".join(r[2] + "\n" for r in rows), encoding="utf-8")
out = subprocess.run(
    [str(replica), "run", str(cases), "fix"], capture_output=True, text=True, check=True
).stdout.splitlines()
assert len(out) == len(rows), (len(out), len(rows))
same = differ = 0
for (group, line, _case, vox, so), o in zip(rows, out, strict=True):
    body, _, ids = o.partition("\t")
    res, _, text = body.rpartition(" | ")
    got_vox = bytes.fromhex(res.split(" ")[-1])
    got_so = text.encode().decode("unicode_escape").encode("latin-1")
    if got_vox == vox and got_so == so and "FS-1233" in ids:
        same += 1
    else:
        differ += 1
        if differ <= 5:
            print("DIFFERS", group, line[:80])
            print("   ids", ids, "so", got_so[:80], "model", so[:80])
            print("   vox", got_vox.hex()[:80])
            print("   mod", vox.hex()[:80])
print(f"closed cross-check: {len(rows)} lines with FS-1233, as the model {same}, differ {differ}")
