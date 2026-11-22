# SPDX-License-Identifier: Apache-2.0
"""mri_info's validation record from the replica's runs: ``docs/validation/freesurfer-mri-info.{json,md}``.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it reads the replica's probe results).

Reads the replay of recon-all's and seg2cc's recorded calls (``replay.py --json``), the probe's
summaries (``check.py --json``), the golden value and the memory figures of
``tests/parity/test_memory_peak.py`` (its printed lines), and writes the record with the commit it
was made from (a clean one: a record stamped ``-dirty`` is redone).

    python -P validation/probes/fs-info/record.py --replay <replay.json> --probe <a.json> ... \\
        --memory <memory.log> --golden <sha>
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
NAME = "freesurfer-mri-info"
CALLS = {
    "rawavg": "`mri_info mri/rawavg.mgz` (recon-all, its output in `recon-all.log`)",
    "orig": "`mri_info mri/orig.mgz` (recon-all, its output in `recon-all.log`, also read for the "
    "field of view)",
    "nframes": "`mri_info --nframes mri/orig/001.mgz` (recon-all's check of each input)",
    "seg2cc": "`mri_info --o <tmp> --conformed-to-min mri/aseg.auto_noCCseg.mgz` (seg2cc, its "
    "answer in `seg2cc.log`)",
}


def commit() -> str:
    def git(*a: str) -> str:
        return subprocess.run(
            ["git", "-C", str(REPO), *a], check=True, capture_output=True, text=True
        ).stdout.strip()

    dirty = git("status", "--porcelain", "--untracked-files=no")
    return git("rev-parse", "--short=12", "HEAD") + ("-dirty" if dirty else "")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--replay", type=Path, required=True)
    ap.add_argument("--probe", type=Path, nargs="+", required=True)
    ap.add_argument("--memory", type=Path)
    ap.add_argument("--golden", required=True)
    ap.add_argument("--out", type=Path, default=REPO / "docs" / "validation")
    a = ap.parse_args(argv)
    stamp = commit()
    replay = json.loads(a.replay.read_text(encoding="utf-8"))
    rows = replay["rows"]
    probes = [json.loads(p.read_text(encoding="utf-8")) for p in a.probe]
    memory = []
    if a.memory and a.memory.is_file():
        pat = re.compile(r"mri_info (\w+) \((\w+)\): larmorx ([\d.]+) MiB, FreeSurfer ([\d.]+)")
        for m in pat.finditer(a.memory.read_text(encoding="utf-8")):
            memory.append(
                {"case": m[1], "mode": m[2], "larmorx_mib": float(m[3]),
                 "freesurfer_mib": float(m[4])}
            )  # fmt: skip
    by_call = {}
    for r in rows:
        c = by_call.setdefault(
            r["call"], {"calls": 0, "official": 0, "fix": 0, "logged": 0, "log": 0}
        )
        c["calls"] += 1
        c["official"] += r["reproduce_identical"]
        c["fix"] += r["fix_identical"]
        if "log_identical" in r:
            c["logged"] += 1
            c["log"] += r["log_identical"]
    payload = {
        "name": NAME,
        "tool": "mri_info (FreeSurfer 8.2.0)",
        "status": "experimental",
        "larmorx_commit": stamp,
        "generated": datetime.date.today().isoformat(),
        "platform": f"{platform.system()} {platform.machine()}",
        "replica": "crates-freesurfer/larmorx-freesurfer-volumes (src/mri_info/), the "
        "larmorx-freesurfer program",
        "reference": "FreeSurfer 8.2.0's recon-all runs (oracles/freesurfer-8.2.0/runs/: their "
        "logs) and its official program ($FREESURFER_HOME/bin/mri_info)",
        "replay": {"runs": replay["runs"], "calls": replay["calls"], "per_call": by_call},
        "probe": [
            {
                "seed": p["seed"],
                "cases": p["cases"],
                "identical": p["reproduce_equal"],
                "different": p["reproduce_differ"],
                "skipped": p["skipped"],
                "fix_different_with_notice": p["fix_differ"] - p["fix_differ_without_notice"],
                "fix_different_without_notice": p["fix_differ_without_notice"],
            }
            for p in probes
        ],
        "golden_sha256": a.golden,
        "memory": memory,
    }
    a.out.mkdir(parents=True, exist_ok=True)
    (a.out / f"{NAME}.json").write_text(json.dumps(payload, indent=1) + "\n", encoding="utf-8")
    lines = [
        "# Validation: FreeSurfer's `mri_info` (replica)",
        "",
        f"Status `experimental`. Generated {payload['generated']} on {payload['platform']} from "
        f"commit `{stamp}` with `validation/probes/fs-info/record.py` (queue item 3.2-08).",
        "",
        f"- **Replica:** {payload['replica']} (`larmorx-freesurfer [--upstream-defects "
        "reproduce|fix] freesurfer mri_info ...`).",
        f"- **Reference:** {payload['reference']}.",
        "",
        "## The recorded calls",
        "",
        "`mri_info` writes no file, so it is no step of the recordings: its calls are those "
        f"recon-all and seg2cc make, on each of the {replay['runs']} recorded runs (6 heads; the "
        "classic stream at four threads and at one, the v8 stream), run by "
        "`validation/probes/fs-info/replay.py` as recon-all runs them, against the official "
        "program now (exit status, both streams, the file written) and against the logs:",
        "",
    ]
    for k, c in by_call.items():
        lines.append(
            f"- {CALLS.get(k, k)}: {c['calls']} calls; `reproduce` {c['official']} identical to "
            f"the official program, `fix` {c['fix']}"
            + (
                f"; {c['log']} of {c['logged']} identical to the log."
                if c["logged"]
                else " (not logged)."
            )
        )
    lines += [
        "",
        "## The whole program against the official one",
        "",
        "`validation/probes/fs-info/check.py`: every option once on two volumes (with every header "
        "tag, and without), every format (MGZ, MGH, NIfTI, NIfTI compressed, Analyze, a COR "
        "folder), morphs and warp fields, names without an extension, missing files, folders, "
        "`--o` and `--otags`, a conformed volume, a voxel size of 0; then seeded volumes (1 to 5 "
        "voxels a side, 1 to 3 frames, four voxel types, every kind of axes) with zero to three "
        "options and one to three inputs. Exit status, both streams and the files written, case "
        "by case; counted apart: the cases where FreeSurfer crashes or its result depends on its "
        "build (`check.py`'s `SKIP`):",
        "",
    ]
    for p in payload["probe"]:
        lines.append(
            f"- seed {p['seed']}: {p['cases']} cases; `reproduce` {p['identical']} identical, "
            f"{p['different']} different, {p['skipped']} counted; `fix` "
            f"{p['fix_different_with_notice']} different with a notice, "
            f"{p['fix_different_without_notice']} without."
        )
    lines += [
        "",
        "## Golden cases",
        "",
        "`crates-freesurfer/larmorx-freesurfer-volumes/tests/mri_info_golden.rs`: 32 cases on two "
        "32^3 inputs made with integer arithmetic (a float volume with every header tag, a "
        "segmentation with a colour table: the report, `--dump`, the four whole-volume options, "
        "every matrix, `--otags` plain and compressed, `--ctab`, `--o`), on every platform, both "
        f"modes: `{a.golden}`, the official program's value on the same inputs "
        "(`check.py --golden`).",
        "",
        "## Memory",
        "",
        "`tests/parity/test_memory_peak.py::test_mri_info_replica_needs_no_more_memory_than_"
        "freesurfer` (ds000005 sub-01's files, one thread; the rule: at most 10 % above "
        "FreeSurfer's peak plus 8 MiB):",
        "",
    ]
    for e in memory:
        lines.append(
            f"- {e['case']} ({e['mode']}): {e['larmorx_mib']:.1f} MiB, FreeSurfer "
            f"{e['freesurfer_mib']:.1f} MiB."
        )
    (a.out / f"{NAME}.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"{NAME}: {stamp}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
