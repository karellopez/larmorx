# SPDX-License-Identifier: Apache-2.0
"""The unit oracle against the official FreeSurfer 8.2.0 binaries: three proofs.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: it runs FreeSurfer's code built from its source and
names its internal functions.

1. ``code``: the machine code of the three programs built from the unit oracle's objects
   (``mri_add_xform_to_header``, ``mri_normalize``, ``mris_smooth``) against the official ones,
   instruction by instruction (``code_identity.py``).
2. ``replay``: those programs on the recorded steps of the reference runs, in place of
   FreeSurfer's (``larmorx_validation.freesurfer.replay`` with a command), their files compared
   with the recorded ones: the queue's check of item 3.1-02.
3. ``units``: the harness's functions against the official programs that call them, on the
   same inputs: ``MRIread`` + ``MRIwrite`` against ``mri_convert`` (MGZ and NIfTI, recorded
   volumes); ``MatrixInverse`` against ``lta_convert --invert`` (whose LTA prints the float
   matrix exactly, ``%18.15le``); ``OpenRan1`` (``vnl_random``'s ``drand64``) against the values
   the official ``mri_normalize`` and ``mris_smooth`` draw, read from their registers under
   ``gdb`` at the function's returns (the official programs are stripped: the addresses come from
   the code alignment of 1.).

    python -P prove.py code|replay|units|all [--json <out.json>] [--work <scratch>]

The official programs run through the oracle-output cache (one thread, FreeSurfer's family of
variables); the replays of the unit oracle's programs are not cached (LARMORX's side of a
comparison). Run it in a memory box (``boxed.sh 14``).
"""

from __future__ import annotations

import argparse
import json
import os
import random
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(REPO / "validation" / "src"))

import code_identity  # noqa: E402
from build import PROGRAMS, workspace  # noqa: E402

WS = workspace()
UNIT_BIN = WS / "oracles" / "freesurfer-8.2.0-unit" / "bin"
FS_HOME = WS / "oracles" / "freesurfer-8.2.0" / "root" / "usr" / "local" / "freesurfer" / "8.2.0"
RUNS = WS / "oracles" / "freesurfer-8.2.0" / "runs"

#: The reference runs replayed: a 1-thread classic run (the replay at 1 thread) and two v8 runs
#: at their recorded 4 threads (an adult at 3 T, a 3.5-year-old).
REPLAY_RUNS = (
    ("ds000005-sub-01-classic-1thread", 1),
    ("ds000005-sub-01-v8", None),
    ("ds000228-sub-pixar008-v8", None),
)
#: Recorded volumes for the reader and writer: conformed uchar, the raw average (its own type and
#: geometry), float intensities, labels.
#: The draws read from a program under gdb at most (the rest of its run goes on without stops).
GDB_HITS = 2000
VOLUMES = ("mri/rawavg.mgz", "mri/orig.mgz", "mri/nu.mgz", "mri/T1.mgz", "mri/aseg.presurf.mgz")


def _env(extra: dict[str, str] | None = None) -> dict[str, str]:
    env = {
        "PATH": f"{FS_HOME / 'bin'}:/usr/bin:/bin",
        "FREESURFER_HOME": str(FS_HOME),
        "OMP_NUM_THREADS": "1",
        "LC_ALL": "C",
        "LANG": "C",
        "HOME": os.environ.get("HOME", "/tmp"),
    }
    env.update(extra or {})
    return env


def official(program: str, args: list[str], *, outputs=(), cwd: Path | None = None):
    """FreeSurfer's own ``program`` through the oracle-output cache (its launcher)."""
    from larmorx_validation import oracle_cache
    from larmorx_validation.freesurfer.package import launcher

    prog = launcher(program, FS_HOME)
    policy = oracle_cache.Policy(threads=1, family="freesurfer", formats="freesurfer")
    return oracle_cache.run(
        [str(prog), *args],
        env=_env(),
        cwd=cwd,
        outputs=list(outputs),
        policy=policy,
        runner="freesurfer-unit-proof",
        timeout=900,
    )


# ---------------------------------------------------------------- 1. code
def prove_code() -> dict:
    out = {}
    for prog in PROGRAMS:
        r = code_identity.compare(UNIT_BIN / prog, FS_HOME / "bin" / prog)
        crt = {"__do_global_dtors_aux", "deregister_tm_clones", "frame_dummy",
               "register_tm_clones", "__do_global_ctors_aux"}  # fmt: skip
        own = [d for d in r["differences"] if not set(d["functions"]) <= crt]
        out[prog] = {
            "instructions": r["instructions"],
            "identical_in_order": r["identical_in_order"],
            "differences_in_gcc_start_files": len(r["differences"]) - len(own),
            "differences_elsewhere": own,
            "sections": r["sections"],
        }
        n_ours, n_off = r["instructions"]
        print(
            f"code {prog}: {r['identical_in_order']} of {n_off} official instructions identical in "
            f"order (ours {n_ours}); {len(own)} differing stretches outside GCC's start files"
        )
        for d in own:
            print(
                f"  {d['tag']} in {', '.join(d['functions'][:3])}: {d['ours_text'][:2]} / {d['official_text'][:2]}"
            )
    return out


# ---------------------------------------------------------------- 2. replay
def prove_replay(work: Path) -> dict:
    from larmorx_validation.freesurfer import replay
    from larmorx_validation.freesurfer.recording import Recording

    results = []
    for run, threads in REPLAY_RUNS:
        root = RUNS / run
        if not (root / "record").is_dir():
            print(f"replay: {run} not recorded here, skipped")
            continue
        rec = Recording(root)
        for step in rec.steps:
            if step.program not in PROGRAMS or step.final:
                continue
            dest = work / "replay" / run / step.label
            report, _rb = replay.replay(
                rec,
                step,
                threads=threads,
                dest=dest,
                command=[str(UNIT_BIN / "{program}")],
                timeout=1800,
            )
            d = report.to_dict()
            results.append(d)
            print(f"replay {run} {step.label}: {d['verdict']} {d['counts']}")
            shutil.rmtree(dest, ignore_errors=True)  # a rebuilt copy of the recording
    return {
        "steps": results,
        "reproduced": sum(r["verdict"].startswith("reproduced") for r in results),
    }


# ---------------------------------------------------------------- 3. units
def _recorded_volume(run: str, rel: str) -> Path | None:
    from larmorx_validation.freesurfer.recording import Recording

    rec = Recording(RUNS / run)
    last = rec.steps[-1]
    state = last.after() if not last.final else last.before
    sha = state.get(rel)
    return rec.blob(sha) if sha else None


def prove_volumes(work: Path) -> list[dict]:
    from larmorx_validation import freesurfer_unit as fu
    from larmorx_validation.freesurfer import files as fsfiles

    rows = []
    d = work / "volumes"
    d.mkdir(parents=True, exist_ok=True)
    for rel in VOLUMES:
        blob = _recorded_volume("ds000005-sub-01-v8", rel)
        if blob is None:
            print(f"units volume {rel}: not recorded, skipped")
            continue
        src = d / Path(rel).name
        shutil.copyfile(blob, src)
        for suffix in (".mgz", ".nii.gz"):
            ours = d / f"{src.stem}.unit{suffix}"
            theirs = d / f"{src.stem}.official{suffix}"
            for p in (ours, theirs):
                p.unlink(missing_ok=True)
            header = fu.mri_read_write(src, ours)
            done = official("mri_convert", [str(src), str(theirs)])
            cmp = fsfiles.compare(theirs, ours) if theirs.exists() and ours.exists() else None
            row = {
                "volume": rel,
                "format": suffix,
                "official_exit": done.returncode,
                "verdict": cmp.verdict if cmp else "missing",
                "detail": cmp.detail if cmp else "",
                "bytes_identical": theirs.exists() and ours.read_bytes() == theirs.read_bytes(),
                "type": header["type"],
                "dims": [header["width"], header["height"], header["depth"], header["nframes"]],
            }
            rows.append(row)
            print(
                f"units volume {rel} -> {suffix}: {row['verdict']} (bytes identical: {row['bytes_identical']})"
            )
    return rows


LTA = """# transform file (larmorx unit-oracle proof)
type      = 1 # LINEAR_RAS_TO_RAS
nxforms   = 1
mean      = 0.0000 0.0000 0.0000
sigma     = 1.0000
1 4 4
{rows}
src volume info
valid = 1  # volume info valid
filename = src.mgz
volume = 256 256 256
voxelsize = 1.000000000000000e+00 1.000000000000000e+00 1.000000000000000e+00
xras   = -1.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00
yras   = 0.000000000000000e+00 0.000000000000000e+00 -1.000000000000000e+00
zras   = 0.000000000000000e+00 1.000000000000000e+00 0.000000000000000e+00
cras   = 0.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00
dst volume info
valid = 1  # volume info valid
filename = dst.mgz
volume = 256 256 256
voxelsize = 1.000000000000000e+00 1.000000000000000e+00 1.000000000000000e+00
xras   = -1.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00
yras   = 0.000000000000000e+00 0.000000000000000e+00 -1.000000000000000e+00
zras   = 0.000000000000000e+00 1.000000000000000e+00 0.000000000000000e+00
cras   = 0.000000000000000e+00 0.000000000000000e+00 0.000000000000000e+00
"""


def _f32(x: float) -> float:
    return struct.unpack("<f", struct.pack("<f", x))[0]


def _matrices(n: int, seed: int = 20261112) -> list[list[list[float]]]:
    """Affine 4 x 4 matrices: rotations and scalings of registration size, shears, nearly
    singular ones, a diagonal one; float32 values (the LTA's matrix is float)."""
    rng = random.Random(seed)
    out = [[[1.0, 0, 0, 0], [0, 1.0, 0, 0], [0, 0, 1.0, 0], [0, 0, 0, 1.0]]]
    out.append([[0.9375, 0, 0, -127.5], [0, 1.0, 0, 128.25], [0, 0, 1.125, -96.0], [0, 0, 0, 1.0]])
    for k in range(n - 3):
        m = [
            [rng.uniform(-1.5, 1.5) for _ in range(3)] + [rng.uniform(-150, 150)] for _ in range(3)
        ]
        if k % 5 == 4:  # nearly singular: one row close to a combination of the others
            m[2][:3] = [m[0][j] + m[1][j] + rng.uniform(-1e-4, 1e-4) for j in range(3)]
        out.append([*m, [0.0, 0.0, 0.0, 1.0]])
    out.append(
        [[1.0, 2.0, 3.0, 0], [2.0, 4.0, 6.0, 0], [1.0, 1.0, 1.0, 0], [0, 0, 0, 1.0]]
    )  # singular
    return [[[_f32(v) for v in row] for row in m] for m in out]


def _read_lta_matrix(path: Path) -> list[list[float]] | None:
    lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    for i, ln in enumerate(lines):
        if ln.strip() == "1 4 4":
            return [[float(x) for x in lines[i + 1 + r].split()] for r in range(4)]
    return None


def prove_matrices(work: Path, n: int = 40) -> list[dict]:
    from larmorx_validation import freesurfer_unit as fu

    d = work / "matrices"
    d.mkdir(parents=True, exist_ok=True)
    rows = []
    for k, m in enumerate(_matrices(n)):
        src = d / f"m{k:03d}.lta"
        dst = d / f"m{k:03d}.inv.lta"
        dst.unlink(missing_ok=True)
        src.write_text(
            LTA.format(rows="\n".join(" ".join(f"{v:.9g}" for v in row) for row in m)),
            encoding="utf-8",
        )
        done = official("lta_convert", ["--inlta", str(src), "--outlta", str(dst), "--invert"])
        theirs = _read_lta_matrix(dst) if dst.exists() else None
        ours = fu.matrix_inverse(m)
        same = (
            theirs is not None
            and ours is not None
            and all(ours[r][c] == theirs[r][c] for r in range(4) for c in range(4))
        )
        rows.append(
            {
                "matrix": k,
                "official_exit": done.returncode,
                "official_inverted": theirs is not None,
                "unit_inverted": ours is not None,
                "identical": same or (theirs is None and ours is None),
            }
        )
    ok = sum(r["identical"] for r in rows)
    print(f"units MatrixInverse: {ok} of {len(rows)} identical to lta_convert --invert")
    return rows


def _gdb_returns(
    program: Path, args: list[str], cwd: Path, env: dict, addresses: list[int]
) -> list[str]:
    """Run ``program`` under gdb; at each address (OpenRan1's returns, relative to the program's
    load address) print xmm0's low float's bits; the values in the order they were returned."""
    script = ["set pagination off", "set confirm off", "starti", "python", "import gdb, re"]
    script += [
        "maps = gdb.execute('info proc mappings', to_string=True)",
        f"base = min(int(m, 16) for m in re.findall(r'^\\s*(0x[0-9a-f]+)\\s.*{re.escape(program.name)}\\s*$', maps, re.M))",
        f"bps = [gdb.Breakpoint('*' + hex(base + a)) for a in {addresses!r}]",
        "hits = [0]",
        "def on_stop(event):",
        "    if isinstance(event, gdb.BreakpointEvent):",
        "        v = int(gdb.parse_and_eval('$xmm0.v4_int32[0]')) & 0xffffffff",
        "        print('OPENRAN1 %08x' % v)",
        "        hits[0] += 1",
        f"        if hits[0] >= {GDB_HITS}:",
        "            print('OPENRAN1-LIMIT')",
        "            for b in bps:",
        "                b.enabled = False",
        "gdb.events.stop.connect(on_stop)",
        "end",
    ]
    script += ["while 1", "continue", "end"]
    with tempfile.NamedTemporaryFile("w", suffix=".gdb", delete=False) as f:
        f.write("\n".join(script) + "\n")
        gdbfile = f.name
    try:
        gdb = shutil.which("gdb") or "gdb"
        p = subprocess.run(
            [gdb, "-nx", "-batch", "-x", gdbfile, "--args", str(program), *args],
            cwd=cwd,
            env=env,
            capture_output=True,
            text=True,
            timeout=3600,
            check=False,
        )
    finally:
        os.unlink(gdbfile)
    return re.findall(r"OPENRAN1 ([0-9a-f]{8})", p.stdout)


def _function_returns(binary: Path, *names: str) -> list[int]:
    """The addresses of ``ret`` in the function (any of ``names``) of a binary with symbols."""
    rows = code_identity.disassemble(binary)
    return [addr for fn, addr, ins in rows if fn in names and ins.split()[0].startswith("ret")]


def _official_addresses(prog: str, ours_addrs: list[int]) -> list[int]:
    """Our addresses mapped onto the stripped official program through the code alignment."""
    a = code_identity.disassemble(UNIT_BIN / prog)
    b = code_identity.disassemble(FS_HOME / "bin" / prog)
    ops = code_identity.align(
        [code_identity.norm(x[2]) for x in a], [code_identity.norm(x[2]) for x in b]
    )
    index = {addr: i for i, (_f, addr, _t) in enumerate(a)}
    out = []
    for addr in ours_addrs:
        i = index[addr]
        for tag, i1, i2, j1, _j2 in ops:
            if tag == "equal" and i1 <= i < i2:
                out.append(b[j1 + (i - i1)][1])
                break
        else:
            raise RuntimeError(f"{prog}: {hex(addr)} is not in an identical stretch")
    return out


def prove_random(work: Path) -> list[dict]:
    """OpenRan1's values in the official programs (gdb) against the harness's."""
    from larmorx_validation import freesurfer_unit as fu
    from larmorx_validation.freesurfer import replay
    from larmorx_validation.freesurfer.recording import Recording

    rows = []
    cases = (("ds000005-sub-01-v8", "mri_normalize"), ("ds000005-sub-01-v8", "mris_smooth"))
    for run, prog in cases:
        rec = Recording(RUNS / run)
        step = next(s for s in rec.steps if s.program == prog)
        seed = int(step.argv[step.argv.index("-seed") + 1])
        ours = _function_returns(UNIT_BIN / prog, "OpenRan1", "_Z8OpenRan1Pl")
        theirs = _official_addresses(prog, ours)
        # relative to the load address: both are PIE (the first mapping is the file's start)
        dest = work / "random" / run / step.label
        rb = replay.rebuild(rec, step, dest, threads=1)
        values = {}
        for label, binary, addrs in (
            ("official", FS_HOME / "bin" / prog, theirs),
            ("unit", UNIT_BIN / prog, ours),
        ):
            values[label] = _gdb_returns(binary, list(rb.argv[1:]), rb.cwd, dict(rb.env), addrs)
            replay.rebuild(rec, step, dest, threads=1)  # the run wrote into it: start again
        shutil.rmtree(dest, ignore_errors=True)
        n = len(values["official"])
        harness = fu.open_ran1(seed, max(n, 1))
        bits = [f"{struct.unpack('<I', struct.pack('<f', v))[0]:08x}" for v in harness]
        row = {
            "run": run,
            "step": step.label,
            "program": prog,
            "seed": seed,
            "draws": n,
            "official_equals_unit_program": values["official"] == values["unit"],
            "official_equals_harness": values["official"] == bits[:n],
            "first": values["official"][:5],
        }
        rows.append(row)
        print(
            f"units OpenRan1 {prog} -seed {seed}: {n} draws; official = unit program: "
            f"{row['official_equals_unit_program']}; official = harness: {row['official_equals_harness']}"
        )
    return rows


def prove_units(work: Path) -> dict:
    return {
        "volumes": prove_volumes(work),
        "matrices": prove_matrices(work),
        "random": prove_random(work),
    }


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("what", choices=("code", "replay", "units", "all"))
    ap.add_argument("--json", type=Path)
    ap.add_argument("--work", type=Path, default=Path(tempfile.gettempdir()) / "lx-fsunit-proof_")
    a = ap.parse_args(argv)
    a.work.mkdir(parents=True, exist_ok=True)
    out: dict = {}
    if a.what in ("code", "all"):
        out["code"] = prove_code()
    if a.what in ("units", "all"):
        out["units"] = prove_units(a.work)
    if a.what in ("replay", "all"):
        out["replay"] = prove_replay(a.work)
    if a.json:
        a.json.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
