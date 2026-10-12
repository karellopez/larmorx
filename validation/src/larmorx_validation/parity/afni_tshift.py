# SPDX-License-Identifier: Apache-2.0
"""Parity of ``larmorx afni 3dTshift`` with AFNI 25.2.09's ``3dTshift``.

Both programs get the same arguments and files. AFNI runs as its own binary (built by
``scripts/build_afni_oracle.sh``; set ``LARMORX_AFNI_BIN`` to use another); larmorx runs
in-process through its Python console entry point, with the implementation forced
(``larmorx.cli.run(..., implementation="original")``). The outputs are read with nibabel, a
reader independent of both, and compared value by value and field by field.

``suite("replica")`` runs the same cases against the GPL-3.0-or-later replica
(``larmorx-gpl afni 3dTshift``, ``crates-gpl/``) instead, the way users reach it: through
larmorx's own dispatch (``implementation="replica"``), which runs the replica's program as a
separate process. This package is Apache-2.0 and never loads the replica. The program under
test is ``LARMORX_GPL_BIN``, else the ``crates-gpl`` build, else ``larmorx-gpl`` on the PATH.

Both implementations are compared with AFNI in D13's ``reproduce`` mode; a fix pass then runs
larmorx's default on the same arguments and reports what the corrections change. In the
replica's suite, the cases where a correction applies also compare the replica's default with
the clean-room original's: ``implementation="auto"`` may run either, so their corrected outputs
must agree.

Every case the Python API can express (a ``-tpattern`` name, fMRIPrep's slice timing, the
options in seconds) also runs through ``lx.afni.tshift(..., implementation=...)``, from the file
and, for unscaled uint8, int16 and float32 inputs, from memory; the results must be the
command line's output, bit for bit.

Cases cover (``specs/3dTshift.md`` §9):
- every file of ``larmorx-testdata`` ``synthetic/tshift/`` (every FFT length class and every
  data type AFNI reads);
- every interpolation method;
- ``-ignore``, ``-tzero``, ``-slice``, ``-rlt``, ``-rlt+``, ``-no_detrend``, ``-TR`` units and
  every ``-tpattern`` form;
- slice timing from NIfTI headers, and the "copy of input" cases;
- the errors both programs must raise;
- real BOLD runs, called as fMRIPrep calls 3dTshift (``SliceTiming`` from the BIDS sidecar).
"""

from __future__ import annotations

import contextlib
import functools
import io
import json
import os
import re
import shutil
import subprocess
import tempfile
import warnings
from collections.abc import Iterator
from dataclasses import dataclass
from pathlib import Path

import larmorx_testdata as td
import numpy as np

from larmorx_validation.parity import defects
from larmorx_validation.parity.harness import (
    BOTH_ERROR,
    EXPECTED,
    PASS,
    Case,
    CaseResult,
    Check,
    CheckList,
    Outcome,
    Suite,
)
from larmorx_validation.report import table

#: specs/3dTshift.md §9 (PLAN.md §11.3, slice timing as deterministic arithmetic).
RTOL = 1e-5
INT_ATOL = 1
#: Geometry is written from the same float32 header values: compare affines to 1e-6 mm.
AFFINE_ATOL = 1e-6

ORACLE = "oracles/afni-25.2.09/bin/3dTshift"
S = "synthetic/tshift"
F32 = f"{S}/dtypes/float32.nii.gz"
I16 = f"{S}/dtypes/int16.nii.gz"
METHODS = ("-Fourier", "-linear", "-cubic", "-quintic", "-heptic", "-wsinc5", "-wsinc9")
PATTERNS = (
    "alt+z",
    "altplus",
    "alt+z2",
    "alt-z",
    "altminus",
    "alt-z2",
    "seq+z",
    "seqplus",
    "seq-z",
    "seqminus",
    "zero",
    "simult",
)


@functools.cache
def afni_binary() -> str | None:
    """The AFNI ``3dTshift`` oracle: ``LARMORX_AFNI_BIN``, else the workspace's
    ``oracles/afni-25.2.09`` build, else ``3dTshift`` on the PATH."""
    if env := os.environ.get("LARMORX_AFNI_BIN"):
        path = Path(env)
        return str(path / "3dTshift" if path.is_dir() else path)
    for parent in Path(__file__).resolve().parents:
        candidate = parent / ORACLE
        if candidate.is_file():
            return str(candidate)
    return shutil.which("3dTshift")


@functools.cache
def afni_version() -> str:
    binary = afni_binary()
    if binary is None:
        return "not found"
    out = subprocess.run([binary, "-help"], capture_output=True, text=True, encoding="utf-8")
    match = re.search(r"AFNI_[0-9.]+", out.stdout + out.stderr)
    return match.group(0) if match else "unknown version"


# ------------------------------------------------------------------------------------------------
# Scenarios


@dataclass(frozen=True)
class Scenario:
    """One 3dTshift invocation.

    ``image`` is a catalog path, or ``gen:<name>`` for an input generated from a catalog file
    (see :func:`_generated`). In ``args``, ``{tmp}`` is the case's temporary directory.
    ``sidecar`` names a BIDS JSON whose ``SliceTiming`` is passed as fMRIPrep passes it.
    """

    image: str
    args: tuple[str, ...] = ()
    tier: str = "smoke"
    expect: str = "pass"  # "pass", "error" (both reject) or "divergence"
    reason: str = ""
    sidecar: str | None = None
    suffix: str = ".nii"
    files: tuple[tuple[str, str], ...] = ()  # (name, text) written to {tmp} first
    output: str | None = None  # the output's file name in {tmp} (default: <tool><suffix>)
    #: AFNI's environment variables for both programs (None removes one), over the suite's
    #: (:func:`_environment`).
    env: tuple[tuple[str, str | None], ...] = ()
    #: Each program runs in a directory of its own (``{tmp}/afni``, ``{tmp}/larmorx``), with
    #: ``files`` written to each and the output named relative to it (``output`` or
    #: ``out.nii``), and their messages are compared too (:func:`_run_separately`). The
    #: arguments are complete when they contain ``{input}`` (the dataset); otherwise
    #: ``-prefix <output> <input>`` is appended.
    separate: bool = False
    #: The file the run writes when it is not the ``-prefix`` (``AFNI_DECONFLICT=YES``).
    written: str | None = None
    #: Compare the whole output file byte for byte, AFNI's header extension included.
    whole_file: bool = False


def _scenarios() -> list[tuple[str, str, str, Scenario]]:
    out: list[tuple[str, str, str, Scenario]] = []

    def add(cid: str, category: str, desc: str, s: Scenario) -> None:
        out.append((cid, category, desc, s))

    synthetic = sorted(e.path for e in td.select(tags={"tshift"}) if e.path.startswith(f"{S}/"))
    for path in synthetic:
        name = path.removeprefix(f"{S}/").removesuffix(".nii.gz")
        add(
            f"synthetic/{name}",
            "synthetic",
            f"default Fourier, -tpattern alt+z: {td.catalog()[path].description}",
            Scenario(path, ("-tpattern", "alt+z")),
        )
    for path in synthetic:
        if "/lengths/" in path:
            continue
        name = path.removeprefix(f"{S}/").removesuffix(".nii.gz")
        for method in METHODS[1:]:
            add(
                f"methods/{name}{method}",
                "methods",
                f"{method}, -tpattern alt-z, on {name}",
                Scenario(path, ("-tpattern", "alt-z", method)),
            )

    # Options, on float32 and int16 data.
    options = [
        ("ignore", ("-ignore", "3"), "-ignore 3: three leading points kept out of fit and shift"),
        ("tzero-0", ("-tzero", "0"), "-tzero 0: every slice moved to the start of the TR"),
        ("tzero-0.5", ("-tzero", "0.5"), "-tzero 0.5"),
        ("tzero-beyond-tr", ("-tzero", "5"), "-tzero 5: beyond the TR (allowed)"),
        ("slice-2", ("-slice", "2"), "-slice 2: align to slice 2's time"),
        ("slice-wins", ("-tzero", "0.5", "-slice", "1"), "-tzero and -slice: -slice wins"),
        ("rlt", ("-rlt",), "-rlt: no trend added back"),
        ("rlt+", ("-rlt+",), "-rlt+: only the intercept added back"),
        ("rlt-last-wins", ("-rlt", "-rlt+"), "-rlt then -rlt+: the last wins"),
        ("no-detrend", ("-heptic", "-no_detrend"), "-heptic -no_detrend: mean removed only"),
        (
            "no-detrend-fourier",
            ("-quintic", "-no_detrend", "-Fourier"),
            "-no_detrend then -Fourier (allowed, with a warning)",
        ),
        ("TR-seconds", ("-TR", "2.5s"), "-TR 2.5s"),
        ("TR-plain", ("-TR", "1.7"), "-TR 1.7 (no unit)"),
        ("TR-ms", ("-TR", "2000ms"), "-TR 2000ms: every time in milliseconds"),
        ("TR-msec", ("-TR", "2000msec", "-tzero", "500"), "-TR 2000msec -tzero 500"),
        ("verbose", ("-verbose",), "-verbose"),
        ("abbreviations", ("-cub", "-verb"), "-cub and -verb: AFNI's abbreviations"),
        ("wsinc-case", ("-WSINC5",), "-WSINC5: case-insensitive"),
        ("ignore-heptic", ("-ignore", "5", "-heptic"), "-ignore 5 with -heptic"),
        ("rlt-wsinc9", ("-rlt", "-wsinc9"), "-rlt with -wsinc9"),
        ("rlt+-linear", ("-rlt+", "-linear"), "-rlt+ with -linear"),
    ]
    for image, tag in [(F32, "float32"), (I16, "int16")]:
        for cid, args, desc in options:
            add(
                f"options/{tag}-{cid}",
                "options",
                f"{desc} ({tag})",
                Scenario(image, ("-tpattern", "alt+z", *args)),
            )

    # Every -tpattern form.
    for pattern in PATTERNS:
        add(
            f"tpattern/{pattern}",
            "tpattern",
            f"-tpattern {pattern}",
            Scenario(F32, ("-tpattern", pattern)),
        )
    files = {
        "file-tabs": ("0.0\t1.0\t0.5\t1.5\n", "one line, tab-separated (as fMRIPrep writes it)"),
        "file-lines": ("0\n1.5\n0.5\n1\n", "one value per line"),
        "file-matrix": ("0 9\n1.5 9\n0.5 9\n1 9\n", "two columns: read column by column"),
        "file-comments": ("# slice times\n0, 1.5, 0.25, 1.75 # end\n", "comments and commas"),
        "file-extra": ("0 1 0.5 1.5 0.25 1.25\n", "more values than slices"),
        "file-repeat": ("2@0 2@1\n", "N@value repetition"),
        "file-tr": ("0 2 1 0.5\n", "a value equal to the TR"),
    }
    for cid, (text, desc) in files.items():
        add(
            f"tpattern/{cid}",
            "tpattern",
            f"-tpattern @file: {desc}",
            Scenario(F32, ("-tpattern", "@{tmp}/st.1D"), files=(("st.1D", text),)),
        )
    add(
        "tpattern/inline-1D",
        "tpattern",
        "-tpattern '@1D: 0 1 0.5 1.5'",
        Scenario(F32, ("-tpattern", "@1D: 0 1 0.5 1.5")),
    )

    # Slice timing from the NIfTI header (inputs generated from the float32 file).
    header = [
        ("code1-seq-inc", "slice_code 1 (sequential increasing)"),
        ("code2-seq-dec", "slice_code 2: AFNI gives every slice time 0"),
        ("code3-alt-inc", "slice_code 3 (alternating increasing)"),
        ("code4-alt-dec", "slice_code 4 (alternating decreasing)"),
        ("code5-alt-inc2", "slice_code 5 (alternating increasing, from slice 1)"),
        ("code6-alt-dec2", "slice_code 6 (alternating decreasing, from nz-2)"),
        ("code3-partial", "slice_code 3 over slices 1..2 only"),
        ("code3-ms", "slice_code 3 with the TR and duration in milliseconds"),
        ("code3-beyond-tr", "times beyond the TR: a copy of the input, timing kept"),
        (
            "code2-beyond-tr",
            "slice_code 2: AFNI gives every slice time 0 and shifts nothing; corrected, the "
            "times exceed the TR and the output is a copy (AFNI-03)",
        ),
        ("code3-no-slice-dim", "no slice axis in dim_info: no timing, a copy"),
        ("code3-zero-duration", "slice_duration 0: no timing, a copy"),
    ]
    for name, desc in header:
        add(
            f"header-timing/{name}",
            "header-timing",
            f"no -tpattern, {desc}",
            Scenario(f"gen:{name}", ("-cubic",)),
        )
    add(
        "header-timing/tpattern-overrides",
        "header-timing",
        "-tpattern overrides the header's timing",
        Scenario("gen:code3-alt-inc", ("-tpattern", "seq-z")),
    )

    # AFNI's defects that larmorx corrects by default (D13; the comparison runs
    # LARMORX_UPSTREAM_DEFECTS=reproduce, the fix pass reports what the corrections change).
    for name, args, desc in [
        (
            "f32-slope-inter",
            ("-tpattern", "alt+z", "-linear"),
            "float32 with scl_slope 2 and scl_inter 10, shifted (AFNI-02)",
        ),
        ("f32-slope-inter", (), "float32 with scl_slope 2 and scl_inter 10, a copy (AFNI-02)"),
        ("metre", ("-tpattern", "alt+z"), "units in metres with an origin (AFNI-07)"),
        ("micron", (), "units in micrometres with an origin, a copy (AFNI-07)"),
    ]:
        add(
            f"defects/{name}{'-copy' if not args else ''}",
            "defects",
            desc,
            Scenario(f"gen:d13-{name}", args),
        )
    add(
        "defects/negative-slope-zero",
        "defects",
        "int16 with scl_slope -0.5 and -tpattern zero: nothing is shifted, so AFNI-01 changes nothing",
        Scenario(f"{S}/dtypes/int16-negative-slope.nii.gz", ("-tpattern", "zero")),
    )
    add(
        "defects/oblique-metre",
        "defects",
        "units in metres, an oblique sform with an origin (AFNI-07)",
        Scenario("gen:d13-oblique-metre", ("-tpattern", "alt+z")),
    )
    add(
        "defects/tiny-offsets-copy",
        "defects",
        "header slice times 0.5 ms apart beyond a 1 ms TR: a copy of the input that keeps "
        "them, all within 3 ms of 0 (AFNI-08)",
        Scenario("gen:d13-tiny-offsets", ()),
    )
    add(
        "geometry/slightly-oblique",
        "geometry",
        "an sform 0.006° from plumb (AFNI writes the plumb matrix below about 0.014°, AFNI-16)",
        Scenario("gen:slightly-oblique", ("-tpattern", "alt+z")),
    )
    add(
        "geometry/slightly-oblique-qform",
        "geometry",
        "a qform only, 0.006° from plumb: AFNI writes the plumb matrix with the header's voxel "
        "sizes as steps (AFNI-16)",
        Scenario("gen:slightly-oblique-qform", ("-tpattern", "alt+z")),
    )
    add(
        "geometry/slightly-oblique-metre",
        "geometry",
        "an sform in metres 0.006° from plumb: AFNI writes the plumb matrix, its steps in "
        "millimetres and its origin in metres (AFNI-16, AFNI-07)",
        Scenario("gen:d13-slightly-oblique-metre", ("-tpattern", "alt+z")),
    )
    add(
        "defects/oblique-metre-zero-origin",
        "defects",
        "units in metres, an oblique sform at the origin: AFNI keeps the whole sform in metres "
        "(AFNI-07)",
        Scenario("gen:d13-oblique-metre-origin0", ("-tpattern", "alt+z")),
    )
    add(
        "copy/short-slice-spacing",
        "copy",
        "header slice times 2 ms apart beyond a 5 ms TR: a copy, whose slice fields AFNI derives "
        "from the times within 3 ms (slice_start 1)",
        Scenario("gen:d13-short-slices", ()),
    )
    add(
        "copy/alt-two-slices",
        "copy",
        "header slice code 3 over slices 1..2, beyond the TR: a copy, written with slice code 1",
        Scenario("gen:d13-alt-two-slices", ()),
    )

    # Copies of the input.
    for path, desc in [
        (I16, "int16"),
        (f"{S}/dtypes/int16-slope.nii.gz", "int16 with a brick factor"),
        (f"{S}/dtypes/int16-slope-inter.nii.gz", "int16 scaled to float32"),
        (f"{S}/dtypes/float64.nii.gz", "float64 converted to float32"),
        (f"{S}/dtypes/float32-nonfinite.nii.gz", "non-finite floats zeroed"),
    ]:
        name = path.split("/")[-1].removesuffix(".nii.gz")
        add(
            f"copy/no-timing-{name}",
            "copy",
            f"no slice timing anywhere: the output is a copy of the input ({desc})",
            Scenario(path, ()),
        )
    add(
        "copy/3d-input",
        "copy",
        "a 3D image: one value per voxel, written back as 3D",
        Scenario("synthetic/resampling/images/axial-1mm.nii.gz", ("-tpattern", "alt+z")),
    )

    # Errors both must raise.
    errors = [
        ("unknown-option", ("-bogus",), "an unknown option"),
        ("no-detrend-fourier", ("-no_detrend",), "-no_detrend while the method is Fourier"),
        ("rlt-no-detrend", ("-linear", "-rlt", "-no_detrend"), "-rlt with -no_detrend"),
        ("unknown-pattern", ("-tpattern", "ALT+Z"), "pattern names are case-sensitive"),
        ("slice-too-large", ("-tpattern", "alt+z", "-slice", "4"), "-slice beyond the last"),
        ("ignore-too-large", ("-tpattern", "alt+z", "-ignore", "96"), "-ignore > nt - 5"),
        ("negative-tzero", ("-tpattern", "alt+z", "-tzero", "-1"), "-tzero < 0"),
        ("negative-ignore", ("-tpattern", "alt+z", "-ignore", "-1"), "-ignore < 0"),
        ("zero-TR", ("-tpattern", "alt+z", "-TR", "0"), "-TR 0"),
        ("bad-TR", ("-tpattern", "alt+z", "-TR", "abc"), "-TR abc"),
        ("missing-file", ("-tpattern", "@{tmp}/none.1D"), "a missing -tpattern file"),
    ]
    for cid, args, desc in errors:
        add(f"errors/{cid}", "errors", desc, Scenario(F32, args, expect="error"))
    for cid, text, desc in [
        ("file-too-short", "0 1 0.5\n", "a -tpattern file with fewer values than slices"),
        ("file-beyond-tr", "0 1 0.5 2.5\n", "a -tpattern value beyond the TR"),
        ("file-negative", "0 1 -0.5 1.5\n", "a negative -tpattern value"),
        ("file-text", "0 1 abc 1.5\n", "a -tpattern file with text"),
    ]:
        add(
            f"errors/{cid}",
            "errors",
            desc,
            Scenario(F32, ("-tpattern", "@{tmp}/st.1D"), expect="error", files=(("st.1D", text),)),
        )
    add(
        "errors/missing-dataset",
        "errors",
        "an input file that does not exist",
        Scenario("missing", ("-tpattern", "alt+z"), expect="error"),
    )
    add(
        "errors/existing-output",
        "errors",
        "an output that exists already (AFNI exits 0 without writing; larmorx exits 1, AFNI-11)",
        Scenario(F32, ("-tpattern", "alt+z"), expect="error", files=(("out.nii", "taken"),)),
    )
    add(
        "errors/prefix-with-blank",
        "errors",
        "a -prefix with a blank (AFNI refuses it; larmorx writes it, AFNI-12)",
        Scenario(F32, ("-tpattern", "alt+z"), expect="error", output="out put.nii"),
    )

    # AFNI's global options and environment variables (specs/3dTshift.md §7.1). Each program
    # runs in a directory of its own, with the same files and environment, and their messages
    # are compared too.
    def separate(cid: str, desc: str, args: tuple[str, ...] = (), image: str = F32, **kw) -> None:
        category = cid.split("/")[0]
        add(cid, category, desc, Scenario(image, args, separate=True, **kw))

    taken = (("out.nii", "taken"),)
    blank = "out put.nii"
    tz = ("-tpattern", "alt+z")
    text_1d = (("st.1D", "0 1 abc 1.5\n"),)
    for cid, desc, args, kw in [
        (
            "overwrite-after-dataset",
            "-overwrite after the dataset replaces an existing output",
            (*tz, "-prefix", "out.nii", "{input}", "-overwrite"),
            {"files": taken},
        ),
        (
            "overwrite-as-option-value",
            "-overwrite where -tpattern's value stands: taken out first, -tpattern reads alt+z",
            ("-tpattern", "-overwrite", "alt+z", "-prefix", "out.nii", "{input}"),
            {"files": taken},
        ),
        (
            "overwrite-copy",
            "-overwrite on a copy of the input (no slice timing)",
            ("-overwrite",),
            {"files": taken},
        ),
        (
            "overwrite-no-existing-file",
            "-overwrite with no output to replace",
            ("-overwrite", *tz),
            {},
        ),
        (
            "overwrite-then-D",
            "-overwrite, then -DAFNI_DECONFLICT=YES: the later one wins (out_AA1.nii)",
            ("-overwrite", "-DAFNI_DECONFLICT=YES", *tz),
            {"files": taken, "written": "out_AA1.nii"},
        ),
        (
            "D-deconflict-yes",
            "-DAFNI_DECONFLICT=YES: an existing output makes the run write out_AA1.nii",
            ("-DAFNI_DECONFLICT=YES", *tz),
            {"files": taken, "written": "out_AA1.nii"},
        ),
        (
            "D-deconflict-no",
            "-DAFNI_DECONFLICT=NO: an existing output stops the shift before the work (exit 1)",
            ("-DAFNI_DECONFLICT=NO", *tz),
            {"files": taken, "expect": "error"},
        ),
        (
            "D-wins-over-environment",
            "-DAFNI_DECONFLICT=OVERWRITE over AFNI_DECONFLICT=YES in the environment",
            ("-DAFNI_DECONFLICT=OVERWRITE", *tz),
            {"files": taken, "env": (("AFNI_DECONFLICT", "YES"),)},
        ),
        (
            "D-arbitrary-filenames",
            "-DAFNI_ALLOW_ARBITRARY_FILENAMES=YES: a -prefix with a blank is written",
            ("-DAFNI_ALLOW_ARBITRARY_FILENAMES=YES", *tz),
            {"output": blank},
        ),
        (
            "D-after-dataset",
            "-D after the dataset applies too",
            (*tz, "-prefix", blank, "{input}", "-DAFNI_ALLOW_ARBITRARY_FILENAMES=YES"),
            {"output": blank},
        ),
        (
            "D-as-option-value",
            "-D where -tpattern's value stands: taken out first",
            (
                "-tpattern",
                "-DAFNI_ALLOW_ARBITRARY_FILENAMES=YES",
                "alt+z",
                "-prefix",
                blank,
                "{input}",
            ),
            {"output": blank},
        ),
        (
            "D-value-with-blank",
            "-DAFNI_ALLOW_ARBITRARY_FILENAMES=Y ES: AFNI sets 'Y=ES', still YES",
            ("-DAFNI_ALLOW_ARBITRARY_FILENAMES=Y ES", *tz),
            {"output": blank},
        ),
        (
            "D-blank-before-value",
            "-DAFNI_ALLOW_ARBITRARY_FILENAMES= YES: AFNI sets '=YES', not YES: refused",
            ("-DAFNI_ALLOW_ARBITRARY_FILENAMES= YES", *tz),
            {"output": blank, "expect": "error"},
        ),
        (
            "D-empty-value",
            "-DAFNI_ALLOW_ARBITRARY_FILENAMES= (no value) sets nothing: refused",
            ("-DAFNI_ALLOW_ARBITRARY_FILENAMES=", *tz),
            {"output": blank, "expect": "error"},
        ),
        (
            "D-empty-name",
            "-D=YES (no name) sets nothing",
            ("-D=YES", *tz),
            {"output": blank, "expect": "error"},
        ),
        (
            "D-without-equals",
            "-DAFNI_ALLOW_ARBITRARY_FILENAMES (no '='): an unknown option",
            ("-DAFNI_ALLOW_ARBITRARY_FILENAMES", *tz),
            {"expect": "error"},
        ),
        ("D-alone", "-D alone: an unknown option", ("-D", *tz), {"expect": "error"}),
        (
            "D-lowercase",
            "-d...=YES: not a global option, an unknown option",
            ("-dAFNI_ALLOW_ARBITRARY_FILENAMES=YES", *tz),
            {"expect": "error"},
        ),
        (
            "D-message-prefix-with-blank",
            "-DAFNI_MESSAGE_PREFIX=a b: AFNI's messages start with 'a=b::' (AFNI-22)",
            ("-DAFNI_MESSAGE_PREFIX=a b",),
            {},
        ),
        (
            "D-unknown-variable",
            "-D of a variable 3dTshift does not read: no effect",
            ("-DAFNI_NOT_A_VARIABLE=1", *tz),
            {},
        ),
        (
            "ok-1D-text",
            "-ok_1D_text: text in the -tpattern file is read as 0",
            ("-ok_1D_text", "-tpattern", "@st.1D"),
            {"files": text_1d},
        ),
        ("skip-afnirc", "-skip_afnirc: accepted", ("-skip_afnirc", *tz), {}),
    ]:
        separate(f"global/{cid}", desc, args, **kw)

    i16_si = f"{S}/dtypes/int16-slope-inter.nii.gz"
    f64 = f"{S}/dtypes/float64.nii.gz"
    for cid, desc, image, args, kw in [
        (
            "deconflict-overwrite",
            "AFNI_DECONFLICT=OVERWRITE: an existing output is replaced, with AFNI's warning",
            F32,
            tz,
            {"files": taken, "env": (("AFNI_DECONFLICT", "OVERWRITE"),)},
        ),
        (
            "deconflict-yes",
            "AFNI_DECONFLICT=YES: the output goes to out_AA1.nii",
            F32,
            tz,
            {"files": taken, "env": (("AFNI_DECONFLICT", "YES"),), "written": "out_AA1.nii"},
        ),
        (
            "deconflict-no-copy",
            "AFNI_DECONFLICT=NO on a copy: AFNI finds the output after the work, exit 0 (AFNI-11)",
            F32,
            (),
            {"files": taken, "env": (("AFNI_DECONFLICT", "NO"),), "expect": "error"},
        ),
        (
            "arbitrary-filenames",
            "AFNI_ALLOW_ARBITRARY_FILENAMES=YES: a -prefix with a blank is written",
            F32,
            tz,
            {"output": blank, "env": (("AFNI_ALLOW_ARBITRARY_FILENAMES", "YES"),)},
        ),
        (
            "input-with-blank",
            "an input in a directory with a blank: AFNI reads a list of datasets and fails (AFNI-18)",
            "gen:blank-dir",
            tz,
            {"expect": "error"},
        ),
        (
            "path-spaces-ok",
            "AFNI_PATH_SPACES_OK=YES: an input in a directory with a blank is one file",
            "gen:blank-dir",
            tz,
            {"env": (("AFNI_PATH_SPACES_OK", "YES"),)},
        ),
        (
            "nifti-priority-Q",
            "AFNI_NIFTI_PRIORITY=Q: the qform rather than the sform (they differ)",
            "gen:sq",
            tz,
            {"env": (("AFNI_NIFTI_PRIORITY", "Q"),)},
        ),
        (
            "nifti-priority-illegal",
            "AFNI_NIFTI_PRIORITY=X: AFNI's warning, the sform",
            "gen:sq",
            tz,
            {"env": (("AFNI_NIFTI_PRIORITY", "X"),)},
        ),
        (
            "nifti-scale-no",
            "AFNI_NIFTI_SCALE=NO: int16 with a slope and an intercept stays int16 (slope kept)",
            i16_si,
            tz,
            {"env": (("AFNI_NIFTI_SCALE", "NO"),)},
        ),
        (
            "nifti-scale-no-copy",
            "AFNI_NIFTI_SCALE=NO on a copy of the input: the intercept dropped (AFNI-23)",
            i16_si,
            (),
            {"env": (("AFNI_NIFTI_SCALE", "NO"),)},
        ),
        (
            "nifti-scale-no-rlt",
            "AFNI_NIFTI_SCALE=NO with -rlt: the trend takes the intercept away, nothing to correct",
            i16_si,
            (*tz, "-rlt"),
            {"env": (("AFNI_NIFTI_SCALE", "NO"),)},
        ),
        (
            "nifti-scale-no-rlt-ignore",
            "AFNI_NIFTI_SCALE=NO with -rlt -ignore 2: the ignored volumes lose the intercept "
            "(AFNI-23; corrected, read as without the variable)",
            i16_si,
            (*tz, "-rlt", "-ignore", "2"),
            {"env": (("AFNI_NIFTI_SCALE", "NO"),)},
        ),
        (
            "type-warn-default",
            "AFNI_NIFTI_TYPE_WARN unset: AFNI's conversion message, with the muting lines",
            f64,
            tz,
            {"env": (("AFNI_NIFTI_TYPE_WARN", None),)},
        ),
        (
            "type-warn-yes",
            "AFNI_NIFTI_TYPE_WARN=YES: the conversion message alone",
            f64,
            tz,
            {"env": (("AFNI_NIFTI_TYPE_WARN", "YES"),)},
        ),
        (
            "nifti-view-tlrc",
            "AFNI_NIFTI_VIEW=TLRC: sform code 2 is the tlrc view, so code 3",
            "gen:code2",
            tz,
            {"env": (("AFNI_NIFTI_VIEW", "TLRC"),)},
        ),
        (
            "default-std-space",
            "AFNI_DEFAULT_STD_SPACE=MNI: sform code 5 gives code 4",
            "gen:code5",
            tz,
            {"env": (("AFNI_DEFAULT_STD_SPACE", "MNI"),)},
        ),
        (
            "sform-code-7",
            "sform code 7: AFNI's warning; AFNI_NIFTI_VIEW=tlrc makes it code 3",
            "gen:code7",
            tz,
            {"env": (("AFNI_NIFTI_VIEW", "tlrc"),)},
        ),
        (
            "write-type-2",
            "AFNI_NIFTI_WRITE_TYPE=2: a NIfTI-2 output",
            F32,
            tz,
            {"env": (("AFNI_NIFTI_WRITE_TYPE", "2"),)},
        ),
        (
            "allow-milliseconds",
            "AFNI_ALLOW_MILLISECONDS=YES with -TR 2000ms: toffset written in ms (AFNI-19)",
            F32,
            (*tz, "-TR", "2000ms"),
            {"env": (("AFNI_ALLOW_MILLISECONDS", "YES"),)},
        ),
        (
            "1D-zero-text",
            "AFNI_1D_ZERO_TEXT=YES: text in the -tpattern file is read as 0",
            F32,
            ("-tpattern", "@st.1D"),
            {"files": text_1d, "env": (("AFNI_1D_ZERO_TEXT", "YES"),)},
        ),
        (
            "1D-text-error",
            "text in the -tpattern file: AFNI's parse error",
            F32,
            ("-tpattern", "@st.1D"),
            {"files": text_1d, "expect": "error"},
        ),
        (
            "use-fgets",
            "AFNI_USE_FGETS=YES: a file with CR line ends is one row, of 8 values (9 > TR)",
            F32,
            ("-tpattern", "@st.1D"),
            {
                "files": (("st.1D", "0 9\r1.5 9\r0.5 9\r1 9\r"),),
                "env": (("AFNI_USE_FGETS", "YES"),),
                "expect": "error",
            },
        ),
        (
            "cr-line-ends",
            "a file with CR line ends: four rows of two columns, read column by column",
            F32,
            ("-tpattern", "@st.1D"),
            {"files": (("st.1D", "0 9\r1.5 9\r0.5 9\r1 9\r"),)},
        ),
        (
            "realpath",
            "a conversion message names the input by its real path ('..' resolved)",
            "gen:dotdot-float64",
            tz,
            {"env": (("AFNI_NIFTI_TYPE_WARN", "YES"),)},
        ),
        (
            "norealpath",
            "AFNI_NOREALPATH=YES: the message names the input as given",
            "gen:dotdot-float64",
            tz,
            {"env": (("AFNI_NIFTI_TYPE_WARN", "YES"), ("AFNI_NOREALPATH", "YES"))},
        ),
        ("oblique-warning", "an oblique input: AFNI's obliquity report", "gen:oblique", tz, {}),
        (
            "no-oblique-warning",
            "AFNI_NO_OBLIQUE_WARNING=YES: no obliquity report",
            "gen:oblique",
            tz,
            {"env": (("AFNI_NO_OBLIQUE_WARNING", "YES"),)},
        ),
        (
            "message-prefix",
            "AFNI_MESSAGE_PREFIX: on every labelled message",
            F32,
            ("-verbose",),
            {"env": (("AFNI_MESSAGE_PREFIX", "run1"),)},
        ),
        (
            "message-colorize-no",
            "AFNI_MESSAGE_COLORIZE=NO: labels without terminal escape codes",
            F32,
            (),
            {"env": (("AFNI_MESSAGE_COLORIZE", "NO"),)},
        ),
        (
            "echo-commandline",
            "AFNI_ECHO_COMMANDLINE=YES (and a log file): the command line is echoed",
            F32,
            tz,
            {"env": (("AFNI_ECHO_COMMANDLINE", "YES"), ("AFNI_DONT_LOGFILE", None))},
        ),
        (
            "quiet-startup",
            "AFNI_QUIET_STARTUP=YES: no version banner (larmorx prints none)",
            F32,
            tz,
            {"env": (("AFNI_QUIET_STARTUP", "YES"),)},
        ),
        (
            "nifti-noext",
            "AFNI_NIFTI_NOEXT=YES: AFNI writes no header extension, as larmorx: the whole file",
            F32,
            tz,
            {"env": (("AFNI_NIFTI_NOEXT", "YES"),), "whole_file": True},
        ),
        (
            "one-row-dataset",
            "a dataset of one row: AFNI writes a 1D text file under the NIfTI name (AFNI-20)",
            "gen:one-row",
            tz,
            {},
        ),
        (
            "write-1D-as-prefix",
            "AFNI_WRITE_1D_AS_PREFIX=YES: a dataset of one row is written as NIfTI",
            "gen:one-row",
            tz,
            {"env": (("AFNI_WRITE_1D_AS_PREFIX", "YES"),)},
        ),
    ]:
        separate(f"environment/{cid}", desc, args, image, **kw)
    # The -D form of the variables that change the output.
    for cid, image, setting in [
        ("nifti-priority-Q", "gen:sq", "AFNI_NIFTI_PRIORITY=Q"),
        ("nifti-view-tlrc", "gen:code2", "AFNI_NIFTI_VIEW=TLRC"),
        ("write-type-2", F32, "AFNI_NIFTI_WRITE_TYPE=2"),
        ("nifti-scale-no", i16_si, "AFNI_NIFTI_SCALE=NO"),
    ]:
        separate(
            f"global/D-{cid}",
            f"-D{setting}: as the environment variable",
            (f"-D{setting}", *tz),
            image,
        )

    # Deliberate differences.
    add(
        "divergence/afni-format-output",
        "divergence",
        "-prefix without .nii: AFNI writes its own BRIK/HEAD format",
        Scenario(
            F32,
            ("-tpattern", "alt+z"),
            expect="divergence",
            suffix="",
            reason="larmorx writes NIfTI only; it rejects a -prefix that does not end in "
            ".nii or .nii.gz, where AFNI writes its own format.",
        ),
    )
    add(
        "divergence/voxshift",
        "divergence",
        "-voxshift: per-voxel shifts from a dataset",
        Scenario(
            F32,
            ("-voxshift", "gen:voxshift"),
            expect="divergence",
            reason="-voxshift is not supported by larmorx (fMRIPrep does not use it).",
        ),
    )

    # Real BOLD runs, as fMRIPrep calls 3dTshift.
    o = "openneuro"
    real = [
        ("ds001600-acq-v4", f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-v4_bold", None, "smoke"),
        (
            "ds001600-acq-v1",
            f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-v1_bold",
            None,
            "standard",
        ),
        (
            "ds001600-acq-v2",
            f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-v2_bold",
            None,
            "standard",
        ),
        (
            "ds001600-acq-PA",
            f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-PA_bold",
            None,
            "standard",
        ),
        *(
            (
                f"ds000210-echo-{e}",
                f"{o}/ds000210/sub-02/func/sub-02_task-cuedSGT_run-01_echo-{e}_bold",
                f"{o}/ds000210/task-cuedSGT_echo-{e}_bold.json",
                "standard",
            )
            for e in (1, 2, 3)
        ),
        ("ds006736", f"{o}/ds006736/sub-004/func/sub-004_task-freeRecall_bold", None, "standard"),
        (
            "ds006010-uint16",
            f"{o}/ds006010/sub-206/func/sub-206_task-category_run-01_bold",
            None,
            "standard",
        ),
        (
            "ds003345-ms-header",
            f"{o}/ds003345/sub-22973/func/sub-22973_task-PenaltyKik_run-02_bold",
            None,
            "standard",
        ),
        (
            "ds005454-mb4-96-slices",
            f"{o}/ds005454/sub-16/func/sub-16_task-rest_bold",
            None,
            "standard",
        ),
    ]
    for cid, stem, sidecar, tier in real:
        add(
            f"real/{cid}",
            "real",
            f"fMRIPrep's call (-ignore 0 -tzero -TR -tpattern @file) on {stem.split('/')[-1]}",
            Scenario(
                f"{stem}.nii.gz",
                ("-ignore", "0"),
                tier=tier,
                sidecar=sidecar or f"{stem}.json",
                suffix=".nii.gz",
            ),
        )
    d3345 = f"{o}/ds003345/sub-22973/func/sub-22973_task-PenaltyKik_run-02_bold"
    for method in METHODS[1:]:
        add(
            f"real/ds003345{method}",
            "real",
            f"fMRIPrep's call with {method}",
            Scenario(
                f"{d3345}.nii.gz",
                ("-ignore", "0", method),
                tier="standard",
                sidecar=f"{d3345}.json",
                suffix=".nii.gz",
            ),
        )
    add(
        "real/ds003345-ignore-4",
        "real",
        "fMRIPrep's call with -ignore 4 (non-steady-state volumes)",
        Scenario(
            f"{d3345}.nii.gz",
            ("-ignore", "4"),
            tier="standard",
            sidecar=f"{d3345}.json",
            suffix=".nii.gz",
        ),
    )
    d210 = f"{o}/ds000210/sub-02/func/sub-02_task-cuedSGT_run-01_echo-1_bold.nii.gz"
    add(
        "real/ds000210-header-timing",
        "real",
        "no -tpattern: the slice timing in the header (slice_code 3, ALT_INC)",
        Scenario(d210, (), tier="standard", suffix=".nii.gz"),
    )
    add(
        "real/ds001600-header-zero-duration",
        "real",
        "no -tpattern: slice_code 5 with slice_duration 0, so a copy of the input",
        Scenario(f"{o}/ds001600/sub-1/func/sub-1_task-rest_acq-v4_bold.nii.gz", ()),
    )
    return out


_TIERS = ("smoke", "standard", "full")


def cases(tier: str) -> list[Case]:
    rank = _TIERS.index(tier)
    return [
        Case(cid, category, desc, scenario)
        for cid, category, desc, scenario in _scenarios()
        if _TIERS.index(scenario.tier) <= rank
    ]


# ------------------------------------------------------------------------------------------------
# Generated inputs


@functools.cache
def _generated_dir() -> Path:
    return Path(tempfile.mkdtemp(prefix="lx-tshift-gen-"))


def _generated(name: str) -> Path:
    """A copy of the float32 file (nz = 4, TR = 2 s) with slice timing in its header, or
    (``voxshift``) a 3D map of 0.25-TR shifts on its grid."""
    import nibabel as nib

    path = _generated_dir() / f"{name}.nii"
    if path.exists():
        return path
    src = nib.load(td.get(F32))
    data = np.asanyarray(src.dataobj)
    if name == "voxshift":
        nib.Nifti1Image(np.full(data.shape[:3], 0.25, np.float32), src.affine).to_filename(path)
        return path
    if name in ("slightly-oblique", "slightly-oblique-qform"):
        affine = src.affine.copy()
        affine[2, 0] = 1e-4 * affine[0, 0]  # 1e-4 rad (0.006°) about y
        affine[0, 2] = -1e-4 * affine[2, 2]
        affine[:3, 3] = (-100.0, 50.0, 20.0)
        img = nib.Nifti1Image(np.asarray(data, np.float32), affine)
        if name.endswith("qform"):  # the qform only
            img.header.set_sform(affine, 0)
        img.header["pixdim"][4] = src.header["pixdim"][4]
        img.to_filename(path)
        return path
    if name.startswith("d13-"):
        return _generated_d13(name, data, src, path)
    if name in ("sq", "code2", "code5", "code7", "oblique", "one-row"):
        return _generated_env(name, data, src, path)
    if name == "blank-dir":  # an input in a directory whose name has a blank
        target = _generated_dir() / "with blank" / "in.nii"
        target.parent.mkdir(exist_ok=True)
        if not target.exists():
            nib.Nifti1Image(np.asarray(data, np.float32), src.affine, src.header).to_filename(
                target
            )
        return target
    if name == "dotdot-float64":  # a float64 input named through 'sub/..'
        target = _generated_dir() / "float64.nii"
        if not target.exists():
            nib.load(td.get(f"{S}/dtypes/float64.nii.gz")).to_filename(target)
        (_generated_dir() / "sub").mkdir(exist_ok=True)
        return _generated_dir() / "sub" / ".." / "float64.nii"
    img = nib.Nifti1Image(data, src.affine, src.header)
    h = img.header
    code = int(name[4])
    h["slice_code"] = code
    h["slice_start"], h["slice_end"] = 0, 3
    h["slice_duration"] = 0.5
    h.set_dim_info(slice=2)
    if name.endswith("partial"):
        h["slice_start"], h["slice_end"] = 1, 2
    elif name.endswith("ms"):
        h.set_xyzt_units("mm", "msec")
        h["pixdim"][4] = 2000.0
        h["slice_duration"] = 500.0
    elif name.endswith("beyond-tr"):
        h["slice_duration"] = 0.7
    elif name.endswith("no-slice-dim"):
        h["dim_info"] = 0
    elif name.endswith("zero-duration"):
        h["slice_duration"] = 0.0
    img.to_filename(path)
    return path


def _generated_d13(name: str, data: np.ndarray, src, path: Path) -> Path:  # type: ignore[no-untyped-def]
    """Inputs for the D13 cases, from the float32 file: float32 with a slope and an
    intercept (written raw), or the units changed to metres or micrometres with the affine
    scaled to them."""
    import nibabel as nib

    if name == "d13-f32-slope-inter":
        hdr = nib.Nifti1Header()
        hdr.set_data_dtype(np.float32)
        hdr.set_data_shape(data.shape)
        hdr.set_sform(src.affine, 1)
        hdr.set_qform(src.affine, 1)
        hdr.set_xyzt_units("mm", "sec")
        hdr["pixdim"][4] = src.header["pixdim"][4]
        hdr["vox_offset"] = 352
        hdr.set_slope_inter(2.0, 10.0)
        with open(path, "wb") as f:
            f.write(hdr.binaryblock + b"\0" * 4)
            f.write(np.asfortranarray(data, dtype=np.float32).tobytes(order="F"))
        return path
    timing = {  # header slice timing: code, start, end, duration, TR
        "d13-tiny-offsets": (1, 0, 3, 0.0005, 0.001),
        "d13-short-slices": (1, 0, 3, 0.002, 0.005),
        "d13-alt-two-slices": (3, 1, 2, 2.5, 2.0),
    }
    if name in timing:
        img = nib.Nifti1Image(data, src.affine, src.header)
        h = img.header
        h["slice_code"], h["slice_start"], h["slice_end"], h["slice_duration"], tr = timing[name]
        h.set_dim_info(slice=2)
        h["pixdim"][4] = tr
        img.to_filename(path)
        return path
    scale = 1000.0 if name == "d13-micron" else 0.001
    affine = src.affine.copy()
    if name.startswith("d13-oblique-metre"):  # a rotation about y with cos 0.8, sin 0.6
        rotation = np.array([[0.8, 0.0, -0.6], [0.0, 1.0, 0.0], [0.6, 0.0, 0.8]])
        affine[:3, :3] = rotation @ affine[:3, :3]
    if name == "d13-slightly-oblique-metre":  # 1e-4 rad (0.006°) about y
        affine[2, 0] = 1e-4 * affine[0, 0]
        affine[0, 2] = -1e-4 * affine[2, 2]
    # an origin, in mm before the change of units
    affine[:3, 3] = (0.0, 0.0, 0.0) if name.endswith("origin0") else (-100.0, 50.0, 20.0)
    affine[:3] *= scale
    img = nib.Nifti1Image(np.asarray(data, np.float32), affine)
    img.header.set_xyzt_units("meter" if scale < 1 else "micron", "sec")
    img.header["pixdim"][4] = src.header["pixdim"][4]
    img.to_filename(path)
    return path


def _generated_env(name: str, data: np.ndarray, src, path: Path) -> Path:  # type: ignore[no-untyped-def]
    """Inputs for AFNI's environment variables, from the float32 file: a qform and an sform of
    different handedness (``sq``), sform codes 2, 5 and 7 (``code<n>``), an oblique sform in
    millimetres, and a dataset of one row (``one-row``)."""
    import nibabel as nib

    values = np.asarray(data, np.float32)
    affine = src.affine.copy()
    if name == "one-row":
        values = values[:, :1, :1, :]
    if name == "oblique":  # a rotation about y with cos 0.8, sin 0.6
        rotation = np.array([[0.8, 0.0, -0.6], [0.0, 1.0, 0.0], [0.6, 0.0, 0.8]])
        affine[:3, :3] = rotation @ affine[:3, :3]
    img = nib.Nifti1Image(values, affine)
    h = img.header
    if name == "sq":  # the qform flips x and moves the origin: the two differ
        q = affine.copy()
        q[0, :3] = -q[0, :3]
        q[:3, 3] += (10.0, 20.0, 30.0)
        h.set_qform(q, 1)
        h.set_sform(affine, 1)
    elif name.startswith("code"):
        code = int(name[4:])
        h.set_qform(affine, 0)
        h.set_sform(affine, min(code, 4) if code > 5 else code)
        h["sform_code"] = code  # nibabel does not take code 7
    else:
        h.set_qform(affine, 1)
        h.set_sform(affine, 1)
    h["pixdim"][4] = src.header["pixdim"][4]
    h.set_xyzt_units("mm", "sec")
    img.to_filename(path)
    return path


def _image_path(image: str) -> Path:
    if image.startswith("gen:"):
        return _generated(image.removeprefix("gen:"))
    if image == "missing":
        return _generated_dir() / "does-not-exist.nii"
    return td.get(image)


# ------------------------------------------------------------------------------------------------
# Running


def _fmriprep_args(sidecar: str, tmp: Path) -> list[str]:
    """``-tzero``, ``-TR`` and ``-tpattern`` as fMRIPrep (nipype's TShift) builds them."""
    meta = json.loads(td.get(sidecar).read_text(encoding="utf-8"))
    times = list(meta["SliceTiming"])
    if str(meta.get("SliceEncodingDirection", "")).endswith("-"):
        times = times[::-1]
    t0 = round(min(times) + 0.5 * (max(times) - min(times)), 3)
    pattern = tmp / "slice_timing.1D"
    pattern.write_text("\t".join(str(float(t)) for t in times) + "\n", encoding="utf-8")
    return ["-tzero", str(t0), "-TR", f"{meta['RepetitionTime']}s", "-tpattern", f"@{pattern}"]


def _args(s: Scenario, tmp: Path) -> list[str]:
    args = [
        str(_generated(a.removeprefix("gen:")))
        if a.startswith("gen:")
        else a.replace("{tmp}", str(tmp))
        for a in s.args
    ]
    if s.sidecar:
        args += _fmriprep_args(s.sidecar, tmp)
    return args


@functools.cache
def _home() -> Path:
    """An empty home directory for both programs: AFNI reads no ``.afnirc`` from it."""
    home = _generated_dir() / "home"
    home.mkdir(exist_ok=True)
    return home


def _environment(s: Scenario | None = None) -> dict[str, str]:
    """The environment both programs run in: this process's without AFNI's variables, with
    ``HOME`` an empty directory (no ``.afnirc``), AFNI's log file and its conversion messages
    off, then the scenario's variables (``None`` removes one)."""
    env = {k: v for k, v in os.environ.items() if not k.upper().startswith(("AFNI_", "NIML_"))}
    env.update(HOME=str(_home()), AFNI_DONT_LOGFILE="YES", AFNI_NIFTI_TYPE_WARN="NO")
    for name, value in s.env if s else ():
        if value is None:
            env.pop(name, None)
        else:
            env[name] = value
    return env


@contextlib.contextmanager
def _process_environment(env: dict[str, str], cwd: Path | None = None) -> Iterator[None]:
    """This process's environment (and working directory) for an in-process larmorx run, which
    starts the replica's program with them; restored after."""
    saved, saved_cwd = dict(os.environ), os.getcwd()
    os.environ.clear()
    os.environ.update(env)
    if cwd is not None:
        os.chdir(cwd)
    try:
        yield
    finally:
        os.environ.clear()
        os.environ.update(saved)
        os.chdir(saved_cwd)


def _run_afni_streams(
    args: list[str], env: dict[str, str], cwd: Path | None = None
) -> tuple[int, str, str]:
    p = subprocess.run(
        [afni_binary(), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        env=env,
        cwd=cwd,
    )
    return p.returncode, p.stdout, p.stderr


def _run_afni(args: list[str]) -> tuple[int, str]:
    code, out, err = _run_afni_streams(args, _environment())
    return code, (out + err).strip()


def _run_larmorx_streams(
    args: list[str],
    implementation: str,
    mode: str,
    env: dict[str, str],
    cwd: Path | None = None,
) -> tuple[int, str, str]:
    """``larmorx afni 3dTshift`` in process, with the implementation forced: the original, or
    the replica's program, which larmorx runs as a separate process. ``mode`` is D13's
    ``LARMORX_UPSTREAM_DEFECTS``: ``reproduce`` for the comparison with AFNI."""
    from larmorx.cli import run

    out, err = io.StringIO(), io.StringIO()
    with (
        _process_environment(env, cwd),
        defects.mode(mode),
        contextlib.redirect_stderr(err),
        contextlib.redirect_stdout(out),
    ):
        code = run(["larmorx", "afni", "3dTshift", *args], implementation=implementation)
    return code, out.getvalue(), err.getvalue()


def _run_larmorx(
    args: list[str], implementation: str = "original", mode: str = defects.REPRODUCE
) -> tuple[int, str]:
    code, _, err = _run_larmorx_streams(args, implementation, mode, _environment())
    return code, err.strip()


@contextlib.contextmanager
def _replica_program(implementation: str):
    """For the replica, larmorx's dispatch finds the program under test through
    ``LARMORX_GPL_BIN`` (:func:`replica_binary`)."""
    if implementation != "replica":
        yield
        return
    previous = os.environ.get("LARMORX_GPL_BIN")
    os.environ["LARMORX_GPL_BIN"] = str(replica_binary())
    try:
        yield
    finally:
        if previous is None:
            os.environ.pop("LARMORX_GPL_BIN", None)
        else:
            os.environ["LARMORX_GPL_BIN"] = previous


REPLICA = "crates-gpl/target/{profile}/larmorx-gpl{exe}"

#: AFNI-20, kept by design in both implementations: AFNI's 1D text file holds a random
#: identifier, so neither reproduces it.
_ONE_ROW = (
    frozenset({"AFNI's output is NIfTI"}),
    "AFNI writes a dataset of one row as a 1D text file under the NIfTI name, with a random "
    "identifier (AFNI-20, unless AFNI_WRITE_1D_AS_PREFIX is YES); larmorx writes NIfTI in both "
    "modes, as AFNI does with that variable set (environment/write-1D-as-prefix)",
)

#: Cases where an implementation is known to differ from AFNI in ``reproduce`` mode, or to name
#: the wrong defects in its default (the fix pass): the checks that differ, and why. Once the
#: implementation matches, the case passes as usual. Both implementations have one, AFNI-20.
#: The replica's two unit cases were expected divergences until 2026-10-12. The clean-room
#: original had 10 more and the 59 cases of AFNI's global options and environment variables
#: until 2026-10-12 (completing 3dTshift, part 2b), when it implemented spec §6.1-6.5 and §7.1;
#: the old list is kept in ``_ORIGINAL_BEFORE_PART_2B`` for the record.
DIVERGENCES: dict[str, dict[str, tuple[frozenset[str], str]]] = {
    "replica": {"environment/one-row-dataset": _ONE_ROW},
    "original": {"environment/one-row-dataset": _ONE_ROW},
}

#: The clean-room original's expected divergences before part 2b (2026-10-12), which it has met
#: since: behaviours ``specs/3dTshift.md`` did not describe until 2026-10-12 (§6.1-6.4).
_ORIGINAL_BEFORE_PART_2B: dict[str, tuple[frozenset[str], str]] = {
    "defects/oblique-metre": (
        frozenset({"geometry (sform)"}),
        "AFNI writes an oblique sform in metres as it is (only the voxel sizes are "
        "converted); the original converts its linear part to millimetres in reproduce mode "
        "too (AFNI-07). Its default, which converts the whole matrix, is the replica's",
    ),
    "defects/tiny-offsets-copy": (
        frozenset({"header fields (scaling, timing, slice fields, units, xform codes)"}),
        "AFNI writes slice_end 0 (AFNI-08) and derives the slice code and duration from the "
        "kept slice times, all within 3 ms of 0: none; the original keeps the header's code "
        "1, duration 0.0005 and slice_end 3, and has no switch for AFNI-08",
    ),
    "geometry/slightly-oblique": (
        frozenset({"geometry (sform)", "geometry (qform)"}),
        "AFNI writes the plumb matrix of a grid less than about 0.014° from plumb (its 0.01° "
        "test, in float); the original keeps the input's matrix (4e-4 mm apart here)",
    ),
    "errors/prefix-with-blank": (
        frozenset({"both succeed", "both reject the arguments"}),
        "AFNI refuses a -prefix with a blank; the original accepts it in reproduce mode too "
        "(no switch for AFNI-12)",
    ),
    "geometry/slightly-oblique-qform": (
        frozenset({"geometry (sform)", "geometry (qform)"}),
        "AFNI writes the plumb matrix of a qform less than about 0.014° from plumb (AFNI-16); "
        "the original keeps the input's matrix",
    ),
    "geometry/slightly-oblique-metre": (
        frozenset({"geometry (sform)", "geometry (qform)"}),
        "AFNI writes the plumb matrix (AFNI-16), its steps in millimetres and its origin in "
        "metres (AFNI-07); the original keeps the tilt",
    ),
    "defects/oblique-metre-zero-origin": (
        frozenset({"geometry (sform)", "geometry (qform)"}),
        "AFNI writes an oblique sform in metres as it is (AFNI-07); the original converts its "
        "linear part to millimetres in reproduce mode too",
    ),
    "copy/short-slice-spacing": (
        frozenset({"header fields (scaling, timing, slice fields, units, xform codes)"}),
        "AFNI derives the slice fields of a copy from the times, within 3 ms: slice_start 1 "
        "here; the original writes the header's fields back (slice_start 0)",
    ),
    "copy/alt-two-slices": (
        frozenset({"header fields (scaling, timing, slice fields, units, xform codes)"}),
        "AFNI derives the slice fields of a copy from the times: two slices are slice code 1; "
        "the original writes the header's code 3 back",
    ),
    "header-timing/code2-beyond-tr": (
        frozenset({defects.FIX_PASS, "D13 notices in both modes"}),
        "the original's default writes the corrected copy but does not name AFNI-03 when the "
        "corrected slice times exceed the TR (its reproduce output is AFNI's)",
    ),
}


@functools.cache
def replica_binary() -> str | None:
    """The ``larmorx-gpl`` binary (the GPL-3.0-or-later replica): ``LARMORX_GPL_BIN``, else
    the release (then debug) build of ``crates-gpl`` in this repository, else ``larmorx-gpl``
    on the PATH.

    The validation package is Apache-2.0: it only runs the replica as a separate process.
    """
    if env := os.environ.get("LARMORX_GPL_BIN"):
        return env
    exe = ".exe" if os.name == "nt" else ""
    for profile in ("release", "debug"):
        for parent in Path(__file__).resolve().parents:
            candidate = parent / REPLICA.format(profile=profile, exe=exe)
            if candidate.is_file():
                return str(candidate)
    return shutil.which("larmorx-gpl")


@functools.cache
def replica_version() -> str:
    binary = replica_binary()
    if binary is None:
        return "not found"
    out = subprocess.run([binary, "--version"], capture_output=True, text=True, encoding="utf-8")
    return out.stdout.strip() or "unknown version"


def _last_error(log: str, tmp: Path) -> str:
    """The last error line of a log, without colour codes or temporary paths."""
    lines = [ln for ln in log.splitlines() if "ERROR" in ln]
    line = lines[-1] if lines else (log.splitlines()[-1] if log else "")
    line = line.replace(str(tmp), "<tmp>").replace(str(_generated_dir()), "<generated>")
    return re.sub(r"\x1b\[[0-9;]*m", "", line)


def _expected_divergence(checks: CheckList, case_id: str, implementation: str) -> None:
    """Ends the case as an expected divergence when exactly its known checks failed."""
    known = DIVERGENCES.get(implementation, {}).get(case_id)
    failed = {c.name for c in checks.checks if not c.passed}
    if known and failed and failed <= known[0]:
        raise Outcome(EXPECTED, known[1])


def _ulps(a: np.ndarray, b: np.ndarray) -> int:
    if a.dtype != np.float32:
        return 0
    ia = a.view(np.int32).astype(np.int64)
    ib = b.view(np.int32).astype(np.int64)
    ia = np.where(ia < 0, np.iinfo(np.int32).min - ia, ia)
    ib = np.where(ib < 0, np.iinfo(np.int32).min - ib, ib)
    return int(np.abs(ia - ib).max()) if a.size else 0


HEADER_FIELDS = (
    "dim",
    "scl_slope",
    "scl_inter",
    "toffset",
    "xyzt_units",
    "dim_info",
    "slice_code",
    "slice_start",
    "slice_end",
    "slice_duration",
    "qform_code",
    "sform_code",
)


def _compare(
    checks: CheckList, a_path: Path, l_path: Path, names: tuple[str, str] = ("AFNI", "larmorx")
) -> None:
    """Compares the output ``l_path`` with the reference ``a_path`` (named ``names``)."""
    import nibabel as nib

    na, nl = names
    a_img, l_img = nib.load(a_path), nib.load(l_path)
    a = np.asanyarray(a_img.dataobj.get_unscaled())
    b = np.asanyarray(l_img.dataobj.get_unscaled())
    checks(
        "data type",
        a.dtype == b.dtype,
        metric="dtype",
        value=str(b.dtype),
        threshold=str(a.dtype),
        detail=f"{na} {a.dtype}, {nl} {b.dtype}",
    )
    checks("shape", a.shape == b.shape, detail=f"{na} {a.shape}, {nl} {b.shape}")
    ha, hb = a_img.header, l_img.header
    differing = [
        f"{k}: {na} {np.asarray(ha[k]).tolist()}, {nl} {np.asarray(hb[k]).tolist()}"
        for k in HEADER_FIELDS
        if not np.array_equal(np.asarray(ha[k]), np.asarray(hb[k]), equal_nan=True)
    ]
    checks(
        "header fields (scaling, timing, slice fields, units, xform codes)",
        not differing,
        detail="; ".join(differing),
    )
    tr_a, tr_b = float(ha["pixdim"][4]), float(hb["pixdim"][4])
    checks("TR (pixdim[4])", tr_a == tr_b, detail=f"{na} {tr_a}, {nl} {tr_b}")
    for name, fa, fb in [
        ("sform", ha.get_sform(), hb.get_sform()),
        ("qform", ha.get_qform(), hb.get_qform()),
    ]:
        diff = float(np.abs(fa - fb).max())
        checks(
            f"geometry ({name})",
            diff <= AFFINE_ATOL,
            metric="max |diff| (mm)",
            value=f"{diff:.1e}",
            threshold=f"{AFFINE_ATOL:g}",
        )
    voxel_sizes = float(np.abs(ha["pixdim"][1:4] - hb["pixdim"][1:4]).max())
    checks(
        "voxel sizes (pixdim[1:4])",
        voxel_sizes <= 1e-5 * float(np.abs(ha["pixdim"][1:4]).max()),
        metric="max |diff|",
        value=f"{voxel_sizes:.1e}",
        threshold="1e-5 relative",
    )
    if a.shape != b.shape:
        return
    differing_voxels = int(np.count_nonzero(a != b))
    if a.dtype.kind in "iu":
        d = int(np.abs(a.astype(np.int64) - b.astype(np.int64)).max()) if a.size else 0
        checks(
            "values (integer output)",
            d <= INT_ATOL,
            metric="max |diff|",
            value=d,
            threshold=INT_ATOL,
            detail=f"{differing_voxels} of {a.size} values differ "
            f"({differing_voxels / max(a.size, 1):.2e})",
        )
    else:
        scale = float(np.abs(a).max()) or 1.0
        rel = float(np.abs(a.astype(np.float64) - b).max()) / scale
        checks(
            "values",
            rel <= RTOL,
            metric="max |diff| / max |AFNI|",
            value=f"{rel:.1e}",
            threshold=f"{RTOL:g}",
            detail=f"{differing_voxels} of {a.size} values differ; at most {_ulps(a, b)} ulp",
        )
    checks.add(
        Check(
            "bit-identical",
            True,
            metric="differing values",
            value=differing_voxels,
            threshold="reported",
            detail="identical" if differing_voxels == 0 else f"{differing_voxels} values differ",
        )
    )


#: The method options, as 3dTshift matches them (4 characters; wsinc 7, any case).
_METHOD_NAMES = {
    "-Fou": "Fourier",
    "-fou": "Fourier",
    "-lin": "linear",
    "-Lin": "linear",
    "-cub": "cubic",
    "-Cub": "cubic",
    "-qui": "quintic",
    "-Qui": "quintic",
    "-hep": "heptic",
    "-Hep": "heptic",
}


def _method_of(args: list[str]) -> str:
    """The interpolation method a command line selects (the last method option wins)."""
    method = "Fourier"
    for a in args:
        if a[:4] in _METHOD_NAMES:
            method = _METHOD_NAMES[a[:4]]
        elif a[:7].lower() in ("-wsinc5", "-wsinc9"):
            method = a[1:7].lower()
    return method


#: AFNI's lines that name its build, not what it does: the version banner and the compile date
#: it prints after a fatal error (larmorx prints neither).
_AFNI_BUILD_LINES = re.compile(
    r"^(?:[^\n]*::)?\+\+ 3dTshift: AFNI version=[^\n]*\n|^\*\* Program compile date = [^\n]*\n",
    re.M,
)
_ANSI = re.compile(r"\x1b\[[0-9;]*m")

#: The name of the check that compares the messages.
MESSAGES = "messages: AFNI's, byte for byte"


def _larmorx_own(line: str) -> bool:
    """A line larmorx adds to AFNI's messages: an upstream-defect notice, ``-verbose``'s lines
    on the implementation, or a note on a global option (ending in ``(larmorx)``)."""
    plain = _ANSI.sub("", line).rstrip()
    return (
        "upstream defect " in plain
        or "++ implementation: " in plain
        or "++ upstream defects: " in plain
        or plain.endswith("(larmorx)")
    )


def _afni_lines(text: str) -> str:
    return _AFNI_BUILD_LINES.sub("", text)


def _larmorx_lines(text: str) -> str:
    return "".join(line for line in text.splitlines(keepends=True) if not _larmorx_own(line))


def _first_difference(a: str, b: str) -> str:
    xs, ys = a.splitlines(), b.splitlines()
    k = next((i for i, (x, y) in enumerate(zip(xs, ys, strict=False)) if x != y), None)
    if k is None:
        k = min(len(xs), len(ys))
    x = xs[k] if k < len(xs) else "(nothing)"
    y = ys[k] if k < len(ys) else "(nothing)"
    return f"line {k + 1}: AFNI {x!r}, larmorx {y!r}"


def _check_messages(
    checks: CheckList, afni: tuple[str, str], ours: tuple[str, str], tmp: Path
) -> None:
    """AFNI's standard output and error, byte for byte (colour codes included), against
    larmorx's, without AFNI's build lines and larmorx's own lines."""
    (a_out, a_err), (l_out, l_err) = afni, ours
    a_err, l_err = _afni_lines(a_err), _larmorx_lines(l_err)
    detail = "identical"
    if a_out != l_out:
        detail = "stdout " + _first_difference(a_out, l_out)
    elif a_err != l_err:
        detail = "stderr " + _first_difference(a_err, l_err)
    checks(
        MESSAGES,
        a_out == l_out and a_err == l_err,
        detail=detail.replace(str(tmp), "<tmp>").replace(str(_generated_dir()), "<generated>"),
    )


def _is_nifti(path: Path) -> bool:
    head = path.read_bytes()[:4]
    return head in (b"\x5c\x01\x00\x00", b"\x1c\x02\x00\x00") or head[:2] == b"\x1f\x8b"


def _run_separately(
    case: Case, s: Scenario, checks: CheckList, tmp: Path, implementation: str
) -> None:
    """A case of AFNI's global options and environment variables: each program in a directory
    of its own (``files`` written to each, the output named relative to it), in the same
    environment (:func:`_environment`); exit statuses, messages and outputs compared, then the
    fix pass."""
    image = _image_path(s.image)
    env = _environment(s)
    output = s.output or "out.nii"
    written = s.written or output
    before = dict(s.files)

    def setup(d: Path) -> list[str]:
        d.mkdir()
        for name, text in s.files:
            (d / name).parent.mkdir(parents=True, exist_ok=True)
            (d / name).write_text(text, encoding="utf-8")
        args = [str(image) if a == "{input}" else a for a in s.args]
        if "{input}" not in s.args:
            args += ["-prefix", output, str(image)]
        return args

    def produced(code: int, d: Path) -> Path | None:
        f = d / written
        if code != 0 or not f.is_file():
            return None
        if written in before and f.read_bytes() == before[written].encode():
            return None
        return f

    runs = {}
    args: list[str] = []
    for tool in ("afni", "larmorx"):
        d = tmp / tool
        args = setup(d)
        if tool == "afni":
            code, out, err = _run_afni_streams(args, env, d)
        else:
            code, out, err = _run_larmorx_streams(args, implementation, defects.REPRODUCE, env, d)
        runs[tool] = (code, out, err, produced(code, d))
    (ca, oa, ea, pa), (cl, ol, el, pl) = runs["afni"], runs["larmorx"]
    checks("exit status", ca == cl, metric="exit status", value=cl, threshold=ca)
    _check_messages(checks, (oa, ea), (ol, el), tmp)
    if pa is None and pl is None:
        _expected_divergence(checks, case.id, implementation)
        if all(c.passed for c in checks.checks):
            raise Outcome(
                BOTH_ERROR,
                f"AFNI exit {ca}: {_last_error(ea, tmp)}; larmorx exit {cl}: "
                f"{_last_error(el, tmp)} (the same messages)",
            )
        return
    checks(
        "both succeed",
        pa is not None and pl is not None,
        detail=f"AFNI exit {ca}: {_last_error(ea, tmp)}; larmorx exit {cl}: {_last_error(el, tmp)}",
    )
    if s.expect == "error":
        checks("both reject the arguments", False, detail="expected an error from both")
    _expected_divergence(checks, case.id, implementation)
    if pa is None or pl is None:
        return
    nifti = _is_nifti(pa)
    checks(
        "AFNI's output is NIfTI",
        nifti,
        detail="" if nifti else f"it starts with {pa.read_bytes()[:30]!r}",
    )
    _expected_divergence(checks, case.id, implementation)
    copy = "just a copy of input" in el
    checks.add(
        Check(
            "method",
            True,
            metric="interpolation",
            value="copy" if copy else _method_of(args),
            threshold="reported",
        )
    )
    _compare(checks, pa, pl)
    if s.whole_file:
        a_bytes, l_bytes = pa.read_bytes(), pl.read_bytes()
        if implementation == "replica":
            checks(
                "the whole file, byte for byte",
                a_bytes == l_bytes,
                detail=f"AFNI {len(a_bytes)} bytes, larmorx {len(l_bytes)}",
            )
        else:
            # The original's data are compared above, with its thresholds (its Fourier
            # method differs from AFNI's in the last bits); the header, byte for byte.
            n = (
                int(np.frombuffer(a_bytes[108:112], "<f4")[0])
                if a_bytes[:4] == b"\x5c\x01\x00\x00"
                else 544
            )
            checks(
                "the header, byte for byte",
                a_bytes[:n] == l_bytes[:n] and len(a_bytes) == len(l_bytes),
                detail=f"{n} header bytes; AFNI {len(a_bytes)} bytes, larmorx {len(l_bytes)}",
            )
    _expected_divergence(checks, case.id, implementation)
    if s.expect != "pass":
        return
    if not copy:
        _check_python_settings(checks, s, image, pl, implementation)
    # The fix pass: larmorx's default on the same arguments, in a directory of its own.
    import nibabel as nib

    d = tmp / "fix"
    args = setup(d)
    code, out, err = _run_larmorx_streams(args, implementation, defects.FIX, env, d)
    target = d / written
    if code != 0 or not target.is_file():
        checks(defects.FIX_PASS, False, detail=f"the fix pass failed (exit {code})")
        return
    a, b = nib.load(target), nib.load(pl)
    change = defects.Change.of(
        np.asanyarray(a.dataobj.get_unscaled()), np.asanyarray(b.dataobj.get_unscaled())
    )
    ha, hb = a.header, b.header
    other = [] if type(ha) is type(hb) else ["the NIfTI version"]
    fields = ("dim", "datatype", "pixdim", "toffset", "qform_code", "sform_code", "qoffset_x")
    fields += ("srow_x", "srow_y", "srow_z", "slice_code", "slice_start", "slice_end")
    other += [
        f
        for f in fields
        if not np.array_equal(np.nan_to_num(np.asarray(ha[f])), np.nan_to_num(np.asarray(hb[f])))
    ]
    for f in ("slope", "inter"):
        if not np.array_equal(
            np.nan_to_num(float(getattr(a.dataobj, f))), np.nan_to_num(float(getattr(b.dataobj, f)))
        ):
            other.append(f"scl_{f}")
    plain = [_ANSI.sub("", _larmorx_lines(x)) for x in (out + err, ol + el)]
    if plain[0] != plain[1]:
        other.append("messages")
    entries = defects.notices_in(err)
    defects.add_fix_check(checks, entries, change, other_changes=", ".join(other))
    reproduced_entries = defects.notices_in(el)
    if entries != reproduced_entries:
        checks(
            "D13 notices in both modes",
            False,
            detail=f"fix: {entries}; reproduce: {reproduced_entries}",
        )
    _expected_divergence(checks, case.id, implementation)


#: AFNI variables set in this process's environment while the Python API runs with
#: ``afni_settings`` (:func:`_check_python_settings`): each would change the output if
#: ``lx.afni.tshift`` passed the environment on (NIfTI-2, int16 kept, the tlrc view).
_ENVIRONMENT_NOISE = {
    "AFNI_NIFTI_WRITE_TYPE": "2",
    "AFNI_NIFTI_SCALE": "NO",
    "AFNI_NIFTI_VIEW": "tlrc",
    "AFNI_NIFTI_PRIORITY": "Q",
}

#: The check that compares ``lx.afni.tshift(afni_settings=...)`` with the command line.
PY_SETTINGS = "Python API with afni_settings: the command line's output"


def _check_python_settings(
    checks: CheckList, s: Scenario, image: Path, cli_out: Path, implementation: str
) -> None:
    """A case of AFNI's variables through the Python API: ``lx.afni.tshift`` with the case's
    variables and ``-D`` options as ``afni_settings`` gives the command line's output, while
    this process's environment holds AFNI variables that would change it if they reached the
    implementation (:data:`_ENVIRONMENT_NOISE`; the Python API never reads the environment,
    CLAUDE.md rule 6). Skipped where the Python API has no equivalent: the variables ``tshift``
    sets itself or does not take, other global options, AFNI 1D files."""
    import larmorx as lx

    if "{input}" in s.args:
        return
    settings = {name: value for name, value in s.env if value is not None}
    rest: list[str] = []
    for a in s.args:
        if a.startswith("-D") and "=" in a:  # the -D wins over the environment, as in AFNI
            name, value = a[2:].split("=", 1)
            settings[name] = value
        elif a in ("-overwrite", "-ok_1D_text", "-skip_afnirc"):
            return
        else:
            rest.append(a)
    kw = _python_call(s, rest)
    if kw is None:
        return
    noisy = {**os.environ, **_ENVIRONMENT_NOISE}
    try:
        with _process_environment(noisy), warnings.catch_warnings():
            warnings.simplefilter("ignore")
            result = lx.afni.tshift(
                str(image),
                implementation=implementation,
                upstream_defects="reproduce",
                afni_settings=settings,
                **kw,
            )
    except ValueError:  # a variable tshift sets itself or does not take, or an empty value
        return
    expected = lx.load(cli_out)
    header = lx.io.read_header(cli_out)
    differing = _differing(result.data, expected.data)
    same_header = result.header.replace(vox_offset=header.vox_offset).to_bytes() == (
        header.to_bytes()
    )
    checks(
        PY_SETTINGS,
        differing == 0 and same_header,
        metric="differing values",
        value=differing,
        threshold=0,
        detail=f"settings {settings}; header {'identical' if same_header else 'differs'}",
    )


def _produced(code: int, target: Path, occupied: bool) -> bool:
    """Whether a run wrote its output (NIfTI, or AFNI's own format for a bare prefix)."""
    if code != 0:
        return False
    if occupied:  # the output existed before: written only if it was replaced
        return target.read_bytes() != b"taken"
    return target.exists() or Path(f"{target}+orig.HEAD").exists()


# ------------------------------------------------------------------------------------------------
# The Python API

#: The checks that compare the Python API with the command line.
PY_FILE = "Python API: the command line's output"
PY_MEMORY = "Python API, image in memory: the same values"


def _python_method(arg: str) -> str | None:
    """``lx.afni.tshift``'s method for a method option (as 3dTshift matches them), or ``None``."""
    if arg[:4] in _METHOD_NAMES:
        return _METHOD_NAMES[arg[:4]].lower()
    if arg[:7].lower() in ("-wsinc5", "-wsinc9"):
        return arg[1:7].lower()
    return None


def _python_call(s: Scenario, args: list[str]) -> dict | None:
    """``lx.afni.tshift``'s keyword arguments for a command line, or ``None`` when the Python
    API has no equivalent: the header's slice timing (no ``-tpattern``), AFNI 1D files and
    strings other than fMRIPrep's, ``-TR`` in milliseconds, options it does not have."""
    kw: dict = {}
    if s.sidecar:  # fMRIPrep's call: the BIDS SliceTiming, as _fmriprep_args writes it
        meta = json.loads(td.get(s.sidecar).read_text(encoding="utf-8"))
        times = [float(t) for t in meta["SliceTiming"]]
        if str(meta.get("SliceEncodingDirection", "")).endswith("-"):
            times = times[::-1]
        kw["slice_times"] = times
    i = 0
    while i < len(args):
        a = args[i]
        value = args[i + 1] if i + 1 < len(args) else ""
        step = 2
        if a == "-tpattern":
            if not value.startswith("@"):
                kw["slice_times"] = value
            elif not s.sidecar:
                return None
        elif a == "-TR":
            m = re.fullmatch(r"([0-9.]+)s?", value)
            if m is None:
                return None
            kw["tr"] = float(m.group(1))
        elif a in ("-tzero", "-slice", "-ignore"):
            kw[a[1:]] = float(value) if a == "-tzero" else int(value)
        else:
            step = 1
            if a in ("-rlt", "-rlt+"):
                kw["restore"] = "none" if a == "-rlt" else "intercept"
            elif a == "-no_detrend":
                kw["detrend"] = False
            elif (method := _python_method(a)) is not None:
                kw["method"] = method
            elif not a.startswith("-verb"):
                return None
        i += step
    return kw if "slice_times" in kw else None


def _differing(a: np.ndarray, b: np.ndarray) -> int:
    """How many values differ (all of them if the types or shapes do)."""
    if a.dtype != b.dtype or a.shape != b.shape:
        return max(a.size, b.size)
    return int(np.count_nonzero(a != b))


def _check_python(
    checks: CheckList, s: Scenario, args: list[str], image: Path, cli_out: Path, implementation: str
) -> None:
    """``lx.afni.tshift(..., implementation=...)`` gives the command line's output, from the
    file and from memory."""
    import larmorx as lx

    kw = _python_call(s, args)
    if kw is None:
        return
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        from_file = lx.afni.tshift(
            str(image), implementation=implementation, upstream_defects="reproduce", **kw
        )
    expected = lx.load(cli_out)
    header = lx.io.read_header(cli_out)
    differing = _differing(from_file.data, expected.data)
    same_header = (
        from_file.header.replace(vox_offset=header.vox_offset).to_bytes() == header.to_bytes()
    )
    ran = from_file.implementation.kind if from_file.implementation else "unknown"
    checks(
        PY_FILE,
        differing == 0 and same_header and ran == implementation,
        metric="differing values",
        value=differing,
        threshold=0,
        detail=f"ran the {ran}; header {'identical' if same_header else 'differs'}",
    )
    # In memory: only data 3dTshift reads as they are (uint8, int16, float32, unscaled).
    loaded = lx.load(image)
    unscaled = lx.io.read_header(image).slope_inter in (None, (1.0, 0.0))
    if loaded.data.dtype in (np.uint8, np.int16, np.float32) and unscaled:
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            in_memory = lx.afni.tshift(
                loaded, implementation=implementation, upstream_defects="reproduce", **kw
            )
        d = _differing(in_memory.data, from_file.data)
        checks(PY_MEMORY, d == 0, metric="differing values", value=d, threshold=0)


#: The implementations a run can compare with AFNI (docs/licensing.md): the clean-room
#: original (larmorx-afni, in-process) or the GPL replica (the larmorx-gpl program, which
#: larmorx runs as a separate process).
IMPLEMENTATIONS = ("original", "replica")


def run_case(case: Case, checks: CheckList, implementation: str = "original") -> None:
    s: Scenario = case.payload
    if afni_binary() is None:
        raise Outcome("skipped", "the AFNI 3dTshift oracle was not found (LARMORX_AFNI_BIN)")
    if implementation == "replica" and replica_binary() is None:
        raise Outcome(
            "skipped", "the larmorx-gpl binary was not found (LARMORX_GPL_BIN, crates-gpl build)"
        )
    ours = functools.partial(_run_larmorx, implementation=implementation)
    with (
        _replica_program(implementation),
        tempfile.TemporaryDirectory(prefix="lx-tshift-") as tmp_name,
    ):
        tmp = Path(tmp_name)
        if s.separate:
            _run_separately(case, s, checks, tmp, implementation)
            return
        for name, text in s.files:
            (tmp / name).write_text(text, encoding="utf-8")
        occupied = any(name == "out.nii" for name, _ in s.files)
        image = _image_path(s.image)
        args = _args(s, tmp)
        outputs, logs, codes, produced = {}, {}, {}, {}
        for tool, runner in [("afni", _run_afni), ("larmorx", ours)]:
            name = f"{tool}-{s.output}" if s.output else f"{tool}{s.suffix}"
            target = tmp / ("out.nii" if occupied else name)
            codes[tool], logs[tool] = runner([*args, "-prefix", str(target), str(image)])
            outputs[tool] = target
            produced[tool] = _produced(codes[tool], target, occupied)
        if s.expect == "divergence":
            ok = produced["afni"] and not produced["larmorx"]
            checks(
                "AFNI succeeds and larmorx refuses",
                ok,
                detail=f"AFNI exit {codes['afni']}; larmorx: {_last_error(logs['larmorx'], tmp)}",
            )
            if ok:
                raise Outcome(EXPECTED, s.reason)
            return
        if not produced["afni"] and not produced["larmorx"]:
            raise Outcome(
                BOTH_ERROR,
                f"AFNI: {_last_error(logs['afni'], tmp)}; larmorx: {_last_error(logs['larmorx'], tmp)}",
            )
        checks(
            "both succeed",
            produced["afni"] and produced["larmorx"],
            detail=f"AFNI exit {codes['afni']}: {_last_error(logs['afni'], tmp)}; "
            f"larmorx exit {codes['larmorx']}: {_last_error(logs['larmorx'], tmp)}",
        )
        if s.expect == "error":
            checks("both reject the arguments", False, detail="expected an error from both")
        _expected_divergence(checks, case.id, implementation)
        if produced["afni"] and produced["larmorx"]:
            copy = "just a copy of input" in logs["larmorx"]
            checks.add(
                Check(
                    "method",
                    True,
                    metric="interpolation",
                    value="copy" if copy else _method_of(args),
                    threshold="reported",
                )
            )
            _compare(checks, outputs["afni"], outputs["larmorx"])
            _expected_divergence(checks, case.id, implementation)
            if s.expect == "pass" and not copy:
                _check_python(checks, s, args, image, outputs["larmorx"], implementation)
            if s.expect == "pass":
                fixed = _fix_pass(
                    checks, args, image, tmp, outputs["larmorx"], logs["larmorx"], implementation
                )
                _expected_divergence(checks, case.id, implementation)
                if implementation == "replica" and fixed is not None:
                    entries, target = fixed
                    if entries or case.category in ("defects", "geometry"):
                        _defaults_agree(checks, case.id, args, image, tmp, target)


#: The name of the check that compares the replica's default with the original's.
DEFAULTS = "D13 defaults: replica = original"


def _defaults_agree(
    checks: CheckList, case_id: str, args: list[str], image: Path, tmp: Path, replica: Path
) -> None:
    """The replica's default output (``replica``) against the clean-room original's on the same
    arguments: ``implementation="auto"`` may run either, so their corrections must agree in
    everything a defect concerns (header fields, geometry, scaling and values), within the
    thresholds that compare the original with AFNI. Bit-identity is reported."""
    target = tmp / f"original-fix{''.join(replica.suffixes)}"
    code, _ = _run_larmorx(
        [*args, "-prefix", str(target), str(image)], implementation="original", mode=defects.FIX
    )
    if code != 0 or not target.exists():
        checks(DEFAULTS, False, detail=f"the original's default run failed (exit {code})")
        return
    sub = CheckList()
    _compare(sub, replica, target, names=("replica", "original"))
    failed = [f"{c.name}: {c.detail or c.value}" for c in sub.checks if not c.passed]
    identical = next((c.value == 0 for c in sub.checks if c.name == "bit-identical"), False)
    known = DIVERGENCES["original"].get(case_id)
    detail = "bit-identical" if identical else "within the thresholds, not bit-identical"
    if failed:
        detail = "differ: " + "; ".join(failed)
        if known:
            detail = f"known (the original diverges from AFNI here): {detail}"
    checks(
        DEFAULTS,
        not failed or known is not None,
        metric="agreement",
        value="bit-identical" if identical and not failed else ("differ" if failed else "within"),
        threshold="the original's thresholds",
        detail=detail,
    )


def _fix_pass(
    checks: CheckList,
    args: list[str],
    image: Path,
    tmp: Path,
    reproduced: Path,
    log: str,
    implementation: str = "original",
) -> tuple[list[str], Path] | None:
    """D13: larmorx's default (``fix``) on the same arguments, compared with its ``reproduce``
    output: the defects named, and what the corrections change (values and header). Returns
    the defects named and the default's output, or ``None`` if the default run failed."""
    import nibabel as nib

    target = tmp / f"fix{''.join(reproduced.suffixes)}"
    code, fix_log = _run_larmorx(
        [*args, "-prefix", str(target), str(image)],
        implementation=implementation,
        mode=defects.FIX,
    )
    if code != 0 or not target.exists():
        checks(defects.FIX_PASS, False, detail=f"the fix pass failed (exit {code})")
        return None
    a, b = nib.load(target), nib.load(reproduced)
    change = defects.Change.of(
        np.asanyarray(a.dataobj.get_unscaled()), np.asanyarray(b.dataobj.get_unscaled())
    )
    ha, hb = a.header, b.header
    # nibabel moves the scaling from the header to the data proxy when it loads a file.
    header = [
        f
        for f in ("qoffset_x", "srow_x", "slice_code", "slice_end")
        if not np.array_equal(np.nan_to_num(ha[f]), np.nan_to_num(hb[f]))
    ]
    for f in ("slope", "inter"):
        if not np.array_equal(
            np.nan_to_num(float(getattr(a.dataobj, f))), np.nan_to_num(float(getattr(b.dataobj, f)))
        ):
            header.append(f"scl_{f}")
    entries = defects.notices_in(fix_log)
    reproduced_entries = defects.notices_in(log)
    other = f"header {', '.join(header)}" if header else ""
    defects.add_fix_check(checks, entries, change, other_changes=other)
    if entries != reproduced_entries:
        checks(
            "D13 notices in both modes",
            False,
            detail=f"fix: {entries}; reproduce: {reproduced_entries}",
        )
    return entries, target


def _differing_fraction(r: CaseResult) -> float:
    for c in r.checks:
        if c.name in ("values", "values (integer output)"):
            match = re.match(r"(\d+) of (\d+) values differ", c.detail)
            if match:
                return int(match.group(1)) / max(int(match.group(2)), 1)
    return 0.0


def _highlights(results: list[CaseResult], implementation: str = "original") -> list[str]:
    compared = [r for r in results if r.status == PASS]

    def identical(r: CaseResult) -> bool:
        return any(c.name == "bit-identical" and c.value == 0 for c in r.checks)

    def method(r: CaseResult) -> str:
        return next((str(c.value) for c in r.checks if c.name == "method"), "")

    def worst(rs: list[CaseResult], name: str) -> float | None:
        values = [float(c.value) for r in rs for c in r.checks if c.name == name]
        return max(values) if values else None

    fourier = [r for r in compared if method(r) == "Fourier"]
    other = [r for r in compared if method(r) != "Fourier"]
    lines = [
        f"**Bit-identical: {sum(map(identical, compared))} of {len(compared)} passing cases** "
        "produce exactly the bytes of AFNI's output data.",
    ]
    if implementation == "replica":
        lines += [
            f"- Fourier: {sum(map(identical, fourier))} of {len(fourier)} bit-identical.",
            f"- Lagrange and weighted-sinc methods and copies: {sum(map(identical, other))} of "
            f"{len(other)} bit-identical.",
        ]
        rest = [r.case for r in compared if not identical(r)]
        if rest:
            lines.append(
                "- Not bit-identical: "
                + ", ".join(f"`{c}`" for c in rest)
                + ". See `docs/findings/afni-tshift.md`."
            )
        lines.append(
            "- The clean-room original (`larmorx afni 3dTshift`, Apache-2.0) has its own record: "
            "[afni-tshift.md](afni-tshift.md)."
        )
    else:
        lines += [
            f"- Lagrange and weighted-sinc methods and copies: {sum(map(identical, other))} of "
            f"{len(other)} bit-identical.",
            f"- Fourier: {sum(map(identical, fourier))} of {len(fourier)} bit-identical. AFNI "
            "computes the FFT in float32 with its own kernels; larmorx computes it in double "
            "precision with its own FFT, so float32 outputs differ in the last bits and integer "
            "outputs occasionally round the other way (by 1).",
        ]
        rest = [r.case for r in other if not identical(r)]
        if rest:
            lines.append(
                "- Not bit-identical outside Fourier: "
                + ", ".join(f"`{c}`" for c in rest)
                + ". Known causes, in the last float32 bit only: `-no_detrend` (its rounding is "
                "not reproduced yet); quintic and heptic weights for some fractions (unresolved); "
                "weighted sinc, where AFNI calls glibc's float `sinf`/`cosf`, which are not "
                "correctly rounded, and larmorx uses correctly rounded functions "
                "(`docs/findings/platform-math.md`). See `docs/api/afni-tshift.md`."
            )
        lines.append(
            "- The bit-exact replica (`larmorx-gpl afni 3dTshift`, GPL-3.0-or-later) has its own "
            "record: [afni-tshift-replica.md](afni-tshift-replica.md)."
        )

    def python(name: str) -> tuple[int, int]:
        rs = [c for r in compared for c in r.checks if c.name == name]
        return sum(c.passed for c in rs), len(rs)

    (file_ok, file_n), (memory_ok, memory_n) = python(PY_FILE), python(PY_MEMORY)
    lines.append(
        f'- Python API (`lx.afni.tshift(..., implementation="{implementation}")`): run on the '
        f"{file_n} passing cases it can express; {file_ok} give the command line's output bit "
        f"for bit (data and header). From an image in memory: {memory_ok} of {memory_n} give "
        "the same values."
    )
    settings_ok, settings_n = python(PY_SETTINGS)
    if settings_n:
        lines.append(
            "- Python API with AFNI's variables (`afni_settings=`, the case's variables and "
            f"`-D` options), on the {settings_n} cases of AFNI's environment variables and "
            f"global options it can express: {settings_ok} give the command line's output bit "
            "for bit (data and header), with AFNI variables in the process environment that "
            f"would change it if the call read the environment ({', '.join(_ENVIRONMENT_NOISE)})."
        )
    messages = [c for r in results for c in r.checks if c.name == MESSAGES]
    if messages:
        lines.append(
            "- **AFNI's global options and environment variables** (categories `global` and "
            f"`environment`, {len(messages)} cases): exit status and messages compared too, "
            "standard output and error byte for byte (colour codes included; AFNI's version "
            "banner and compile date, and larmorx's own lines, set aside): "
            f"{sum(c.passed for c in messages)} of {len(messages)} identical."
        )
    lines += defects.summary(results)
    if implementation == "replica":
        lines += _defaults_summary(results)
    rows = []
    for category in sorted({r.category for r in compared}):
        rs = [r for r in compared if r.category == category]
        f, i = worst(rs, "values"), worst(rs, "values (integer output)")
        rows.append(
            (
                category,
                len(rs),
                sum(map(identical, rs)),
                "–" if f is None else f"{f:.1e}",
                "–" if i is None else f"{i:g}",
                f"{max(map(_differing_fraction, rs)):.1e}",
            )
        )
    lines += [
        "",
        table(
            (
                "Category",
                "Compared",
                "Bit-identical",
                "Worst float diff (× max AFNI)",
                "Worst integer diff",
                "Most values differing (fraction)",
            ),
            rows,
        ),
    ]
    return lines


def _defaults_summary(results: list[CaseResult]) -> list[str]:
    """The report's lines on the replica's default against the original's."""
    rs = [(r.case, c) for r in results for c in r.checks if c.name == DEFAULTS]
    if not rs:
        return []
    same = [case for case, c in rs if c.value == "bit-identical"]
    within = [case for case, c in rs if c.value == "within"]
    known = [(case, c) for case, c in rs if c.value == "differ" and c.passed]
    bad = [(case, c) for case, c in rs if not c.passed]
    lines = [
        f'- **D13 defaults, replica against the clean-room original** (`upstream_defects="fix"`, '
        f"the {len(rs)} cases where a correction applies or that test one): {len(same)} "
        f"bit-identical, {len(within)} within the original's thresholds (Fourier and other "
        f"last bits, as in the original's own record), {len(known)} where the original is "
        f"known to diverge from AFNI, {len(bad)} disagreeing."
    ]
    for case, c in known + bad:
        lines.append(f"  - `{case}`: {c.detail}")
    return lines


def suite(implementation: str = "original") -> Suite:
    """The 3dTshift suite for the clean-room original (``larmorx afni 3dTshift``, in-process)
    or the GPL replica (``larmorx-gpl afni 3dTshift``, which larmorx runs as a separate
    process); both through larmorx's command line and its Python API."""
    if implementation not in IMPLEMENTATIONS:
        raise ValueError(f"implementation must be one of {IMPLEMENTATIONS}, not {implementation!r}")
    replica = implementation == "replica"
    tool = (
        f"`larmorx-gpl afni 3dTshift` ({replica_version()}; crate larmorx-gpl-afni, the "
        "GPL-3.0-or-later replica translated from AFNI 25.2.09's source), run by larmorx as a "
        "separate process: `larmorx afni 3dTshift` with `LARMORX_IMPLEMENTATION=replica` and "
        '`lx.afni.tshift(..., implementation="replica")`'
        if replica
        else "`larmorx afni 3dTshift` / `lx.afni.tshift` (crate larmorx-afni, clean-room from "
        '`specs/3dTshift.md`), with `implementation="original"`'
    )
    who = "larmorx-gpl" if replica else "larmorx"
    return Suite(
        name="afni-tshift-replica" if replica else "afni-tshift",
        title="3dTshift (replica)" if replica else "3dTshift",
        tool=tool,
        reference=f"3dTshift from AFNI ({afni_version()}), the binary built by `scripts/build_afni_oracle.sh`",
        thresholds=[
            ("Exit status", "both succeed, or both reject the arguments"),
            (
                "Output data type, shape, scaling (`scl_slope`), TR, `toffset`, units, slice fields, xform codes",
                "identical",
            ),
            ("Geometry: sform and qform (read by nibabel)", f"max |diff| ≤ {AFFINE_ATOL:g} mm"),
            (
                "float32 outputs",
                f"max |diff| ≤ {RTOL:g} × max |AFNI output| (specs/3dTshift.md §9); bit-identity reported",
            ),
            ("Integer outputs", f"max |diff| ≤ {INT_ATOL}, with the fraction of differing values"),
        ],
        cases=cases,
        run_case=functools.partial(run_case, implementation=implementation),
        packages=("nibabel",),
        highlights=functools.partial(_highlights, implementation=implementation),
        notes=(
            "Both programs read the same files with the same arguments; outputs are read with "
            "nibabel and compared as stored (before `scl_slope`). Real runs pass the BIDS "
            "`SliceTiming` as fMRIPrep does: a tab-separated `-tpattern` file of `str(float)` "
            "values (reversed when `SliceEncodingDirection` ends in `-`) and "
            "`-tzero round(min + 0.5 * (max - min), 3)`. Header-timing inputs are generated from "
            "the synthetic float32 file by setting `slice_code`, `slice_start`, `slice_end`, "
            f"`slice_duration`, `dim_info` and the time unit. {who} uses all logical CPUs "
            "(`OMP_NUM_THREADS` unset) on the command line and 1 thread through the Python API "
            "(its default); its results do not depend on the thread count. The Python API runs "
            "every passing case it can express (a `-tpattern` name or fMRIPrep's slice timing, "
            "options in seconds; not the header's timing, AFNI 1D files or copies of the input) "
            "and must give the command line's output: the same values and the same header bytes "
            "(`vox_offset` aside); for unscaled `uint8`, `int16` and `float32` inputs it also runs "
            "on the image loaded in memory. Both programs run without AFNI's environment "
            "variables of this process and with `HOME` an empty directory (no `.afnirc`), "
            "`AFNI_DONT_LOGFILE=YES` and `AFNI_NIFTI_TYPE_WARN=NO`, plus each case's variables. "
            "The cases of AFNI's global options and environment variables run each program in a "
            "directory of its own and compare exit statuses and messages too."
        ),
    )
