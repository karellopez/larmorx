# SPDX-License-Identifier: Apache-2.0
"""Black-box runs of a random-testing recipe through AFNI 25.2.09, larmorx's clean-room original
and the GPL replica program, side by side, as the clean-room implementer of the 2026-10-29 fixes
used them. Nothing here reads AFNI's source or the replica's: the programs are run, and their
exit status, messages, header fields and values printed.

    python -P probe.py show <finding.json> [--drawn]          # the recipe's inputs and arguments
    python -P probe.py run <finding.json> <work> [--drawn] [--mode fix|reproduce] [--brief]
                       [--who afni,lx,gpl] [--env A=B]...

`--brief` prints, instead of each program's standard error, its differences from the first
program's (AFNI's, unless `--who` puts another first).

`run` writes the recipe's inputs into <work>/in (the random-testing generators), runs each program
in <work>/<who>-<mode>, and prints each one's exit status, standard error (AFNI's banner left out)
and, for every NIfTI file written, its header fields and a digest of its values; then the fields
where the programs differ. 3dTshift's recipes get `-prefix out.nii <input>` after their options,
as its parity runner gives them.

Programs: AFNI in $AFNI_BIN (default <workspace>/oracles/afni-25.2.09/bin), larmorx in $LX_BIN
(default <repo>/target/release/larmorx, LARMORX_IMPLEMENTATION=original), the replica in
$LARMORX_GPL_BIN (default <repo>/crates-gpl/target/release/larmorx-gpl). Run with
`PYTHONPATH=<repo>/python:<repo>/validation/src` and $LARMORX_WORKSPACE set.
"""

from __future__ import annotations

import difflib
import gzip
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
AFNI = Path(os.environ.get("AFNI_BIN", WORKSPACE / "oracles/afni-25.2.09/bin"))
LX = os.environ.get("LX_BIN", str(REPO / "target/release/larmorx"))
GPL = os.environ.get("LARMORX_GPL_BIN", str(REPO / "crates-gpl/target/release/larmorx-gpl"))
PROGRAMS = {"afni-tshift": "3dTshift", "afni-automask": "3dAutomask", "afni-unifize": "3dUnifize"}
BANNER = re.compile(r"^(?:[^\n]*::)?(?:\x1b\[7m)?\+\+ 3d\w+: AFNI version=[^\n]*\n", re.M)


def recipe_of(path: str, drawn: bool) -> dict:
    j = json.loads(Path(path).read_text())
    if "shrunk" in j:
        return j["recipe"] if drawn else (j["shrunk"] or j["recipe"])
    return j.get("recipe", j)


def build(recipe: dict, work: Path) -> list[str]:
    from larmorx_validation.difftest import gen

    inp = work / "in"
    inp.mkdir(parents=True, exist_ok=True)
    paths = {}
    for name, spec in recipe["inputs"].items():
        target = inp / (name + spec.get("suffix", ".nii"))
        if spec.get("kind", "nifti") == "nifti":
            gen.write_nifti(spec, target)
        else:
            target.write_text(spec.get("text", ""), encoding="utf-8")
        paths[name] = str(target)
    words = [a for g in recipe["groups"] for a in g["args"]]
    words = [re.sub(r"\{in:(\w+)\}", lambda m: paths[m.group(1)], w) for w in words]
    if recipe["tool"] == "afni-tshift":
        words = [*words, "-prefix", "out.nii", paths["series"]]
    return words


def header(path: Path) -> dict:
    import nibabel as nib

    img = nib.load(str(path))
    h = img.header
    out = {}
    for k in h.keys():
        v = h[k]
        out[k] = v.tolist() if hasattr(v, "tolist") else v
    data = np.asanyarray(img.dataobj.get_unscaled())
    raw = np.ascontiguousarray(data)
    out["_values"] = hashlib.sha256(raw.tobytes()).hexdigest()[:16]
    out["_signbits"] = int(np.count_nonzero(np.signbit(raw))) if raw.dtype.kind == "f" else None
    out["_shape"] = list(data.shape)
    rawb = path.read_bytes()
    if rawb[:2] == b"\x1f\x8b":
        rawb = gzip.decompress(rawb)
    out["_header_sha"] = hashlib.sha256(rawb[:348]).hexdigest()[:16]
    return out


def run_one(who: str, recipe: dict, words: list[str], work: Path, mode: str, env_extra: dict):
    prog = PROGRAMS[recipe["tool"]]
    d = work / f"{who}-{mode}"
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    home = work / "home"
    home.mkdir(exist_ok=True)
    env = {
        "HOME": str(home),
        "PATH": "/usr/bin:/bin",
        "AFNI_DONT_LOGFILE": "YES",
        "AFNI_NIFTI_NOEXT": "YES",
        "OMP_NUM_THREADS": "1",
        "LARMORX_UPSTREAM_DEFECTS": mode,
    }
    env.update(recipe.get("extra", {}).get("env", {}) or {})
    env.update(env_extra)
    if who == "afni":
        cmd = [str(AFNI / prog)]
    elif who == "lx":
        cmd = [LX, "afni", prog]
        env["LARMORX_IMPLEMENTATION"] = "original"
    else:
        cmd = [GPL, "afni", prog]
    p = subprocess.run([*cmd, *words], cwd=d, env=env, capture_output=True)
    err = BANNER.sub("", p.stderr.decode("utf-8", "replace"))
    files = {}
    for f in sorted(d.iterdir()):
        if f.name.endswith((".nii", ".nii.gz")):
            try:
                files[f.name] = header(f)
            except Exception as e:  # noqa: BLE001 - a probe: show what failed
                files[f.name] = {"_error": str(e)}
    return {"exit": p.returncode, "stdout": p.stdout.decode("utf-8", "replace"), "stderr": err,
            "files": files}  # fmt: skip


def main(argv: list[str]) -> int:
    if not argv or argv[0] not in ("show", "run"):
        print(__doc__, file=sys.stderr)
        return 2
    drawn = "--drawn" in argv
    brief = "--brief" in argv
    argv = [a for a in argv if a not in ("--drawn", "--brief")]
    env_extra, mode, whos = {}, "fix", ["afni", "lx", "gpl"]
    rest = []
    i = 1
    while i < len(argv):
        if argv[i] == "--env":
            k, v = argv[i + 1].split("=", 1)
            env_extra[k] = v
            i += 2
        elif argv[i] == "--mode":
            mode = argv[i + 1]
            i += 2
        elif argv[i] == "--who":
            whos = argv[i + 1].split(",")
            i += 2
        else:
            rest.append(argv[i])
            i += 1
    recipe = recipe_of(rest[0], drawn)
    if argv[0] == "show":
        print(json.dumps({k: v for k, v in recipe.items() if k != "features"}, indent=1))
        return 0
    work = Path(rest[1]).resolve()
    words = build(recipe, work)
    print("args:", " ".join(words))
    results = {who: run_one(who, recipe, words, work, mode, env_extra) for who in whos}
    if brief:
        first = whos[0]
        for who, r in results.items():
            print(f"===== {who} ({mode}): exit {r['exit']}")
            if who == first:
                continue
            diff = difflib.unified_diff(
                results[first]["stderr"].splitlines(), r["stderr"].splitlines(),
                first, who, n=0, lineterm="")  # fmt: skip
            for k, line in enumerate(diff):
                if k >= 2 and not line.startswith("@@"):
                    print("   ", line)
    for who, r in [] if brief else results.items():
        print(f"===== {who} ({mode}): exit {r['exit']}")
        if r["stdout"].strip():
            print("--- stdout ---\n" + r["stdout"].rstrip())
        print("--- stderr ---\n" + r["stderr"].rstrip())
    names = sorted({n for r in results.values() for n in r["files"]})
    for n in names:
        print(f"===== file {n}")
        keys = sorted({k for r in results.values() for k in r["files"].get(n, {})})
        for k in keys:
            vals = {who: r["files"].get(n, {}).get(k, "<absent>") for who, r in results.items()}
            reps = {json.dumps(v, default=str) for v in vals.values()}
            if len(reps) > 1:
                print(f"  DIFF {k}: " + " | ".join(f"{w}={json.dumps(v, default=str)}"
                                                   for w, v in vals.items()))  # fmt: skip
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
