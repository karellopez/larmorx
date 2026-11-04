# SPDX-License-Identifier: Apache-2.0
"""The triage of the random-testing trial of fslmaths' and fslstats' clean-room originals
(``oracles/difftest/2026-11-03-fsl-maths-original``): every finding in one category, by its
re-check with the final build (``recheck_trial.py``'s rows: agree or notice means a fix reached
it) and its trial detail and operations (the rules below, in order), with counts and one
reproducer (the lowest case number) per category. The categories' causes are findings 24 to 30
of ``docs/findings/fsl-maths-original.md`` and the probes of this folder (README).

Usage: ``python -P triage_counts.py <night> <recheck.jsonl> [--json out.json]``.
"""

import collections
import json
import re
import sys
from pathlib import Path

F24 = "findings 24"


def _fixed(d: dict, ops: list[str], tool: str, dtype: str) -> str:
    det = d.get("detail", "") + " " + d.get("shrunk_detail", "")
    sig = " ".join(d.get("signature") or [])
    if tool == "fsl-stats-original":
        if "JSON" in sig or "mask is empty" in det or re.search(r"\d+ vs \d+ lines", det):
            return "larmorx mistake, fixed: fslstats' histograms without a histogram (findings 28)"
        return "harness, fixed: §10's bound over a histogram's lines and an output cut short"
    if "double free" in det or "corruption" in det:
        return f"larmorx mistake, fixed: FSL-75 named in -subsamp2 ({F24}: FSL's heap abort)"
    if "exception" in sig:
        return "harness, fixed: the suite's tensor check (single slices) and NIfTI-2 quaternions"
    if "quatern" in det and not re.search(r"srow|qoffset", det):
        return f"larmorx mistake, fixed: a subsampled qform's quaternion ({F24})"
    if "header" in det and re.search(r"srow|qoffset", det):
        return f"larmorx mistake, fixed: the geometry of a new grid along x ({F24})"
    if "-roc" in ops:
        return "larmorx mistake, fixed: -roc's false-positive candidates (findings 29)"
    if "D13 fix pass" in det and "header changed" in det:
        return "larmorx mistake, fixed: FSL-157 named for every int8 -odt input (findings 29)"
    if "D13 fix pass" in det:
        return "larmorx mistake, fixed: FSL-154 named for every uint8 image computed unsigned"
    if "Empty mask" in det or "Off-centre morphfilter" in det:
        return "larmorx mistake, fixed: the morphology's messages with a kernel image (findings 25)"
    if "TFCE failed" in det:
        return "larmorx mistake, fixed: TFCE's step tested before the sizes (findings 26)"
    if any(o in ops for o in ("-fillh", "-fillh26")):
        return "larmorx mistake, fixed: FSL-165 on every image with an axis of size 1 (findings 27)"
    if "-tfce" in ops:
        if dtype == "float64":
            return (
                "larmorx mistake, fixed: TFCE of a double image in single precision (findings 26)"
            )
        return "harness, fixed: §21's TFCE metric in the suite"
    if "-edge" in ops:
        return "larmorx mistake, fixed: -edge's integer differences (findings 29)"
    if any(o.startswith("-subsamp2") for o in ops):
        return "larmorx mistake, fixed: the subsampling's sums in double (findings 29)"
    if "-ptoz" in ops or "-ztop" in ops:
        return "larmorx mistake, fixed: -ptoz's and -ztop's NaN (findings 30)"
    if "-tensor_decomp" in ops:
        return "harness, fixed: the suite's tensor check (compressed names, single slices)"
    if "zstd" in det or "backports" in det:
        return "harness, fixed: zstd images read through FSL's zstd"
    return "fixed: a chain the fixes above reach"


def _left(r: dict) -> str:
    d, ops = r["detail"], r["ops"]
    if r["tool"] == "fsl-stats-original":
        if re.search(r"'-?nan' / original '-?nan'", d):
            return "explained: fslstats -c/-C of images holding NaN and infinities: the NaN's sign"
        return (
            "explained: fslstats' last printed digit at §10's bound (or bounded words unattributed)"
        )
    if "files written" in d and any(o.startswith("-subsamp2") for o in ops):
        return "register: FSL-75 (FSL's heap corruption crashes it before it writes; named)"
    if "-tensor_decomp" in ops:
        return "explained: -tensor_decomp of degenerate tensors (eigenvalues at the rounding level)"
    names = [x.strip() for x in d.split("  ")[0].split(",")]
    only_header = all("header" in x for x in names if x)
    if only_header and re.search(r"srow|qoffset", d):
        return "explained: the header's offsets after a grid change (at most 6 ulp of one offset)"
    if "sums' share" in d or "sums' bound" in d:
        if "-dog_edge" in ops:
            return "explained: -dog_edge's crossings at exact ties (within the sums' bound)"
        if "-subsamp2offc" in ops:
            return "expected: -subsamp2offc's means (findings 13 and 29)"
        if "-subsamp2" in ops:
            return "explained: -subsamp2's last bits under the floating types (spec question 13)"
        if "-ztop" in ops or "-ptoz" in ops:
            return "explained: -ztop's and -ptoz's last bits (Φ, Φ⁻¹), within the sums' bound"
        return "explained: sums beyond 4 ulp where they cancel to near 0 (spec question 14)"
    if "-dog_edge" in ops:
        return "explained: -dog_edge's crossings at exact ties (within the sums' bound)"
    if "-randn" in ops:
        return "explained: -randn in double: Φ⁻¹'s last bits"
    if "-fmedian" in ops or "-dilD" in ops:
        return "explained: the median's and mode's signed zero (§18: ±0's order)"
    if any(o.startswith("-subsamp2") for o in ops):
        return "explained: -subsamp2's last bits under the floating types (spec question 13)"
    if "-tfce" in ops:
        return "explained: TFCE's last bits through a discontinuous operation (-rem)"
    return "open: " + (names[0][:80] if names else "unclear")


def main(argv: list[str]) -> int:
    night, rows_path = Path(argv[0]), Path(argv[1])
    rows = [json.loads(x) for x in rows_path.read_text(encoding="utf-8").splitlines() if x]
    counts: collections.Counter = collections.Counter()
    first: dict[str, tuple[int, str]] = {}
    for r in rows:
        f = night / "findings" / r["tool"] / r["finding"]
        d = json.loads(f.read_text(encoding="utf-8"))
        rec = d.get("shrunk") or d["recipe"]
        dtype = (rec.get("inputs", {}).get("img") or {}).get("dtype", "")
        if r["verdict"] in ("agree", "notice"):
            cat = _fixed(d, r["ops"], r["tool"], dtype)
        else:
            cat = _left(r)
        key = f"{r['tool']}: {cat}"
        counts[key] += 1
        n = int(d["n"])
        if key not in first or n < first[key][0]:
            first[key] = (n, f"{r['tool']}/{r['finding'].removesuffix('.json')}")
    out = {
        "counts": dict(counts.most_common()),
        "reproducers": {k: first[k][1] for k, _ in counts.most_common()},
    }
    for k, n in counts.most_common():
        print(f"{n:6d}  {k}  (e.g. {first[k][1]})")
    if "--json" in argv:
        Path(argv[argv.index("--json") + 1]).write_text(
            json.dumps(out, indent=1, ensure_ascii=False) + "\n", encoding="utf-8"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
