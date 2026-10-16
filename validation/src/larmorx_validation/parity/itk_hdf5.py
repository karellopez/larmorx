# SPDX-License-Identifier: Apache-2.0
"""Parity of ITK's HDF5 transform files (``.h5``): larmorx's pure-Rust reader and writer
(crate ``larmorx-hdf5``; ``larmorx_io::itk_hdf5``; ``lx.transforms.read``/``write``;
``lx.transforms.load_itk_composite``) against ITK 5.4.5 on its HDF5 1.12.1 (the ANTs 2.6.5
oracle's ``antsApplyTransforms`` and ``CompositeTransformUtil``), h5py and nitransforms.

Three kinds of cases:

- ``read/...``: every HDF5 transform file at hand, written by ITK 4.11 to 5.4.5 on HDF5 1.8.17
  to 1.14.6 (the ANTs oracle, fMRIPrep 25.2.6's runs, the test-data catalog, TemplateFlow),
  and files written with h5py in the ways HDF5 allows (superblocks 0 and 3, compact,
  contiguous and chunked storage, deflate, shuffle, Fletcher-32, big-endian, half, single and
  extended precision, fixed-length strings, ITK 4's misspelt names, dense groups). Checks:
  every dataset as h5py reads it; the transforms as ITK reads them, which ANTs writes back
  with ``-o CompositeTransform[...]`` (in double, and with ``--float 1``); the chain
  nitransforms loads (fMRIPrep's reader). Files ITK refuses are compared through
  ``antsApplyTransforms``: exit status and the messages, addresses and build paths aside.
- ``write/...``: what ITK writes (``-o CompositeTransform[...]``, ``-o Linear[...]``,
  ``--float 1``, ``-o DisplacementField[...]``, ``CompositeTransformUtil --assemble``).
  larmorx writes the same transforms with the same object time and strings, and the bytes are
  compared: with ITK's own compressed chunks, every byte must be ITK's; with larmorx's deflate
  (zlib-rs, where ITK's HDF5 calls zlib-ng 2.1.6) the compressed streams may differ, and with
  them their sizes and the addresses after them. Then ANTs reads larmorx's file (the same
  output image as from ITK's file), and so do h5py and nitransforms.
- ``fix``: D13, larmorx's default reading and writing where ITK is defective.
"""

from __future__ import annotations

import contextlib
import io
import os
import re
import subprocess
import tempfile
import warnings
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

import larmorx_testdata as td
import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity.ants_programs import ants_bin, ants_version
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    SKIPPED,
    Case,
    CaseResult,
    CheckList,
    Outcome,
    Suite,
)

_TIERS = ("smoke", "standard", "full")
S = "synthetic/resampling"
T = f"{S}/transforms"
IMAGE = f"{S}/images/axial-1mm.nii.gz"
REFERENCE = f"{S}/references/oblique-1.5mm.nii.gz"
THREADS = "1"

#: larmorx writes what HDF5 1.12.1 writes for these, except as stated.
WRITE_DIVERGENCES = {
    "write/composite-38-affines": (
        "39 links do not fit the first block of TransformGroup's fractal heap: HDF5 adds an "
        "indirect block, which larmorx's writer does not cover; larmorx keeps the links as link "
        "messages (a group info storing 39 compact links), a layout HDF5 itself writes for "
        "such a group property list. Every reader reads the same transforms"
    ),
}


@dataclass(frozen=True)
class Spec:
    """A case's input: a file (catalog path, absolute path or a recipe) and what to check."""

    kind: str  # "read", "error", "write", "partial", "fix"
    source: str
    tier: str = "smoke"
    #: For "write": the arguments of the oracle program that writes the file ({out} is it).
    program: str = "antsApplyTransforms"
    args: tuple[str, ...] = ()
    #: For "write": the transforms the file holds, as -t arguments (to check larmorx's own
    #: reading of the inputs gives ITK's parts), in file order; empty: not checked.
    inputs: tuple[str, ...] = ()
    notes: str = field(default="", compare=False)


# ------------------------------------------------------------------------------------------------
# Files written with h5py (every way HDF5 can store what ITK reads)

AFFINE = np.array(
    [1.05, -0.08, -0.06, 0.07, 0.98, 0.02, 0.05, -0.03, 1.01, 1.5, -2.25, 3.125], dtype="f8"
)
CENTER = np.array([0.35, -1.2, 2.4], dtype="f8")


def _h5py_file(path: Path, recipe: str) -> None:
    """Writes the h5py-made file ``recipe`` at ``path``."""
    import h5py

    vlen = h5py.string_dtype("ascii")

    def write(groups, *, libver=None, top=True, string=vlen, **kw):
        with h5py.File(path, "w", libver=libver) as f:
            if top:
                for key, val in [
                    ("ITKVersion", "5.4.5"),
                    ("HDFVersion", "test"),
                    ("OSName", "Linux"),
                    ("OSVersion", "test"),
                ]:
                    f.create_dataset(key, data=np.array([val], dtype=object), dtype=vlen)
            tg = f.create_group("TransformGroup")
            for key, items in groups:
                g = tg.create_group(key)
                for dname, value in items.items():
                    if dname == "TransformType" and isinstance(value, str) and string is not vlen:
                        g.create_dataset(dname, data=np.array([value.encode()], dtype="S40"))
                    elif dname == "TransformType" and isinstance(value, str):
                        g.create_dataset(dname, data=np.array([value], dtype=object), dtype=string)
                    elif callable(value):
                        value(g, dname)
                    else:
                        k = dict(kw)
                        if k.get("chunks") is not None:
                            k["chunks"] = (min(k["chunks"][0], np.asarray(value).size),)
                        g.create_dataset(dname, data=value, **k)

    def affine(t="AffineTransform_double_3_3", params=AFFINE, fixed=CENTER, prefix="Transform"):
        return {
            "TransformType": t,
            f"{prefix}Parameters": params,
            f"{prefix}FixedParameters": fixed,
        }

    comp = ("0", {"TransformType": "CompositeTransform_double_3_3"})
    many = [comp] + [(str(i), affine(params=AFFINE + i)) for i in range(1, 12)]
    recipes: dict[str, Callable[[], None]] = {
        "single": lambda: write([("0", affine())]),
        "composite": lambda: write([comp, ("1", affine())]),
        "superblock3": lambda: write([comp, ("1", affine())], libver="latest"),
        "superblock3-chunked": lambda: write(
            [comp, ("1", affine())], libver="latest", chunks=(5,), compression="gzip"
        ),
        "shuffle-fletcher32": lambda: write(
            [comp, ("1", affine())], chunks=(5,), shuffle=True, fletcher32=True, compression="gzip"
        ),
        "misspelt-itk4": lambda: write([comp, ("1", affine(prefix="Tranform"))]),
        "float32": lambda: write([comp, ("1", affine(params=AFFINE.astype("f4")))]),
        "big-endian-f8": lambda: write(
            [comp, ("1", affine(params=AFFINE.astype(">f8"), fixed=CENTER.astype(">f8")))]
        ),
        "big-endian-f4": lambda: write([comp, ("1", affine(params=AFFINE.astype(">f4")))]),
        "half": lambda: write(
            [comp, ("1", affine(params=AFFINE.astype("f2"), fixed=CENTER.astype("f2")))]
        ),
        "long-double": lambda: write([comp, ("1", affine(params=AFFINE.astype(np.longdouble)))]),
        "no-version-strings": lambda: write([comp, ("1", affine())], top=False),
        "scalar-type": lambda: write(
            [
                comp,
                (
                    "1",
                    {
                        "TransformType": lambda g, n: g.create_dataset(
                            n, data="AffineTransform_double_3_3", dtype=vlen
                        ),
                        "TransformParameters": AFFINE,
                        "TransformFixedParameters": CENTER,
                    },
                ),
            ]
        ),
        "dense-11": lambda: write(many),
        "dense-11-superblock3": lambda: write(many, libver="latest"),
        "composite-second": lambda: write(
            [("0", affine()), ("1", {"TransformType": "CompositeTransform_double_3_3"})]
        ),
        "empty-composite": lambda: write([comp]),
        "two-linear": lambda: write([("0", affine()), ("1", affine(params=AFFINE * 2))]),
        "fixed-string": lambda: write(
            [comp, ("1", affine())], string=h5py.string_dtype("ascii", 40)
        ),
        "two-strings": lambda: write(
            [
                comp,
                (
                    "1",
                    {
                        "TransformType": lambda g, n: g.create_dataset(
                            n,
                            data=np.array(["AffineTransform_double_3_3", "x"], dtype=object),
                            dtype=vlen,
                        ),
                        "TransformParameters": AFFINE,
                        "TransformFixedParameters": CENTER,
                    },
                ),
            ]
        ),
        "int-parameters": lambda: write([comp, ("1", affine(params=np.arange(12, dtype="i4")))]),
        "2d-parameters": lambda: write([comp, ("1", affine(params=AFFINE.reshape(3, 4)))]),
        "2d-fixed": lambda: write([comp, ("1", affine(fixed=CENTER.reshape(1, 3)))]),
        "int-fixed": lambda: write([comp, ("1", affine(fixed=np.arange(3, dtype="i4")))]),
        "scalar-parameters": lambda: write(
            [
                comp,
                (
                    "1",
                    {
                        "TransformType": "TranslationTransform_double_3_3",
                        "TransformParameters": lambda g, n: g.create_dataset(
                            n, data=np.float64(1.5)
                        ),
                        "TransformFixedParameters": np.zeros(0),
                    },
                ),
            ]
        ),
        "unknown-type": lambda: write([comp, ("1", affine(t="FancyTransform_double_3_3"))]),
        "no-precision-in-name": lambda: write([comp, ("1", affine(t="AffineTransform"))]),
        "gap-in-names": lambda: write([comp, ("2", affine())]),
        "no-transform-group": lambda: write([]),
        "missing-parameters": lambda: write(
            [
                comp,
                (
                    "1",
                    {
                        "TransformType": "AffineTransform_double_3_3",
                        "TransformFixedParameters": CENTER,
                    },
                ),
            ]
        ),
        "lzf": lambda: write([comp, ("1", affine())], chunks=(6,), compression="lzf"),
    }
    if recipe == "compact":
        # A compact dataset needs the low-level API in h5py.
        with h5py.File(path, "w") as f:
            tg = f.create_group("TransformGroup")
            g = tg.create_group("0")
            g.create_dataset(
                "TransformType",
                data=np.array(["AffineTransform_double_3_3"], dtype=object),
                dtype=vlen,
            )
            for name, value in (
                ("TransformParameters", AFFINE),
                ("TransformFixedParameters", CENTER),
            ):
                dcpl = h5py.h5p.create(h5py.h5p.DATASET_CREATE)
                dcpl.set_layout(h5py.h5d.COMPACT)
                space = h5py.h5s.create_simple(value.shape)
                dset = h5py.h5d.create(g.id, name.encode(), h5py.h5t.IEEE_F64LE, space, dcpl=dcpl)
                dset.write(h5py.h5s.ALL, h5py.h5s.ALL, value)
        return
    if recipe == "not-hdf5":
        path.write_text("#Insight Transform File V1.0\n", encoding="utf-8")
        return
    if recipe == "truncated":
        _h5py_file(path, "composite")
        data = path.read_bytes()
        path.write_bytes(data[: len(data) // 2])
        return
    recipes[recipe]()


#: h5py recipes that ITK reads, and those it refuses.
H5PY_READ = (
    "single",
    "composite",
    "superblock3",
    "compact",
    "shuffle-fletcher32",
    "misspelt-itk4",
    "float32",
    "big-endian-f8",
    "big-endian-f4",
    "half",
    "long-double",
    "no-version-strings",
    "scalar-type",
    "dense-11",
    "dense-11-superblock3",
    "empty-composite",
    "two-linear",
    "composite-second",
)
H5PY_ERRORS = (
    "fixed-string",
    "two-strings",
    "int-parameters",
    "2d-parameters",
    "2d-fixed",
    "int-fixed",
    "scalar-parameters",
    "unknown-type",
    "no-precision-in-name",
    "gap-in-names",
    "no-transform-group",
    "missing-parameters",
    "lzf",
    "not-hdf5",
    "truncated",
    "superblock3-chunked",
)
SMOKE_H5PY = ("composite", "superblock3", "misspelt-itk4", "dense-11", "fixed-string")
#: Files h5py reads and larmorx, like ITK's HDF5 1.12, refuses: LZF (a filter of h5py's own),
#: and a chunked dataset of HDF5 2.0's newest format (layout message version 5: h5py's HDF5
#: 2.0 with ``libver="latest"``).
H5PY_ONLY = {
    "lzf": "a filter only h5py has",
    "superblock3-chunked": "HDF5 2.0's layout message version 5, newer than ITK's HDF5 1.12.1",
}


# ------------------------------------------------------------------------------------------------
# Files written by ITK (the oracle) and files found in the workspace


def _workspace() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracles").is_dir():
            return parent
    return None


def _found_files(tier: str) -> list[tuple[str, str, str]]:
    """(id, path, tier) for the HDF5 transform files of fMRIPrep's runs and TemplateFlow."""
    ws = _workspace()
    out: list[tuple[str, str, str]] = []
    if ws is None:
        return out
    runs = ws / "oracles/fmriprep-25.2.6"
    for p in sorted(runs.glob("*/out/**/*_xfm.h5")):
        out.append(
            (f"read/fmriprep-25.2.6/{p.relative_to(runs).parts[0]}/{p.name}", str(p), "standard")
        )
    for p in sorted(runs.glob("*/work/**/ants_t1_to_mni*Composite.h5")):
        out.append(
            (f"read/fmriprep-25.2.6-work/{p.relative_to(runs).parts[0]}/{p.name}", str(p), "full")
        )
    tf = ws / "oracles/downloads/templateflow"
    for p in sorted(tf.glob("**/*_xfm.h5")):
        if p.stat().st_size:
            out.append((f"read/templateflow/{p.name}", str(p), "full"))
    return out


WRITES: dict[str, tuple[str, tuple[str, ...], tuple[str, ...], str, str]] = {
    # id: (program, args, inputs in file order, tier, description)
    "composite-affine": (
        "antsApplyTransforms",
        ("-d", "3", "-o", "CompositeTransform[{out}]", "-t", "{t:affine-centered.mat}"),
        ("{t:affine-centered.mat}",),
        "smoke",
        "-o CompositeTransform[...]: one affine",
    ),
    "composite-warp-affine": (
        "antsApplyTransforms",
        (
            "-d",
            "3",
            "-o",
            "CompositeTransform[{out}]",
            "-t",
            "{t:warp-lps-vector.nii.gz}",
            "-t",
            "{t:affine-centered.mat}",
        ),
        ("{t:affine-centered.mat}", "{t:warp-lps-vector.nii.gz}"),
        "smoke",
        "-o CompositeTransform[...]: a displacement field and an affine (chunk of 190 KB)",
    ),
    "composite-float": (
        "antsApplyTransforms",
        (
            "-d",
            "3",
            "--float",
            "1",
            "-o",
            "CompositeTransform[{out}]",
            "-t",
            "{t:warp-lps-vector.nii.gz}",
            "-t",
            "{t:affine-centered.mat}",
        ),
        (),
        "standard",
        "--float 1 -o CompositeTransform[...]: float parameters",
    ),
    "composite-types": (
        "antsApplyTransforms",
        (
            "-d",
            "3",
            "-o",
            "CompositeTransform[{out}]",
            "-t",
            "{t:euler-centered.mat}",
            "-t",
            "{t:versor-rigid.txt}",
            "-t",
            "{t:similarity.mat}",
            "-t",
            "{t:translation.txt}",
            "-t",
            "{t:euler-zyx.txt}",
            "-t",
            "{t:affine-float.mat}",
        ),
        (
            "{t:affine-float.mat}",
            "{t:euler-zyx.txt}",
            "{t:translation.txt}",
            "{t:similarity.mat}",
            "{t:versor-rigid.txt}",
            "{t:euler-centered.mat}",
        ),
        "standard",
        "-o CompositeTransform[...]: six transform types (seven links, compact)",
    ),
    "composite-identity": (
        "antsApplyTransforms",
        ("-d", "3", "-o", "CompositeTransform[{out}]", "-t", "identity"),
        (),
        "standard",
        "-o CompositeTransform[...] -t identity",
    ),
    "linear": (
        "antsApplyTransforms",
        (
            "-d",
            "3",
            "-o",
            "Linear[{out}]",
            "-t",
            "{t:affine-centered.mat}",
            "-t",
            "{t:euler-centered.mat}",
        ),
        (),
        "smoke",
        "-o Linear[...]: two transforms collapsed into one affine",
    ),
    "linear-inverse": (
        "antsApplyTransforms",
        ("-d", "3", "-o", "Linear[{out},1]", "-t", "{t:affine-centered.mat}"),
        (),
        "standard",
        "-o Linear[...,1]: the inverse",
    ),
    "linear-float": (
        "antsApplyTransforms",
        (
            "-d",
            "3",
            "--float",
            "1",
            "-o",
            "Linear[{out}]",
            "-t",
            "{t:affine-centered.mat}",
            "-t",
            "{t:euler-centered.mat}",
        ),
        (),
        "smoke",
        "--float 1 -o Linear[...]: float parameters",
    ),
    "displacement-field": (
        "antsApplyTransforms",
        (
            "-d",
            "3",
            "-r",
            "{in:" + REFERENCE + "}",
            "-o",
            "DisplacementField[{out}]",
            "-t",
            "{t:warp-lps-vector.nii.gz}",
            "-t",
            "{t:affine-centered.mat}",
        ),
        (),
        "standard",
        "-o DisplacementField[...]: a collapsed field on the reference grid",
    ),
    "assemble": (
        "CompositeTransformUtil",
        ("--assemble", "{out}", "{t:affine-centered.mat}", "{t:warp-lps-vector.nii.gz}"),
        ("{t:affine-centered.mat}", "{t:warp-lps-vector.nii.gz}"),
        "standard",
        "CompositeTransformUtil --assemble (itk::ants::WriteTransform)",
    ),
    "fmriprep-rewritten": (
        "antsApplyTransforms",
        (
            "-d",
            "3",
            "-o",
            "CompositeTransform[{out}]",
            "-t",
            "{in:openneuro-derivatives/ds000005-fmriprep/sub-01/anat/sub-01_from-T1w_to-MNI152NLin2009cAsym_mode-image_xfm.h5}",
        ),
        (),
        "standard",
        "fMRIPrep's T1w→MNI composite rewritten by ITK 5.4.5: a field of several 1 Mi-value chunks",
    ),
}
for _n in (8, 9, 11, 37, 38):
    WRITES[f"composite-{_n}-affines"] = (
        "antsApplyTransforms",
        ("-d", "3", "-o", "CompositeTransform[{out}]", *(["-t", "{t:affine-centered.mat}"] * _n)),
        (),
        "smoke" if _n == 9 else "standard",
        f"-o CompositeTransform[...] of {_n} affines: {_n + 1} links"
        + (" (compact)" if _n < 8 else " (dense: a fractal heap and a v2 B-tree)"),
    )


def cases(tier: str) -> list[Case]:
    rank = _TIERS.index(tier)
    out: list[Case] = []

    def add(cid: str, category: str, desc: str, spec: Spec) -> None:
        if _TIERS.index(spec.tier) <= rank:
            out.append(Case(cid, category, desc, spec))

    for recipe in H5PY_READ:
        add(
            f"read/h5py/{recipe}",
            "read/h5py",
            f"written by h5py: {recipe}",
            Spec("read", f"h5py:{recipe}", "smoke" if recipe in SMOKE_H5PY else "standard"),
        )
    for recipe in H5PY_ERRORS:
        add(
            f"errors/h5py/{recipe}",
            "errors",
            f"written by h5py, refused by ITK: {recipe}",
            Spec("error", f"h5py:{recipe}", "smoke" if recipe in SMOKE_H5PY else "standard"),
        )
    for entry in td.select(tier="full"):
        if entry.name.endswith(".h5"):
            add(
                f"read/catalog/{entry.path}",
                "read/catalog",
                entry.description or entry.name,
                Spec(
                    "read",
                    f"catalog:{entry.path}",
                    "smoke" if entry.tier == "smoke" else "standard",
                ),
            )
    for cid, path, ftier in _found_files(tier):
        add(
            cid, "read/" + cid.split("/")[1], f"found: {Path(path).name}", Spec("read", path, ftier)
        )
    for name, (program, args, inputs, wtier, desc) in WRITES.items():
        add(
            f"read/itk/{name}",
            "read/itk",
            f"written by ITK: {desc}",
            Spec("read", f"itk:{name}", wtier),
        )
        add(
            f"write/{name}",
            "write",
            desc,
            Spec("write", f"itk:{name}", wtier, program, args, inputs),
        )
    add(
        "write/itk15-identity-compressed",
        "write",
        "CompositeTransformUtil --assemble with an IdentityTransform: ITK fails (ITK-15) and "
        "leaves a partial file",
        Spec("partial", "itk15", "standard"),
    )
    return sorted(out, key=lambda c: (c.category, c.id))


# ------------------------------------------------------------------------------------------------
# Running the programs


def _env() -> dict[str, str]:
    return {**os.environ, "ITK_GLOBAL_DEFAULT_NUMBER_OF_THREADS": THREADS}


def _program(name: str) -> Path:
    b = ants_bin()
    if b is None or not (b / name).is_file():
        raise Outcome(
            SKIPPED, f"the ANTs oracle's {name} was not found (scripts/build_ants_oracle.sh)"
        )
    return b / name


def _run(name: str, args: list[str]) -> tuple[int, str, str]:
    p = subprocess.run(
        [str(_program(name)), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=_env(),
        timeout=3600,
    )
    return p.returncode, p.stdout, p.stderr


def _run_larmorx(args: list[str], mode: str = defects.REPRODUCE) -> tuple[int, str]:
    from larmorx.cli import run

    err = io.StringIO()
    with (
        defects.mode(mode),
        contextlib.redirect_stderr(err),
        contextlib.redirect_stdout(io.StringIO()),
    ):
        code = run(["larmorx", "ants", "antsApplyTransforms", *args])
    return code, err.getvalue()


def _expand(arg: str, tmp: Path, out: Path) -> str:
    def sub(m: re.Match[str]) -> str:
        key, _, value = m.group(1).partition(":")
        if key == "out":
            return str(out)
        if key == "t":
            return str(td.get(f"{T}/{value}"))
        if key == "in":
            return str(td.get(value))
        raise KeyError(key)

    return re.sub(r"\{([^{}]+)\}", sub, arg)


def itk_file(name: str, tmp: Path) -> Path:
    """The file ITK writes for ``WRITES[name]``."""
    program, args, _, _, _ = WRITES[name]
    out = tmp / f"itk-{name}.h5"
    code, _, err = _run(program, [_expand(a, tmp, out) for a in args])
    if code != 0 or not out.is_file():
        raise Outcome(
            SKIPPED, f"the oracle did not write the file (exit {code}): {err.strip()[-300:]}"
        )
    return out


def _source(spec: Spec, tmp: Path) -> Path:
    s = spec.source
    if s.startswith("h5py:"):
        path = tmp / f"{s.removeprefix('h5py:')}.h5"
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            _h5py_file(path, s.removeprefix("h5py:"))
        return path
    if s.startswith("catalog:"):
        return Path(td.get(s.removeprefix("catalog:")))
    if s.startswith("itk:"):
        return itk_file(s.removeprefix("itk:"), tmp)
    path = Path(s)
    if not path.is_file():
        raise Outcome(SKIPPED, f"{path} not found")
    return path


# ------------------------------------------------------------------------------------------------
# Comparing


_ADDRESS = re.compile(r"\b0x[0-9a-f]+\b")


def normalise(text: str) -> str:
    """ANTs' and larmorx's messages as compared: HDF5's error stack left out (compared on
    its own), addresses written as 0x0, ITK's source tree as ``<ITK source>``."""
    out, skip = [], False
    for line in text.splitlines():
        if line.startswith("HDF5-DIAG"):
            skip = True
            continue
        if skip and line.startswith(("  #", "    ")):
            continue
        skip = False
        line = _ADDRESS.sub("0x0", line)
        line = re.sub(r"File: .*?/(Modules/)", r"File: <ITK source>/\1", line)
        out.append(line)
    return "\n".join(out).strip()


def diag_messages(text: str) -> list[str]:
    """The messages of HDF5's error stack frames."""
    msgs = []
    for line in text.splitlines():
        if (m := re.match(r"^  #\d+: .*?\(\): (.*)$", line)) or (
            m := re.match(r"^  #\d+: (?!in )(.*)$", line)
        ):
            msgs.append(m.group(1))
    return msgs


def _kind(dtype: np.dtype) -> str:
    import h5py

    if h5py.check_string_dtype(dtype) is not None or dtype.kind == "S":
        return "string"
    if dtype.kind == "f":
        return f"float{dtype.itemsize}"
    if dtype.kind in "iu":
        return f"integer{dtype.itemsize}"
    return f"other:{dtype}"


def h5py_datasets(path: Path) -> list[tuple[str, str, list[int], Any]]:
    """Every group and dataset as h5py reads them, in ``visititems`` order, in the form of
    ``larmorx._core.hdf5_datasets``."""
    import h5py

    out: list[tuple[str, str, list[int], Any]] = []

    def visit(name: str, obj: Any) -> None:
        if isinstance(obj, h5py.Group):
            out.append((name, "group", [], None))
            return
        kind = _kind(obj.dtype)
        value: Any = None
        if kind == "string":
            raw = obj[()]
            items = np.atleast_1d(np.asarray(raw, dtype=object)).ravel()
            value = [None if v is None else bytes(v) for v in items]
        elif kind.startswith("float"):
            value = np.asarray(obj[()]).astype(np.float64).ravel()
        out.append((name, kind, list(obj.shape), value))

    with h5py.File(path, "r") as f:
        f.visititems(visit)
    return out


def _same_value(a: Any, b: Any) -> bool:
    if a is None or b is None:
        return a is None and b is None
    if isinstance(a, list):
        return list(a) == list(b)
    a = np.asarray(a, dtype=np.float64).ravel()
    b = np.asarray(b, dtype=np.float64).ravel()
    return a.shape == b.shape and bool(np.array_equal(a.view(np.int64), b.view(np.int64)))


def compare_datasets(checks: CheckList, name: str, ours: list, theirs: list) -> bool:
    """Names, datatypes, shapes and values of every dataset, larmorx's against h5py's."""
    names = [o[0] for o in ours], [t[0] for t in theirs]
    if names[0] != names[1]:
        checks(f"{name}: objects", False, detail=f"larmorx {names[0]} / h5py {names[1]}")
        return False
    bad = []
    for (n, k1, s1, v1), (_, k2, s2, v2) in zip(ours, theirs, strict=True):
        if k1 != k2 or list(s1) != list(s2):
            bad.append(f"{n}: {k1}{list(s1)} / {k2}{list(s2)}")
        elif (k1 == "string" or k1.startswith("float")) and not _same_value(v1, v2):
            bad.append(f"{n}: values differ")
    checks(
        f"{name}: every dataset (names, types, shapes, values bit for bit)",
        not bad,
        detail="; ".join(bad[:5]) or f"{sum(1 for o in ours if o[1] != 'group')} datasets",
    )
    return not bad


def _parts_of(path: Path) -> list[tuple[str, np.ndarray, np.ndarray]]:
    """The transforms an ITK-written file holds, read with h5py: (type, parameters as stored,
    fixed parameters)."""
    import h5py

    out = []
    with h5py.File(path, "r") as f:
        g = f["TransformGroup"]
        for i in range(len(g)):
            m = g[str(i)]
            name = bytes(np.atleast_1d(m["TransformType"][()])[0]).decode()
            if "CompositeTransform" in name:
                out.append((name, np.zeros(0), np.zeros(0)))
                continue
            out.append(
                (
                    name,
                    np.asarray(m["TransformParameters"][()]),
                    np.asarray(m["TransformFixedParameters"][()]),
                )
            )
    return out


def _flattened(parts: list) -> list:
    """The transforms ANTs' composite holds after reading a file's parts
    (``itk::ants::ReadTransform``, ``GetCompositeTransformFromParserOption``)."""
    if parts and "CompositeTransform" in parts[0][0]:
        return list(parts[1:])
    return list(parts)


def itk_name(name: str, dtype: str) -> str:
    """ITK's ``CorrectTransformPrecisionType``: the name of the transform a reader of
    precision ``dtype`` (``f4``, ``f8``) creates for a stored name."""
    own, other = ("float", "double") if dtype == "f4" else ("double", "float")
    return name if own in name else name.replace(other, own, 1)


def itk_fixed(name: str, fixed: Any, dtype: str) -> np.ndarray:
    """The fixed parameters a transform of precision ``dtype`` gives back: a float linear
    transform keeps its centre as a float point (``MatrixOffsetTransformBase::m_Center``), so
    its fixed parameters come back rounded to float; a displacement field's (its grid) do
    not."""
    f = np.asarray(fixed, dtype=np.float64)
    if dtype == "f4" and "DisplacementField" not in name:
        return f.astype(np.float32).astype(np.float64)
    return f


def _same_parts(a: list, b: list, dtype: str) -> tuple[bool, str]:
    if [p[0] for p in a] != [p[0] for p in b]:
        return False, f"types {[p[0] for p in a]} / {[p[0] for p in b]}"
    for (n, p1, f1), (_, p2, f2) in zip(a, b, strict=True):
        x = np.asarray(p1).astype(dtype)
        y = np.asarray(p2).astype(dtype)
        if x.shape != y.shape or not np.array_equal(x, y):
            return False, f"{n}: parameters differ"
        if not np.array_equal(np.asarray(f1, np.float64), np.asarray(f2, np.float64)):
            return False, f"{n}: fixed parameters differ"
    return True, f"{len(a)} transforms"


def _check_itk_view(checks: CheckList, path: Path, tmp: Path, single: bool) -> None:
    """The transforms as ITK reads them (ANTs writes back what it read) against larmorx's
    reading in ``reproduce`` mode."""
    from larmorx import _core

    out = tmp / ("itk-view-float.h5" if single else "itk-view.h5")
    args = ["-d", "3", "-o", f"CompositeTransform[{out}]", "-t", str(path)]
    if single:
        args[2:2] = ["--float", "1"]
    code, _, err = _run("antsApplyTransforms", args)
    label = "ITK's float reading (--float 1)" if single else "ITK's reading"
    try:
        ours, _ = _core.itk_transform_read(
            str(path), "float32" if single else "float64", defects.REPRODUCE
        )
        lx_err = ""
    except (ValueError, OSError) as e:
        ours, lx_err = None, str(e)
    if code != 0 or not out.is_file():
        # ANTs cannot read the transforms, or cannot write them back (a composite after the
        # first transform): antsApplyTransforms on an image must then fail for both, or
        # succeed for both with the same output (checked by _image_route).
        a_code, l_code, _, _ = _image_route(checks, path, tmp, single=single)
        checks(
            f"{label}: ANTs cannot write the transforms back; antsApplyTransforms agrees",
            (a_code == 0) == (l_code == 0),
            detail=f"ANTs exit {code} ({err.strip()[-160:]}); on an image: ANTs {a_code}, larmorx {l_code}",
        )
        return
    if ours is None:
        checks(label, False, detail=f"ANTs reads it, larmorx refuses: {lx_err[:200]}")
        return
    theirs = _parts_of(out)
    dtype = "f4" if single else "f8"
    named = [(itk_name(n, dtype), p, itk_fixed(n, f, dtype)) for n, p, f in _flattened(ours)]
    if not named:
        # antsApplyTransforms applies the identity when the files hold no transform
        # (``GetCompositeTransformFromParserOption``): a MatrixOffsetTransformBase.
        named = [
            (
                itk_name("MatrixOffsetTransformBase_double_3_3", dtype),
                np.eye(4)[:3, :3].ravel().tolist() + [0.0] * 3,
                np.zeros(3),
            )
        ]
    ok, detail = _same_parts(named, _flattened(theirs), dtype)
    checks(f"{label}: the transforms, bit for bit", ok, detail=detail)


def _check_nitransforms(checks: CheckList, path: Path) -> None:
    """``lx.transforms.load_itk_composite`` (``reproduce``) against nitransforms 25.1.0."""
    import nitransforms as nt

    import larmorx as lx

    if path.suffix != ".h5":
        return
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            theirs = nt.manip.TransformChain.from_filename(str(path), fmt="h5").transforms
        their_err = ""
    except Exception as e:  # nitransforms refusing the file
        theirs, their_err = None, f"{type(e).__name__}: {e}"
    try:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            ours = lx.transforms.load_itk_composite(
                path, upstream_defects=defects.REPRODUCE
            ).transforms
        our_err = ""
    except Exception as e:
        ours, our_err = None, f"{type(e).__name__}: {e}"
    if theirs is None or ours is None:
        checks(
            "nitransforms: both refuse the file",
            theirs is None and ours is None,
            detail=f"nitransforms: {their_err[:120] or 'reads it'}; larmorx: {our_err[:120] or 'reads it'}",
        )
        return
    bad = []
    if len(theirs) != len(ours):
        bad.append(f"{len(ours)} / {len(theirs)} transforms")
    for k, (a, b) in enumerate(zip(ours, theirs, strict=False)):
        if hasattr(b, "_deltas"):
            if not (
                hasattr(a, "deltas")
                and np.array_equal(np.asarray(a.deltas), np.asarray(b._deltas, np.float64))
                and np.array_equal(np.asarray(a.affine), np.asarray(b.reference.affine))
            ):
                bad.append(f"transform {k}: field differs")
        elif not (
            hasattr(a, "matrix") and np.array_equal(np.asarray(a.matrix), np.asarray(b.matrix))
        ):
            bad.append(f"transform {k}: matrix differs")
    checks(
        "nitransforms: the chain, bit for bit",
        not bad,
        detail="; ".join(bad[:3]) or f"{len(ours)} transforms",
    )


def _image_route(
    checks: CheckList, path: Path, tmp: Path, mode: str = defects.REPRODUCE, single: bool = False
) -> tuple[int, int, str, str]:
    """antsApplyTransforms with ``-t path`` on an image, ANTs and larmorx: exit status, the
    messages and the output (both must be bit-identical when both succeed)."""
    import nibabel as nib

    a_out, l_out = tmp / "ants.nii.gz", tmp / "larmorx.nii.gz"
    base = ["-d", "3", "-i", str(td.get(IMAGE)), "-r", str(td.get(IMAGE)), "-t", str(path)]
    if single:
        base += ["--float", "1"]
    a_code, _, a_err = _run("antsApplyTransforms", [*base, "-o", str(a_out)])
    l_code, l_err = _run_larmorx([*base, "-o", str(l_out)], mode)
    if a_code == 0 and l_code == 0:
        a = np.asanyarray(nib.load(a_out).dataobj)
        b = np.asanyarray(nib.load(l_out).dataobj)
        checks("output image of antsApplyTransforms, bit for bit", np.array_equal(a, b))
    return a_code, l_code, a_err, l_err


# ------------------------------------------------------------------------------------------------
# The cases


def run_case(case: Case, checks: CheckList) -> None:
    spec: Spec = case.payload
    with tempfile.TemporaryDirectory(prefix="lx-h5-") as tmp_name:
        tmp = Path(tmp_name)
        if spec.kind == "partial":
            _run_partial(checks, tmp)
            return
        path = _source(spec, tmp)
        if spec.kind == "read":
            _run_read(case, checks, path, tmp)
        elif spec.kind == "error":
            _run_error(case, checks, path, tmp)
        elif spec.kind == "write":
            _run_write(case, checks, path, tmp, spec)


def _compare_with_h5py(checks: CheckList, path: Path) -> None:
    """Every dataset as larmorx and h5py read it; when h5py refuses the file, larmorx must
    too."""
    from larmorx import _core

    try:
        theirs = h5py_datasets(path)
    except Exception as e:  # h5py refusing the file
        try:
            _core.hdf5_datasets(str(path))
            ours = "reads it"
        except Exception as f:
            ours = f"refuses it ({f})"
        checks(
            "h5py: both refuse the file",
            ours.startswith("refuses"),
            detail=f"h5py: {type(e).__name__}; larmorx {ours[:160]}",
        )
        return
    try:
        ours_list = _core.hdf5_datasets(str(path))
    except Exception as e:
        only = path.stem in H5PY_ONLY
        checks(
            "h5py reads the file, larmorx refuses it"
            + (f" ({H5PY_ONLY[path.stem]})" if only else ""),
            only,
            metric="report" if only else "exact",
            detail=str(e)[:200],
        )
        return
    compare_datasets(checks, "h5py", ours_list, theirs)


def _run_read(case: Case, checks: CheckList, path: Path, tmp: Path) -> None:
    _compare_with_h5py(checks, path)
    _check_itk_view(checks, path, tmp, single=False)
    _check_itk_view(checks, path, tmp, single=True)
    _check_nitransforms(checks, path)
    _fix_read(checks, path)


def _fix_read(checks: CheckList, path: Path) -> None:
    """D13: larmorx's default reading against ``reproduce``: the defects it names must be
    exactly those whose correction changed the result."""
    import larmorx as lx

    results = {}
    for mode in (defects.FIX, defects.REPRODUCE):
        with warnings.catch_warnings(record=True) as record:
            warnings.simplefilter("always")
            try:
                parts = lx.transforms.read(path, upstream_defects=mode)
                results[mode] = ([(t.name, t.parameters, t.fixed_parameters) for t in parts], None)
            except (ValueError, OSError) as e:
                results[mode] = (None, str(e))
        results[mode] = (*results[mode], defects.warning_entries(record))
    fix, rep = results[defects.FIX], results[defects.REPRODUCE]
    same = (fix[0] is None) == (rep[0] is None) and (
        fix[0] is None or _same_parts(fix[0], rep[0], "f8")[0]
    )
    named = fix[2]
    checks(
        defects.FIX_PASS,
        same == (not named),
        detail=("unchanged" if same else "changed") + (f"; named {named}" if named else ""),
    )


def _run_error(case: Case, checks: CheckList, path: Path, tmp: Path) -> None:

    _compare_with_h5py(checks, path)
    a_code, l_code, a_err, l_err = _image_route(checks, path, tmp)
    checks("exit status", (a_code == 0) == (l_code == 0), detail=f"ANTs {a_code}, larmorx {l_code}")
    if a_code != 0:
        same = normalise(a_err) == normalise(l_err)
        checks(
            "messages (ITK's exception, addresses and build paths aside)",
            same,
            detail=""
            if same
            else f"ANTs: {normalise(a_err)[-400:]!r}\nlarmorx: {normalise(l_err)[-400:]!r}",
        )
        ours, theirs = diag_messages(l_err), diag_messages(a_err)
        checks(
            "HDF5's error stack: larmorx's frames are among HDF5's",
            bool(ours) == bool(theirs) and all(m in theirs for m in ours[1:]),
            detail=f"larmorx {ours}; HDF5 {theirs[-3:]}",
        )
    _fix_read(checks, path)
    if a_code != 0 and l_code != 0 and all(c.passed for c in checks.checks):
        last = normalise(a_err).splitlines()
        raise Outcome(BOTH_ERROR, last[-1][:200] if last else "")


def _run_write(case: Case, checks: CheckList, itk: Path, tmp: Path, spec: Spec) -> None:
    import h5py

    import larmorx as lx
    from larmorx import _core

    r = _core.itk_h5_replay(str(itk))
    divergence = WRITE_DIVERGENCES.get(case.id)
    checks(
        "bytes with ITK's compressed chunks (object times set alike)",
        r["identical_with_itk_chunks"] or divergence is not None,
        detail=f"ITK {r['size'][0]} bytes, larmorx {r['size_with_itk_chunks']}",
    )
    streams = [c for c in r["chunks"] if not c[4]]
    checks(
        "bytes with larmorx's deflate",
        True,
        metric="report",
        value="identical"
        if r["identical_except_times"]
        else f"{r['differing_bytes']} bytes differ",
        detail=(
            f"{len(r['chunks'])} chunks, {len(streams)} compressed differently: "
            + ", ".join(
                f"{c[0].rsplit('/', 2)[-2]}#{c[1]} {c[2]}→{c[3]} bytes" for c in streams[:4]
            )
            if r["chunks"]
            else "no compressed chunk"
        ),
    )
    checks(
        "bytes as written (ITK records the second each object was written)",
        True,
        metric="report",
        value="identical"
        if r["identical"]
        else "times differ"
        if r["identical_except_times"]
        else "differ",
    )
    # larmorx's file, with its defaults (time 0, larmorx's strings), read by ANTs, h5py and
    # nitransforms as ITK's file is read.
    ours = tmp / "larmorx.h5"
    parts = lx.transforms.read(itk, upstream_defects=defects.REPRODUCE)
    lx.transforms.write(ours, parts)
    if spec.inputs:
        inputs = [_expand(a, tmp, tmp / "unused") for a in spec.inputs]
        built: list = []
        for p in inputs:
            built += _flattened(_core.itk_transform_read(p, "float64", defects.REPRODUCE)[0])
        ok, detail = _same_parts(built, _flattened(_parts_of(itk)), "f8")
        checks("larmorx's reading of the inputs gives ITK's transforms", ok, detail=detail)
    with h5py.File(itk, "r") as f, h5py.File(ours, "r") as g:
        same = _same_group(f["TransformGroup"], g["TransformGroup"])
    checks("h5py reads larmorx's file as ITK's (TransformGroup)", same)
    _check_nitransforms_pair(checks, itk, ours)
    import nibabel as nib

    outs = []
    for name, f in (("itk", itk), ("larmorx", ours)):
        out = tmp / f"apply-{name}.nii.gz"
        code, _, err = _run(
            "antsApplyTransforms",
            [
                "-d",
                "3",
                "-i",
                str(td.get(IMAGE)),
                "-r",
                str(td.get(REFERENCE)),
                "-t",
                str(f),
                "-o",
                str(out),
            ],
        )
        outs.append((code, out, err))
    if outs[0][0] == 0:
        same = outs[1][0] == 0 and np.array_equal(
            np.asanyarray(nib.load(outs[0][1]).dataobj), np.asanyarray(nib.load(outs[1][1]).dataobj)
        )
        checks(
            "ANTs reads larmorx's file: the same output image as from ITK's file",
            same,
            detail="" if same else outs[1][2].strip()[-300:],
        )
    if divergence is not None and not r["identical_with_itk_chunks"]:
        raise Outcome(EXPECTED, divergence)


def _same_group(a: Any, b: Any) -> bool:
    import h5py

    if sorted(a.keys()) != sorted(b.keys()):
        return False
    for k in a:
        x, y = a[k], b[k]
        if isinstance(x, h5py.Group) != isinstance(y, h5py.Group):
            return False
        if isinstance(x, h5py.Group):
            if not _same_group(x, y):
                return False
            continue
        if x.dtype != y.dtype or x.shape != y.shape:
            return False
        vx, vy = x[()], y[()]
        if x.dtype.kind == "f":
            if not np.array_equal(np.asarray(vx).view(np.uint8), np.asarray(vy).view(np.uint8)):
                return False
        elif not np.array_equal(np.asarray(vx, dtype=object), np.asarray(vy, dtype=object)):
            return False
    return True


def _check_nitransforms_pair(checks: CheckList, itk: Path, ours: Path) -> None:
    import nitransforms as nt

    def load(p: Path) -> Any:
        try:
            with warnings.catch_warnings():
                warnings.simplefilter("ignore")
                return nt.manip.TransformChain.from_filename(str(p), fmt="h5").transforms
        except Exception as e:
            return f"{type(e).__name__}"

    a, b = load(itk), load(ours)
    if isinstance(a, str) or isinstance(b, str):
        checks(
            "nitransforms reads larmorx's file as ITK's",
            a == b,
            detail=f"ITK's: {a if isinstance(a, str) else 'read'}; larmorx's: {b if isinstance(b, str) else 'read'}",
        )
        return
    same = len(a) == len(b) and all(
        (
            np.array_equal(x._deltas, y._deltas)
            and np.array_equal(x.reference.affine, y.reference.affine)
        )
        if hasattr(x, "_deltas")
        else np.array_equal(x.matrix, y.matrix)
        for x, y in zip(a, b, strict=False)
    )
    checks("nitransforms reads larmorx's file as ITK's", same, detail=f"{len(a)} transforms")


def _run_partial(checks: CheckList, tmp: Path) -> None:
    """ITK-15: CompositeTransformUtil assembles an affine and an IdentityTransform; ITK's
    writer fails and leaves the file written so far. larmorx in ``reproduce`` mode writes the
    same file and reports the same failure; by default it stores the empty parameters."""
    import struct

    import larmorx as lx
    from larmorx import _core

    ident = tmp / "identity.txt"
    ident.write_text(
        "#Insight Transform File V1.0\n#Transform 0\nTransform: IdentityTransform_double_3_3\n"
        "Parameters: \nFixedParameters: \n",
        encoding="utf-8",
    )
    itk = tmp / "itk.h5"
    affine = str(td.get(f"{T}/affine-centered.mat"))
    code, _, err = _run("CompositeTransformUtil", ["--assemble", str(itk), affine, str(ident)])
    checks("ITK fails", code != 0, detail=f"exit {code}")
    if not itk.is_file():
        raise Outcome(SKIPPED, "ITK left no file")
    data = itk.read_bytes()
    time = struct.unpack_from("<I", data, struct.unpack_from("<Q", data, 36)[0] + 6)[0]
    strings = {
        k: v[0].decode()
        for k, _, _, v in _core.hdf5_datasets(str(itk))
        if k in ("ITKVersion", "HDFVersion", "OSName", "OSVersion")
    }
    options = lx.transforms.Hdf5WriteOptions(
        time=time,
        itk_version=strings["ITKVersion"],
        hdf_version=strings["HDFVersion"],
        os_name=strings["OSName"],
        os_version=strings["OSVersion"],
    )
    comp = lx.transforms.ItkTransform("CompositeTransform_double_3_3", np.zeros(0), np.zeros(0))
    parts = [comp, *lx.transforms.read(affine), *lx.transforms.read(ident)]
    ours = tmp / "larmorx.h5"
    try:
        lx.transforms.write(ours, parts, hdf5=options, upstream_defects=defects.REPRODUCE)
        message = ""
    except ValueError as e:
        message = str(e)
    checks("larmorx fails (reproduce)", bool(message), detail=message[-200:])
    same = ours.is_file() and _core.hdf5_set_times(ours.read_bytes(), time) == _core.hdf5_set_times(
        data, time
    )
    checks("the file left behind, byte for byte", same)
    report = re.sub(r"Can't write transform file \S+", "Can't write transform file <file>", err)
    checks(
        "ITK's report names H5Pset_chunk",
        "H5Pset_chunk failed" in report and "all chunk dimensions must be positive" in report,
        detail=normalise(report)[-200:],
    )
    fixed = tmp / "fixed.h5"
    with warnings.catch_warnings(record=True) as record:
        warnings.simplefilter("always")
        lx.transforms.write(fixed, parts, hdf5=options)
    named = defects.warning_entries(record)
    back = lx.transforms.read(fixed)
    checks(
        defects.FIX_PASS,
        named == ["ITK-15"] and [t.name for t in back] == [t.name for t in parts],
        detail=f"named {named}; read back {len(back)} transforms",
    )
    a_code, l_code, _, _ = _image_route(checks, fixed, tmp)
    checks(
        "ANTs reads larmorx's corrected file",
        a_code == 0,
        detail=f"ANTs exit {a_code}, larmorx {l_code}",
    )


# ------------------------------------------------------------------------------------------------
# The suite


def _highlights(results: list[CaseResult]) -> list[str]:
    def passed(r: CaseResult, prefix: str) -> bool | None:
        found = [c for c in r.checks if c.name.startswith(prefix)]
        return all(c.passed for c in found) if found else None

    reads = [r for r in results if r.case.startswith("read/")]
    writes = [
        r
        for r in results
        if r.case.startswith("write/")
        and r.status != SKIPPED
        and any(c.name.startswith("bytes with ITK's compressed chunks") for c in r.checks)
    ]
    lines = []
    h5 = [passed(r, "h5py:") for r in reads + [r for r in results if r.case.startswith("errors/")]]
    lines.append(
        f"Datasets as h5py reads them, bit for bit: {sum(1 for x in h5 if x)} of {sum(1 for x in h5 if x is not None)} files"
    )
    itk = [passed(r, "ITK's reading:") for r in reads]
    lines.append(
        f"Transforms as ITK reads them (double), bit for bit: {sum(1 for x in itk if x)} of {sum(1 for x in itk if x is not None)} files"
    )
    fl = [passed(r, "ITK's float reading") for r in reads]
    lines.append(
        f"Transforms as ITK reads them (float), bit for bit: {sum(1 for x in fl if x)} of {sum(1 for x in fl if x is not None)} files"
    )
    nit = [passed(r, "nitransforms:") for r in reads]
    lines.append(
        f"nitransforms' chain, bit for bit: {sum(1 for x in nit if x)} of {sum(1 for x in nit if x is not None)} files"
    )
    errs = [r for r in results if r.case.startswith("errors/")]
    lines.append(
        f"Files ITK refuses: {sum(1 for r in errs if r.status in (BOTH_ERROR,))} of {len(errs)} refused alike "
        "(exit status and messages)"
    )
    with_chunks = [
        r
        for r in writes
        if passed(r, "bytes with ITK's compressed chunks") and r.status != EXPECTED
    ]
    lines.append(
        f"Files written: {len(with_chunks)} of {len(writes)} byte-identical to ITK's given ITK's compressed "
        "chunks and object times"
    )
    own = [
        r
        for r in writes
        if any(
            c.name == "bytes with larmorx's deflate" and c.value == "identical" for c in r.checks
        )
    ]
    lines.append(
        f"Files written with larmorx's deflate: {len(own)} of {len(writes)} byte-identical (object times set alike)"
    )
    return lines


def suite() -> Suite:
    import h5py
    import nitransforms

    return Suite(
        name="itk-hdf5",
        title="ITK's HDF5 transform files",
        tool="`lx.transforms.read`/`write`, `lx.transforms.load_itk_composite`, `-t x.h5` of `larmorx ants antsApplyTransforms` (crates larmorx-hdf5, larmorx-io)",
        reference=(
            f"ITK 5.4.5 on HDF5 1.12.1 through {ants_version()} (antsApplyTransforms, "
            f"CompositeTransformUtil), h5py {h5py.__version__} on HDF5 {h5py.version.hdf5_version}, "
            f"nitransforms {nitransforms.__version__}"
        ),
        thresholds=[
            (
                "Datasets",
                "every group and dataset h5py reads: names, types, shapes, values bit for bit",
            ),
            (
                "Transforms",
                "as ITK reads them (written back by ANTs), bit for bit, in double and float",
            ),
            ("nitransforms", "the chain it loads: matrices and fields bit for bit"),
            ("Refused files", "the same exit status and messages (addresses, build paths aside)"),
            (
                "Written files",
                "every byte ITK's, given ITK's compressed chunks and object times; read back by ANTs "
                "(identical output image), h5py and nitransforms as ITK's file",
            ),
        ],
        cases=cases,
        run_case=run_case,
        packages=("h5py", "nitransforms", "nibabel"),
        highlights=_highlights,
        notes=(
            "larmorx reads in `reproduce` mode for the comparisons (ITK's defects reproduced, D13), "
            "and a fix pass reports what the default changes. ITK records the time of writing in "
            "every object header; larmorx writes 0 unless told otherwise, so the byte comparisons "
            "give larmorx ITK's time and set each object's times alike on both sides. The "
            "compressed chunks are the only bytes larmorx's own output may change: zlib-ng 2.1.6 "
            "(ITK's) hashes with CRC-32C on x86-64 CPUs with SSE4.2, CRC-32 on Arm with the CRC "
            "extension, and multiplicatively elsewhere, so ITK's own compressed bytes depend on "
            "the CPU; larmorx's deflate (zlib-rs) gives the same bytes on every platform "
            "(golden case `py.transforms.write_h5`)."
        ),
    )


__all__ = ["suite"]
