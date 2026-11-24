# SPDX-License-Identifier: Apache-2.0
"""mri_label2vol's recorded calls on the recorded heads: the official program against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica).

Both streams call ``mri_label2vol`` from ``defect2seg`` (``recon-all.log``, after the topology
fix): ``--defects surf/lh.orig.nofix surf/lh.defect_labels mri/orig.mgz 1000 0
mri/surface.defects.mgz label/lh.nofix.cortex.label``, then the right hemisphere's into the same
volume (``... surf/rh.defect_labels mri/surface.defects.mgz 2000 1 mri/surface.defects.mgz
label/rh.nofix.cortex.label``). On each recorded run (``oracles/freesurfer-8.2.0/runs/*-v8`` and
``*-classic``) the two calls run on the run's own files; the right hemisphere's on the left's
official output (both sides read the same bytes). ``seg2cc`` calls ``mri_label2vol --seg <seg>
--temp <vol> --regheader --o <out>`` for heads that are not conformed (none of the recorded six):
its form runs here with the head's ``aseg.mgz`` onto its native grid (``rawavg.mgz``) and back onto
``norm.mgz``. Each call: the official program through the oracle-output cache (one thread,
``FS_DISABLE_LANG=1``, ``FREESURFER_SEED=1234``: the defects' colour table takes random colours,
which recon-all leaves to the clock), the port (``--port <larmorx-freesurfer>``) with ``reproduce``
and ``fix``, each in a folder of its own; exit status, both streams and the files written compared
byte for byte (a volume's recorded command line masked); the official defect volume's header and
voxels against the run's recorded ``mri/surface.defects.mgz`` (its colour table, clock-seeded in
the recording, left out).

    python -P validation/probes/fs-label2vol/real.py --port <larmorx-freesurfer> [--json out.json]
"""

from __future__ import annotations

import argparse
import gzip
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))
sys.path.insert(0, str(HERE))

from check import canon  # noqa: E402
from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

POLICY = oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer")
SIZES = {0: 1, 1: 4, 3: 4, 4: 2}  # uchar, int, float, short


def files(d: Path) -> dict[str, bytes]:
    return {
        str(p.relative_to(d)): canon(p.read_bytes()) for p in sorted(d.rglob("*")) if p.is_file()
    }


def image(b: bytes) -> bytes:
    """An MGH (or MGZ) file's header and voxels, its tags left out."""
    if b[:2] == b"\x1f\x8b":
        b = gzip.decompress(b)
    w, h, d, nf, ty = (int.from_bytes(b[4 * k + 4 : 4 * k + 8], "big") for k in range(5))
    return b[: 284 + w * h * d * nf * SIZES[ty]]


def compare(port: Path, exe: Path, env: dict, work: Path, run: str, name: str,
            args: list[str], inputs: dict[str, Path], recorded: Path | None = None) -> dict:  # fmt: skip
    """One call: the official program, the port with ``reproduce`` and ``fix``, each in a folder
    of its own (the files ``inputs`` names copied in first); the row (identical or not, the notices
    of ``fix``; the official output's image against ``recorded``'s)."""
    outs = {}
    for side in ("official", "reproduce", "fix"):
        d = work / run / name / side
        if d.exists():
            shutil.rmtree(d)
        d.mkdir(parents=True)
        for k, src in inputs.items():
            shutil.copyfile(src, d / k)
        given = {k: (d / k).read_bytes() for k in inputs}
        if side == "official":
            done = oracle_cache.run([exe.name, *args], env=env, cwd=d, outputs=[d],
                                    policy=POLICY, timeout=1200, runner="fs-label2vol-real",
                                    inputs=[d / k for k in inputs])  # fmt: skip
            rc, so, se = done.returncode, done.stdout, done.stderr
            ids: list[str] = []
        else:
            p = subprocess.run([str(port), "--upstream-defects", side, "freesurfer",
                                "mri_label2vol", *args], env=env, cwd=d,
                               capture_output=True, text=True, timeout=1200,
                               check=False)  # fmt: skip
            lines = p.stderr.splitlines()
            se = "".join(x + "\n" for x in lines if not x.startswith("larmorx:"))
            ids = sorted({m[1] for x in lines
                          for m in [re.match(r"larmorx: upstream defect (\S+)", x)] if m})  # fmt: skip
            rc, so = p.returncode, p.stdout
        written = {k: v for k, v in files(d).items() if canon(given.get(k, b"")) != v}
        outs[side] = (rc, so, se, written, ids, d)
    o = outs["official"][:4]
    row = {"run": run, "call": name, "status": outs["official"][0],
           "reproduce": outs["reproduce"][:4] == o, "fix": outs["fix"][:4] == o,
           "fix_notices": outs["fix"][4]}  # fmt: skip
    if recorded is not None:
        out = outs["official"][5] / args[-2] if args[0] == "--defects" else None
        row["official_image_is_recorded"] = (
            out is not None and image(out.read_bytes()) == image(recorded.read_bytes())
        )
    print(json.dumps(row), flush=True)
    return row


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--port", type=lambda s: Path(s).resolve(), required=True)
    ap.add_argument("--json", type=Path)
    ap.add_argument("--runs", nargs="*", help="the runs (default: every -v8 and -classic run)")
    ap.add_argument("--no-seg", action="store_true", help="only the defects' calls")
    a = ap.parse_args(argv)
    ws = workspace()
    fsh = ws / FS_HOME
    exe = fsh / "bin" / "mri_label2vol"
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "LC_ALL": "C", "LANG": "C",
           "OMP_NUM_THREADS": "1", "FS_DISABLE_LANG": "1", "USER": "probe",
           "FREESURFER_SEED": "1234", **freesurfer_env()}  # fmt: skip
    runs_dir = ws / "oracles" / "freesurfer-8.2.0" / "runs"
    runs = (
        a.runs
        if a.runs is not None
        else sorted(p.name for p in runs_dir.iterdir() if p.name.endswith(("-v8", "-classic")))
    )
    work = ws / "oracles" / "inputs" / "fs-label2vol-real"
    rows = []
    for run in runs:
        subj = next(p for p in (runs_dir / run / "subjects").iterdir() if p.name != "fsaverage")
        env["SUBJECTS_DIR"] = str(subj.parent)
        sf, mri, lab = subj / "surf", subj / "mri", subj / "label"
        rec = mri / "surface.defects.mgz"
        lh = compare(a.port, exe, env, work, run, "defects-lh",
                     ["--defects", "lh.orig.nofix", "lh.defect_labels", "orig.mgz", "1000", "0",
                      "surface.defects.mgz", "lh.nofix.cortex.label"],
                     {"lh.orig.nofix": sf / "lh.orig.nofix",
                      "lh.defect_labels": sf / "lh.defect_labels", "orig.mgz": mri / "orig.mgz",
                      "lh.nofix.cortex.label": lab / "lh.nofix.cortex.label"})  # fmt: skip
        rows.append(lh)
        lh_out = work / run / "defects-lh" / "official" / "surface.defects.mgz"
        rows.append(compare(a.port, exe, env, work, run, "defects-rh",
                            ["--defects", "rh.orig.nofix", "rh.defect_labels",
                             "surface.defects.mgz", "2000", "1", "surface.defects.mgz",
                             "rh.nofix.cortex.label"],
                            {"rh.orig.nofix": sf / "rh.orig.nofix",
                             "rh.defect_labels": sf / "rh.defect_labels",
                             "surface.defects.mgz": lh_out,
                             "rh.nofix.cortex.label": lab / "rh.nofix.cortex.label"},
                            recorded=rec))  # fmt: skip
        if not a.no_seg:
            rows.append(compare(a.port, exe, env, work, run, "seg-to-native",
                                ["--seg", "aseg.mgz", "--temp", "rawavg.mgz", "--regheader",
                                 "--o", "aseg.native.mgz"],
                                {"aseg.mgz": mri / "aseg.mgz", "rawavg.mgz": mri / "rawavg.mgz"}))  # fmt: skip
            native = work / run / "seg-to-native" / "official" / "aseg.native.mgz"
            rows.append(compare(a.port, exe, env, work, run, "seg-to-conformed",
                                ["--seg", "aseg.native.mgz", "--temp", "norm.mgz", "--regheader",
                                 "--o", "aseg.conf.mgz"],
                                {"aseg.native.mgz": native, "norm.mgz": mri / "norm.mgz"}))  # fmt: skip
            shutil.rmtree(work / run / "seg-to-native")
            shutil.rmtree(work / run / "seg-to-conformed")
        shutil.rmtree(work / run / "defects-lh")
        shutil.rmtree(work / run / "defects-rh")
    summary = {
        "calls": len(rows),
        "runs": runs,
        "reproduce_identical": sum(r["reproduce"] for r in rows),
        "fix_identical": sum(r["fix"] for r in rows),
        "fix_differ_without_notice": sum(not r["fix"] and not r["fix_notices"] for r in rows),
        "official_image_is_recorded": sum(r.get("official_image_is_recorded", False) for r in rows),
        "with_a_record": sum("official_image_is_recorded" in r for r in rows),
        "rows": rows,
    }
    print(json.dumps({k: v for k, v in summary.items() if k != "rows"}))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    ok = summary["reproduce_identical"] == len(rows)
    return 0 if ok and summary["official_image_is_recorded"] == summary["with_a_record"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
