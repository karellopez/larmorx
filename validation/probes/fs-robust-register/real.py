# SPDX-License-Identifier: Apache-2.0
"""mri_robust_register on the recorded heads: FreeSurfer 8.2.0's official program against the port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it runs the replica; the official program is run as
a black box).

fMRIPrep's call (sMRIPrep's ``RobustRegister``: ``--satit --iscale``, the subject's ``T1.mgz``
onto the T1w reference; here each recorded head's ``rawavg.mgz``, the T1w in its own space, of
``<workspace>/oracles/freesurfer-8.2.0/runs/<head>-classic/subjects/<subject>/mri/``) through the
official ``mri_robust_register`` (``$FREESURFER_HOME/bin``, by the oracle-output cache, one
thread) and through the port (``larmorx-freesurfer freesurfer mri_robust_register``) with
``--upstream-defects`` ``reproduce`` and ``fix``; with ``--outputs`` also ``--mapmov``,
``--weights``, ``--halfmov``, ``--halfdst``, ``--halfweights``, ``--halfmovlta`` and
``--halfdstlta``. Compares the exit status, standard output (the line of the time taken masked)
and error, and every file written (byte for byte). The LTA's creation line is fixed by
``FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING`` and ``USER`` on both sides; ``FS_DISABLE_LANG=1``
on the official side (``docs/findings/freesurfer-info.md``); no ``DIAG_NO``.

    python -P validation/probes/fs-robust-register/real.py [--heads a,b] [--outputs] [--extra '<args>']
        [--preload <lib>] [--json out.json]
"""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "validation" / "src"))

from larmorx_validation import oracle_cache  # noqa: E402
from larmorx_validation.freesurfer_unit import FS_HOME, freesurfer_env  # noqa: E402
from larmorx_validation.oracle_cache.store import workspace  # noqa: E402

HEADS = (
    "ds000005-sub-01",
    "ds000228-sub-pixar008",
    "ds002422-sub-02",
    "ds004173-sub-603309",
    "ds005003-sub-RS001",
    "ds005454-sub-16",
)
MANIFEST = REPO / "crates-freesurfer" / "Cargo.toml"
CLI = REPO / "crates-freesurfer" / "target" / "release" / "larmorx-freesurfer"
TIME = re.compile(rb"Registration took \d+ minutes and \d+ seconds\.")
# relay 45: the entropy image's time (masked in the comparison only; the cache's policy keeps TIME)
ENTROPY_TIME = re.compile(rb" Entropy computation took \d+ seconds\.")


def masked(b: bytes) -> bytes:
    return ENTROPY_TIME.sub(b" Entropy computation took <TIME> seconds.", TIME.sub(b"<TIME>", b))


FIXED = {
    "LC_ALL": "C",
    "LANG": "C",
    "OMP_NUM_THREADS": "1",
    "USER": "larmorx",
    "TZ": "UTC0",
    "FREESURFER_REPLACEMENT_FOR_CREATION_TIME_STRING": "Thu Jan  1 00:00:00 1970",
}
OUTS = [
    ("--mapmov", "mapmov.mgz"),
    ("--weights", "weights.mgz"),
    ("--halfmov", "halfmov.mgz"),
    ("--halfdst", "halfdst.mgz"),
    ("--halfweights", "halfweights.mgz"),
    ("--halfmovlta", "halfmov.lta"),
    ("--halfdstlta", "halfdst.lta"),
]


def files(d: Path) -> dict[str, str]:
    return {
        p.name: hashlib.sha256(p.read_bytes()).hexdigest()
        for p in sorted(d.iterdir())
        if p.is_file()
    }


#: relay 45 of track C: arguments added after fMRIPrep's (``--extra``)
EXTRA: list[str] = []


def args_of(mri: Path, outputs: bool) -> list[str]:
    a = ["--mov", str(mri / "T1.mgz"), "--dst", str(mri / "rawavg.mgz"), "--lta", "reg.lta"]
    a += ["--satit", "--iscale", *EXTRA]
    if outputs:
        for opt, name in OUTS:
            a += [opt, name]
    return a


def official(exe: Path, mri: Path, d: Path, outputs: bool, preload: Path | None = None) -> dict:
    env = {"PATH": f"{exe.parent}:/usr/bin:/bin", "FS_DISABLE_LANG": "1", **FIXED,
           **freesurfer_env()}  # fmt: skip
    if preload is not None:
        # relay 45 of track C: the decided cabs and csqrt (decided_complex.sh)
        env["LD_PRELOAD"] = str(preload)
    if any(k.startswith("DIAG") for k in env):
        raise SystemExit("DIAG_NO would print the licence file")
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    t0 = time.monotonic()
    done = oracle_cache.run(
        [exe.name, *args_of(mri, outputs)],
        env=env,
        cwd=d,
        outputs=[d],
        policy=oracle_cache.Policy(
            threads=1, family="freesurfer", formats="freesurfer", volatile=(TIME.pattern.decode(),)
        ),
        timeout=None,
        runner="fs-robust-register-real",
        peak=True,
    )
    return {
        "status": done.returncode,
        "stdout": masked(done.stdout.encode("utf-8", "surrogateescape")).decode(),
        "stderr": done.stderr,
        "files": files(d),
        "seconds": round(time.monotonic() - t0, 1),
        "served": done.cached,
        "peak_mib": getattr(done, "peak_mib", None),
    }


def port(mri: Path, d: Path, mode: str, outputs: bool) -> dict:
    if d.exists():
        shutil.rmtree(d)
    d.mkdir(parents=True)
    env = {"PATH": "/usr/bin:/bin", **FIXED}
    t0 = time.monotonic()
    done = subprocess.run(
        ["/usr/bin/time", "-f", "PEAK_KIB %M", str(CLI), "--upstream-defects", mode, "freesurfer",
         "mri_robust_register", *args_of(mri, outputs)],
        env=env, cwd=d, capture_output=True, timeout=7200,
    )  # fmt: skip
    err = done.stderr.decode("utf-8", "surrogateescape")
    peak = re.search(r"PEAK_KIB (\d+)\n?$", err)
    err = err[: peak.start()] if peak else err
    # the notices the port adds after the program's own lines
    err = "".join(x for x in err.splitlines(True) if not x.startswith("larmorx: "))
    return {
        "status": done.returncode,
        "stdout": masked(done.stdout).decode("utf-8", "surrogateescape"),
        "stderr": err,
        "files": files(d),
        "seconds": round(time.monotonic() - t0, 1),
        "peak_mib": round(int(peak.group(1)) / 1024, 1) if peak else None,
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--heads", default=",".join(HEADS))
    ap.add_argument("--outputs", action="store_true", help="every output option too")
    ap.add_argument("--parallel", type=int, default=6)
    ap.add_argument("--jobs", type=int, default=6)
    ap.add_argument("--json", type=Path)
    ap.add_argument(
        "--extra",
        default="",
        help="arguments added after fMRIPrep's, space-separated (e.g. the entropy images: "
        "'--cost ROBENT --entmov em.mgz --entdst ed.mgz'); outputs in real-extra/",
    )
    ap.add_argument(
        "--preload",
        type=Path,
        help="a library for the official program's LD_PRELOAD (decided_complex.sh's: the decided "
        "cabs and csqrt); its outputs in real-preload/",
    )
    a = ap.parse_args(argv)
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = ws / FS_HOME / "bin" / "mri_robust_register"
    env = {k: v for k, v in os.environ.items() if k != "FSLDIR"}
    subprocess.run(
        ["cargo", "build", "--release", "-p", "larmorx-freesurfer-cli", "-j", str(a.jobs),
         "--manifest-path", str(MANIFEST)],
        check=True, env=env,
    )  # fmt: skip
    root = (
        ws / "oracles" / "inputs" / "fs-robust-register" / ("real-outputs" if a.outputs else "real")
    )
    if a.preload is not None:
        root = root.with_name(root.name + "-preload")
    if a.extra:
        EXTRA[:] = a.extra.split()
        root = root.with_name(root.name + "-extra")
    heads = a.heads.split(",")
    runs = ws / "oracles" / "freesurfer-8.2.0" / "runs"

    def one(head: str) -> dict:
        sub = head.split("-", 1)[1]
        mri = runs / f"{head}-classic" / "subjects" / sub / "mri"
        o = official(exe, mri, root / head / "official", a.outputs, a.preload)
        rep = port(mri, root / head / "reproduce", "reproduce", a.outputs)
        fix = port(mri, root / head / "fix", "fix", a.outputs)
        same = {
            k: all(o[k] == r[k] for r in (rep,)) for k in ("status", "stdout", "stderr", "files")
        }
        fix_same = {k: o[k] == fix[k] for k in ("status", "stdout", "files")}
        return {
            "head": head,
            "reproduce_identical": same,
            "fix_identical": fix_same,
            "fix_notices": fix["stderr"][-400:] if fix["stderr"] != o["stderr"] else "",
            "seconds": {
                "official": o["seconds"],
                "reproduce": rep["seconds"],
                "fix": fix["seconds"],
            },
            "peak_mib": {
                "official": o["peak_mib"],
                "reproduce": rep["peak_mib"],
                "fix": fix["peak_mib"],
            },
            "served": o["served"],
            "lta_sha256": o["files"].get("reg.lta"),
            "first_difference": None
            if all(same.values())
            else {
                "official": o["stdout"][-1500:],
                "reproduce": rep["stdout"][-1500:],
                "official_err": o["stderr"][-600:],
                "reproduce_err": rep["stderr"][-600:],
                "files": [o["files"], rep["files"]],
            },
        }

    with concurrent.futures.ThreadPoolExecutor(a.parallel) as pool:
        rows = list(pool.map(one, heads))
    summary = {
        "heads": len(rows),
        "reproduce_identical": sum(all(r["reproduce_identical"].values()) for r in rows),
        "fix_identical": sum(all(r["fix_identical"].values()) for r in rows),
        "rows": rows,
    }
    print(json.dumps(summary, indent=1))
    if a.json:
        a.json.write_text(json.dumps(summary, indent=1) + "\n", encoding="utf-8")
    return 0 if summary["reproduce_identical"] == len(rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
