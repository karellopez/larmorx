# SPDX-License-Identifier: Apache-2.0
"""Black-box runs of FSL 6.0.7.23's mcflirt, larmorx's clean-room head-motion correction and the
FSL replica program on the input and output rules mcflirt 6.0.7.23 changed (`specs/mcflirt.md`
§4.1, §9.3, §10.1): the zstd and bzip2 output types, reading by content, names completed without
an extension, the FSL-60 renaming of names holding `.img`, an unknown FSLOUTPUTTYPE, and the
FSL-59 final interpolation. Nothing here reads FSL's source or the replica's: the programs are run
and their exit status, messages and files printed.

    python -P fsl_io_probe.py <case> <work> [--who mcflirt,lx,nc] [--mode fix|reproduce]

Cases: `types` (every FSLOUTPUTTYPE: the files written, their first bytes, the decompressed
header fields and values against the NIFTI_GZ run), `read` (zstd, bzip2 and gzip files under
other names as `-in`), `names` (a name without extension: one match, two matches, none),
`unknown` (an unknown FSLOUTPUTTYPE), `fsl60` (names holding `.img`), `final` (`-sinc_final`
and `-nn_final`: the series against the default run's), `all`.

Programs: mcflirt in `~/fsl-6.0.7.23/bin` (`$FSL_623`), larmorx in `$LX_BIN` (default
`<repo>/target/release/larmorx`), the replica in `$LARMORX_NC_BIN`. The input is the synthetic
series of the recorded runs (`larmorx-testdata` `synthetic/hmc/motion/small.nii.gz`).
"""

from __future__ import annotations

import bz2
import gzip
import hashlib
import os
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[3]
WORKSPACE = Path(os.environ.get("LARMORX_WORKSPACE", REPO.parent))
FSL = Path(os.environ.get("FSL_623", Path.home() / "fsl-6.0.7.23"))
LX = Path(os.environ.get("LX_BIN", REPO / "target" / "release" / "larmorx"))
NC = Path(
    os.environ.get("LARMORX_NC_BIN", REPO / "crates-nc" / "target" / "release" / "larmorx-nc")
)
SERIES = WORKSPACE / "larmorx-testdata" / "data" / "synthetic" / "hmc" / "motion" / "small.nii.gz"

TYPES = [
    "NIFTI_GZ",
    "NIFTI",
    "NIFTI_PAIR",
    "NIFTI_PAIR_GZ",
    "NIFTI2",
    "NIFTI2_GZ",
    "NIFTI2_PAIR",
    "NIFTI2_PAIR_GZ",
    "NIFTI_ZST",
    "NIFTI_PAIR_ZST",
    "NIFTI2_ZST",
    "NIFTI2_PAIR_ZST",
    "NIFTI_BZ2",
    "NIFTI_PAIR_BZ2",
    "NIFTI2_BZ2",
    "NIFTI2_PAIR_BZ2",
]


GREP: str | None = None


def run(who: str, args: list[str], cwd: Path, otype: str | None, mode: str) -> tuple[int, str]:
    env = {"HOME": str(Path.home()), "PATH": "/usr/bin:/bin"}
    if otype is not None:
        env["FSLOUTPUTTYPE"] = otype
    if who == "mcflirt":
        env["FSLDIR"] = str(FSL)
        argv = [str(FSL / "bin" / "mcflirt"), *args]
    elif who == "lx":
        env["LARMORX_UPSTREAM_DEFECTS"] = mode
        env["OMP_NUM_THREADS"] = "4"
        argv = [str(LX), "mri", "hmc", *args]
    else:
        env["LARMORX_UPSTREAM_DEFECTS"] = mode
        env["FSLDIR"] = str(FSL)
        argv = [str(NC), "fsl", "mcflirt", *args]
    cwd.mkdir(parents=True, exist_ok=True)
    p = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, text=True, timeout=600)
    text = (p.stdout + p.stderr).strip()
    if GREP is not None:
        text = "\n".join(line for line in text.splitlines() if GREP in line)
    return p.returncode, text


def decompress(path: Path) -> bytes:
    raw = path.read_bytes()
    if raw[:2] == b"\x1f\x8b":
        return gzip.decompress(raw)
    if raw[:3] == b"BZh":
        return bz2.decompress(raw)
    if raw[:4] == b"\x28\xb5\x2f\xfd":
        out = subprocess.run(
            [str(FSL / "bin" / "zstd"), "-d", "-c", str(path)], capture_output=True, check=True
        )
        return out.stdout
    return raw


def kind(path: Path) -> str:
    head = path.read_bytes()[:4]
    if head[:2] == b"\x1f\x8b":
        return "gzip"
    if head[:3] == b"BZh":
        return "bzip2"
    if head == b"\x28\xb5\x2f\xfd":
        return "zstd"
    return "plain"


def digest(files: list[Path]) -> str:
    """The header's dimensions, datatype and magic, and a digest of the voxel values."""
    import nibabel as nib

    parts = []
    for f in files:
        if f.name.endswith((".img", ".img.gz", ".img.zst", ".img.bz2")):
            continue
        body = decompress(f)
        tmp = f.parent / (
            "_probe_"
            + f.name.split(".")[0]
            + (".nii" if body[344:347] in (b"n+1", b"n+2") or body[4:7] == b"n+2" else ".hdr")
        )
        tmp.write_bytes(body)
        if tmp.suffix == ".hdr":
            data_name = f.name.replace(".hdr", ".img")
            tmp.with_suffix(".img").write_bytes(decompress(f.parent / data_name))
        img = nib.load(tmp)
        x = np.asarray(img.dataobj)
        h = img.header
        parts.append(
            f"{f.name}: dim {tuple(int(v) for v in h['dim'][:5])} dtype {x.dtype} "
            f"values {hashlib.sha256(np.ascontiguousarray(x).tobytes()).hexdigest()[:12]} "
            f"cal {float(h['cal_min']):g}/{float(h['cal_max']):g}"
        )
        tmp.unlink()
        if tmp.with_suffix(".img").exists():
            tmp.with_suffix(".img").unlink()
    return "; ".join(parts)


def listing(d: Path) -> str:
    out = []
    for p in sorted(d.rglob("*")):
        if p.is_file():
            out.append(f"{p.relative_to(d)} [{kind(p)}]")
    return ", ".join(out)


def case_types(work: Path, who: list[str], mode: str) -> None:
    for w in who:
        for t in TYPES:
            d = work / "types" / w / t
            if d.exists():
                shutil.rmtree(d)
            code, msg = run(w, ["-in", str(SERIES), "-out", "out", "-plots"], d, t, mode)
            imgs = sorted(
                p for p in d.iterdir() if p.name.startswith("out") and ".par" not in p.name
            )
            print(f"{w:8s} {t:16s} exit {code} files: {listing(d)}")
            if code == 0:
                print(f"{'':26s}{digest(imgs)}")
            if msg:
                print(f"{'':26s}messages: {msg[:300]!r}")


def case_read(work: Path, who: list[str], mode: str) -> None:
    src = work / "read" / "src"
    if src.exists():
        shutil.rmtree(src)
    src.mkdir(parents=True)
    raw = gzip.decompress(SERIES.read_bytes())
    (src / "plain.nii").write_bytes(raw)
    (src / "bz2-named.nii").write_bytes(bz2.compress(raw, 9))
    (src / "bz2-named.nii.gz").write_bytes(bz2.compress(raw, 9))
    (src / "bz2.nii.bz2").write_bytes(bz2.compress(raw, 9))
    zst = subprocess.run(
        [str(FSL / "bin" / "zstd"), "-q", "-c", str(src / "plain.nii")],
        capture_output=True,
        check=True,
    ).stdout
    (src / "zst.nii.zst").write_bytes(zst)
    (src / "zst-named.nii.gz").write_bytes(zst)
    (src / "zst-named.nii").write_bytes(zst)
    (src / "gz-named.nii").write_bytes(SERIES.read_bytes())
    (src / "gz-named.nii.zst").write_bytes(SERIES.read_bytes())
    (src / "plain-named.nii.bz2").write_bytes(raw)
    for w in who:
        for f in sorted(src.iterdir()):
            d = work / "read" / w / f.name
            if d.exists():
                shutil.rmtree(d)
            code, msg = run(w, ["-in", str(f), "-out", "out", "-plots"], d, "NIFTI_GZ", mode)
            par = (d / "out.par").read_text() if (d / "out.par").exists() else ""
            print(
                f"{w:8s} {f.name:22s} exit {code} par {hashlib.sha256(par.encode()).hexdigest()[:12]}"
                f" {msg[:200]!r}"
            )


def case_names(work: Path, who: list[str], mode: str) -> None:
    raw = gzip.decompress(SERIES.read_bytes())
    for w in who:
        for label, files in (
            ("one-zst", {"in.nii.zst": "zst"}),
            ("one-bz2", {"in.hdr.bz2": "pair-bz2"}),
            ("gz-and-zst", {"in.nii.gz": "gz", "in.nii.zst": "zst"}),
            ("none", {}),
            ("nii-and-niigz", {"in.nii.gz": "gz", "in.nii": "plain"}),
        ):
            d = work / "names" / w / label
            if d.exists():
                shutil.rmtree(d)
            d.mkdir(parents=True)
            for name, how in files.items():
                if how == "gz":
                    (d / name).write_bytes(SERIES.read_bytes())
                elif how == "plain":
                    (d / name).write_bytes(raw)
                elif how == "zst":
                    (d / name).write_bytes(
                        subprocess.run(
                            [str(FSL / "bin" / "zstd"), "-q", "-c", "-"],
                            input=raw,
                            capture_output=True,
                            check=True,
                        ).stdout
                    )
                elif how == "pair-bz2":
                    # The header (348 bytes, magic ni1) and the data file, both bzip2.
                    hdr = bytearray(raw[:348])
                    hdr[344:348] = b"ni1\x00"
                    hdr[108:112] = np.float32(0).tobytes()
                    (d / name).write_bytes(bz2.compress(bytes(hdr)))
                    (d / "in.img.bz2").write_bytes(bz2.compress(raw[352:]))
            code, msg = run(w, ["-in", "in", "-out", "out", "-plots"], d, "NIFTI_GZ", mode)
            print(f"{w:8s} {label:14s} exit {code} files: {listing(d)} {msg[:300]!r}")


def case_unknown(work: Path, who: list[str], mode: str) -> None:
    for w in who:
        for t in ("NIFTI_XZ", "nifti_gz", ""):
            d = work / "unknown" / w / (t or "empty")
            if d.exists():
                shutil.rmtree(d)
            code, msg = run(w, ["-in", str(SERIES), "-out", "out", "-mats", "-plots"], d, t, mode)
            print(f"{w:8s} {t!r:12s} exit {code} files: {listing(d)}\n{'':22s}{msg[:600]!r}")


def case_fsl60(work: Path, who: list[str], mode: str) -> None:
    for w in who:
        base = work / "fsl60" / w
        if base.exists():
            shutil.rmtree(base)
        # -in in a directory holding .img: the renamed directory exists or not.
        for label, dirs, args in (
            ("in-img-dir-both", ["a.img.d", "a.hdr.d"], ["-in", "a.img.d/in.nii.gz", "-out", "o1"]),
            ("in-img-dir-only", ["a.img.d"], ["-in", "a.img.d/in.nii.gz", "-out", "o2"]),
            ("in-img-stem", [], ["-in", "sub.img2.nii.gz", "-out", "o3"]),
            (
                "out-img-dir-both",
                ["b.img.d", "b.hdr.d"],
                ["-in", "in.nii.gz", "-out", "b.img.d/out"],
            ),
            (
                "out-img-dir-only",
                ["b.img.d"],
                ["-in", "in.nii.gz", "-out", "b.img.d/out", "-plots"],
            ),
            ("out-img-stem", [], ["-in", "in.nii.gz", "-out", "o.img7"]),
            ("in-img-ext", [], ["-in", "pair.img", "-out", "o4"]),
            (
                "out-pair-img-dir-both",
                ["c.img.d", "c.hdr.d"],
                ["-in", "in.nii.gz", "-out", "c.img.d/out", "-stats"],
            ),
            ("out-pair-img-dir-only", ["c.img.d"], ["-in", "in.nii.gz", "-out", "c.img.d/out"]),
            ("in-pair-img-dir", ["p.img.d", "p.hdr.d"], ["-in", "p.img.d/pair.img", "-out", "o5"]),
            (
                "out-mats-img-dir",
                ["m.img.d", "m.hdr.d"],
                ["-in", "in.nii.gz", "-out", "m.img.d/out", "-mats"],
            ),
        ):
            otype = (
                "NIFTI_PAIR" if "pair-img-dir" in label and label.startswith("out") else "NIFTI_GZ"
            )
            d = base / label
            d.mkdir(parents=True)
            for sub in dirs:
                (d / sub).mkdir()
            (d / "in.nii.gz").write_bytes(SERIES.read_bytes())
            (d / "sub.img2.nii.gz").write_bytes(SERIES.read_bytes())
            (d / "sub.hdr2.nii.gz").write_bytes(SERIES.read_bytes())
            for sub in dirs:
                (d / sub / "in.nii.gz").write_bytes(SERIES.read_bytes())
            raw = gzip.decompress(SERIES.read_bytes())
            hdr = bytearray(raw[:348])
            hdr[344:348] = b"ni1\x00"
            hdr[108:112] = np.float32(0).tobytes()
            for where in [d, *(d / sub for sub in dirs)]:
                (where / "pair.hdr").write_bytes(bytes(hdr))
                (where / "pair.img").write_bytes(raw[352:])
            before = {p for p in d.rglob("*")}
            code, msg = run(w, args, d, otype, mode)
            new = sorted(
                str(p.relative_to(d)) for p in d.rglob("*") if p not in before and p.is_file()
            )
            print(f"{w:8s} {label:18s} exit {code} new: {new} {msg[:300]!r}")


def case_final(work: Path, who: list[str], mode: str) -> None:
    import nibabel as nib

    for w in who:
        ref = None
        for opt in ("", "-sinc_final", "-nn_final", "-spline_final"):
            d = work / "final" / w / (opt.strip("-") or "default")
            if d.exists():
                shutil.rmtree(d)
            args = ["-in", str(SERIES), "-out", "out", "-plots", "-rmsabs"] + ([opt] if opt else [])
            code, msg = run(w, args, d, "NIFTI_GZ", mode)
            x = np.asarray(nib.load(d / "out.nii.gz").dataobj) if code == 0 else None
            if ref is None:
                ref = x
            same = x is not None and ref is not None and np.array_equal(x, ref)
            print(
                f"{w:8s} {opt or 'default':14s} exit {code} series == default: {same} {msg[:300]!r}"
            )


def case_outext(work: Path, who: list[str], mode: str) -> None:
    """`-out` names with the new extensions (dropped before FSLOUTPUTTYPE's), and a `-reffile`
    name without an extension (completed)."""
    raw = gzip.decompress(SERIES.read_bytes())
    for w in who:
        for label, args in (
            ("out-nii-zst", ["-in", "in.nii.gz", "-out", "o1.nii.zst"]),
            ("out-hdr-bz2", ["-in", "in.nii.gz", "-out", "o2.hdr.bz2"]),
            ("out-img-gz", ["-in", "in.nii.gz", "-out", "o3.img.gz"]),
            ("reffile-noext", ["-in", "in.nii.gz", "-out", "o4", "-reffile", "ref"]),
        ):
            d = work / "outext" / w / label
            if d.exists():
                shutil.rmtree(d)
            d.mkdir(parents=True)
            (d / "in.nii.gz").write_bytes(SERIES.read_bytes())
            # The reference: the series' first volume, zstd, named .nii.zst.
            hdr = bytearray(raw[:352])
            hdr[40:42] = np.int16(3).tobytes()
            hdr[48:50] = np.int16(1).tobytes()
            nvox = int(np.prod(np.frombuffer(bytes(raw[42:48]), "<i2")))
            bpv = int(np.frombuffer(bytes(raw[72:74]), "<i2")[0]) // 8
            vol = raw[352 : 352 + nvox * bpv]
            zst = subprocess.run(
                [str(FSL / "bin" / "zstd"), "-q", "-c", "-"],
                input=bytes(hdr) + vol,
                capture_output=True,
                check=True,
            ).stdout
            (d / "ref.nii.zst").write_bytes(zst)
            before = {p for p in d.rglob("*")}
            code, msg = run(w, args, d, "NIFTI_GZ", mode)
            new = sorted(str(p.relative_to(d)) for p in d.rglob("*") if p not in before)
            print(f"{w:8s} {label:14s} exit {code} new: {new} {msg[:200]!r}")


CASES = {
    "types": case_types,
    "read": case_read,
    "names": case_names,
    "unknown": case_unknown,
    "fsl60": case_fsl60,
    "final": case_final,
    "outext": case_outext,
}


def main(argv: list[str]) -> int:
    who, mode = ["mcflirt", "lx"], "reproduce"
    if "--who" in argv:
        i = argv.index("--who")
        who = argv[i + 1].split(",")
        del argv[i : i + 2]
    if "--mode" in argv:
        i = argv.index("--mode")
        mode = argv[i + 1]
        del argv[i : i + 2]
    if "--grep" in argv:
        global GREP
        i = argv.index("--grep")
        GREP = argv[i + 1]
        del argv[i : i + 2]
    case, work = argv[0], Path(argv[1]).resolve()
    for name in CASES if case == "all" else [case]:
        print(f"== {name} ({mode})")
        CASES[name](work, who, mode)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
