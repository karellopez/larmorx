# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""The triage of a random-testing trial of ``fsl-topup-original``: every finding put in one
category by its recipe and its first failed check (the rules below, in order), with counts and
one reproducer (the lowest finding number) per category. The categories' causes are the ones
the re-checks and probes established (README, "Random testing (relay 5b)"). Usage:
  triage_counts.py <trial dir> [--json out.json]
"""

import collections
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from recheck import group_of  # noqa: E402


def pe_lengths(r):
    shape = r["inputs"]["pair"]["shape"]
    acq = [ln for ln in r["inputs"]["acq"]["text"].split("\n") if ln.strip()]
    axes = sorted({a for ln in acq for a, v in enumerate(map(float, ln.split()[:3])) if v != 0})
    return [shape[a] for a in axes]


def category(doc):
    r = doc["recipe"]
    f = r["features"]
    g = group_of(doc)
    first = g.split(" / ")[-1]
    names = [c["name"] for c in doc["checks"] if not c["passed"]]
    kind = f.get("kind")
    if kind == "applytopup":
        m = f.get("--method")
        if m in ("lsr", "vb2D", "vb3D", "vb4D") and any(2**32 % n for n in pe_lengths(r)):
            return "explained: FSL-133 (lines whose length does not divide 2^32)"
        if m in ("vb3D", "vb4D"):
            return "explained: FSL-143 (vb3D/vb4D's indefinite prior, reproduced; its divergence)"
        if m == "vb2D":
            return "explained: vb2D's transients (the record's at/vb2d-aniso)"
        if f.get("datatype") in ("char", "short") or (
            f.get("datatype") == "preserve" and f.get("dtype") in ("uint8", "int16")
        ):
            return "explained: integer outputs (float values a rounding apart land on neighbouring integers)"
        return f"open: applytopup {m} on lines that divide 2^32"
    if f.get("acquisition transposed") and not names:
        return "harness, fixed: the transposed acquisition file"
    if "FSL's configuration" in g and first == "values identical":
        return "harness, fixed: outputs named by a configuration file"
    if first.startswith("Standard output") and f.get("--trace") and f.get("-v"):
        return "harness, fixed: --trace with -v"
    if "quadratic" in g and first == "header":
        return "larmorx mistake, fixed: the quadratic B-splines' count"
    if "mixed axes" in g:
        return "FSL-146 (rule A; reproduce agrees) and, with movements, finding 14"
    if first == "Files written":
        return "explained: --debug's attempt files (the minimisation's path)"
    if "/ movements" in g or kind == "fmriprep":
        return "explained: finding 14 (FSL's movement derivatives)"
    if "quadratic" in g:
        return "open: quadratic splines' regulariser Hessian (k even)"
    return "fixed in part (the SCG weight) / explained: the path of ill-determined fits"


def main():
    trial = Path(sys.argv[1])
    cats = collections.Counter()
    first = {}
    for p in sorted(trial.glob("findings/*/*.json")):
        doc = json.loads(p.read_text())
        c = category(doc)
        cats[c] += 1
        first.setdefault(c, p.stem)
    for c, n in cats.most_common():
        print(f"{n:5d}  {c}  (e.g. {first[c]})")
    print(f"{sum(cats.values()):5d}  in all")
    if "--json" in sys.argv:
        Path(sys.argv[sys.argv.index("--json") + 1]).write_text(
            json.dumps({"counts": cats, "reproducers": first}, indent=1)
        )


main()
