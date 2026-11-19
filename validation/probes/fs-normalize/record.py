# SPDX-License-Identifier: Apache-2.0
"""mri_normalize's validation record from the replica's runs: ``docs/validation/freesurfer-mri-normalize.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the replays of recon-all's recorded calls (``freesurfer-replay run ... --json``, one file per
run and mode), the whole-program probe's summaries (``prog.py --json``, one per seed), and the
memory figures of ``tests/parity/test_memory_peak.py`` (its printed lines), and writes the record
with the commit it was made from (a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-normalize/record.py --replays <dir> --modes reproduce,fix \\
        --prog <prog1.json> <prog2.json> --memory <memory.log> --golden <sha>
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
NAME = "freesurfer-mri-normalize"


def commit() -> str:
    head = subprocess.run(
        ["git", "-C", str(REPO), "rev-parse", "--short=12", "HEAD"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    dirty = subprocess.run(
        ["git", "-C", str(REPO), "status", "--porcelain", "--untracked-files=no"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()
    return head + ("-dirty" if dirty else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--replays", type=Path, required=True, help="<dir>/replay-<mode>/<run>.json")
    ap.add_argument("--modes", default="reproduce,fix")
    ap.add_argument("--prog", type=Path, nargs="+", required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    steps = {}
    for mode in a.modes.split(","):
        rows = []
        for f in sorted((a.replays / f"replay-{mode}").glob("*.json")):
            rows.extend(json.loads(f.read_text(encoding="utf-8")))
        steps[mode] = rows
    progs = [json.loads(p.read_text(encoding="utf-8")) for p in a.prog]
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(
            r"mri_normalize (\w+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)"
        )
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append(
                {
                    "case": m[1],
                    "mode": m[2],
                    "larmorx_mib": float(m[3]),
                    "freesurfer_mib": float(m[4]),
                }
            )
    payload = {
        "name": NAME,
        "tool": "mri_normalize (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_normalize/), the "
        "larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's recon-all runs (oracles/freesurfer-8.2.0/runs/) and the "
        "unit oracle's bin/mri_normalize (byte-identical to the official program)",
        "replay": {
            mode: {
                "steps": len(rows),
                "reproduced": sum(r["verdict"] == "reproduced" for r in rows),
                "runs": sorted({r["run"] for r in rows}),
                "seconds_replayed": round(sum(r["seconds"]["replayed"] for r in rows), 1),
                "seconds_recorded": round(sum(r["seconds"]["recorded"] for r in rows), 1),
            }
            for mode, rows in steps.items()
        },
        "program": [
            {"seed": p["seed"], "cases": p["cases"], "verdicts": p["verdicts"]} for p in progs
        ],
        "golden_sha256": a.golden,
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    r = payload["replay"]
    lines = [
        "# Validation: FreeSurfer's `mri_normalize` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-normalize/record.py` (queue item 3.2-25).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_normalize ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## recon-all's calls, replayed",
        "",
        "Every recorded `mri_normalize` step of the 12 runs (6 heads, the classic and v8 streams; "
        "`-g 1 -seed 1234 -mprage nu.mgz T1.mgz` and `-seed 1234 -mprage -aseg aseg.presurf.mgz "
        "-mask brainmask.mgz norm.mgz brain.mgz`), rebuilt and run by the replay tool "
        "(`docs/validation/freesurfer-replay.md`) at one thread:",
        "",
    ]
    for mode, v in r.items():
        lines.append(
            f"- `{mode}`: **{v['reproduced']} of {v['steps']} steps reproduced** (the output "
            f"identical to the recorded one, FreeSurfer's provenance aside, and the exit status); "
            f"{v['seconds_replayed']:.0f} s for all ({v['seconds_recorded']:.0f} s recorded, "
            "FreeSurfer at four threads)."
        )
    lines += [
        "",
        "## The whole program against the official binary",
        "",
        "`validation/probes/fs-normalize/prog.py` (the open probe's 82 command lines, "
        "`validation/investigations/freesurfer-mri-normalize-probe/`): every option, the three "
        "programs of their own (`-renorm`, `-surface`, `-r`), the failures; exit status, standard "
        "output and error, and every file written compared:",
        "",
    ]
    for p in payload["program"]:
        v = p["verdicts"]
        lines.append(
            f"- seed {p['seed']}: {p['cases']} cases; `reproduce` {v.get('identical', 0)} "
            f"identical, {v.get('different', 0)} different, {v.get('reproduce-skip', 0)} counted "
            "(FreeSurfer reads memory it does not own or crashes there: FS-597, FS-601, FS-1016; "
            f"or PE); `fix` {v.get('fix-identical', 0)} identical, {v.get('fix-explained', 0)} "
            f"different with a notice, {v.get('fix-unexplained', 0)} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_normalize_golden.rs`: recon-all's "
        "two calls and `-w` on 32^3 inputs made with integer arithmetic, on every platform, both "
        f"modes: `{a.golden}`, the official program's value on the same inputs (`prog.py "
        "--golden`).",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_normalize_replica_needs_no_more_memory_than_"
        "freesurfer` (ds000005 sub-01's inputs, one thread; the rule: at most 10 % above "
        "FreeSurfer's peak plus 8 MiB):",
        "",
    ]
    for m in memory:
        lines.append(
            f"- {m['case']} ({m['mode']}): {m['larmorx_mib']:.0f} MiB, FreeSurfer "
            f"{m['freesurfer_mib']:.0f} MiB."
        )
    (a.out / f"{NAME}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {a.out / NAME}.json and .md from {stamp}")
    return 1 if stamp.endswith("-dirty") else 0


if __name__ == "__main__":
    raise SystemExit(main())
