# SPDX-License-Identifier: Apache-2.0
"""The NaN matrix of the BET replicas: heads with NaN and infinite voxels x bet2's, bet's and
betsurf's options x both D13 modes, each run by FSL (reproduce only), by a release and a debug
build of ``larmorx-nc``, and by the release build again (``--trap-debug``: both) with AArch64's
default NaN emulated
(``libnan_trap.so`` from ``validation/probes/fsl-mcflirt-affine-trace/nan_trap.c``,
``NAN_TRAP_ARM=1``: after each SSE operation that made a NaN, the destination's default NaN gets
the sign AArch64 gives it). Every output file is compared bit for bit (images decompressed):
in ``reproduce`` with FSL's where FSL finishes, and in both modes with the release build's.

    python -P nan_matrix.py <work dir> --release <larmorx-nc> --debug <larmorx-nc>
        [--trap <libnan_trap.so> [--trap-debug]] [--only <substring>]

FSL 6.0.7.17 at ``~/fsl`` (never on the PATH); the inputs are the parity suite's synthetic
head (``larmorx_validation.parity.fsl_bet``) with NaN or infinite voxels written in.
"""

from __future__ import annotations

import gzip
import os
import shutil
import subprocess
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation.parity import fsl_bet as suite

#: The head's grid: the suite's head at half its size (a 4 mm grid's worth of anatomy on 2 mm
#: voxels), so that the debug builds run in seconds.
SHAPE = (48, 56, 42)
#: (name, [(x, y, z, float32 bits)]): voxels set in the head.
NAN = 0x7FC00000
NEGNAN = 0xFFC00000
PAYLOAD = 0x7FC12345
INF = 0x7F800000
NEGINF = 0xFF800000
INPUTS = {
    "brain-nan": [(24, 28, 21, NAN), (26, 30, 19, NAN)],
    "brain-negnan": [(24, 28, 21, NEGNAN)],
    "brain-payload-neginf": [(24, 28, 21, PAYLOAD), (22, 26, 23, NEGINF)],
    "brain-inf": [(24, 28, 21, INF), (26, 27, 20, NEGINF)],
    "border-nan": [(0, 0, 0, NAN), (47, 1, 3, NEGNAN), (10, 55, 41, NAN)],
    "first-voxel-nan": [(0, 0, 0, NAN)],
}
#: (name, tool, arguments after the input, where {in}, {mesh}, {mat} are the inputs).
OPTIONS = [
    ("bet2-m", "bet2", ["{in}", "out", "-m"]),
    ("bet2-all", "bet2", ["{in}", "out", "-m", "-s", "-o", "-e"]),
    ("bet2-t-g", "bet2", ["{in}", "out", "-t", "-f", "0.3", "-g", "0.2"]),
    ("bet-R", "bet", ["{in}", "out", "-R", "-m"]),
    ("betsurf-t1only", "betsurf", ["--t1only", "-m", "-s", "{in}", "{mesh}", "{mat}", "out"]),
]


def write_inputs(work: Path) -> dict[str, Path]:
    head, _ = suite._head(suite.T1_LEVELS, 1, shape=SHAPE)
    out = {}
    for name, voxels in INPUTS.items():
        data = np.array(head, dtype=np.float32)
        bits = data.view(np.uint32)
        for x, y, z, b in voxels:
            bits[x, y, z] = b
        img = nib.Nifti1Image(data, suite.AFFINE)
        img.set_qform(suite.AFFINE, 1)
        img.set_sform(suite.AFFINE, 1)
        p = work / f"{name}.nii"
        nib.save(img, p)
        out[name] = p
    return out


def fsl_mesh(fsl: Path, inp: Path, work: Path) -> Path | None:
    """FSL's bet2 surface of the clean head (betsurf's start: the same for every input)."""
    d = work / "mesh"
    d.mkdir(exist_ok=True)
    mesh = d / "head_mesh.vtk"
    if not mesh.exists():
        head, _ = suite._head(suite.T1_LEVELS, 1, shape=SHAPE)
        img = nib.Nifti1Image(np.asarray(head, dtype=np.float32), suite.AFFINE)
        img.set_qform(suite.AFFINE, 1)
        img.set_sform(suite.AFFINE, 1)
        nib.save(img, d / "head.nii")
        env = {"PATH": "/usr/bin:/bin", "FSLOUTPUTTYPE": "NIFTI", "FSLDIR": str(fsl)}
        subprocess.run(
            [str(fsl / "bin/bet2"), str(d / "head.nii"), str(d / "head"), "-e", "-n"],
            cwd=d,
            env=env,
            check=True,
            capture_output=True,
        )
    return mesh


def outputs(d: Path) -> dict[str, bytes]:
    """Every file a run wrote (images decompressed; the header's descrip blanked: the
    replica's is FSL's build string, so it is compared too), its streams without the replica's
    own lines, and its exit status."""
    out = {}
    for p in sorted(d.rglob("*")):
        rel = p.relative_to(d).as_posix()
        if p.is_file() and not rel.startswith(("trap/", "stderr", "stdout")):
            data = p.read_bytes()
            if data[:2] == b"\x1f\x8b":
                data = gzip.decompress(data)
            out[rel] = data
    for stream in ("stdout", "stderr"):
        lines = (d / stream).read_text(errors="replace").splitlines(keepends=True)
        out[f"<{stream}>"] = "".join(
            x
            for x in lines
            if not x.startswith(("larmorx-nc:", "larmorx:"))
            and not x.rstrip().endswith("(larmorx)")
        ).encode()
    return out


def crashed(code: int, err: str) -> bool:
    return code < 0 or code in (134, 139) or "terminate called" in err or "Segmentation" in err


def main() -> int:
    argv = sys.argv[1:]

    def opt(name: str, default: str | None = None) -> str | None:
        return argv[argv.index(name) + 1] if name in argv else default

    work = Path(argv[0]).resolve()
    work.mkdir(parents=True, exist_ok=True)
    trap = opt("--trap")
    builds = {"rel": opt("--release"), "dbg": opt("--debug")}
    only = opt("--only", "") or ""
    # The trap single-steps every SSE instruction that raises the invalid exception: a debug
    # build under it takes hours on surfaces of NaN; the release build's emulation suffices for
    # the bits (the debug build adds the integer-overflow checks).
    trap_debug = "--trap-debug" in argv
    fsl = Path(os.environ.get("LARMORX_FSL_DIR", Path.home() / "fsl"))
    base = {"PATH": "/usr/bin:/bin", "FSLOUTPUTTYPE": "NIFTI", "HOME": os.environ["HOME"]}
    inputs = write_inputs(work)
    mesh = fsl_mesh(fsl, inputs["brain-nan"], work)
    mat = work / "head_to_std.mat"
    mat.write_text("1 0 0 -8\n0 1 0 4\n0 0 1 -12\n0 0 0 1\n", encoding="utf-8")
    bad, n, fsl_crashes, timeouts = [], 0, [], []
    for iname, inp in inputs.items():
        for oname, tool, oargs in OPTIONS:
            args = [a.format(**{"in": str(inp), "mesh": str(mesh), "mat": str(mat)}) for a in oargs]
            for mode in ("reproduce", "fix"):
                name = f"{iname}-{oname}-{mode}"
                if only not in name:
                    continue
                runs: dict[str, tuple[list[str], dict[str, str]]] = {}
                if mode == "reproduce":
                    runs["fsl"] = ([str(fsl / "bin" / tool)], dict(base, FSLDIR=str(fsl)))
                for b, path in builds.items():
                    if not path:
                        continue
                    env = dict(base, LARMORX_UPSTREAM_DEFECTS=mode, OMP_NUM_THREADS="2")
                    runs[b] = ([path, "fsl", tool], env)
                    if trap and (b == "rel" or trap_debug):
                        runs[b + "+arm"] = (
                            [path, "fsl", tool],
                            dict(env, LD_PRELOAD=trap, NAN_TRAP_ARM="1"),
                        )
                got, codes = {}, {}
                for label, (cmd, env) in runs.items():
                    d = work / name / label
                    if d.exists():
                        shutil.rmtree(d)
                    (d / "trap").mkdir(parents=True)
                    env = dict(env, NAN_TRAP_DIR=str(d / "trap")) if "+arm" in label else env
                    with (
                        open(d / "stderr", "w", encoding="utf-8") as e,
                        open(d / "stdout", "w", encoding="utf-8") as o,
                    ):
                        try:
                            p = subprocess.run(
                                [*cmd, *args],
                                cwd=d,
                                env=env,
                                stdout=o,
                                stderr=e,
                                check=False,
                                timeout=900 if "+arm" in label else 3600,
                            )
                            code = p.returncode
                        except subprocess.TimeoutExpired:
                            code = -999
                            timeouts.append(f"{name}/{label}")
                    got[label] = outputs(d)
                    got[label]["<exit>"] = str(code).encode()
                    codes[label] = (code, (d / "stderr").read_text(errors="replace"))
                diffs = []
                refs = ["rel"]
                if mode == "reproduce" and not crashed(*codes["fsl"]):
                    refs.insert(0, "fsl")
                elif mode == "reproduce":
                    fsl_crashes.append(name)
                for label, o in got.items():
                    if f"{name}/{label}" in timeouts:
                        continue  # not compared: the trap's single steps took too long
                    for ref in refs:
                        if label == ref or label == "fsl":
                            continue
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
    print(
        f"{n} cases; FSL crashed in {len(fsl_crashes)} (compared among the replica's builds only)"
    )
    print(f"{len(timeouts)} runs stopped after their time limit (not compared): {timeouts}")
    print(f"{len(bad)} differ: {bad}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
