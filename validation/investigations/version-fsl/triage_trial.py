# SPDX-License-Identifier: Apache-2.0
"""The version update's random-testing trial against FSL 6.0.7.23, sorted: per tool and
implementation the verdicts, and every difference by its signature, with the known reason it
meets (FSL-59: `-sinc_final`/`-nn_final` with the clean-room original; Armadillo 14.2.1's last
bits; an exception in larmorx's own process, which a library swap during the trial can cause)
or none (a finding to look at).

Usage: python -P triage_trial.py <the trial's directory (results.jsonl)>
"""

from __future__ import annotations

import collections
import json
import sys
from pathlib import Path


def reason(tool: str, impl: str, row: dict, sig: list[str], detail: str) -> str:
    opts = " ".join(row.get("options", []))
    if "exception" in sig:
        return "exception in larmorx's process (see the detail)"
    if impl == "original" and ("-sinc_final" in opts or "-nn_final" in opts):
        return "FSL-59 (mcflirt 6.0.7.23 resamples trilinearly; the clean-room original follows the option)"
    if impl == "original":
        return "the clean-room original's own thresholds (as in every night)"
    if any(s.endswith((".rms: values", ".par: values")) or ".rms" in s for s in sig):
        return "Armadillo 14.2.1's LAPACK last bits (rms of nearly equal matrices), if within 1e-12"
    return "none known (a finding)"


def main() -> int:
    d = Path(sys.argv[1])
    rows = [json.loads(line) for line in (d / "results.jsonl").read_text().splitlines() if line]
    verdicts: collections.Counter = collections.Counter()
    sigs: collections.Counter = collections.Counter()
    for r in rows:
        for impl, v in r.get("impls", {}).items():
            verdicts[(r["tool"], impl, v.get("verdict"))] += 1
            if v.get("verdict") not in ("agree", None, "notice"):
                sig = v.get("signature") or []
                why = reason(r["tool"], impl, r, sig, v.get("detail", ""))
                sigs[(r["tool"], impl, "; ".join(sig)[:120], why)] += 1
    print(f"{len(rows)} cases")
    for k, n in sorted(verdicts.items(), key=str):
        print(f"  {k[0]:12s} {k[1]:9s} {k[2]}: {n}")
    print("differences:")
    for (tool, impl, sig, why), n in sorted(sigs.items(), key=lambda kv: -kv[1]):
        print(f"  {n:4d} {tool} {impl}: [{sig}] -> {why}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
