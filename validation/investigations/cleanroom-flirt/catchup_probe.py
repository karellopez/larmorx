# SPDX-License-Identifier: Apache-2.0
"""catchup_probe.py <probes.json> <outdir> [name-regex]: black-box probes of FLIRT's other
programs for the clean-room catch-up (2026-11-05). Each probe is {"name", "program", "args",
"files": {name: text}, "images": {name: {"shape", "voxel", "intent"}}, "crash": bool}; images
are seeded random NIfTI-1 float images written by this script; ``@name`` arguments are the
replica suite's inputs
(``fsl_flirt_replica._prepare``). FSL runs through the oracle cache (use mode; a probe that
crashes FSL is never cached), the original in ``reproduce`` and in its default; prints each
run's exit status and whether its streams and files are FSL's, and saves them under
<outdir>/<name>/."""

import json
import re
import sys
from pathlib import Path

import nibabel as nib
import numpy as np
from larmorx_validation import oracle_cache
from larmorx_validation.parity import defects
from larmorx_validation.parity import fsl_flirt as orig
from larmorx_validation.parity import fsl_flirt_replica as rep

probes = json.loads(Path(sys.argv[1]).read_text())
root = Path(sys.argv[2])
pat = re.compile(sys.argv[3]) if len(sys.argv) > 3 else None


def write_images(d: Path, images: dict) -> None:
    for k, (name, spec) in enumerate(sorted(images.items())):
        rng = np.random.default_rng(20261105 + k)
        data = rng.integers(0, 100, size=spec["shape"]).astype(np.float32)
        aff = np.diag([*spec.get("voxel", [2.0, 2.0, 2.0]), 1.0])
        im = nib.Nifti1Image(data, aff)
        im.header.set_xyzt_units("mm", "sec")
        if spec.get("intent"):
            im.header.set_intent(spec["intent"])
        nib.save(im, str(d / name))


def files_of(d: Path, before: set[str]) -> dict[str, bytes]:
    return {p.name: p.read_bytes() for p in sorted(d.iterdir()) if p.name not in before}


for pr in probes:
    if pat and not pat.search(pr["name"]):
        continue
    run = rep.Run(
        pr["program"],
        tuple(pr["args"]),
        files=tuple(pr.get("files", {}).items()),
    )
    out = root / pr["name"]
    d = out / "fsl"
    d.mkdir(parents=True, exist_ok=True)
    args = rep._prepare(run, d)
    write_images(d, pr.get("images", {}))
    before = {p.name for p in d.iterdir()}
    env = rep._environment({})
    policy = oracle_cache.Policy(threads=1, family="fsl")
    if pr.get("crash"):
        policy = policy.but_never("FSL crashes on this probe (undefined behaviour)")
    p = oracle_cache.run(
        [str(rep.fsldir() / "bin" / run.program), *args],
        env=env,
        cwd=d,
        outputs=[d],
        policy=policy,
        runner="catchup-probe",
        inputs=[x for x in sorted(d.rglob("*")) if x.is_file()],
        timeout=600,
    )
    theirs = files_of(d, before)
    (out / "fsl.out").write_text(p.stdout)
    (out / "fsl.err").write_text(p.stderr)
    line = [f"{pr['name']}: FSL {p.returncode}"]
    for mode in (defects.REPRODUCE, defects.FIX):
        m = out / mode
        m.mkdir(parents=True, exist_ok=True)
        rep._prepare(run, m)
        write_images(m, pr.get("images", {}))
        before = {x.name for x in m.iterdir()}
        r = orig.run_original(run.program, args, cwd=m, settings={}, mode=mode)
        mine = files_of(m, before)
        (out / f"{mode}.out").write_text(r.stdout)
        (out / f"{mode}.err").write_text(r.stderr)
        so = r.stdout == p.stdout
        se = orig._own(r.stderr) == p.stderr
        same_files = set(mine) == set(theirs) and all(mine[k] == theirs[k] for k in mine)
        named = ",".join(defects.notices_in(r.stderr))
        line.append(
            f"{mode} {r.code} out {'=' if so else 'X'} err {'=' if se else 'X'} "
            f"files {'=' if same_files else 'X'}" + (f" [{named}]" if named else "")
        )
    print(" | ".join(line))
