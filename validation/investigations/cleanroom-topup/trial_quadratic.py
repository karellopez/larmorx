# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
import collections, json, sys
from pathlib import Path
from larmorx_validation.difftest import core, tools

trial = Path(sys.argv[1])
t = tools.get("fsl-topup-original")
c = collections.Counter()
for line in open(trial / "results.jsonl"):
    rr = json.loads(line)
    r = core.make_recipe(t, "topup-trial", rr["n"])
    if r["features"].get("kind") != "topup":
        continue
    opts = {
        a.split("=", 1)[0]: a.split("=", 1)[1]
        for g in r["groups"]
        for a in g["args"]
        if a.startswith("--") and "=" in a
    }
    if opts.get("--splineorder") != "2" or "--config" in opts:
        continue
    if any(x != "0" for x in opts.get("--estmov", "1").split(",")):
        continue
    sp = r["inputs"]["pair"]["geometry"].get("spacing", [1, 1, 1])
    sh = r["inputs"]["pair"]["shape"][:3]
    wr = [float(x) for x in opts.get("--warpres", "10").split(",")]
    ss = [int(x) for x in opts.get("--subsamp", "1").split(",")]
    n = max(len(wr), len(ss))
    wr += [wr[-1]] * (n - len(wr))
    ss += [ss[-1]] * (n - len(ss))
    short = False
    for w, s in zip(wr, ss):
        for nn, v in zip([x // s for x in sh], sp):
            k = max(1, round(w / (s * v)))
            if (k >= 3 and k <= nn < 2 * k) or (k == 2 and nn in (2, 3)) or nn < k:
                short = True
    lv = len(wr)
    c[("short axis" if short else "no short axis", f"{lv} level(s)", rr["verdict"])] += 1
for k, v in sorted(c.items()):
    print(k, v)
