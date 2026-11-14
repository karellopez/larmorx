# SPDX-License-Identifier: Apache-2.0
"""The NIfTI-1 pair writer's long names (FS-451), against FreeSurfer 8.2.0's official code.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names FreeSurfer's functions; running fsio is a
black-box run of FreeSurfer's code).

Nine ``mgw`` cases (piece 14, ``MRIwriteType`` as ``NIFTI1_FILE``) of one small volume written
under names of 1,019 to 1,027 bytes, in four nested folders of 200-byte names (each name within the
file system's 255 bytes): names without an extension of 1,019 to 1,023 bytes (the header's and the
image's names cut to 1,023 bytes from 1,020: both the same file from 1,022), ``.img`` names of
1,023, 1,024 and 1,027 bytes and a ``.hdr`` name of 1,024 bytes (FreeSurfer aborts in the C
library's overflow check). Runs ``fsio`` through the oracle-output cache and the port's example
``fs_io_probe`` in both modes, and prints each case's comparison. Linux only (the recorded test's
cases stay below the 260-character paths Windows refuses).

    python -P validation/probes/fs-io/nifti1_long_names.py
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import check  # noqa: E402
from check import oracle_cache, workspace  # noqa: E402

# a generated piece-14 case (seed 7): a 4 x 1 x 1 float volume, its setup and name replaced below
TEMPLATE = (
    "mgw 0 - 1 2 4 1 1 4 1 1 3f800000 3f800000 40200000 bf800000 00000000 3f000000 00000000 "
    "00000000 3f800000 00000000 bf800000 00000000 c0600000 bf800000 3f000000 2 41f00000 44610000 "
    "80000000 80000000 7ff8000000000003 3fc00000 -3 282d46f954a3b809 N N 0 0 34000000 -1 -1 -1 "
    "-1 -1 0 -1 -1 -1 x 0"
)


def cases() -> list[str]:
    d = ["w/" + "a" * 200]
    for c in "bcd":
        d.append(d[-1] + "/" + c * 200)
    spec = ",".join("D" + x.encode().hex() for x in d)
    top = d[-1] + "/"
    names = [top + "x" * (n - len(top)) for n in (1019, 1020, 1021, 1022, 1023)]
    names += [top + "y" * (n - len(top) - 4) + ".img" for n in (1023, 1024, 1027)]
    names.append(top + "z" * (1024 - len(top) - 4) + ".hdr")
    out = []
    for name in names:
        t = TEMPLATE.split()
        t[2] = spec
        t[-2] = "x" + name.encode().hex()
        out.append(" ".join(t))
    return out


def main() -> int:
    ws = workspace()
    if ws is None:
        raise SystemExit("no workspace with oracles/")
    exe = check.probe_binary(ws)
    port = check.port_binary(6)
    path = ws / "oracles" / "inputs" / "fs-io" / "nifti1-long-names.txt"
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = cases()
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C", "LANG": "C", "OMP_NUM_THREADS": "1"}
    done = oracle_cache.run(
        [exe, path],
        env=env,
        policy=oracle_cache.Policy(threads=1, family="freesurfer"),
        timeout=600,
        runner="fs-io-probe",
    )
    official = done.stdout.splitlines()
    bad = 0
    for mode in ("reproduce", "fix"):
        out = subprocess.run(
            [str(port), "run", str(path), mode], check=True, capture_output=True, text=True
        ).stdout.splitlines()
        for case, o, p in zip(lines, official, out, strict=True):
            body, _, ids = p.partition("\t")
            same = check.matches(o, body)
            name = bytes.fromhex(case.split()[-2][1:])
            print(f"{mode:9} {len(name):5} bytes {'identical' if same else 'differs'} {ids}")
            # reproduce gives FreeSurfer's line; fix differs only with FS-451 noted
            if not same and (mode == "reproduce" or "FS-451" not in ids):
                bad += 1
    print("FAIL" if bad else "PASS", bad)
    return 1 if bad else 0


if __name__ == "__main__":
    raise SystemExit(main())
