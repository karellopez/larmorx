# SPDX-License-Identifier: Apache-2.0
"""The fs-io probe on a subset of its kinds: the cases of ``--seed``/``--n`` whose kind is listed,
run by the official probe (through the oracle-output cache, one thread) and by the port in both
modes; a summary in JSON (equal, different by kind, SKIP, VARY, the notices of the differences) and
the first differing cases. Run it inside ``scripts/orchestration/boxed.sh``:

    boxed.sh 12 python3 -P validation/probes/fs-io/subset.py <worktree> <seed> <n> <kind,kind,...>

(``SHOW``/``VSHOW`` in the environment: how many differing / VARY cases to print.)
"""
import sys, subprocess, json
from collections import Counter
from pathlib import Path
wt = Path(sys.argv[1]); seed, n = sys.argv[2], sys.argv[3]; kinds = set(sys.argv[4].split(","))
sys.path.insert(0, str(wt / "validation/src")); sys.path.insert(0, str(wt / "validation/probes/fs-io"))
import check
from larmorx_validation import oracle_cache
ws = check.workspace()
exe = ws / "oracles/freesurfer-8.2.0-probes/fs-io/fsio"
port = check.port_binary(12, "larmorx-freesurfer-io", "fs_io_probe")
text = subprocess.run([str(port), "cases", seed, n], check=True, capture_output=True, text=True).stdout
lines = [l for l in text.splitlines() if l.split()[0] in kinds]
cp = ws / "oracles/inputs/fs-io" / f"sub-{seed}-{n}-{'_'.join(sorted(kinds))}.txt"
cp.write_text("\n".join(lines) + "\n")
env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
done = oracle_cache.run([exe, cp], env=env, policy=oracle_cache.Policy(threads=1, family="freesurfer"), timeout=7200, runner="fs-io-probe")
off = done.stdout.splitlines()
res = {}
for mode in ("reproduce", "fix"):
    out = subprocess.run([str(port), "run", str(cp), mode], check=True, capture_output=True, text=True).stdout.splitlines()
    assert len(out) == len(off) == len(lines)
    eq, diff, skip, vary, masked, noexpl, ids_c = Counter(), Counter(), 0, 0, 0, Counter(), Counter()
    shown = 0
    vshown = 0
    veq, vdiff = Counter(), Counter()
    neq = Counter()
    for c, o, p in zip(lines, off, out):
        body, _, ids = p.partition("\t"); ids = [x for x in ids.split(",") if x]
        key = c.split()[0]
        if "SKIP" in ids: skip += 1; continue
        if "VARY" in ids:
            vary += 1
            if check.matches(o, body): veq[key] += 1
            else:
                vdiff[key] += 1
                if mode == "reproduce" and vshown < int(__import__("os").environ.get("VSHOW","0")):
                    vshown += 1; print("VCASE", c[:200]); print("VOFF ", o[:300]); print("VPORT", body[:300], ids)
            continue
        if check.matches(o, body):
            eq[key] += 1; masked += "?" in body
            for x in ids: neq[x] += 1
            tk = c.split()
            if key == "ixw" and len(tk) > 8 and tk[8] == "41" and body.startswith("0 "): neq["ITK-written"] += 1
            if key == "ixw" and "FS-722" in ids and body.startswith("0 "): neq["SGI-written"] += 1
        else:
            diff[key] += 1
            real = [x for x in ids if x not in ("SKIP", "VARY")]
            for x in real: ids_c[x] += 1
            if not real: noexpl[key] += 1
            if mode == "reproduce" and shown < int(__import__("os").environ.get("SHOW","8")):
                shown += 1; print("CASE", c[:200]); print("OFF ", o[:300]); print("PORT", body[:300], ids)
    res[mode] = {"equal": sum(eq.values()), "masked": masked, "differ": dict(diff), "skip": skip, "vary": vary, "differ_without_notice": dict(noexpl), "notices": dict(ids_c), "per_kind_equal": dict(eq), "vary_equal": dict(veq), "vary_differ": dict(vdiff), "notice_equal": dict(neq)}
print(json.dumps({"cases": len(lines), "served": done.cached, **res}, indent=1))
