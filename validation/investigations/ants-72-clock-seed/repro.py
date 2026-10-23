# SPDX-License-Identifier: Apache-2.0
"""Investigation (task 3): does 'ANTs with the B-spline transform as a displacement field'
vary from run to run? The construction of ants-registration-corrections' chain cases, repeated
in identical conditions: the field written by antsApplyTransforms, then antsRegistration with
-r <field> and a linear stage, at one thread with ANTS_RANDOM_SEED=42."""

import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path

from larmorx_validation.parity import (
    ants_inputs,
    ants_registration,  # noqa: F401 (its builders)
)
from larmorx_validation.parity.ants_registration_bspline import transform_file

B = (
    Path(os.environ.get("LARMORX_WORKSPACE", Path(__file__).resolve().parents[4]))
    / "oracles/ants-2.6.5/bin"
)
out = Path(sys.argv[1])
reps = int(sys.argv[2]) if len(sys.argv) > 2 else 5
out.mkdir(parents=True, exist_ok=True)
env = {k: v for k, v in os.environ.items() if k not in ("ANTS_RANDOM_SEED", "LD_PRELOAD")}
env["ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS"] = "1"
env["ANTS_RANDOM_SEED"] = "42"
if os.environ.get("FIXCLOCK"):  # the shim, built from fixclock.c
    env["LD_PRELOAD"] = os.environ["FIXCLOCK"]
F = ants_inputs.path("reg-fixed")
M = ants_inputs.path("reg-moving")
K = ants_inputs.path("reg-mask")
bsp = transform_file(out, "bspline-3d.txt")


def md5(p: Path) -> str:
    return hashlib.md5(p.read_bytes()).hexdigest()[:12] if p.exists() else "missing"


# 1. The field, several times.
fields = []
for r in range(reps):
    fld = out / f"field-{r}.nii.gz"
    subprocess.run(
        [
            str(B / "antsApplyTransforms"),
            "-d",
            "3",
            "-i",
            str(M),
            "-r",
            str(M),
            "-t",
            str(bsp),
            "-o",
            f"[{fld},1]",
        ],
        env=env,
        check=True,
        capture_output=True,
    )
    fields.append(fld)
print("fields:", [md5(f) for f in fields])
# 2. The registration with the first field, several times, two linear stages.
for t in ("Affine[0.1]", "Rigid[0.1]"):
    results = []
    for r in range(reps):
        d = out / f"reg-{t[:5]}-{r}"
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir()
        p = subprocess.run(
            [
                str(B / "antsRegistration"),
                "-d",
                "3",
                "-v",
                "1",
                "-o",
                str(d / "r"),
                "-r",
                str(fields[0]),
                "-x",
                str(K),
                "-t",
                t,
                "-m",
                f"MeanSquares[{F},{M},1]",
                "-c",
                "[100,1e-8,10]",
                "-f",
                "1",
                "-s",
                "0vox",
            ],
            env=env,
            capture_output=True,
            text=True,
        )
        files = sorted(x.name for x in d.iterdir())
        results.append((p.returncode, [md5(d / f) for f in files if f.endswith(".mat")], files))
        (d / "stdout.txt").write_text(p.stdout)
    print(t, [r[:2] for r in results], results[0][2])
