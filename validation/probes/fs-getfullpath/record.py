# SPDX-License-Identifier: Apache-2.0
"""getfullpath's validation record: ``docs/validation/freesurfer-getfullpath.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the probe's summaries (``check.py --json``: the fixed cases, the seeds, ``--golden``), the
recorded calls (``real.py --json``) and the memory figures of ``tests/parity/test_memory_peak.py``
(its printed lines), and writes the record with the commit it was made from (a clean one: a
record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-getfullpath/record.py --probe all.json --seeds s1.json ... \\
        --golden golden.json --real real.json --memory memory.log
"""

from __future__ import annotations

import argparse
import datetime
import json
import platform
import re
import subprocess
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
NAME = "freesurfer-getfullpath"
KEYS = ("cases", "plain", "identical", "different", "unmodelled", "fix_differs", "fix_unexplained")


def commit() -> str:
    def git(*a: str) -> str:
        return subprocess.run(
            ["git", "-C", str(REPO), *a], check=True, capture_output=True, text=True
        ).stdout.strip()

    dirty = git("status", "--porcelain", "--untracked-files=no")
    return git("rev-parse", "--short=12", "HEAD") + ("-dirty" if dirty else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--probe", type=Path, required=True)
    ap.add_argument("--seeds", type=Path, nargs="+", required=True)
    ap.add_argument("--golden", type=Path, required=True)
    ap.add_argument("--real", type=Path, required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()

    def load(p: Path) -> dict:
        return json.loads(p.read_text(encoding="utf-8"))

    fixed = load(a.probe)
    seeds = {p.stem: {k: load(p)[k] for k in KEYS} for p in a.seeds}
    golden = load(a.golden)
    real = {k: v for k, v in load(a.real).items() if k != "rows"}
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"getfullpath ([\w-]+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append({"case": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                           "freesurfer_mib": float(m[4])})  # fmt: skip
    payload = {
        "name": NAME,
        "tool": "getfullpath (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/getfullpath/, src/csh.rs), "
        "the larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's official script ($FREESURFER_HOME/bin/getfullpath, "
        "/bin/tcsh 6.21.00, GNU coreutils 8.32) and the reference runs' recorded calls",
        "fixed": {k: fixed[k] for k in KEYS},
        "fixed_sha256": {"unix": fixed["OFFICIAL_SHA256"], "plain": fixed["OFFICIAL_SHA256_PLAIN"]},
        "seeds": seeds,
        "golden": {"cases": golden["cases"], "sha256": golden["OFFICIAL_SHA256"]},
        "recorded_calls": real,
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    f = payload["fixed"]
    lines = [
        "# Validation: FreeSurfer's `getfullpath` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-getfullpath/record.py` (queue item 3.2-12).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer getfullpath ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## The recorded calls",
        "",
        "seg2cc's two calls and fs-synthmorph-reg's three in every reference run's "
        f"`recon-all.log` (`real.py`): {real['calls']} calls on {len(real['runs'])} runs, "
        f"`reproduce` {real['reproduce_identical']} identical to the official script, the default "
        f"{real['fix_identical']}.",
        "",
        "## The whole script against the official one",
        "",
        "`validation/probes/fs-getfullpath/check.py`: the scripts' calls and every way tcsh reads "
        "a name again (blanks and the assignments they make, patterns of none, one and several "
        "matches, braces, `~`, `=N` before and after `pushd`, `+N` folders, leading dashes and "
        "`dirname`'s options, the test's arithmetic, `echo`'s escapes, folders whose paths hold "
        "blanks, tabs, patterns or backslashes), each in a folder holding the same files; exit "
        f"status and both streams, case by case. Fixed cases: {f['cases']} ({f['plain']} plain), "
        f"`reproduce` {f['identical']} identical, {f['different']} different, {f['unmodelled']} "
        f"not reproduced; `fix` differs on {f['fix_differs']}, each with a notice "
        f"({f['fix_unexplained']} without). Seeded random names of those pieces:",
        "",
    ]
    for name, s in seeds.items():
        lines.append(
            f"- {name}: {s['cases']} names, {s['identical']} identical, {s['different']} "
            f"different, {s['unmodelled']} not reproduced, `fix` {s['fix_unexplained']} without "
            "a notice."
        )
    lines += [
        "",
        "Not reproduced (both modes give the name as given, with FS-1173's notice): a new line in "
        "the name, bracket ranges (`[a-c]`), and the forms `specs/getfullpath.md` §5 lists. "
        "Recorded: `crates-freesurfer/larmorx-freesurfer-volumes/tests/getfullpath_official.rs` "
        f"(`{payload['fixed_sha256']['unix'][:8]}...` on Unix; the plain cases "
        f"`{payload['fixed_sha256']['plain'][:8]}...` on every system).",
        "",
        "## Golden cases",
        "",
        f"`tests/getfullpath_golden.rs`: {golden['cases']} cases (seg2cc's call, a relative name, "
        "a name in the folder itself, a folder that does not exist, no argument): "
        f"`{golden['OFFICIAL_SHA256']}`, the official script's value, with the default on every "
        "platform and `reproduce` on Unix.",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_getfullpath_replica_needs_no_more_memory_than_"
        "freesurfer` (the rule: at most 10 % above FreeSurfer's peak plus 8 MiB; the same output):",
        "",
    ]
    for e in memory:
        lines.append(
            f"- {e['case']} ({e['mode']}): larmorx {e['larmorx_mib']} MiB, FreeSurfer "
            f"{e['freesurfer_mib']} MiB (tcsh)."
        )
    (a.out / f"{NAME}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {a.out / NAME}.json and .md ({stamp})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
