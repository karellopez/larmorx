# SPDX-License-Identifier: Apache-2.0
"""The NaN matrix: inputs with NaN and infinite voxels (``nan_inputs.py``) x mcflirt's options x
both D13 modes, each run by FSL's mcflirt (reproduce only), a release and a debug build of the
replica, and both builds again with AArch64's default NaN emulated (``libnan_trap.so``,
``NAN_TRAP_ARM=1``: after each SSE add, subtract, multiply, divide or square root that made a
NaN, the destination's default NaN gets the sign AArch64 gives it). Every output file is
compared bit for bit: in ``reproduce`` with mcflirt's, in ``fix`` with the release build's.

    python -P nan_matrix.py <work dir> --release <larmorx-nc> --debug <larmorx-nc>
        [--probe <dir with libnan_trap.so>] [--only <substring>]

FSL 6.0.7.17 at ``~/fsl`` (never on the PATH). On 2026-10-25 (after the crate::sse work), the
release and debug builds and both with ARM's NaN agreed in all 160 cases; in reproduce they
gave mcflirt's bytes except where NaN voxels lie in the 2-voxel border (``bgnan``,
``bgnegnan``: FSL's ``calc_bval`` sorts them with ``std::sort``, whose order with NaNs the
replica does not reproduce; ``docs/findings/fsl-mcflirt.md``).
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent

INPUTS = {
    "golden": [],
    "negnan": ["--nan-bits", "ffc00000"],
    "payload": ["--nan-bits", "7fc12345", "--neg-inf"],
    "bgnan": ["--background-nan", "7fc00000", "--neg-inf"],
    "bgnegnan": ["--background-nan", "ffc00000"],
}
OPTIONS = {
    "default": ["-mats", "-plots", "-rmsabs", "-rmsrel"],
    "sinc": ["-mats", "-plots", "-sinc_final"],
    "spline": ["-mats", "-plots", "-spline_final"],
    "nn": ["-mats", "-nn_final"],
    "stats": ["-mats", "-stats", "-meanvol"],
    "corratio": ["-mats", "-plots", "-cost", "corratio"],
    "mutualinfo": ["-mats", "-plots", "-cost", "mutualinfo"],
    "normmi": ["-mats", "-plots", "-cost", "normmi"],
    "leastsquares": ["-mats", "-plots", "-cost", "leastsquares"],
    "woods": ["-mats", "-plots", "-cost", "woods"],
    "stages4": ["-mats", "-plots", "-stages", "4"],
    "fudge": ["-mats", "-plots", "-fudge"],
    "dof12": ["-mats", "-plots", "-dof", "12"],
    "verbose": ["-mats", "-plots", "-verbose", "20", "-stages", "2"],
    "gdt": ["-mats", "-plots", "-gdt"],
    "report": ["-mats", "-plots", "-report"],
}


def outputs(d: Path) -> dict[str, bytes]:
    """Every file a run wrote (image data without the header: descrip differs by design),
    its standard error without the replica's own lines, and its exit status."""
    out = {}
    for p in sorted(d.rglob("*")):
        rel = p.relative_to(d).as_posix()
        if p.is_file() and not rel.startswith(("trap/", "stderr")):
            data = p.read_bytes()
            out[rel] = data[352:] if p.name.endswith(".nii") else data
    err = (d / "stderr").read_text(errors="replace").splitlines(keepends=True)
    out["<stderr>"] = "".join(x for x in err if not x.startswith("larmorx-nc:")).encode()
    return out


def main() -> int:
    argv = sys.argv[1:]

    def opt(name: str, default: str | None = None) -> str | None:
        return argv[argv.index(name) + 1] if name in argv else default

    work = Path(argv[0]).resolve()
    probe = Path(opt("--probe", str(HERE)) or HERE) / "libnan_trap.so"
    builds = {"rel": opt("--release"), "dbg": opt("--debug")}
    only = opt("--only", "") or ""
    fsl = Path.home() / "fsl"
    base = {"PATH": "/usr/bin:/bin", "FSLOUTPUTTYPE": "NIFTI", "HOME": os.environ["HOME"]}
    work.mkdir(parents=True, exist_ok=True)
    bad, n = [], 0
    for iname, iargs in INPUTS.items():
        inp = work / f"{iname}.nii"
        if not inp.exists():
            subprocess.run(
                [sys.executable, "-P", str(HERE / "nan_inputs.py"), str(inp), *iargs], check=True
            )
        for oname, oargs in OPTIONS.items():
            for mode in ("reproduce", "fix"):
                name = f"{iname}-{oname}-{mode}"
                if only not in name:
                    continue
                runs: dict[str, tuple[list[str], dict[str, str]]] = {}
                if mode == "reproduce":
                    runs["fsl"] = ([str(fsl / "bin/mcflirt")], dict(base, FSLDIR=str(fsl)))
                for b, path in builds.items():
                    if not path:
                        continue
                    env = dict(base, LARMORX_UPSTREAM_DEFECTS=mode, OMP_NUM_THREADS="2")
                    runs[b] = ([path, "fsl", "mcflirt"], env)
                    runs[b + "+arm"] = (
                        [path, "fsl", "mcflirt"],
                        dict(env, LD_PRELOAD=str(probe), NAN_TRAP_ARM="1"),
                    )
                got = {}
                for label, (cmd, env) in runs.items():
                    d = work / name / label
                    if d.exists():
                        shutil.rmtree(d)
                    (d / "trap").mkdir(parents=True)
                    env = dict(env, NAN_TRAP_DIR=str(d / "trap")) if "+arm" in label else env
                    with open(d / "stderr", "w", encoding="utf-8") as e:
                        p = subprocess.run(
                            [*cmd, "-in", str(inp), "-out", "out", *oargs],
                            cwd=d,
                            env=env,
                            stdout=subprocess.DEVNULL,
                            stderr=e,
                            check=False,
                        )
                    got[label] = outputs(d)
                    got[label]["<exit>"] = str(p.returncode).encode()
                ref = "fsl" if mode == "reproduce" else "rel"
                diffs = []
                for label, o in got.items():
                    if label != ref:
                        k = [
                            f for f in sorted(set(o) | set(got[ref])) if o.get(f) != got[ref].get(f)
                        ]
                        if k:
                            diffs.append(f"{label} vs {ref}: {', '.join(k[:6])}")
                n += 1
                print(f"{name}: {'ok' if not diffs else 'DIFFER'}", flush=True)
                for line in diffs:
                    print("    ", line, flush=True)
                if diffs:
                    bad.append(name)
    print(f"{n} cases; {len(bad)} differ: {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
