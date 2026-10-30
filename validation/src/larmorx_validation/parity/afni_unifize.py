# SPDX-License-Identifier: Apache-2.0
"""Parity of the 3dUnifize replica (``larmorx afni 3dUnifize`` with
``LARMORX_IMPLEMENTATION=replica``, in-process; until 2026-10-29 the GPL program
``larmorx-gpl``) with AFNI's ``3dUnifize`` (26.3.05; 25.2.09 kept): exit status, messages and every output (the
unifized image, ``-ssave``'s scale factors, ``-amsave``'s automasked input), byte for byte
(:mod:`larmorx_validation.parity.afni_replica`).

Cases: every option and its paths on the phantoms of
:mod:`larmorx_validation.parity.afni_phantoms` (``-Urad``, ``-rbt``/``-param``, ``-clfrac``,
``-T2`` once, twice and three times, ``-T2up``, ``-GM``, ``-nosquash``, ``-noduplo`` and the
half-size path above a million voxels, ``-EPI``, ``-ssave``, ``-amsave``, ``-input``/``-inset``,
``-quiet``, ``-verb``, ``-useall``, AFNI's case-insensitive options); every storage type,
scaling and geometry AFNI reads; 2D, 3D and 4D inputs; AFNI's global options and environment
variables; the errors and crashes; the defects (D13: AFNI-01, -02, -07, -11, -12, -16, -18,
-23, -26, -45, -47); and real data: fMRIPrep's own call (``3dUnifize -clfrac 0.2 -rbt 18.3
65.0 90.0 -T2``) on the inputs of every oracle run's BOLD reference, compared with fMRIPrep's
own outputs too; AFNI's default on the oracle runs' T1w images (the half-size path); ``-GM`` on
a T1w; ``-EPI`` on BOLD series. The real cases also measure AFNI's own variability (OpenMP
thread counts, fMRIPrep's build of AFNI in its image). The clean-room original also has AFNI-75's
guard cases and GLIBC-05's (``-EPI``'s radius from glibc 2.35's cube root with ``reproduce``,
the correctly rounded one by default: :data:`CBRTF_VOXELS`).
"""

from __future__ import annotations

import functools
from pathlib import Path

from larmorx_validation.parity import afni_phantoms as ph
from larmorx_validation.parity import afni_replica as ar
from larmorx_validation.parity import afni_versions
from larmorx_validation.parity import defects as d13
from larmorx_validation.parity.afni_automask import _input
from larmorx_validation.parity.harness import Case, CheckList, Suite

PROGRAM = "3dUnifize"
U = "uni.nii"
S = ar.Scenario


def _py(outputs=None, **kw):
    return {"python": kw, "python_outputs": outputs or {"image": U}}


def _scenarios() -> list[tuple[str, str, str, ar.Scenario]]:
    # fmt: off
    out: list[tuple[str, str, str, ar.Scenario]] = []

    def add(cid: str, category: str, desc: str, s: ar.Scenario) -> None:
        out.append((f"{category}/{cid}", category, desc, s))

    t1, t2 = "head-f32", "head-t2-f32"
    p = ("-prefix", U)
    add("default", "options", "the defaults on the T1-like phantom", S((*p, "{0}"), (t1,), (U,), **_py()))
    add("fmriprep-call", "options", "fMRIPrep's options on the T2-like phantom: -clfrac 0.2 -rbt 18.3 65 90 -T2",
        S(("-clfrac", "0.2", "-rbt", "18.3", "65.0", "90.0", *p, "-T2", "-input", "{0}"), (t2,), (U,),
          **_py(clfrac=0.2, radius=18.3, percentiles=(65.0, 90.0), t2=True)))
    for r, tier in (("5", "smoke"), ("12", "standard"), ("30", "standard")):
        add(f"Urad-{r}", "options", f"-Urad {r}", S(("-Urad", r, *p, "{0}"), (t1,), (U,), tier=tier,
                                                   **(_py(radius=float(r)) if r == "5" else {})))
    add("rbt", "options", "-rbt 10 30 60", S(("-rbt", "10", "30", "60", *p, "{0}"), (t1,), (U,),
                                             **_py(radius=10.0, percentiles=(30.0, 60.0))))
    add("param", "options", "-param, the same as -rbt", S(("-param", "14", "75", "90", *p, "{0}"), (t1,), (U,), tier="standard"))
    for c in ("0.1", "0.5", "0.9"):
        add(f"clfrac-{c}", "options", f"-clfrac {c}", S(("-clfrac", c, *p, "{0}"), (t1,), (U,),
                                                      tier="smoke" if c == "0.5" else "standard"))
    add("mfrac", "options", "-mfrac, the same as -clfrac", S(("-mfrac", "0.3", *p, "{0}"), (t1,), (U,), tier="standard"))
    add("T2", "options", "-T2 on the T2-like phantom", S(("-T2", *p, "{0}"), (t2,), (U,), **_py(t2=True)))
    add("T2-T2", "options", "-T2 -T2: no second inversion, bright edges clipped",
        S(("-T2", "-T2", *p, "{0}"), (t2,), (U,), **_py(t2=2)))
    # AFNI-48 (rule B, decided 2026-10-23): three or more -T2 are refused by default; AFNI leaves the
    # output inverted, which `reproduce` gives.
    add("T2-T2-T2", "options", "-T2 three times: AFNI neither inverts back nor clips; refused by default (AFNI-48)",
        S(("-T2", "-T2", "-T2", *p, "{0}"), (t2,), (U,), fix_refuses="AFNI-48"))
    add("T2-x4", "options", "-T2 four times: as three (AFNI-48)",
        S(("-T2", "-T2", "-T2", "-T2", *p, "{0}"), (t2,), (U,), tier="standard", fix_refuses="AFNI-48"))
    add("T2-x3-EPI", "options", "-T2 three times with -EPI: -EPI sets one -T2, nothing refused",
        S(("-T2", "-T2", "-T2", "-EPI", *p, "{0}"), ("series-f32",), (U,), tier="standard"))
    add("T2up-95", "options", "-T2 -T2up 95", S(("-T2", "-T2up", "95", *p, "{0}"), (t2,), (U,), **_py(t2=True, t2_percentile=95.0)))
    add("GM", "options", "-GM", S(("-GM", *p, "{0}"), (t1,), (U,), **_py(gm=True)))
    add("GM-T2", "options", "-GM -T2", S(("-GM", "-T2", *p, "{0}"), (t2,), (U,), tier="standard"))
    add("nosquash", "options", "-nosquash", S(("-nosquash", *p, "{0}"), (t1,), (U,), **_py(squash=False)))
    add("oink", "options", "-oink, the same as -nosquash", S(("-oink", *p, "{0}"), (t1,), (U,), tier="standard"))
    add("ssave-amsave", "options", "-ssave and -amsave", S(("-ssave", "scale.nii", "-amsave", "am.nii", *p, "{0}"), (t1,),
                                                           (U, "scale.nii", "am.nii"),
                                                           **_py({"image": U, "scale": "scale.nii", "automasked": "am.nii"},
                                                                 save_scale=True, save_automask=True)))
    add("quiet", "options", "-quiet", S(("-quiet", *p, "{0}"), (t1,), (U,)))
    add("verb", "options", "-verb: the automask's messages too", S(("-verb", *p, "{0}"), (t1,), (U,)))
    add("verb-verb", "options", "-verb -verb", S(("-verb", "-verb", *p, "{0}"), (t1,), (U,), tier="standard"))
    add("useall", "options", "-useall: disabled, a warning", S(("-useall", *p, "{0}"), (t1,), (U,), tier="standard"))
    add("inset", "options", "-inset", S(("-inset", "{0}", *p), (t1,), (U,), tier="standard"))
    add("case-insensitive", "options", "-t2 -gm -NOSQUASH: AFNI's strcasecmp options",
        S(("-t2", "-gm", "-NOSQUASH", *p, "{0}"), (t2,), (U,), tier="standard"))
    add("duplo", "options", "1.2 million voxels: the half-size path", S(("-prefix", "uni.nii.gz", "{0}"), ("big",), ("uni.nii.gz",)))
    add("noduplo", "options", "1.2 million voxels with -noduplo", S(("-noduplo", "-prefix", "uni.nii.gz", "{0}"), ("big",),
                                                                   ("uni.nii.gz",), tier="standard", **_py({"image": "uni.nii.gz"}, duplo=False)))
    add("nodouble", "options", "-nodouble, the same as -noduplo", S(("-nodouble", "-Urad", "8", "-prefix", "uni.nii.gz", "{0}"), ("big",),
                                                                   ("uni.nii.gz",), tier="full"))
    add("duplo-T2", "options", "the half-size path with -T2 and -ssave", S(("-T2", "-ssave", "scale.nii.gz", "-prefix", "uni.nii.gz", "{0}"),
                                                                          ("big-t2",), ("uni.nii.gz", "scale.nii.gz"), tier="standard"))
    add("EPI", "options", "-EPI on a 4D series", S(("-EPI", *p, "{0}"), ("series-i16",), (U,), **_py(epi=True)))
    add("EPI-GM", "options", "-EPI -GM: -GM turned off", S(("-EPI", "-GM", *p, "{0}"), ("series-f32",), (U,), tier="standard"))
    add("EPI-3D", "options", "-EPI on one volume: a warning, -T2 kept", S(("-EPI", *p, "{0}"), (t2,), (U,), tier="standard"))
    add("4D-without-EPI", "options", "a 4D series without -EPI: sub-brick #0 only", S((*p, "{0}"), ("series-i16",), (U,)))

    # --- Storage types, scaling and geometry ---------------------------------------------------
    for img in ("head-i16", "head-u8", "head-i16-slope2", "head-i16-slope1", "head-f64", "head-i8", "head-nan",
                "head-oblique", "head-qform-only", "head-las"):
        tier = "smoke" if img in ("head-i16", "head-oblique") else "standard"
        add(img, "datatypes", f"{img}, with -ssave", S(("-ssave", "scale.nii", *p, "{0}"), (img,), (U, "scale.nii"), tier=tier))
    add("head-f32-slope-inter", "defects", "float32 with scl_slope and scl_inter (AFNI-02)",
        S((*p, "{0}"), ("head-f32-slope-inter",), (U,)))
    add("head-metre", "defects", "a grid in metres (AFNI-07)", S((*p, "{0}"), ("head-metre",), (U,)))
    add("head-nearly-plumb", "defects", "a grid 0.009 degrees from plumb (AFNI-16)", S((*p, "{0}"), ("head-nearly-plumb",), (U,)))
    add("nifti-scale-no", "defects", "AFNI_NIFTI_SCALE=NO on int16 with an intercept (AFNI-23)",
        S((*p, "{0}"), ("head-i16-intercept",), (U,), env=(("AFNI_NIFTI_SCALE", "NO"),)))
    add("T2up-100", "defects", "-T2 -T2up 100: one past the sorted values (AFNI-45)",
        S(("-T2", "-T2up", "100", *p, "{0}"), (t2,), (U,), **_py(t2=True, t2_percentile=100.0)))
    add("T2up-99.99", "defects", "-T2 -T2up 99.99 on a small mask: one past the values too (AFNI-45)",
        S(("-T2", "-T2up", "99.99", *p, "{0}"), ("slice",), (U,), tier="standard"))
    add("EPI-negative-slope", "defects", "-EPI on a series with scl_slope -2 (AFNI-01)",
        S(("-EPI", *p, "{0}"), ("series-negative",), (U,)))
    add("slab", "defects", "1 million voxels with an axis of 2: the half-size grid keeps 1 (AFNI-47)",
        S(("-prefix", "uni.nii.gz", "{0}"), ("slab",), ("uni.nii.gz",), tier="standard",
          undefined="AFNI-47", defined=720 * 720))

    # --- Dimensions ----------------------------------------------------------------------------
    for img, desc, tier in (
        ("slice", "a 2D slice (64 x 64 x 1)", "smoke"),
        ("two-slices", "two slices", "standard"),
        ("tiny", "2 x 2 x 2 voxels", "standard"),
        ("zeros", "an image of zeros", "standard"),
        ("constant", "a constant image", "standard"),
        ("series-one", "a 4D file of one volume", "standard"),
    ):
        add(img, "dimensions", desc, S((*p, "{0}"), (img,), (U,), tier=tier))
    add("slice-small", "errors", "a 2D slice smaller than 16 x 16: no automask", S((*p, "{0}"), ("slice-small",), (U,), expect="error"))
    add("slice-small-T2", "errors", "-T2 on a 2D slice smaller than 16 x 16: AFNI crashes (AFNI-46)",
        S(("-T2", *p, "{0}"), ("slice-small",), (U,), expect="crash"))

    # --- Errors --------------------------------------------------------------------------------
    for cid, args, expect in (
        ("Urad-low", ("-Urad", "3", "{0}"), "error"),
        ("Urad-high", ("-Urad", "120", "{0}"), "error"),
        ("rbt-bottom", ("-rbt", "18", "20", "80", "{0}"), "error"),
        ("rbt-order", ("-rbt", "18", "70", "60", "{0}"), "error"),
        ("rbt-top", ("-rbt", "18", "70", "95", "{0}"), "error"),
        ("rbt-missing", ("-rbt", "18", "60"), "error"),
        ("clfrac-low", ("-clfrac", "0.05", "{0}"), "error"),
        ("clfrac-missing", ("-clfrac",), "error"),
        ("T2up-low", ("-T2up", "89", "{0}"), "error"),
        ("T2up-high", ("-T2up", "101", "{0}"), "error"),
        ("T2up-missing", ("-T2up",), "crash"),
        ("prefix-missing", ("-prefix",), "error"),
        ("ssave-missing", ("-ssave",), "error"),
        ("input-twice", ("-input", "{0}", "-input", "{0}"), "error"),
        ("unknown-option", ("-bogus", "{0}"), "error"),
        ("no-dataset", ("-quiet",), "error"),
        ("missing-input", (*p, "/nonexistent/x.nii"), "error"),
    ):
        add(cid, "errors", " ".join(args).replace("{0}", "<input>"),
            S(args, (t1,), (U,), expect=expect, tier="smoke" if cid in ("unknown-option", "T2up-missing") else "standard"))
    add("default-prefix", "divergences", "no -prefix: AFNI writes its own format (Unifized+orig)",
        S(("{0}",), (t1,), ("Unifized+orig.HEAD",), expect="divergence", tier="standard",
          reason="AFNI's default prefix 'Unifized' is its own dataset format, which larmorx does "
          "not write: it refuses (NIfTI only)."))

    # --- AFNI's global options and environment variables --------------------------------------
    taken = ((U, "taken"),)
    add("exists", "global", "an existing output (AFNI-11)", S((*p, "{0}"), (t1,), (U,), files=taken, fix_refuses="AFNI-11"))
    add("overwrite", "global", "-overwrite", S(("-overwrite", *p, "{0}"), (t1,), (U,), files=taken))
    add("deconflict-yes", "global", "AFNI_DECONFLICT=YES: renamed", S((*p, "{0}"), (t1,), (U, "uni_AA1.nii"), files=taken,
                                                                     env=(("AFNI_DECONFLICT", "YES"),)))
    add("ssave-exists", "global", "an existing -ssave output (AFNI-11)", S(("-ssave", "scale.nii", *p, "{0}"), (t1,), (U, "scale.nii"),
                                                                          files=(("scale.nii", "taken"),), fix_refuses="AFNI-11", tier="standard"))
    add("D-write-type-2", "global", "-DAFNI_NIFTI_WRITE_TYPE=2", S(("-DAFNI_NIFTI_WRITE_TYPE=2", *p, "{0}"), (t1,), (U,), tier="standard"))
    add("quiet-startup", "environment", "AFNI_QUIET_STARTUP=YES", S((*p, "{0}"), (t1,), (U,), env=(("AFNI_QUIET_STARTUP", "YES"),)))
    add("message-prefix", "environment", "AFNI_MESSAGE_PREFIX", S(("-useall", *p, "{0}"), (t1,), (U,),
                                                                  env=(("AFNI_MESSAGE_PREFIX", "UN"),), tier="standard"))
    add("echo-commandline", "environment", "AFNI_ECHO_COMMANDLINE=YES", S((*p, "{0}"), (t1,), (U,),
                                                                          env=(("AFNI_ECHO_COMMANDLINE", "YES"), ("AFNI_DONT_LOGFILE", None)), tier="standard"))
    add("prefix-with-blank", "global", "a -prefix with a blank (AFNI-12)", S(("-prefix", "a b.nii", "{0}"), (t1,), ("a b.nii",), fix_writes="AFNI-12"))
    add("input-dir-with-blank", "global", "an input in a directory with a blank (AFNI-18)",
        S((*p, "{0}"), ("blank:head-f32",), (U,), tier="standard", fix_writes="AFNI-18"))
    add("missing-directory", "global", "an output in a missing directory (AFNI-26)",
        S(("-prefix", "nodir/uni.nii", "{0}"), (t1,), ("nodir/uni.nii",), tier="standard",
          fix_refuses="AFNI-26"))

    # --- Real data -----------------------------------------------------------------------------
    for i, (label, _) in enumerate(ph.fmriprep_runs()):
        add(label, "fmriprep", f"fMRIPrep's call on its BOLD reference's input ({label})",
            S(("-clfrac", "0.2", "-rbt", "18.3", "65.0", "90.0", "-prefix", "uni.nii.gz", "-T2", "-input", "{0}"),
              (f"fmriprep:{label}:unifize",), ("uni.nii.gz",), tier="smoke" if i == 0 else "standard",
              band=(1, 6), fmriprep=(("uni.nii.gz", f"fmriprep:{label}:uni"),),
              **_py({"image": "uni.nii.gz"}, clfrac=0.2, radius=18.3, percentiles=(65.0, 90.0), t2=True)))
    o = "td:openneuro"
    for name, t1w, tier in (
        ("ds000005", f"{o}/ds000005/sub-01/anat/sub-01_T1w.nii.gz", "standard"),
        ("ds001600", f"{o}/ds001600/sub-1/anat/sub-1_T1w.nii.gz", "full"),
        ("ds005040", f"{o}/ds005040/sub-002/anat/sub-002_T1w.nii.gz", "full"),
        ("ds005454", f"{o}/ds005454/sub-16/anat/sub-16_acq-mp2rage_T1w.nii.gz", "full"),
        ("ds005752", f"{o}/ds005752/sub-ON01016/ses-01/anat/sub-ON01016_ses-01_acq-FSPGR_rec-SCIC_T1w.nii.gz", "full"),
    ):
        add(f"T1w-{name}", "real", f"AFNI's default on the T1w of {name} (the half-size path)",
            S(("-ssave", "scale.nii.gz", "-prefix", "uni.nii.gz", "{0}"), (t1w,), ("uni.nii.gz", "scale.nii.gz"),
              tier=tier, band=(6,)))
    add("T1w-GM-ds000005", "real", "-GM on the T1w of ds000005",
        S(("-GM", "-prefix", "uni.nii.gz", "{0}"), (f"{o}/ds000005/sub-01/anat/sub-01_T1w.nii.gz",), ("uni.nii.gz",), tier="full"))
    for name, bold, tier in (
        ("ds000005", f"{o}/ds000005/sub-01/func/sub-01_task-mixedgamblestask_run-01_bold.nii.gz", "standard"),
        ("ds005454", f"{o}/ds005454/sub-16/func/sub-16_task-rest_bold.nii.gz", "full"),
    ):
        add(f"EPI-{name}", "real", f"-EPI on the BOLD series of {name}",
            S(("-EPI", "-prefix", "uni.nii.gz", "{0}"), (bold,), ("uni.nii.gz",), tier=tier, band=(6,)))
    # fmt: on
    return out


@functools.cache
def _scenario_list() -> list[tuple[str, str, str, ar.Scenario]]:
    return _scenarios()


#: AFNI-75's cases (``specs/3dUnifize.md`` §9.1, §10): ``-EPI`` on 24 × 24 × 16 series of tiny
#: voxels, as (name, voxel size in the header's units, NIfTI's unit code, volumes, radius).
EPI_VOXELS = (
    ("um3", 3.0, 3, 3, "6100.0"),
    ("um1", 1.0, 3, 3, "18300.0"),
    ("mm0.05", 0.05, 2, 3, "366.0"),
    ("um3-one", 3.0, 3, 1, "6100.0"),
    ("mm0.3", 0.3, 2, 3, "61.0"),
    ("mm3", 3.0, 2, 3, "6.1"),
)
#: The memory each guarded case needs with the suite's two OpenMP threads (§9.1's formula).
EPI_NEED = {
    "um3": "2.834e4 GiB",
    "um1": "7.651e5 GiB",
    "mm0.05": "6.12 GiB",
    "um3-one": "2.834e4 GiB",
}
HINT = "(a header whose units are micrometres makes the voxels 1000 times smaller than they look)"


def _epi_phantom(path: Path, sizes: tuple[float, float, float], units: int, nt: int) -> Path:
    """A head-like EPI phantom of 24 × 24 × 16 voxels and ``nt`` volumes (plain arithmetic and
    seeded noise) at ``path``, its voxel sizes in the header's own units (NIfTI's code
    ``units``)."""
    import nibabel as nib
    import numpy as np

    if path.exists():
        return path
    shape = (24, 24, 16)
    x, y, z = np.meshgrid(*(np.arange(n) for n in shape), indexing="ij")
    r2 = ((x - 11.5) / 9) ** 2 + ((y - 11.5) / 10) ** 2 + ((z - 7.5) / 6) ** 2
    base = 800.0 * np.clip(1.2 - r2, 0, 1) + 300.0 * (r2 < 0.5)
    rng = np.random.default_rng(75)
    data = np.stack([base * (1 + 0.01 * t) + rng.normal(0, 5, shape) for t in range(nt)], -1)
    affine = np.diag([*sizes, 1.0])
    affine[:3, 3] = [-(n - 1) * d / 2 for n, d in zip(shape, sizes, strict=True)]
    img = nib.Nifti1Image(data.astype(np.float32), affine)
    img.header["xyzt_units"] = units | 8
    img.header["pixdim"][4] = 2.0
    nib.save(img, path)
    return path


def _epi_voxels(name: str) -> Path:
    """The series ``epi-voxels:<name>`` of :data:`EPI_VOXELS`."""
    _, size, units, nt, _ = next(e for e in EPI_VOXELS if e[0] == name)
    return _epi_phantom(ar.generated_dir() / f"epi-voxels-{name}.nii", (size,) * 3, units, nt)


def _f32(bits: int) -> float:
    """The float32 of a bit pattern, as a Python float (exact)."""
    import struct

    return struct.unpack("<f", struct.pack("<I", bits))[0]


#: GLIBC-05's cases (``specs/cbrtf.md``; ``specs/3dUnifize.md`` §7): ``-EPI`` on the phantom of
#: :func:`_epi_phantom` with voxel sizes in millimetres (float32; bit patterns where no short
#: decimal gives them) whose radius glibc 2.35's ``cbrtf`` (AFNI's, and larmorx's ``reproduce``)
#: and the correctly rounded cube root (larmorx's default) set differently, as (name, voxel
#: sizes, volumes, whether the default names GLIBC-05, description). ``reproduce`` must give
#: AFNI's bytes in every case; the default its own result, GLIBC-05 named exactly where the
#: ball of neighbours within the grid or the printed radius changes.
CBRTF_VOXELS = (
    ("ball", (2.0, 2.0, _f32(0x403F83E6)), 3, True,
     "-EPI on 3 volumes of 2 x 2 x 2.9924254 mm voxels (11.969702 mm³): radius 8 voxels with "
     "glibc's cube root, 7.9999995 correctly rounded; the ball of neighbours differs by its 6 "
     "points 8 voxels away"),
    ("ball-one", (2.0, 2.0, _f32(0x403F83E6)), 1, True,
     "the same voxels, one volume (the plain path with -EPI's radius): the ball differs"),
    ("printed", (1.0, 1.0, _f32(0x3F85647F)), 3, True,
     "-EPI on 1 x 1 x 1.0421294 mm voxels: radius 18.050001 voxels (printed 18.1) with glibc's "
     "cube root, 18.05 (printed 18.0) correctly rounded; the same ball"),
    ("same-result", (2.4, 2.4, 2.78), 3, False,
     "-EPI on 2.4 x 2.4 x 2.78 mm voxels: radii one unit apart in the last place (7.2604237 "
     "voxels with glibc's cube root, 7.2604246 correctly rounded), the same ball and printed "
     "radius: no notice"),
    ("ordinary", (3.4375, 3.4375, 4.0), 3, False,
     "-EPI on ordinary 3.4375 x 3.4375 x 4 mm voxels: the same radius (5.061384 voxels) with "
     "both cube roots, no notice"),
)  # fmt: skip


def _epi_cbrtf(name: str) -> Path:
    """The series ``epi-cbrtf:<name>`` of :data:`CBRTF_VOXELS`."""
    _, sizes, nt, _, _ = next(e for e in CBRTF_VOXELS if e[0] == name)
    return _epi_phantom(ar.generated_dir() / f"epi-cbrtf-{name}.nii", sizes, 2, nt)


def _unifize_input(name: str) -> Path:
    if name.startswith("epi-voxels:"):
        return _epi_voxels(name.removeprefix("epi-voxels:"))
    if name.startswith("epi-cbrtf:"):
        return _epi_cbrtf(name.removeprefix("epi-cbrtf:"))
    return _input(name)


def _guard_scenarios() -> list[tuple[str, str, str, ar.Scenario]]:
    """AFNI-75's guard (§10), for the clean-room original: the voxels whose ball exceeds the
    limit given (``LARMORX_MEMORY_LIMIT=4G``, so the cases depend on no machine) are refused by
    default with the memory, the radius and the voxels named, nothing written (AFNI is not run:
    it would fill the machine); 0.3 mm and 3 mm voxels run, compared with AFNI."""
    out = []
    for name, size, units, nt, radius in EPI_VOXELS:
        unit = "µm" if units == 3 else "mm"
        vols = "one volume" if nt == 1 else f"{nt} volumes"
        desc = f"-EPI on {vols} of {size:g} {unit} voxels (radius {radius} voxels)"
        if name in EPI_NEED:
            mm = f"{size / 1000 if units == 3 else size:g}"
            text = (
                f"about {EPI_NEED[name]}",
                f"radius {radius} voxels",
                f"{mm} x {mm} x {mm} mm",
                "the 4.00 GiB of LARMORX_MEMORY_LIMIT",
                *((HINT,) if units == 3 else ()),
            )
            s = S(("-EPI", "-prefix", U, "{0}"), (f"epi-voxels:{name}",), (U,),
                  tier="smoke" if name == "um3" else "standard", expect="guard",
                  env=(("LARMORX_MEMORY_LIMIT", "4G"),), guard_text=text)  # fmt: skip
            out.append((f"guard/epi-{name}", "AFNI-75's guard", f"{desc}: refused (AFNI-75)", s))
        else:
            s = S(("-EPI", "-prefix", U, "{0}"), (f"epi-voxels:{name}",), (U,), tier="standard",
                  env=(("LARMORX_MEMORY_LIMIT", "4G"),))  # fmt: skip
            out.append((f"guard/epi-{name}", "AFNI-75's guard", f"{desc}: not refused", s))
    return out


def _cbrtf_scenarios() -> list[tuple[str, str, str, ar.Scenario]]:
    """GLIBC-05's cases (:data:`CBRTF_VOXELS`), for the clean-room original: compared with AFNI
    in ``reproduce`` (glibc's cube root, AFNI's bytes), the default's fix pass naming GLIBC-05
    exactly where the result changes; the first also through the Python API."""
    out = []
    for name, _, _, _, desc in CBRTF_VOXELS:
        s = S(("-EPI", "-prefix", U, "{0}"), (f"epi-cbrtf:{name}",), (U,),
              tier="smoke" if name == "ball" else "standard",
              **(_py(epi=True) if name == "ball" else {}))  # fmt: skip
        out.append((f"glibc-05/epi-{name}", "GLIBC-05's cube root", desc, s))
    return out


def _check_cbrtf_case(case: Case, checks: CheckList) -> None:
    """GLIBC-05's own checks on a case of :func:`_cbrtf_scenarios`: ``reproduce`` gives AFNI's
    values bit for bit (not only within the suite's tolerance); the default names GLIBC-05
    exactly where :data:`CBRTF_VOXELS` says; and the default's output and messages are AFNI's
    with CORE-MATH's correctly rounded functions preloaded (``cbrtf`` among them): the default
    is AFNI's method with the correctly rounded cube root."""
    import tempfile

    name = case.id.removeprefix("glibc-05/epi-")
    named = next(e[3] for e in CBRTF_VOXELS if e[0] == name)
    s: ar.Scenario = case.payload
    lib = ar.crmath_preload()
    if lib is None:
        checks("the default: AFNI with correctly rounded functions", True, value="not run",
               threshold="reported", detail="no CORE-MATH preload with tanhf and cbrtf")  # fmt: skip
    else:
        with (
            afni_versions.using("original"),
            tempfile.TemporaryDirectory(prefix="lx-glibc05-") as t,
        ):
            a_dir, l_dir = Path(t) / "afni-correctly-rounded", Path(t) / "larmorx-fix"
            ar._setup(a_dir, s)
            ar._setup(l_dir, s)
            args = ar._arguments(s, [_unifize_input(i) for i in s.inputs])
            env = ar.environment(s.env)
            afni = ar.run_afni(PROGRAM, args, dict(env, LD_PRELOAD=str(lib)), a_dir,
                               never=ar.case_never(s))  # fmt: skip
            ours = ar.run_larmorx(PROGRAM, args, d13.FIX, env, l_dir, "original")
            # Without colour codes: larmorx writes AFNI's colours in `reproduce` only.
            a_err, l_err = (
                ar._ANSI.sub("", t) for t in (ar.afni_text(afni.err), ar.larmorx_text(ours.err))
            )
            same_text = afni.code == ours.code and afni.out == ours.out and a_err == l_err
            c = ar.compare_files(a_dir / U, l_dir / U) if (a_dir / U).is_file() else None
            checks(
                "the default: AFNI's output and messages with CORE-MATH's correctly rounded functions",
                same_text and c is not None and c.identical,
                metric="values that differ",
                value=None if c is None else c.differing,
                threshold=0,
                detail=("messages identical (colour codes aside)" if same_text
                        else ar.first_difference(a_err, l_err))
                + ("" if c is None else f"; {U}: {c.detail}"),
            )  # fmt: skip
    data = next((c for c in checks.checks if c.name == f"{U}: data"), None)
    checks(
        "reproduce: AFNI's values bit for bit (glibc 2.35's cube root)",
        data is not None and data.value == 0,
        metric="values that differ",
        value=None if data is None else data.value,
        threshold=0,
    )
    fix = next((c for c in checks.checks if c.name == d13.FIX_PASS), None)
    entries = (fix.detail.partition(": ")[0] if fix is not None else "").split(", ")
    checks(
        "the default names GLIBC-05" if named else "the default names no GLIBC-05",
        fix is not None and ("GLIBC-05" in entries) == named,
        detail=fix.detail if fix is not None else "no fix pass",
    )


@functools.cache
def _original_scenario_list() -> list[tuple[str, str, str, ar.Scenario]]:
    return [*_scenarios(), *_guard_scenarios(), *_cbrtf_scenarios()]


#: The implementations this suite validates.
IMPLEMENTATIONS = ("original", "replica")


def spec(implementation: str = "original") -> ar.ToolSpec:
    """The suite's runner for ``implementation``. The clean-room original's float32 outputs
    may differ from AFNI's in the last bits where AFNI's squash calls glibc's ``tanhf``, which
    is not correctly rounded (the original's is): within PLAN.md §11.3's deterministic
    arithmetic threshold, bit-identity reported."""
    if implementation not in IMPLEMENTATIONS:
        raise ValueError(f"implementation must be one of {IMPLEMENTATIONS}, not {implementation!r}")
    tolerance = ar.FLOAT_RTOL if implementation == "original" else None
    scenarios = _original_scenario_list if implementation == "original" else _scenario_list
    return ar.ToolSpec(PROGRAM, "unifize", scenarios, _unifize_input, implementation, tolerance)


def cases(tier: str) -> list[Case]:
    return ar.cases_of(spec(), tier)


def suite(implementation: str = "original") -> Suite:
    """The 3dUnifize suite for the clean-room original (``larmorx afni 3dUnifize``, in
    process) or the bit-exact replica (in process as well); both through larmorx's command line
    and its Python API."""
    tool_spec = spec(implementation)
    replica = implementation == "replica"
    who = "replica" if replica else "original"

    def run_case(case: Case, checks: CheckList) -> None:
        ar.run_case(tool_spec, case, checks)
        if case.id.startswith("glibc-05/"):
            _check_cbrtf_case(case, checks)

    def suite_cases(tier: str) -> list[Case]:
        # GLIBC-05's cases are the clean-room original's until the replica takes glibc's cube
        # root in `reproduce` too (a replica author's part, docs/next-steps.md).
        return [c for c in cases(tier) if not (replica and c.id.startswith("glibc-05/"))]

    tool = (
        f"the bit-exact replica ({ar.replica_version()}: the translation of AFNI 26.3.05's "
        "source, Apache-2.0 with CC BY 4.0's attribution, part of larmorx since 2026-10-29), "
        "in-process: `larmorx afni 3dUnifize` with `LARMORX_IMPLEMENTATION=replica` and "
        '`lx.afni.unifize(..., implementation="replica")`'
        if replica
        else "`larmorx afni 3dUnifize` / `lx.afni.unifize` (crate larmorx-afni, the Apache-2.0 "
        "clean-room original written from `specs/3dUnifize.md`), with "
        '`implementation="original"`'
    )
    outputs = (
        "the same values, bit for bit, and the same NIfTI header bytes"
        if replica
        else "the same NIfTI header bytes; the values bit for bit, or float32 values within "
        "max |diff| ≤ 1e-5 × max |AFNI output| (PLAN.md §11.3) with bit-identity reported (AFNI's "
        "squash calls glibc's tanhf, which is not correctly rounded on some arguments)"
    )
    return Suite(
        name="afni-unifize-replica" if replica else "afni-unifize",
        title="3dUnifize (replica)" if replica else "3dUnifize",
        tool=tool,
        reference=f"3dUnifize from AFNI ({ar.afni_build(PROGRAM, implementation)}, with OpenMP), the binary built by `scripts/build_afni_oracle.sh`",
        thresholds=[
            (
                "Exit status",
                "the same (both succeed, or both reject the arguments); where AFNI crashes, larmorx exits 1",
            ),
            (
                "Messages",
                "standard output and error byte for byte (AFNI's banner and compile date, larmorx's own lines and the elapsed time set aside)",
            ),
            ("Every output (the unifized image, -ssave, -amsave)", outputs),
        ],
        cases=suite_cases,
        run_case=run_case,
        packages=("nibabel",),
        highlights=lambda results: ar.highlights(
            results,
            PROGRAM,
            "unifize",
            "a missing option value, AFNI-40; an empty automask dereferenced, AFNI-46",
            who,
        ),
        notes=(
            "Both programs run in directories of their own on the same input files, in the same "
            "environment (no AFNI variables of this process, `HOME` an empty directory, "
            "`AFNI_DONT_LOGFILE=YES`, `AFNI_NIFTI_NOEXT=YES`, plus each case's variables); "
            "outputs are compared as stored (data bytes, header bytes decompressed). AFNI runs "
            "with OpenMP at this machine's default thread count unless a case sets "
            "`OMP_NUM_THREADS`; the replica with its own threads (results do not depend on "
            f"them). The {who} is compared in `reproduce` mode; the fix pass runs its default. "
            "The real cases also run AFNI at other OpenMP thread counts and fMRIPrep 25.2.6's own "
            "AFNI (in its image) and report how far they are from the oracle; the fMRIPrep cases "
            f"compare the {who} with fMRIPrep's own output of the node as well. "
            + afni_versions.record_line(implementation)
        ),
    )
