# SPDX-License-Identifier: Apache-2.0
# ruff: noqa  (an investigation script, kept as run)
"""The clean-room catch-up (relay 9, 2026-11-05): the drawn cases of the catch-up trials' "fixed" categories replayed with the final build
(difftest --replay, FSL from the oracle cache); one line per finding, then the counts."""

import collections, concurrent.futures, json, os, subprocess, sys
from pathlib import Path

WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[3].parent))
WS = WORKSPACE / "oracles" / "difftest"
out = Path(sys.argv[1])
out.mkdir(parents=True, exist_ok=True)
sel = []
for topic, tool, triage, prefix in [
    ("flirt", "fsl-flirt-original", "cleanroom-flirt", "20261103"),
    ("topup", "fsl-topup-original", "cleanroom-topup", "topup-trial"),
]:
    tri = json.loads(
        Path(f"validation/investigations/{triage}/trial_catchup_triage.json").read_text()
    )
    for cat, members in tri["members"].items():
        if cat.startswith("fixed") or cat == "open: applytopup lsr":
            for n in members:
                sel.append(
                    (
                        topic,
                        tool,
                        cat,
                        WS
                        / f"2026-11-05-catchup-{topic}/findings/{tool}/{prefix}-{n:05d}-original.json",
                    )
                )


def one(item):
    topic, tool, cat, path = item
    doc = json.loads(path.read_text())
    doc.pop("shrunk", None)
    p = out / path.name
    p.write_text(json.dumps(doc))
    r = subprocess.run(
        [
            sys.executable,
            "-m",
            "larmorx_validation",
            "difftest",
            "--replay",
            str(p),
            "--memory-gib",
            "3",
        ],
        capture_output=True,
        text=True,
        timeout=1800,
    )
    (out / f"{path.stem}.log").write_text(r.stdout + r.stderr)
    lines = [ln for ln in r.stdout.splitlines() if ln.startswith(f"{tool} (")]
    v = lines[-1].split("): ", 1)[1] if lines else f"exit {r.returncode}"
    return cat, path.stem, v


res = collections.Counter()
with concurrent.futures.ThreadPoolExecutor(int(sys.argv[2])) as ex:
    for cat, stem, v in ex.map(one, sel):
        print(stem, "|", cat, "|", v[:160], flush=True)
        res[(cat, v.split(" ", 1)[0])] += 1
for k, n in sorted(res.items()):
    print(n, k)
print("REPLAY-END")
