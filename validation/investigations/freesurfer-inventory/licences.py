# SPDX-License-Identifier: Apache-2.0
"""The third-party audit (CLAUDE.md rule 3): for every file the programs on a default path are
built from, the notice in its header, every line elsewhere in it that points at another origin,
its licence family and what that means for porting; and the audit's findings (code inside
FreeSurfer's files that comes from elsewhere), with the programs that reach each one.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: this tool reads FreeSurfer's source and prints lines
of it.

The files: every file holding a function that ``callgraph.py``'s full view reaches from a program
on a default path (what the official binaries contain), marked ``core`` when the core view
reaches it too; every source file of those programs' CMake targets (the 4dfp tools' Fortran
included); the scripts and Python programs of ``programs.toml`` (``source``) and the files beside a
Python program's main file in its own package directory.

    python -P validation/investigations/freesurfer-inventory/licences.py \\
        <workspace>/tags/freesurfer-8.2.0 docs/freesurfer/programs.toml <cmake.json> \\
        <full.json> <core.json> --json <scratch>/licences.json \\
        --toml-out docs/freesurfer/third-party.toml --rng <scratch>/rng.txt

``--toml-out`` writes the machine-readable table of docs/freesurfer/third-party.md (the files and
the findings); ``--rng`` lists the reached functions whose bodies call a random-number source
(the findings about seeds). The licence families, the porting classes and the findings are the
audit's reading of the files (``FAMILY_BY_PACKAGE``, ``FAMILY_BY_FILE``, ``FINDINGS`` below,
each with its evidence in third-party.md); the tool applies them and computes the reach. It lists;
it decides nothing the rules leave to the user (porting class ``question``).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from collections import defaultdict
from pathlib import Path

# The header's notice, first match wins (the first 80 lines of the file)
NOTICES = [
    (
        "FreeSurfer (MGH)",
        r"FreeSurfer Software License|General Hospital Corporation|Martinos Center|freesurfer\.net",
    ),
    ("Washington University", r"Washington University|Mallinckrodt"),
    ("MNI (MINC)", r"Montreal Neurological|McConnell Brain Imaging"),
    ("UCAR/Unidata (netCDF)", r"University Corporation for Atmospheric Research|Unidata"),
    ("GPL", r"GNU General Public License|GNU GPL|\bGPL\b"),
    ("LGPL", r"Lesser General Public|Library General Public|\bLGPL\b"),
    ("Apache", r"Apache License"),
    ("MIT-style", r"Permission is hereby granted, free of charge"),
    ("BSD-style", r"Redistribution and use in source and binary forms"),
    ("permissive, own wording", r"Permission to use, copy, modify"),
    ("public domain", r"public domain"),
    ("other copyright", r"Copyright|\(c\) *\d|©"),
]
# Lines anywhere in a file that point at another origin (first set: copyrights and licences)
MARKERS = re.compile(
    r"Numerical Recipes|Recipes in C|Press,? (et al|Teukolsky|Flannery)|"
    r"Copyright|\(c\) *\d{4}|"
    r"adapted from|taken from|copied from|ported from|translated from|borrowed from|"
    r"based on (the )?(code|algorithm|implementation|work)|with permission|"
    r"GNU General Public|Lesser General Public|\bGPL\b|\bLGPL\b|public domain|"
    r"netlib|LAPACK|EISPACK|LINPACK|MINPACK|Cephes|Moshier|"
    r"Apache License|MIT License|BSD licen[cs]e|Permission is hereby granted",
    re.I,
)
# Second set (relay 3): authors and works that code may come from
ORIGINS = re.compile(
    r"derived from|adapted|port of|Graphics Gems|Knuth|Sedgewick|Burkardt|Algorithm AS|TOMS|"
    r"Th[eé]venaz|Unser|Devillard|Wirth|Singleton|Bevington|M[oö]ller|Trumbore|Eberly|"
    r"Geometric Tools|Winkler|L[uü]e?scher|Marsaglia|\bGSL\b|neurite|Nocedal|Botsch|Kob+elt|"
    r"Dawson|stackoverflow|wikipedia|github\.com",
    re.I,
)
# FreeSurfer's own copyright lines are not markers of another origin
OWN = re.compile(
    r"General Hospital Corporation|FreeSurfer|Martinos|freesurfer\.net|surfer\.nmr", re.I
)
# Calls of a random-number source in a reached function's body (--rng)
RNG = re.compile(
    r"\b(OpenRan1|randomNumber|setRandomSeed|rand|random|srand|srandom|drand48|lrand48|erand48|"
    r"srand48|rand_r|random_r|sc_rng_\w+|sc_ran_\w+|sc_uni\w*|vnl_random|std::mt19937\w*|"
    r"std::default_random_engine|std::random_device|std::shuffle|std::random_shuffle|"
    r"PDFgaussian|PDFerlang|PDFtodSeed)\s*\("
)

# Porting classes (what a finding or a file means for larmorx; third-party.md section 1):
#   portable               into larmorx-freesurfer under FreeSurfer's licence (Part B)
#   portable-with-notice   permissive third-party terms: portable, keeping that notice too
#   clean-room-only        no permission to copy: a clean-room original only
#   published-description  to be written from a published description (paper, book, standard)
#   nothing-to-port        no behaviour of its own (memory helpers, platform code)
#   question               the rules do not settle it: NOTES.md asks the orchestrator
FREESURFER = ("FreeSurfer's licence (LICENSE.txt Part B)", "portable")
WASHU = (
    "Washington University: all rights reserved, no copying or distribution without written "
    "permission",
    "clean-room-only",
)
FAMILY_BY_PACKAGE = {
    "packages/minc": (
        "MNI's MINC notice (permissive); FreeSurfer's overhaul",
        "portable-with-notice",
    ),
    "packages/netcdf": ("UCAR/Unidata's notice (permissive)", "portable-with-notice"),
    "packages/dicom": (
        "RSNA and Washington University's CTN notice (permissive)",
        "portable-with-notice",
    ),
    "packages/xml2": ("libxml2's MIT terms", "portable-with-notice"),
    "packages/expat": ("expat's MIT terms", "portable-with-notice"),
    "packages/jpeg": ("the Independent JPEG Group's licence (permissive)", "portable-with-notice"),
    "packages/tiff": (
        "libtiff's notice (Sam Leffler, Silicon Graphics; permissive)",
        "portable-with-notice",
    ),
    "packages/nrrdio": (
        "NrrdIO's zlib-style terms (Gordon Kindlmann, University of Utah)",
        "portable-with-notice",
    ),
    "packages/nifti": ("public domain (niftilib, znzlib)", "portable"),
    "packages/gifti": ("public domain (gifticlib, NIH/NIMH)", "portable"),
    "packages/dcm2niix": (
        "dcm2niix's BSD terms (Chris Rorden); parts public domain, MIT",
        "portable-with-notice",
    ),
    "packages/cephes": ("Cephes, BSD-3-Clause by the author's permission", "portable-with-notice"),
}
FAMILY_BY_FILE = {
    "tkregister2/tkregister2.cpp": (
        "FreeSurfer/CorTechs Software License (non-commercial)",
        "question",
    ),
    "include/utilsmath.h": ("Geometric Tools, Boost Software License 1.0", "portable-with-notice"),
    "include/json.h": ("nlohmann/json, MIT", "portable-with-notice"),
    "include/argparse.h": (
        "derived from Hilton Bristow's argparse, BSD-3-Clause",
        "portable-with-notice",
    ),
    "utils/argparse.cpp": (
        "derived from Hilton Bristow's argparse, BSD-3-Clause",
        "portable-with-notice",
    ),
    "include/fs_vnl/fs_powell.h": (
        "taken from VXL's vnl_powell, BSD-3-Clause",
        "portable-with-notice",
    ),
    "utils/fs_powell.cpp": ("taken from VXL's vnl_powell, BSD-3-Clause", "portable-with-notice"),
    "utils/fs_lbfgs.cpp": ("FreeSurfer's wrapper of VNL's netlib L-BFGS", "portable-with-notice"),
    "utils/MARS_DT_Boundary.cpp": (
        "MIT CSAIL's MARS project: a copyright line, no terms",
        "question",
    ),
    "utils/min_heap.cpp": ("MIT CSAIL's MARS project: a copyright line, no terms", "question"),
    "AntsDenoiseImageFs/itkAdaptiveNonLocalMeansDenoisingImageFilter.hxx": (
        "Apache-2.0 (ITK)",
        "portable-with-notice",
    ),
    "mri_synthseg/mri_synthseg": (
        "FreeSurfer's licence; neurite's layers Apache-2.0",
        "portable-with-notice",
    ),
    "mri_sclimbic_seg/mri_sclimbic_seg": (
        "FreeSurfer's licence; neurite's layers Apache-2.0",
        "portable-with-notice",
    ),
    "talairach_avi/talairach_avi": FREESURFER,
}
# Header classifications corrected by reading the file
NOTICE_FIXES = {"include/csv.h": "no notice"}  # its "(c)" is code: push_back(c)

# The findings: code inside the audited files that comes from elsewhere. ``where`` entries are
# "file" or "file:function"; programs are computed from the call graph's views.
FINDINGS = [
    {
        "id": "TP-01",
        "title": "OpenRan1, setRandomSeed and randomNumber draw from VXL's vnl_random",
        "where": [
            "utils/numerics.cpp:OpenRan1",
            "utils/utils.cpp:setRandomSeed",
            "utils/utils.cpp:randomNumber",
        ],
        "origin": "VXL's vnl_random (Marsaglia-Zaman subtract-with-borrow seeded by an LCG), as ITK 4.13.2 bundles it in the official build; not Numerical Recipes' ran1",
        "terms": "BSD-3-Clause (VXL)",
        "porting": "portable-with-notice",
        "runs": "every program that sets a seed (-seed, FREESURFER_SEED) or draws numbers",
    },
    {
        "id": "TP-02",
        "title": "The Open* wrappers of numerics.cpp call VNL's linear algebra and optimisers",
        "where": [
            "utils/numerics.cpp:OpenSvdcmp",
            "utils/numerics.cpp:OpenLUMatrixInverse",
            "utils/numerics.cpp:OpenMatrixDeterminant",
            "utils/numerics.cpp:OpenEigenSystem",
            "utils/numerics.cpp:OpenNonSymmetricEigenSystem",
            "utils/numerics.cpp:OpenQRdecomposition",
            "utils/numerics.cpp:OpenDFPMin",
            "utils/numerics.cpp:OpenPowell",
            "utils/numerics.cpp:OpenPowell2",
            "utils/numerics.cpp:sc_linalg_cholesky_decomp",
        ],
        "origin": "VXL's VNL (vnl_svd<float>, vnl_matrix_inverse, vnl_determinant, vnl_symmetric_eigensystem, vnl_real_eigensystem, vnl_qr, vnl_cholesky, fs_powell, fs_lbfgs) in ITK 4.13.2",
        "terms": "BSD-3-Clause (VXL); the netlib parts through v3p",
        "porting": "portable-with-notice",
        "runs": "default paths (matrix inverses, determinants and SVDs in nearly every program)",
    },
    {
        "id": "TP-03",
        "title": "numerics.cpp's 'GSL library replacement': RANLUX389 and the random distributions",
        "where": [
            "utils/numerics.cpp:sc_inc_status",
            "utils/numerics.cpp:sc_rng_get",
            "utils/numerics.cpp:sc_rng_set",
            "utils/numerics.cpp:sc_rng_alloc",
            "utils/numerics.cpp:sc_uni",
            "utils/numerics.cpp:sc_uni_pos",
            "utils/numerics.cpp:sc_ran_flat",
            "utils/numerics.cpp:sc_ran_gaussian",
            "utils/numerics.cpp:sc_ugauss",
            "utils/numerics.cpp:sc_gamma_int",
            "utils/numerics.cpp:sc_gamma_large",
            "utils/numerics.cpp:sc_gamma_frac",
            "utils/numerics.cpp:sc_ran_gamma",
            "utils/numerics.cpp:sc_ran_fdist",
            "utils/numerics.cpp:sc_ran_chisq",
            "utils/numerics.cpp:sc_ran_tdist",
            "utils/numerics.cpp:sc_ran_exponential",
        ],
        "origin": "a statement-by-statement transcription, renamed, of GSL 1.8's rng/ranlux.c and randist/{gamma,gauss,tdist,fdist,chisq,exponential,flat}.c (James Theiler, Brian Gough); the algorithms are Luescher's RANLUX with F. James's seeding and Knuth's and Marsaglia's methods",
        "terms": "GSL is GPL-2.0-or-later (1.8; GPL-3.0-or-later since 1.12); the file carries FreeSurfer's notice only",
        "porting": "question",
        "runs": "mri_coreg by default (coordinate and intensity dithering on, seed 53: bbregister --init-coreg in fMRIPrep); mris_expand only with a random thickness map (-tmap_std)",
    },
    {
        "id": "TP-04",
        "title": "The cubic spline of mri_normalize is John Burkardt's spline library",
        "where": [
            "utils/numerics.cpp:SplineCubicSet",
            "utils/numerics.cpp:SplineCubicValue",
            "utils/numerics.cpp:d3_np_fs",
            "utils/numerics.cpp:OpenSpline",
            "utils/numerics.cpp:OpenSplint",
        ],
        "origin": "John Burkardt's C++ spline library (Florida State University, the csit.fsu.edu address of about 2006); the function comments are his",
        "terms": "his page says MIT today (checked 2026-11-12); the terms of the copy FreeSurfer took are not recorded",
        "porting": "question",
        "runs": "mri_normalize on every default path (MRInormalize calls MRIsplineNormalize unconditionally, utils/mrinorm.cpp 749)",
    },
    {
        "id": "TP-05",
        "title": "The B-spline interpolation and pyramids of mriBSpline.cpp follow EPFL's code",
        "where": ["utils/mriBSpline.cpp"],
        "origin": "Martin Reuter's 3D version of P. Thevenaz's 2D code (bigwww.epfl.ch, coeff.c: no licence statement in the file) and M. Unser's 1992 NIH pyramid filters; Thevenaz, Blu, Unser, IEEE TMI 19 (2000) 739-758",
        "terms": "the file carries FreeSurfer's notice; EPFL's code states no terms; Unser's 1992 filters were written at NIH",
        "porting": "portable-with-notice",
        "runs": "mri_robust_template's final resampling (cubic B-spline by default) with several T1w images; the robust tools' pyramid for large images; mri_convert and mri_vol2vol only with cubic interpolation",
    },
    {
        "id": "TP-06",
        "title": "Numerical Recipes' nrutil allocators",
        "where": [
            "utils/numerics.cpp:vector",
            "utils/numerics.cpp:matrix",
            "utils/numerics.cpp:free_vector",
            "utils/numerics.cpp:free_matrix",
        ],
        "origin": "Numerical Recipes in C's nrutil.c allocation helpers, verbatim (names, NR_END, FREE_ARG, comments); the 'silly numerical recipes' comments of matrix.cpp and dmatrix.cpp are FreeSurfer's own code keeping NR's 1-based convention",
        "terms": "Numerical Recipes' terms (not checked: nothing of it is ported)",
        "porting": "nothing-to-port",
        "runs": "memory layout only (1-based float arrays)",
    },
    {
        "id": "TP-07",
        "title": "Quickselect and medians",
        "where": [
            "utils/utils.cpp:kth_smallest",
            "utils/utils.cpp:quick_select",
            "mri_robust_register/RobustGaussian.cpp",
        ],
        "origin": "N. Devillard's 1998 code (Wirth's algorithm and NR's description)",
        "terms": "public domain (stated in the files)",
        "porting": "portable",
        "runs": "medians in many programs; mri_robust_template's robust statistics",
    },
    {
        "id": "TP-08",
        "title": "fs_powell and fs_lbfgs",
        "where": ["utils/fs_powell.cpp", "include/fs_vnl/fs_powell.h", "utils/fs_lbfgs.cpp"],
        "origin": "fs_powell is VXL's vnl_powell (with vnl_brent) changed by FreeSurfer; fs_lbfgs calls VNL's v3p_netlib_lbfgs_ (J. Nocedal's 1990 Fortran L-BFGS with the More-Thuente line search, f2c-translated)",
        "terms": "BSD-3-Clause (VXL); netlib's code states no terms",
        "porting": "portable-with-notice",
        "runs": "mri_em_register, mri_ca_register, mri_ca_label, mri_coreg, mri_segreg",
    },
    {
        "id": "TP-09",
        "title": "Washington University's 4dfp tools (classic Talairach)",
        "where": ["talairach_avi"],
        "origin": "Avi Snyder's 4dfp registration tools (C and Fortran); the Fortran files without a notice are the same collection (their RCS logs: avi); fftsol.f is R. C. Singleton's 1968 mixed-radix FFT, polfit.f is Bevington's POLFIT (1969, p. 141)",
        "terms": "all rights reserved; no copying or distribution without Washington University's written permission",
        "porting": "clean-room-only",
        "runs": "the classic stream's talairach_avi (mpr2mni305); v8 uses rca-talairach",
    },
    {
        "id": "TP-10",
        "title": "tkregister2_cmdl is under the FreeSurfer/CorTechs licence",
        "where": ["tkregister2/tkregister2.cpp"],
        "origin": "Martin Sereno and Anders Dale, 1996; Doug Greve, 2002; CorTechs Labs and MGH",
        "terms": "non-commercial internal research and education by not-for-profit entities, no commercially sponsored research, modifications' source to be made available (distribution/docs/license.cortechs.txt, paragraphs 2(d)-(f))",
        "porting": "question",
        "runs": "fMRIPrep's bbregister: the header registration (--regheader) and the conversions --ltaout, --fslregout",
    },
    {
        "id": "TP-11",
        "title": "The ITK in the official binaries is 4.13.2, not 5.4.5",
        "where": [
            "AntsN4BiasFieldCorrectionFs/AntsN4BiasFieldCorrectionFs.cpp",
            "AntsDenoiseImageFs/AntsDenoiseImageFs.cpp",
            "AntsDenoiseImageFs/itkAdaptiveNonLocalMeansDenoisingImageFilter.hxx",
        ],
        "origin": "ITK 4.13.2 (CMakeLists.txt 690: Ubuntu22 with GCC 8 takes itk/4.13.2_ubuntu20; the binaries' build paths name itk/4.13.2), with its VNL; the adaptive non-local means filter is ITK's (Insight Software Consortium)",
        "terms": "Apache-2.0 (ITK 4.x), BSD-3-Clause (VXL)",
        "porting": "portable-with-notice",
        "runs": "N4 and the denoising on both streams; every VNL call",
    },
    {
        "id": "TP-12",
        "title": "The network programs carry upstream code under Apache-2.0",
        "where": [
            "mri_synthseg/mri_synthseg",
            "mri_sclimbic_seg/mri_sclimbic_seg",
            "mri_synthstrip/mri_synthstrip",
            "mri_synthmorph/mri_synthmorph",
            "mri_synthmorph/synthmorph/registration.py",
            "mri_synthmorph/synthmorph/utils.py",
        ],
        "origin": "neurite's U-Net layers 'copied with permission' (github.com/adalca/neurite); SynthSeg (upstream BBillot/SynthSeg), SynthMorph (voxelmorph), SynthStrip are FreeSurfer's distributions; surfa is FreeSurfer's I/O package",
        "terms": "neurite, voxelmorph, SynthSeg upstream: Apache-2.0; surfa: MIT (GitHub, 2026-11-12); the FreeSurfer files carry no notice (Part B)",
        "porting": "portable-with-notice",
        "runs": "v8 and classic: SynthSeg, SynthStrip, SynthMorph, ScLimbic (weights: data-files.md)",
    },
    {
        "id": "TP-13",
        "title": "Small permissive pieces",
        "where": [
            "include/json.h",
            "include/argparse.h",
            "utils/argparse.cpp",
            "include/utilsmath.h",
            "include/fnv_hash.h",
            "include/fnvhash.h",
        ],
        "origin": "nlohmann/json 3.x (MIT; Grisu2 by F. Loitsch, MIT; UTF-8 decoder by B. Hoehrmann, MIT); argparse derived from H. Bristow's (BSD-3-Clause); Geometric Tools' distance and box code (D. Eberly, Boost 1.0); the FNV hash (public domain)",
        "terms": "MIT, BSD-3-Clause, BSL-1.0, public domain",
        "porting": "portable-with-notice",
        "runs": "JSON outputs (number formatting), command lines of the newer programs, mris_volmask's distances, hashing",
    },
    {
        "id": "TP-14",
        "title": "MIT CSAIL's MARS distance transform",
        "where": ["utils/MARS_DT_Boundary.cpp", "utils/min_heap.cpp"],
        "origin": "the MARS project (Thomas Yeo, Mert Sabuncu), MIT CSAIL, 2006-2008",
        "terms": "a copyright line, no licence terms",
        "porting": "question",
        "runs": "no default path at run time: only MRIScomputeAllDistances (MRISunfold with complete_dist_mat, which mris_sphere never sets) reaches it",
    },
    {
        "id": "TP-15",
        "title": "The C library's generators (not FreeSurfer's code)",
        "where": [
            "AntsN4BiasFieldCorrectionFs/AntsN4BiasFieldCorrectionFs.cpp:main",
            "mri_mask/mri_mask.cpp:main",
            "utils/pdf.cpp:PDFgaussian",
            "utils/mri.cpp:MRIfixTkReg",
            "utils/mri.cpp:MRIrandn",
            "mri_robust_register/mri_robust_template.cpp:getRandomNumber",
            "utils/utils.cpp:setRandomSeed",
        ],
        "origin": "drand48/srand48 (defined by POSIX), rand/srand and srandom (glibc 2.35's random_r on the official build's Ubuntu 22.04)",
        "terms": "drand48: a published definition; glibc: LGPL-2.1-or-later, never copied (a behaviour spec, as specs/glibc-dbl.md)",
        "porting": "published-description",
        "runs": "N4's zero replacement option (srand48(53)), mri_mask's noise option, MRIrandn's noise options, mri_robust_template's random first time point (only without --inittp)",
    },
    {
        "id": "TP-16",
        "title": "Credits with no code copied, and code not compiled",
        "where": [
            "utils/mrisutils.cpp:MRISvolumeTH3",
            "mris_curvature_stats/mris_curvature_stats.cpp:AlmostEqual2sComplement",
            "mris_remesh/remesher.cpp",
        ],
        "origin": "MRISvolumeTH3 follows A. M. Winkler's srf2vol method; AlmostEqual2sComplement B. Dawson's article; the remesher Botsch and Kobbelt (2004); DICOMRead.cpp's scandir/alphasort (J.-R. Hill, 2000) are inside #ifdef SunOS and not compiled on Linux; MRISOBBTree.h's Moller-Trumbore test is commented out",
        "terms": "FreeSurfer's licence",
        "porting": "portable",
        "runs": "mris_anatomical_stats and mris_convert (volumes), mris_curvature_stats, mris_remesh (both streams)",
    },
]

PORTING_ORDER = [
    "question",
    "clean-room-only",
    "published-description",
    "portable-with-notice",
    "portable",
    "nothing-to-port",
]


def classify(head: str) -> str:
    for name, pattern in NOTICES:
        if re.search(pattern, head, re.I if name not in ("GPL", "LGPL") else 0):
            return name
    return "no notice"


def area(rel: str) -> str:
    parts = Path(rel).parts
    if parts[0] == "packages":
        return f"packages/{parts[1]}"
    if parts[0] in ("utils", "include", "scripts", "talairach_avi"):
        return parts[0]
    return "program directories"


def family(rel: str, notice: str) -> tuple[str, str]:
    if rel in FAMILY_BY_FILE:
        return FAMILY_BY_FILE[rel]
    a = area(rel)
    if a in FAMILY_BY_PACKAGE:
        return FAMILY_BY_PACKAGE[a]
    if a == "talairach_avi":
        return WASHU
    if notice == "Apache":
        return ("Apache-2.0", "portable-with-notice")
    return FREESURFER


def q(s: str) -> str:
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'


def qlist(items) -> str:
    return "[" + ", ".join(q(str(i)) for i in items) + "]"


def finding_programs(
    view: dict, where: list[str], on_path: set[str], scripts: dict[str, str]
) -> list[str]:
    """The programs on a default path reaching a finding; scripts and Python programs (not in
    the call graph) by their ``source`` in programs.toml."""
    progs = {scripts[w] for w in where if w in scripts}
    for name, v in view["programs"].items():
        if name not in on_path:
            continue
        for key in v["reached"]:
            f, _, fn = key.split(":", 2)
            for w in where:
                wf, _, wfn = w.partition(":")
                # "talairach_avi": a whole directory; "file:function": one function
                in_file = f == wf or ("." not in wf and f.startswith(wf + "/"))
                if in_file and (not wfn or fn == wfn or fn.endswith("::" + wfn)):
                    progs.add(name)
                    break
    return sorted(progs)


def rng_calls(root: Path, view: dict, on_path: set[str]) -> list[str]:
    reached: dict[str, set[str]] = defaultdict(set)
    for name, v in view["programs"].items():
        if name in on_path:
            for key in v["reached"]:
                reached[key].add(name)
    cache: dict[str, list[str]] = {}
    out: dict[tuple[str, str], set[str]] = defaultdict(set)
    for key, progs in reached.items():
        path, line, _name = key.split(":", 2)
        info = view["functions"].get(key)
        if not info:
            continue
        if path not in cache:
            p = root / path
            cache[path] = (
                p.read_text(encoding="utf-8", errors="replace").splitlines() if p.is_file() else []
            )
        body = cache[path][int(line) - 1 : int(line) - 1 + info["lines"]]
        for b in body:
            for m in RNG.finditer(re.sub(r"//.*", "", b)):
                out[(key, m.group(1))].update(progs)
    rows = []
    for (key, fn), progs in sorted(out.items()):
        shown = ",".join(sorted(progs)) if len(progs) <= 8 else f"{len(progs)} programs"
        rows.append(f"{fn:22s} {key:75s} {shown}")
    return rows


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("root", type=Path)
    parser.add_argument("programs", type=Path)
    parser.add_argument("cmake_json", type=Path)
    parser.add_argument("full_json", type=Path)
    parser.add_argument("core_json", type=Path)
    parser.add_argument("--json", type=Path, default=None)
    parser.add_argument("--toml-out", type=Path, default=None)
    parser.add_argument("--rng", type=Path, default=None, help="write the random-number callers")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    table = tomllib.loads(args.programs.read_text(encoding="utf-8"))["program"]
    on_path = {p["name"] for p in table if p["streams"]}
    scripts = {
        p["source"]: p["name"]
        for p in table
        if p["streams"] and p["source"] and p["kind"] not in ("c++", "c", "c+fortran")
    }
    cmake = json.loads(args.cmake_json.read_text(encoding="utf-8"))
    full = json.loads(args.full_json.read_text(encoding="utf-8"))
    core = json.loads(args.core_json.read_text(encoding="utf-8"))

    files: dict[str, dict[str, set]] = {}

    def add(rel: str, why: str, program: str) -> None:
        entry = files.setdefault(rel, {"why": set(), "programs": set()})
        entry["why"].add(why)
        entry["programs"].add(program)

    for view, data in (("full", full), ("core", core)):
        for name, v in data["programs"].items():
            if name not in on_path:
                continue
            for key in v["reached"]:
                add(key.split(":", 1)[0], view, name)
    for name, p in cmake["programs"].items():
        if p is not None and name in on_path:
            for s in p["sources"]:
                add(s, "target source", name)
    for p in table:
        if not p["streams"] or not p["source"] or p["kind"] in ("c++", "c", "c+fortran"):
            continue
        add(p["source"], p["kind"], p["name"])
        if p["kind"].startswith("python"):
            for extra in sorted((root / p["source"]).parent.rglob("*.py")):
                add(str(extra.relative_to(root)), p["kind"] + " package", p["name"])

    finding_of: dict[str, list[str]] = defaultdict(list)
    for f in FINDINGS:
        for w in f["where"]:
            finding_of[w.partition(":")[0]].append(f["id"])

    report: dict[str, dict[str, object]] = {}
    for rel in sorted(files):
        path = root / rel
        if not path.is_file():
            continue
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        notice = NOTICE_FIXES.get(rel) or classify("\n".join(lines[:80]))
        markers, origins = [], []
        for n, line in enumerate(lines, 1):
            if OWN.search(line):
                continue
            if MARKERS.search(line):
                markers.append([n, line.strip()[:140]])
            elif ORIGINS.search(line):
                origins.append([n, line.strip()[:140]])
        fam, porting = family(rel, notice)
        ids = sorted(set(finding_of.get(rel, []) + finding_of.get(Path(rel).parts[0], [])))
        report[rel] = {
            "area": area(rel),
            "lines": len(lines),
            "notice": notice,
            "family": fam,
            "porting": porting,
            "findings": ids,
            "views": sorted(files[rel]["why"]),
            "programs": len(files[rel]["programs"]),
            "markers": markers,
            "origins": origins,
        }
    by: dict[tuple[str, str], list[int]] = defaultdict(lambda: [0, 0])
    for v in report.values():
        key = (v["area"], v["notice"])
        by[key][0] += 1
        by[key][1] += v["lines"]
    print(f"files: {len(report)}, lines: {sum(v['lines'] for v in report.values()):,}")
    for (a, notice), (n, nlines) in sorted(by.items()):
        print(f"  {a:28s} {notice:30s} {n:4d} files {nlines:8,d} lines")
    per_class: dict[str, list[int]] = defaultdict(lambda: [0, 0])
    for v in report.values():
        per_class[v["porting"]][0] += 1
        per_class[v["porting"]][1] += v["lines"]
    for c in PORTING_ORDER:
        if c in per_class:
            print(f"  porting {c:24s} {per_class[c][0]:4d} files {per_class[c][1]:8,d} lines")
    if args.json:
        args.json.write_text(json.dumps(report, indent=1, sort_keys=True) + "\n", encoding="utf-8")
    if args.rng:
        args.rng.write_text("\n".join(rng_calls(root, core, on_path)) + "\n", encoding="utf-8")
    if args.toml_out:
        out = [
            "# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: generated from FreeSurfer 8.2.0's source",
            "# (docs/freesurfer/README.md) by validation/investigations/freesurfer-inventory/licences.py;",
            "# what each field means: docs/freesurfer/third-party.md. Do not edit by hand: regenerate.",
            "",
            "schema = 1",
            'freesurfer = "8.2.0"',
            f"files = {len(report)}",
            f"lines = {sum(v['lines'] for v in report.values())}",
            "",
        ]
        for f in FINDINGS:
            pc = finding_programs(core, f["where"], on_path, scripts)
            pf = finding_programs(full, f["where"], on_path, scripts)
            out += [
                "[[finding]]",
                f"id = {q(f['id'])}",
                f"title = {q(f['title'])}",
                f"where = {qlist(f['where'])}",
                f"origin = {q(f['origin'])}",
                f"terms = {q(f['terms'])}",
                f"porting = {q(f['porting'])}",
                f"runs = {q(f['runs'])}",
                f"programs_core = {qlist(pc)}",
                f"programs_full_only = {qlist(sorted(set(pf) - set(pc)))}",
                "",
            ]
        for rel, v in report.items():
            out += [
                "[[file]]",
                f"path = {q(rel)}",
                f"area = {q(v['area'])}",
                f"lines = {v['lines']}",
                f"notice = {q(v['notice'])}",
                f"family = {q(v['family'])}",
                f"porting = {q(v['porting'])}",
                f"findings = {qlist(v['findings'])}",
                f"core = {'true' if 'core' in v['views'] else 'false'}",
                f"programs = {v['programs']}",
                "",
            ]
        args.toml_out.write_text("\n".join(out), encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
