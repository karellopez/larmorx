# SPDX-License-Identifier: Apache-2.0
"""How AFNI 25.2.09's 3dTshift turns a header's slice timing into slice times, to the bit (item 2
of the 2026-10-29 fixes: `toffset` 0.36000001 in AFNI, 0.35999995 in larmorx, for a header in
milliseconds). `-slice k` makes `tzero` slice k's time, and the output's `toffset` (float32,
seconds) shows it exactly, so every slice time is read back; `toffset` without `-slice` shows the
mean. Each candidate rule is checked against every time. Black-box only.

    python -P tshift_times.py <work>
"""

import os
import struct
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import nibabel as nib
import numpy as np

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
AFNI = Path(os.environ.get("AFNI_BIN", WORKSPACE / "oracles/afni-25.2.09/bin"))
f32 = np.float32


# NIfTI's slice orders: the position of each slice (start..end) in acquisition order.
def order(code: int, n: int) -> list[int]:
    idx = list(range(n))
    if code == 1:  # seq inc
        acq = idx
    elif code == 2:  # seq dec
        acq = idx[::-1]
    elif code == 3:  # alt inc
        acq = idx[0::2] + idx[1::2]
    elif code == 4:  # alt dec
        acq = idx[::-1][0::2] + idx[::-1][1::2]
    elif code == 5:  # alt inc 2
        acq = idx[1::2] + idx[0::2]
    else:  # 6, alt dec 2
        acq = idx[::-1][1::2] + idx[::-1][0::2]
    pos = [0] * n
    for p, s in enumerate(acq):
        pos[s] = p
    return pos


def candidates(code, nz, start, end, dur, per_second):
    pos = order(code, end - start + 1)
    d_s = f32(np.float64(dur) / per_second)  # the duration in seconds, float32
    out = {}

    def full(times):
        t = [f32(0)] * nz
        for i, v in enumerate(times):
            t[start + i] = v
        return t

    # accumulate in float32 seconds, in acquisition order
    acc = {}
    x = f32(0)
    for p in range(len(pos)):
        acc[p] = x
        x = f32(x + d_s)
    out["accumulate seconds"] = full([acc[p] for p in pos])
    out["index x duration (float32 s)"] = full([f32(f32(p) * d_s) for p in pos])
    out["index x duration (double, s)"] = full([f32(p * np.float64(d_s)) for p in pos])
    d_u = f32(dur)
    acc = {}
    x = f32(0)
    for p in range(len(pos)):
        acc[p] = x
        x = f32(x + d_u)
    out["accumulate units, then /"] = full([f32(np.float64(acc[p]) / per_second) for p in pos])
    out["index x duration units (float32), then /"] = full(
        [f32(np.float64(f32(f32(p) * d_u)) / per_second) for p in pos]
    )
    # A float32 running sum of a double step: the duration divided (or multiplied by the
    # reciprocal) in double, never rounded to float32.
    for label, step in (
        ("float sum of double step (dur / units)", np.float64(f32(dur)) / per_second),
        ("float sum of double step (dur * 1/units)", np.float64(f32(dur)) * (1.0 / per_second)),
    ):
        acc = {}
        x = f32(0)
        for p in range(len(pos)):
            acc[p] = x
            x = f32(np.float64(x) + step)
        out[label] = full([acc[p] for p in pos])
    return out


def run(work: Path, name: str, inp: Path, args: list[str]) -> float:
    d = work / name
    d.mkdir(parents=True, exist_ok=True)
    out = d / "o.nii"
    if out.exists():
        out.unlink()
    env = {"HOME": str(work), "PATH": "/usr/bin:/bin", "AFNI_DONT_LOGFILE": "YES",
           "AFNI_NIFTI_NOEXT": "YES"}  # fmt: skip
    subprocess.run([str(AFNI / "3dTshift"), *args, "-prefix", "o.nii", str(inp)], cwd=d,
                   env=env, capture_output=True)  # fmt: skip
    return struct.unpack("<f", out.read_bytes()[136:140])[0] if out.exists() else float("nan")


def main(work: Path) -> None:
    work.mkdir(parents=True, exist_ok=True)
    cases = []
    rng = np.random.default_rng(7)
    for t in range(16):
        units = ["msec", "sec", "usec"][t % 3]
        per_second = {"msec": 1e3, "sec": 1.0, "usec": 1e6}[units]
        nz = int(rng.integers(5, 14))
        start = int(rng.integers(0, 2))
        end = nz - 1 - int(rng.integers(0, 2))
        code = int(rng.choice([1, 3, 4, 5, 6]))
        tr_s = float(rng.choice([0.8, 1.0, 2.0, 2.5]))
        dur_s = float(np.round(tr_s / (end - start + 1) * rng.uniform(0.7, 1.0), 4))
        cases.append(
            (units, per_second, nz, start, end, code, tr_s * per_second, dur_s * per_second)
        )
    rows = []
    for c, (units, per_second, nz, start, end, code, tr, dur) in enumerate(cases):
        h = nib.Nifti1Header()
        h.set_data_shape((2, 2, nz, 6))
        h.set_data_dtype(np.float32)
        h.set_xyzt_units("mm", units)
        h["pixdim"][1:5] = [2, 2, 2, tr]
        h["dim_info"] = 48
        h["slice_code"], h["slice_start"], h["slice_end"] = code, start, end
        h["slice_duration"] = dur
        h["qform_code"] = 1
        data = np.random.default_rng(c).normal(100, 5, (2, 2, nz, 6)).astype(np.float32)
        p = work / f"in{c}.nii"
        nib.save(nib.Nifti1Image(data, None, h), p)

        def one(k, p=p, c=c):
            return run(work, f"c{c}-k{k}", p, ["-slice", str(k)])

        with ThreadPoolExecutor(8) as ex:
            got = [f32(v) for v in ex.map(one, range(nz))]
        mean = f32(run(work, f"c{c}-mean", p, []))
        cand = candidates(code, nz, start, end, f32(dur), per_second)
        hits = {k: v == got for k, v in cand.items()}
        means = {}
        for k, v in cand.items():
            s = f32(0)
            for x in v:
                s = f32(s + x)
            means[k] = f32(s / f32(nz)) == mean
        rows.append((units, code, nz, start, end, dur, hits, means))
        print(
            units, "code", code, "nz", nz, "range", start, end, "dur", dur, "AFNI mean", repr(mean)
        )
        for k in cand:
            print(
                f"   {k:44s} times {'yes' if hits[k] else 'no ':3s} mean {'yes' if means[k] else 'no'}"
            )
        if not hits["accumulate seconds"]:
            print("   AFNI      ", [float(v) for v in got])
            print("   accumulate", [float(v) for v in cand["accumulate seconds"]])


if __name__ == "__main__":
    main(Path(sys.argv[1]).resolve())
