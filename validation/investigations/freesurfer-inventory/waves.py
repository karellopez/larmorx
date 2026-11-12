# SPDX-License-Identifier: Apache-2.0
"""The dependency map, the waves, the crates and the queue of FreeSurfer 8.2.0's port.

CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: its input is read from FreeSurfer's source (the
call graph of ``callgraph.py``) and its tables name FreeSurfer's files and functions.

    python -P validation/investigations/freesurfer-inventory/waves.py \\
        docs/freesurfer/programs.toml docs/freesurfer/code.toml <core.json> \\
        --runs <workspace>/oracles/freesurfer-8.2.0/runs \\
        --toml docs/freesurfer/waves.toml \\
        --markdown docs/freesurfer/waves.md --queue docs/freesurfer/queue.md

``<core.json>`` is ``callgraph.py --core --json`` (code.md §6). ``--runs`` counts the recorded
steps of each program in the reference runs (the checks of the queue). The Markdown files'
regions between ``<!-- waves.py: <name> -->`` and ``<!-- waves.py: end <name> -->`` are
replaced; the prose around them is written by hand.

What it computes (code.md's core view, a static superset of what a run executes):

- every reached file of FreeSurfer's tree belongs to one **module** (``MODULES``: a piece of a
  crate); a file no module claims is an error;
- every program belongs to one **wave** and one crate (``PLACE``), checked against the user's
  order (the programs both streams share, then the classic-only ones, the v8 networks as a
  parallel track) and against the scripts' calls (a script never comes before what it runs);
- each reached function goes to the first queue item that needs it: **3.1 foundations** take the
  functions that programs of two or more waves reach, or half of the compiled programs; a
  **wave's foundation** takes those that two or more programs of that wave alone reach; a
  **program item** takes its own files and the functions only it reaches. So every line is
  counted once, in the item that brings it.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import tomllib
from collections import Counter, defaultdict
from pathlib import Path

# --------------------------------------------------------------------------------------------
# Modules: (id, crate, kind, title, patterns). kind: "replica" (crates-freesurfer/, FreeSurfer's
# licence), "permissive" (a permissive third-party piece: its replica in the main package,
# placement.toml), "clean-room-only" (no replica: placement.toml), "program" (a program's own
# files). Patterns are matched against the path under the tag; the first match wins.

MODULES: list[tuple[str, str, str, str, list[str]]] = [
    (
        "perm.gifti",
        "larmorx-io",
        "permissive",
        "GIFTI: gifticlib and expat (P-05); the writer's long-double line formatting",
        [r"^packages/gifti/", r"^packages/expat/"],
    ),
    (
        "perm.minc",
        "larmorx-io",
        "permissive",
        "MINC's volume_io: the .xfm reader and writer (P-05; FreeSurfer's changes to the MINC files"
        " in larmorx-freesurfer-io)",
        [r"^packages/minc/", r"^packages/netcdf/"],
    ),
    (
        "perm.nifti",
        "larmorx-io",
        "permissive",
        "niftilib and znzlib (P-05)",
        [r"^packages/nifti/"],
    ),
    (
        "perm.dicom",
        "larmorx-io",
        "permissive",
        "CTN's DICOM library (P-05): the format probe of every volume reader; DICOM input later",
        [r"^packages/dicom/"],
    ),
    (
        "perm.misc",
        "larmorx-core",
        "permissive",
        "miniz, libtiff, NrrdIO, Cephes, the FNV hash (P-05, P-06)",
        [
            r"^packages/dcm2niix/",
            r"^packages/tiff/",
            r"^packages/nrrdio/",
            r"^packages/cephes/",
            r"^packages/jpeg/",
            r"^packages/xml2/",
            r"^include/fnv_?hash\.h$",
        ],
    ),
    (
        "perm.json",
        "larmorx-core",
        "permissive",
        "nlohmann/json (P-06): its number formatting decides JSON outputs' bytes",
        [r"^include/json\.h$"],
    ),
    (
        "perm.geomtools",
        "larmorx-core",
        "permissive",
        "Geometric Tools' distances and bounding boxes (P-06, mris_volmask)",
        [r"^include/utilsmath\.h$"],
    ),
    (
        "perm.argparse",
        "larmorx-core",
        "permissive",
        "the argparse derived from Hilton Bristow's ArgumentParser (P-06): the newer programs'"
        " command lines and messages",
        [r"^utils/argparse\.cpp$", r"^include/argparse\.h$"],
    ),
    (
        "fs.core",
        "larmorx-freesurfer-core",
        "replica",
        "messages, diagnostics, program name, environment, version stamps, the ROMP switch,"
        " timers, paths, signals, the licence check's messages",
        [
            r"^utils/(utils|diag|log|timer|version|machine|fsinit|Progname|cmdargs|fsenv|chklc|"
            r"romp_support|path|sig|xDebug|hips|rgb|error|proto|fsPrintHelp)\.cpp$",
            r"^include/(log|timer|base|fio|romp_support|romp_for_begin|romp_for_end|error|"
            r"proto|macros|const|diag|utils|version|cmdargs|fsenv|chklc|fsinit|Progname|path|"
            r"machine|sig|xDebug|hips|rgb)\.h$",
        ],
    ),
    (
        "fs.maths",
        "larmorx-freesurfer-core",
        "replica",
        "matrices (float and long double), the numerics wrappers over VNL and Cephes, fs_powell,"
        " fs_lbfgs, histograms, statistics, PDFs, random fields",
        [
            r"^utils/(matrix|dmatrix|numerics|fs_powell|fs_lbfgs|fs_lbfgs_observer|"
            r"fs_lbfgs_subject|fs_cost_function|histo|stats|pdf|randomfields|prime|sort|"
            r"evschutils|fsglm|fmriutils_glm)\.cpp$",
            r"^include/fs_vnl/",
            r"^include/(affine|matrix|dmatrix|numerics|histo|stats|pdf|randomfields|prime)\.h$",
            r"^include/affine\.hpp$",
        ],
    ),
    (
        "fs.io",
        "larmorx-freesurfer-io",
        "replica",
        "volume formats: MRIread/MRIwrite (MGH/MGZ, NIfTI, ANALYZE, the format probe, the other"
        " readers), FreeSurfer's tags, .mat, .npy, GIFTI volumes",
        [
            r"^utils/(mriio|fio|fstagsio|tags|matfile|fscnpy|mri_identify|DICOMRead|DICOMRead2|"
            r"signa|Bruker|bfileio|dti|mosaic|MRIio_old|selxavgio|image|gifti|dtk\.fs|fsgdf|"
            r"imageio|corio|afni|nifti_io|mghendian)\.cpp$",
        ],
    ),
    (
        "fs.labels",
        "larmorx-freesurfer-io",
        "replica",
        "colour tables, annotations, labels, register.dat, control points, point sets, CSV",
        [
            r"^utils/(colortab|annotation|label|registerio|ctrpoints|pointset|targetpointset)\.cpp$",
            r"^include/(pointset|csv|colortab|annotation|label|registerio|ctrpoints)\.h$",
        ],
    ),
    (
        "fs.volume",
        "larmorx-freesurfer-volume",
        "replica",
        "the MRI structure, voxels and frames, conforming, regions, voxel lists, resampling,"
        " transforms (LTA, .xfm, FSL matrices), the Talairach helpers, warp fields, CMA labels",
        [
            r"^utils/(mri|mri2|mriset|mrivoxel|mri_conform|mrihisto|region|voxlist|mriprob|"
            r"resample|transform|talairachex|warpfield|mriTransform|cma|mosaic2)\.cpp$",
            r"^include/(mri|transform|resample|cma|warpfield|region|voxlist)\.h$",
        ],
    ),
    (
        "fs.bspline",
        "larmorx-freesurfer-volume",
        "replica",
        "EPFL's B-splines in Reuter's mriBSpline.cpp (P-10: the replica here crediting EPFL; its"
        " own queue item with mri_robust_template)",
        [r"^utils/mriBSpline\.cpp$", r"^include/mriBSpline\.h$"],
    ),
    (
        "fs.filter",
        "larmorx-freesurfer-filter",
        "replica",
        "volume filters and morphology, intensity normalisation (with Burkardt's spline, P-09),"
        " segmentation helpers, flood fill, connected components, fast marching, clusters",
        [
            r"^utils/(mrifilter|mrinorm|mrisegment|mriflood|connectcomp|mripolv|filter|"
            r"mri_fastmarching|ventfix|volcluster|fmriutils|gtm|rfa|diffuse|mriclass|kernel|"
            r"mri_topology)\.cpp$",
        ],
    ),
    (
        "fs.gca",
        "larmorx-freesurfer-gca",
        "replica",
        "GCA atlases and GCA morphs (.gca, .m3z), the linear GCA nodes, MRI morphs, FLASH, the"
        " autoencoder",
        [
            r"^utils/(gca|gcamorph|gcalinearnode|gcalinearprior|gcamcomputeLabelsLinearCPU|"
            r"mrimorph|flash|autoencoder)\.cpp$",
            r"^include/gcalinear(node|prior)\.hpp$",
            r"^include/(gca|gcamorph|mrimorph|flash|autoencoder)\.h$",
        ],
    ),
    (
        "fs.surfio",
        "larmorx-freesurfer-surf",
        "replica",
        "the surface structure and its I/O: MRIS with its generated accessors, reading and"
        " writing surfaces, curvature, patches and overlays, and the metric properties every"
        " read computes (MRISread calls MRIScomputeMetricProperties)",
        [
            r"^utils/(mrisurf|mrisurf_base|mrisurf_io|mrisurf_io_stl|mrisurf_metricProperties|"
            r"mrisurf_metricProperties_faster|MRISurfOverlay)\.cpp$",
            r"^utils/mrisurf_metricProperties_(fast|slow)\.h$",
            r"^include/mrisurf",
        ],
    ),
    (
        "fs.surf",
        "larmorx-freesurfer-surf",
        "replica",
        "the rest of the surface library: topology, values, volume-surface sampling,"
        " projections, the surface hash, realms, icosahedra, spherical maps (mrisp and the .tif"
        " atlas), surface clusters, gcsa, the surface-gradient code (long double)",
        [
            r"^utils/(mrisurf|mrisurf_base|mrisurf_io|mrisurf_io_stl|mrisurf_metricProperties|"
            r"mrisurf_metricProperties_faster|mrisurf_topology|mrisurf_mri|mrisurf_vals|"
            r"mrisurf_project|mrishash|realm|icosahedron|mrisutils|mrisp|surfcluster|"
            r"MRISurfOverlay|vertexRotator|gcsa|tritri|surfgrad|MRISrigidBodyAlignGlobal|field_code)\.cpp$",
            r"^utils/mrisurf_metricProperties_(fast|slow)\.h$",
            r"^include/mrisurf",
            r"^include/(MRISOBBTree|MRISdistancefield|face_barycentric_coords|mrishash|realm|"
            r"icosahedron|mrisutils|mrisp|gcsa|surfgrad|tritri)\.h$",
        ],
    ),
    (
        "fs.mars",
        "larmorx-freesurfer-surf",
        "replica",
        "MIT CSAIL's MARS distance transform (P-13: on no default path; later, with permission)",
        [r"^utils/(MARS_DT_Boundary|min_heap)\.cpp$"],
    ),
    (
        "fs.deform",
        "larmorx-freesurfer-deform",
        "replica",
        "surface deformation: the time steps, the integration, the gradients (dx, dy, dz),"
        " the SSE terms",
        [
            r"^utils/mrisurf_(deform|integrate|compute_dxyz|timeStep|sseTerms)\.cpp$",
            r"^utils/mrisurf_deform\.h$",
        ],
    ),
    (
        "fs.defect",
        "larmorx-freesurfer-defect",
        "replica",
        "the topology fixer's defect code (mrisurf_defect.cpp)",
        [r"^utils/mrisurf_defect\.cpp$"],
    ),
    (
        "fs.robust",
        "larmorx-freesurfer-robust",
        "replica",
        "Reuter's robust registration library (mri_robust_register/'s shared files): RegRobust,"
        " RegPowell, the cost functions, MyMRI, MyMatrix, MultiRegistration, quaternions",
        [r"^mri_robust_register/(?!mri_robust_(register|template)\.cpp$)"],
    ),
    (
        "cr.4dfp",
        "larmorx-fs-classic",
        "clean-room-only",
        "Washington University's 4dfp tools (P-12): a clean room only, at the process boundary"
        " talairach_avi already has",
        [r"^talairach_avi/(?!talairach_avi$)[^/]+\.(c|f)$"],
    ),
]

# --------------------------------------------------------------------------------------------
# Program placement: wave, crate of the replica, and why (when it differs from the plan or from
# what the streams alone would give). Waves: 3.2 volumes, 3.3 atlas, 3.4 first surfaces, 3.5 final
# surfaces and statistics (the shared programs and fMRIPrep's), 3.6 the classic-only programs,
# N the v8 networks' parallel track, 3.7 recon-all and its helpers, later (on no default path).

WAVES = ("3.1", "3.2", "3.3", "3.4", "3.5", "3.6", "3.7", "N", "later")
WAVE_TITLE = {
    "3.1": "Foundations",
    "3.2": "Volumes",
    "3.3": "Atlas",
    "3.4": "First surfaces",
    "3.5": "Final surfaces and statistics",
    "3.6": "The classic-only programs",
    "3.7": "recon-all",
    "N": "The v8 networks (a parallel track)",
    "later": "On no default path (completeness, later)",
}
CRATE = {
    "3.2": "larmorx-freesurfer-volumes",
    "3.3": "larmorx-freesurfer-atlas",
    "3.4": "larmorx-freesurfer-topology",
    "3.5": "larmorx-freesurfer-cortex",
    "3.6": "larmorx-freesurfer-classic",
    "3.7": "larmorx-freesurfer-recon",
    "N": "larmorx-freesurfer-networks",
    "later": "",
}
CLEAN = {
    "3.2": "larmorx-fs-volumes",
    "3.3": "larmorx-fs-atlas",
    "3.4": "larmorx-fs-topology",
    "3.5": "larmorx-fs-cortex",
    "3.6": "larmorx-fs-classic",
    "3.7": "larmorx-fs-recon",
    "N": "larmorx-fs-networks",
    "later": "",
}

PLACE: dict[str, tuple[str, str]] = {
    # 3.2 volumes: both streams' volume programs and fMRIPrep's volume and registration programs
    "mri_convert": ("3.2", ""),
    "mri_info": ("3.2", "the scripts parse its printed header (not in the plan's waves)"),
    "mri_add_xform_to_header": ("3.2", ""),
    "lta_convert": ("3.2", ""),
    "fs_temp_file": ("3.2", "recon-all's temporary file names (not in the plan's waves)"),
    "getfullpath": ("3.2", "a helper of seg2cc, rca-talairach and fs-synthmorph-reg"),
    "mri_robust_template": (
        "3.2",
        "with EPFL's B-splines (fs.bspline) as an item of their own, the user's request",
    ),
    "mri_robust_register": ("3.2", ""),
    "mri_coreg": ("3.2", "uses the clean-room RANLUX389 and distributions (P-08)"),
    "AntsN4BiasFieldCorrectionFs": (
        "3.2",
        "8.2.0's default intensity correction (not in the plan's waves; relay 1)",
    ),
    "AntsDenoiseImageFs": ("3.2", "on by default before mri_segment (not in the plan's waves)"),
    "mri_binarize": ("3.2", "mri_nu_correct.mni's rescale and several scripts"),
    "mri_segstats": (
        "3.2",
        "two items: its volume options in 3.2 (mri_nu_correct.mni's means), the surface-based"
        " volumes of aseg.stats with 3.5",
    ),
    "mris_calc": (
        "3.2",
        "two items: its volume operations in 3.2 (mri_nu_correct.mni), surface overlays with"
        " vertexvol in 3.5",
    ),
    "mri_make_uchar": ("3.2", "mri_nu_correct.mni's last step"),
    "mri_nu_correct.mni": ("3.2", "a script: N4 by default (not MNI's N3)"),
    "mri_normalize": ("3.2", "uses Burkardt's spline (P-09: the clean-room piece meanwhile)"),
    "mri_mask": ("3.2", ""),
    "mri_cc": ("3.2", ""),
    "seg2cc": ("3.2", "the script that runs mri_cc"),
    "mergeseg": ("3.2", "seg2cc's path for an input not conformed to its smallest voxel"),
    "mri_edit_wm_with_aseg": ("3.2", ""),
    "mri_segment": ("3.2", ""),
    "mri_pretess": ("3.2", ""),
    "mri_fill": ("3.2", ""),
    "mri_vol2vol": (
        "3.2",
        "moved from 3.5: a volume resampler the v8 scripts and sMRIPrep use",
    ),
    "mri_concat": ("3.2", "a volume program (mri_mcadura_seg, pctsurfcon)"),
    "mri_label2vol": ("3.2", "seg2cc's and defect2seg's (not in the plan's waves)"),
    # 3.3 atlas: the GCA programs both streams run
    "mri_em_register": ("3.3", "both streams (the plan says classic only)"),
    "mri_ca_normalize": ("3.3", "both streams (the plan says classic only)"),
    "mri_ca_register": (
        "3.3",
        "classic runs the registration, v8 its -invert-and-save path (fs-synthmorph-reg)",
    ),
    # 3.4 first surfaces
    "mri_tessellate": ("3.4", ""),
    "mris_extract_main_component": ("3.4", ""),
    "mris_smooth": ("3.4", ""),
    "mris_inflate": ("3.4", ""),
    "mris_sphere": ("3.4", ""),
    "mris_fix_topology": ("3.4", "with the defect crate"),
    "mris_euler_number": ("3.4", ""),
    "mris_topo_fixer": ("3.4", "runs only when mris_fix_topology leaves a defect"),
    "mris_remesh": ("3.4", "on by default (not in the plan's waves)"),
    "mris_remove_intersection": ("3.4", ""),
    "defect2seg": ("3.4", ""),
    "mris_defects_pointset": ("3.4", "defect2seg's (not in the plan's waves)"),
    "mri_label2label": ("3.4", "moved from 3.5: defect2seg (stage 52) runs it first"),
    # 3.5 final surfaces and statistics
    "mris_autodet_gwstats": ("3.5", ""),
    "mris_place_surface": ("3.5", ""),
    "label-cortex": ("3.5", "a script (not in the plan's waves)"),
    "mri_cor2label": ("3.5", "label-cortex's v8 option path (--fix-ga)"),
    "fscalc": ("3.5", "label-cortex's v8 option path (--fix-ga)"),
    "mri_mergelabels": ("3.5", "label-cortex's v8 option path (--fix-ga)"),
    "mris_curvature": ("3.5", ""),
    "rca-surfreg": ("3.5", "the script that runs mris_register"),
    "mris_register": ("3.5", ""),
    "mris_jacobian": ("3.5", ""),
    "mrisp_paint": ("3.5", ""),
    "mris_ca_label": ("3.5", ""),
    "mris_curvature_stats": ("3.5", "not in the plan's waves"),
    "vertexvol": ("3.5", "a script (not in the plan's waves)"),
    "mris_volmask": ("3.5", ""),
    "mris_anatomical_stats": ("3.5", ""),
    "mris_label2annot": ("3.5", "not in the plan's waves"),
    "mri_relabel_hypointensities": (
        "3.5",
        "moved from 3.3: it reads the white surfaces (autorecon3)",
    ),
    "mri_surf2volseg": ("3.5", ""),
    "mri_brainvol_stats": ("3.5", ""),
    "pctsurfcon": ("3.5", ""),
    "mri_vol2surf": ("3.5", ""),
    "mris_convert": ("3.5", ""),
    "mris_expand": ("3.5", ""),
    "mri_surf2surf": ("3.5", "bbregister's RMS check"),
    "bbregister": ("3.5", ""),
    "mri_segreg": ("3.5", "bbregister's optimiser (not in the plan's waves)"),
    "tkregister2_cmdl": (
        "3.5",
        "CorTechs' terms (P-11): the clean room here; the replica in a non-commercial package"
        " (the user's P1)",
    ),
    # 3.6 the classic-only programs
    "mri_watershed": ("3.6", "classic only (the plan: 3.2)"),
    "talairach_avi": ("3.6", "classic only; its 4dfp tools a clean room only (the plan: 3.2)"),
    "mpr2mni305": ("3.6", "talairach_avi's"),
    "avi2talxfm": ("3.6", "talairach_avi's"),
    "analyzeto4dfp": ("3.6", "Washington University's: a clean room only (P-12)"),
    "gauss_4dfp": ("3.6", "Washington University's: a clean room only (P-12)"),
    "imgreg_4dfp": ("3.6", "Washington University's: a clean room only (P-12)"),
    "compute_vox2vox": ("3.6", "Washington University's: a clean room only (P-12)"),
    "mri_matrix_multiply": ("3.6", "avi2talxfm's"),
    "fs_temp_dir": ("3.6", "avi2talxfm's"),
    "extract_talairach_avi_QA.awk": ("3.6", "the Talairach check's"),
    "talairach_afd": ("3.6", "classic only (the plan: 3.2)"),
    "mri_ca_label": ("3.6", "classic only (the plan: 3.3); in the atlas crate"),
    # N: the v8 networks and what only their scripts run
    "mri_synthstrip": ("N", ""),
    "mri_synthseg": ("N", ""),
    "csvprint": ("N", "reads SynthSeg's volumes (v8)"),
    "mri_synthmorph": ("N", ""),
    "mri_sclimbic_seg": ("N", "a fourth network program (relay 1), three models"),
    "fs-synthmorph-reg": ("N", "after 3.3: it runs mri_ca_register -invert-and-save"),
    "rca-talairach": ("N", "v8's talairach.xfm"),
    "mri_concatenate_lta": ("N", "only fs-synthmorph-reg runs it (the plan: 3.2)"),
    "mri_warp_convert": ("N", "only fs-synthmorph-reg runs it"),
    "mri_diff": ("N", "only fs-synthmorph-reg's --test print"),
    "mri_mcadura_seg": ("N", ""),
    "mri_vsinus_seg": ("N", ""),
    "mri_entowm_seg": ("N", ""),
    "mri_compute_seg_overlap": ("N", "mri_vsinus_seg's --dice option"),
    # 3.7 recon-all and its helpers
    "recon-all": ("3.7", ""),
    "rca-config": ("3.7", ""),
    "rca-config2csh": ("3.7", ""),
    "fsr-getxopts": ("3.5", "the expert options of label-cortex and rca-surfreg, then recon-all's"),
    "fsr-checkxopts": ("3.7", ""),
    "UpdateNeeded": ("3.7", ""),
    "fs-check-version": ("3.7", ""),
    "fs-check-os": ("3.7", ""),
    "mri_seg_diff": ("3.7", "a resumed recon-all with edits"),
    # on no default path
    "nu_correct": ("later", "MNI's N3: only with -no-ants-n4"),
    "talairach": ("later", "MINC's mritotal: only after the Talairach check fails twice"),
}

# The wave that brings each module (its functions that two or more programs reach, or one program
# of an earlier wave). A program of an earlier wave that reaches a later module's functions has
# option paths that wait for it: they complete in that wave ("the paths into ..." items).
MODULE_WAVE = {
    "perm.gifti": "3.1",
    "perm.minc": "3.1",
    "perm.nifti": "3.1",
    "perm.dicom": "3.1",
    "perm.misc": "3.1",
    "perm.json": "3.1",
    "perm.geomtools": "3.5",
    "perm.argparse": "3.1",
    "fs.core": "3.1",
    "fs.maths": "3.1",
    "fs.io": "3.1",
    "fs.labels": "3.1",
    "fs.volume": "3.1",
    "fs.bspline": "3.2",
    "fs.filter": "3.1",
    "fs.gca": "3.3",
    "fs.surfio": "3.1",
    "fs.surf": "3.4",
    "fs.mars": "later",
    "fs.deform": "3.4",
    "fs.defect": "3.4",
    "fs.robust": "3.2",
    "cr.4dfp": "3.6",
}
# the order of work; N runs beside 3.2-3.5 (its network programs need only 3.1, its scripts 3.3)
QUEUE_WAVES = ("3.1", "3.2", "3.3", "3.4", "3.5", "3.6", "N", "3.7")
RANK = {"3.1": 1, "3.2": 2, "3.3": 3, "N": 3.5, "3.4": 4, "3.5": 5, "3.6": 6, "3.7": 7, "later": 9}

# Items that are not a module's or a program's code (written by hand: kind, crate, what, size
# in lines of the source they follow where known, inputs, checks). "first": before the wave's
# module items.
EXTRA: list[dict] = [
    {
        "wave": "3.1",
        "kind": "plumbing",
        "crate": "crates-freesurfer/",
        "what": "the workspace crates-freesurfer/ (the crates of this layout, empty), the program"
        " and wheel larmorx-freesurfer (larmorx-freesurfer-cli: FreeSurfer's licence text and"
        " preface, NOTICE, PROVENANCE.md, the settings overlay of larmorx's three options), its"
        " CI check and test steps (minutes estimated first), lx.freesurfer's dispatch between"
        " replica and clean room; the clean-room crates crates/larmorx-fs-* (empty), each added"
        " to the clean-room audit's lists by the orchestrator",
        "lines": 0,
        "inputs": "this layout; crates-nc/ and crates-gpl/ as models",
        "checks": "the three Rust workspaces' gates, check_spdx.py, test_release.py",
    },
    {
        "wave": "3.1",
        "kind": "oracle",
        "crate": "validation/investigations/freesurfer-unit-oracle/",
        "what": "a unit oracle: FreeSurfer 8.2.0's libutils and the packages it links, built from"
        " the tag with GCC 8.4.0 (oracles/toolchains/gcc-8.4) and the official flags (code.toml)"
        " against ITK 4.13.2's VNL, with a harness that calls single functions (the foundations'"
        " checks below; a proposal: the replay checks whole programs only)",
        "lines": 0,
        "inputs": "code.md section 1 (the build as the official one ran), relay 5's gcc84/",
        "checks": "three programs built from it give the official binaries' outputs on recorded"
        " steps (mri_add_xform_to_header, mri_normalize, mris_smooth)",
    },
    {
        "wave": "3.1",
        "kind": "permissive",
        "crate": "larmorx-core",
        "what": "x87 80-bit extended arithmetic (long double on x86-64 Linux): add, subtract,"
        " multiply, divide, square root, conversions to and from float, double and integers,"
        " comparisons, at the Linux default precision control (64-bit significand,"
        " round-to-nearest-even), written from Intel's manual and IEEE 754's extended format"
        " (no FreeSurfer code); used by MRIgaussianSmoothNI, dmatrix, surfgrad and gifticlib's"
        " writer (variation.md section 1)",
        "lines": 0,
        "inputs": "Intel 64 and IA-32 Architectures Software Developer's Manual, vol. 1 ch. 4 and 8",
        "checks": "random operands and the 12 reached functions' patterns against the machine's"
        " x87 (a C probe compiled by GCC 8.4.0); golden cases on the six platforms",
    },
    {
        "wave": "3.1",
        "kind": "permissive",
        "crate": "larmorx-core",
        "what": "random numbers: VXL's vnl_random (Marsaglia-Zaman, drand64's measured left-to-right"
        " order, TP-01), POSIX drand48/srand48/erand48 (TP-15); glibc 2.35's rand, srand, random"
        " and srandom from a behaviour spec only (a spec writer, then a clean-room implementer:"
        " the GNU family's precedent, specs/glibc-dbl.md); FreeSurfer's setRandomSeed and"
        " randomNumber come with fs.core",
        "lines": 0,
        "inputs": "ITK 4.13.2's vnl_random.cxx (third-party.md section 8), POSIX",
        "checks": "sequences against the unit oracle and the machine's glibc; mri_normalize's,"
        " mris_smooth's and mris_ca_label's -seed 1234 replays",
    },
    {
        "wave": "3.1",
        "kind": "spec + clean room",
        "crate": "larmorx-core (clean-room files, GSL family)",
        "what": "RANLUX389 and GSL 1.8's distributions (P-08, TP-03): specs/ranlux.md by a spec"
        " writer (from Lüscher 1994, James 1994, Knuth, Marsaglia, with the exact expressions"
        " FreeSurfer evaluates), then the clean room by an implementer who has read neither GSL"
        " nor numerics.cpp (GSL's source joins the audit's restriction table); mri_coreg's"
        " replica uses it; the GSL replica as its bit-exact reference in larmorx-gpl (the"
        " user's P5)",
        "lines": 375,
        "inputs": "the placement table P-08; numerics.cpp 1856-2230 (the spec writer only)",
        "checks": "streams against the unit oracle's sc_rng_* for many seeds; mri_coreg's default"
        " dithering (seed 53) through its replay",
    },
    {
        "wave": "3.1",
        "kind": "spec + clean room",
        "crate": "larmorx-core (clean-room files)",
        "what": "the cubic spline mri_normalize uses (P-09, TP-04: Burkardt's 2006 routines, the"
        " clean-room piece until Burkardt confirms his MIT terms cover them, the user's P2)",
        "lines": 0,
        "inputs": "the placement table P-09",
        "checks": "SplineCubicSet/SplineCubicValue against the unit oracle; mri_normalize's replays",
    },
    {
        "wave": "3.1",
        "kind": "permissive",
        "crate": "larmorx-core",
        "what": "the VNL and netlib routines FreeSurfer calls that larmorx-core lacks (vnl_powell,"
        " vnl_brent, vnl_brent_minimizer, vnl_bracket_minimum, vnl_cholesky, vnl_real_eigensystem"
        " with EISPACK's rg, vnl_determinant, vnl_matrix_inverse, netlib's L-BFGS through f2c,"
        " vnl_fft where larmorx-image's differs), at ITK 4.13.2 (the same arithmetic as 5.4.5,"
        " third-party.md section 10)",
        "lines": 0,
        "inputs": "ITK 4.13.2's tarball (<scratch>/r5/itk-4.13.2/), itk-diff.toml",
        "checks": "against the unit oracle; the existing vnl_svd_f32, vnl_qr, vnl_eigen ports"
        " re-checked at FreeSurfer's call sites",
    },
    {
        "wave": "3.1",
        "kind": "inventory + spec",
        "crate": "larmorx-core",
        "what": "the C library behind the results: every libm function the reached code calls"
        " (exp, log, pow, sqrt, sin, cos, atan2, floor, ...) checked against larmorx_core::math"
        " (glibc 2.35's bits for reproduce where it lacks them: specs and clean rooms, the GNU"
        " family's precedent), glibc's qsort (a behaviour spec) and libstdc++'s std::sort (FSL-22's"
        " precedent)",
        "lines": 0,
        "inputs": "the call graph (full view); larmorx_core::math",
        "checks": "the unit oracle; the first replays",
    },
    {
        "wave": "N",
        "kind": "permissive",
        "crate": "larmorx-nn (new, main package)",
        "what": "the inference core of the four network programs: the layers and operators of"
        " TensorFlow 2.13.1 / Keras 2.13.1 and PyTorch 2.1.2 they use (convolutions, pooling,"
        " up-sampling, activations, normalisation, softmax, the spatial transformer of"
        " voxelmorph and neurite), from their Apache-2.0 and BSD source (P-07); the weight readers"
        " (Keras HDF5 through larmorx-hdf5; PyTorch's .pt archive read as data, never running its"
        " pickle code, data-files.md section 6); memory for synthmorph.deform.3.h5 (3.5 GB, about"
        " 18 GiB at inference)",
        "lines": 0,
        "inputs": "the installed package's Python (TensorFlow, PyTorch, neurite, voxelmorph, surfa)",
        "checks": "each layer against TensorFlow/PyTorch in the installed fspython (a black-box"
        " run), then the programs' replays (equivalence, decision log 2026-10-19)",
        "first": True,
    },
]

# Program items that are split (name -> [(suffix, wave, what)]); the first part carries the code.
SPLIT: dict[str, list[tuple[str, str, str]]] = {
    "mri_segstats": [
        ("a", "3.2", "the volume options (mri_nu_correct.mni's means, the scripts' --id sums)"),
        ("b", "3.5", "the surface-based volumes of aseg.stats and wmparc.stats, --annot --snr"),
    ],
    "mris_calc": [
        ("a", "3.2", "the volume operations (mri_nu_correct.mni's rescale)"),
        ("b", "3.5", "surface overlays (vertexvol, fscalc)"),
    ],
}

# Scripts too large for one item: their parts (programs.md section 3's stages and recon-all's lines).
SCRIPT_PARTS: dict[str, list[str]] = {
    "recon-all": [
        "the command line, its flags and defaults (rca-config's YAML, the v8 options file,"
        " check_params), the subject directory, the logs and status files, the version and OS checks",
        "autorecon1, stages 1-25 (both streams' branches)",
        "autorecon2's volume stages 26-43",
        "autorecon2's surface stages 44-61 (per hemisphere)",
        "autorecon3, stages 62-83, and recon-all.done",
        "the resume logic (UpdateNeeded's file times), -parallel, the hemisphere runs and"
        " sMRIPrep's step-by-step calls (-autorecon1 -noskullstrip, -autorecon-hemi, -cortribbon),"
        " the error paths",
    ],
}

# The queue's order of program items within a wave (the rest follow in programs.toml's order).
ORDER = {
    "3.2": [
        "mri_convert",
        "mri_info",
        "lta_convert",
        "mri_add_xform_to_header",
        "fs_temp_file",
        "getfullpath",
        "mri_robust_register",
        "mri_robust_template",
        "mri_coreg",
        "AntsN4BiasFieldCorrectionFs",
        "mri_binarize",
        "mri_segstats",
        "mris_calc",
        "mri_make_uchar",
        "mri_nu_correct.mni",
        "mri_normalize",
        "mri_mask",
        "mri_cc",
        "seg2cc",
        "mergeseg",
        "AntsDenoiseImageFs",
        "mri_segment",
        "mri_edit_wm_with_aseg",
        "mri_pretess",
        "mri_fill",
        "mri_vol2vol",
        "mri_concat",
        "mri_label2vol",
    ],
    "3.4": [
        "mri_tessellate",
        "mris_extract_main_component",
        "mris_smooth",
        "mris_inflate",
        "mris_sphere",
        "mris_fix_topology",
        "mris_euler_number",
        "mris_topo_fixer",
        "mri_label2label",
        "mris_defects_pointset",
        "defect2seg",
        "mris_remesh",
        "mris_remove_intersection",
    ],
    "N": [
        "mri_synthstrip",
        "mri_synthseg",
        "csvprint",
        "mri_sclimbic_seg",
        "mri_synthmorph",
        "mri_concatenate_lta",
        "mri_warp_convert",
        "mri_diff",
        "fs-synthmorph-reg",
        "rca-talairach",
        "mri_mcadura_seg",
        "mri_vsinus_seg",
        "mri_compute_seg_overlap",
        "mri_entowm_seg",
    ],
}

FMRIPREP = {
    "mri_robust_register": "sMRIPrep's RobustRegister (--satit --iscale)",
    "mri_robust_template": "sMRIPrep's StructuralReference with several T1w; recon-all's MotionCor",
    "mri_coreg": "fMRIPrep's MRICoreg, bbregister --init-coreg",
    "bbregister": "fMRIPrep's and sMRIPrep's BBRegister",
    "mri_segreg": "bbregister's",
    "tkregister2_cmdl": "bbregister's",
    "mri_surf2surf": "bbregister's RMS check",
    "mri_vol2surf": "fMRIPrep's SampleToSurface (--trgsubject fsaverage)",
    "mris_convert": "sMRIPrep's MRIsConvert (GIFTI)",
    "mris_expand": "sMRIPrep's MakeMidthickness",
    "mri_vol2vol": "sMRIPrep's ApplyVolTransform",
    "mri_convert": "niworkflows' licence check, bbregister",
    "lta_convert": "sMRIPrep, bbregister",
}


def load_toml(path: Path) -> dict:
    with path.open("rb") as fh:
        return tomllib.load(fh)


def module_of(path: str) -> str | None:
    for mid, _crate, _kind, _title, patterns in MODULES:
        for pat in patterns:
            if re.search(pat, path):
                return mid
    return None


def group_of(streams: list[str]) -> str:
    """shared (both streams, or v8 and fMRIPrep), fmriprep, classic-only, v8-only, conditional."""
    s = set(streams)
    if {"v8", "classic"} <= s or ("fmriprep" in s and len(s) > 1):
        return "shared"
    if s == {"fmriprep"}:
        return "fmriprep"
    if s == {"classic"}:
        return "classic-only"
    if s == {"v8"}:
        return "v8-only"
    return "conditional"


def recorded_steps(runs: Path | None) -> dict[str, Counter]:
    """program -> Counter(run -> number of recorded steps whose argv[0] is the program)."""
    out: dict[str, Counter] = defaultdict(Counter)
    if runs is None or not runs.is_dir():
        return out
    for run in sorted(p for p in runs.iterdir() if p.is_dir()):
        steps = run / "record" / "steps"
        if not steps.is_dir():
            continue
        for f in steps.glob("*.json"):
            try:
                argv = json.loads(f.read_text(encoding="utf-8")).get("argv") or []
            except (OSError, ValueError):
                continue
            if argv:
                out[Path(argv[0]).name][run.name] += 1
    return out


def run_main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("programs", type=Path)
    ap.add_argument("code", type=Path)
    ap.add_argument("core", type=Path)
    ap.add_argument("--data", type=Path, help="docs/freesurfer/data-files.toml")
    ap.add_argument("--tag", type=Path, help="<workspace>/tags/freesurfer-8.2.0 (scripts' lines)")
    ap.add_argument("--runs", type=Path)
    ap.add_argument("--toml", type=Path)
    ap.add_argument("--markdown", type=Path)
    ap.add_argument("--queue", type=Path)
    ap.add_argument("--budget", type=int, default=2500, help="lines per relay-sized item")
    args = ap.parse_args()

    programs = load_toml(args.programs)["program"]
    by_name = {p["name"]: p for p in programs}
    core = json.loads(args.core.read_text(encoding="utf-8"))
    fun_lines = {k: v.get("lines", 0) for k, v in core["functions"].items()}
    reached = {p: set(v["reached"]) for p, v in core["programs"].items()}
    data_of: dict[str, list[tuple[str, int]]] = defaultdict(list)
    if args.data:
        for f in load_toml(args.data)["file"]:
            for p in f["programs"]:
                data_of[p].append((f["path"], f["bytes"]))
    problems: list[str] = []

    # ------------------------------------------------------------ placement checks
    for name in by_name:
        if name not in PLACE:
            problems.append(f"program without a wave: {name}")
    for name in PLACE:
        if name not in by_name:
            problems.append(f"PLACE names an unknown program: {name}")
    allowed = {
        "shared": {"3.2", "3.3", "3.4", "3.5", "3.7"},
        "fmriprep": {"3.2", "3.5"},
        "classic-only": {"3.6"},
        "v8-only": {"N", "3.5"},
        "conditional": {"3.2", "3.4", "3.7", "N", "later"},
    }
    for name, p in by_name.items():
        wave = PLACE.get(name, ("?", ""))[0]
        group = group_of(p["streams"])
        if wave not in allowed[group]:
            problems.append(f"{name}: group {group} in wave {wave}")
    calls: dict[str, list[str]] = defaultdict(list)
    for name, p in by_name.items():
        for caller in p["called_by"]:
            if caller in by_name:
                calls[caller].append(name)
    for caller, callees in calls.items():
        if caller == "recon-all":
            continue
        cw = PLACE[caller][0]
        for callee in callees:
            ew = PLACE[callee][0]
            if "later" in (cw, ew):
                continue
            if RANK[ew] > RANK[cw]:
                problems.append(f"{caller} ({cw}) runs {callee} ({ew}) of a later wave")

    # ------------------------------------------------------------ modules and owners
    file_module: dict[str, str] = {}
    for funs in reached.values():
        for r in funs:
            f = r.split(":")[0]
            if f in file_module:
                continue
            m = module_of(f)
            if m is None:
                if f.split("/")[0] in ("utils", "include", "packages"):
                    problems.append(f"no module for {f}")
                    m = "?"
                else:
                    m = "own"
            file_module[f] = m
    for m in [x[0] for x in MODULES]:
        if m not in MODULE_WAVE:
            problems.append(f"module without a wave: {m}")
    wave_of = {p: PLACE.get(p, ("later", ""))[0] for p in reached}
    fun_progs: dict[str, set[str]] = defaultdict(set)
    for prog, funs in reached.items():
        for r in funs:
            fun_progs[r].add(prog)
    owner: dict[str, str] = {}
    waits: dict[str, Counter] = defaultdict(Counter)  # program -> module -> lines
    for r, progs in fun_progs.items():
        m = file_module[r.split(":")[0]]
        n = fun_lines.get(r, 0)
        if m == "own":
            owner[r] = "prog:" + sorted(progs)[0]
            continue
        mw = MODULE_WAVE.get(m, "3.1")
        if len(progs) == 1:
            (p,) = progs
            if RANK[wave_of[p]] >= RANK[mw]:
                owner[r] = "prog:" + p
                continue
        owner[r] = "mod:" + m
        for p in progs:
            if RANK[wave_of[p]] < RANK[mw]:
                waits[p][m] += n
    item_file: dict[str, Counter] = defaultdict(Counter)
    item_funs: dict[str, list[str]] = defaultdict(list)
    for r, it in owner.items():
        item_file[it][r.split(":")[0]] += fun_lines.get(r, 0)
        item_funs[it].append(r)
    mod_total: Counter = Counter()
    mod_progs: dict[str, set[str]] = defaultdict(set)
    prog_mod: dict[str, Counter] = defaultdict(Counter)
    for r, progs in fun_progs.items():
        m = file_module[r.split(":")[0]]
        mod_total[m] += fun_lines.get(r, 0)
        for p in progs:
            mod_progs[m].add(p)
            prog_mod[p][m] += fun_lines.get(r, 0)
    steps = recorded_steps(args.runs)

    def checks_of(n: str) -> str:
        st = steps.get(n)
        parts = []
        if st:
            parts.append(f"replay: {sum(st.values())} recorded steps in {len(st)} runs")
        else:
            callers = [c for c in by_name[n]["called_by"] if c in by_name and steps.get(c)]
            if callers:
                k = sum(sum(steps[c].values()) for c in callers)
                parts.append(f"through {', '.join(callers)}'s recorded steps ({k})")
        if n in FMRIPREP:
            parts.append(f"fMRIPrep's calls on the recorded subjects ({FMRIPREP[n]})")
        if not parts:
            parts.append("synthetic cases and a recorded call of its own (no default step runs it)")
        return "; ".join(parts)

    def inputs_of(n: str, wave: str) -> str:
        mods = [
            m.split(".", 1)[1]
            for m, k in sorted(prog_mod[n].items())
            if m != "own" and k and RANK[MODULE_WAVE.get(m, "3.1")] <= RANK[wave]
        ]
        out = []
        if mods:
            out.append("modules " + ", ".join(mods))
        sub = [c for c in calls.get(n, []) if c in by_name]
        if sub:
            out.append("runs " + ", ".join(sorted(sub)))
        d = data_of.get(n, [])
        if d:
            big = sum(b for _, b in d)
            names = ", ".join(Path(x).name for x, _ in d[:3]) + (" ..." if len(d) > 3 else "")
            out.append(f"data: {len(d)} files, {big / 1e6:,.1f} MB ({names})")
        w = waits.get(n)
        if w:
            out.append(
                "waits for "
                + ", ".join(f"{m.split('.', 1)[1]} ({k:,})" for m, k in sorted(w.items()))
            )
        return "; ".join(out)

    def fun_key(r: str) -> tuple[str, int]:
        f, line, _name = r.split(":", 2)
        return f, int(line) if line.isdigit() else 0

    def bins(item: str) -> list[tuple[int, str, list[str]]]:
        """Relay-sized pieces of an item: its functions in file and line order, at most the
        budget each (one function larger than it alone); a label naming each file, with the
        lines of its functions when a file is cut between pieces."""
        funs = sorted(item_funs[item], key=fun_key)
        raw: list[list[str]] = []
        cur: list[str] = []
        n = 0
        for r in funs:
            k = fun_lines.get(r, 0)
            if cur and n + k > args.budget:
                raw.append(cur)
                cur, n = [], 0
            cur.append(r)
            n += k
        if cur:
            raw.append(cur)
        whole = Counter(fun_key(r)[0] for r in funs)
        out = []
        for piece in raw:
            here = Counter(fun_key(r)[0] for r in piece)
            names = []
            for f in sorted(
                here, key=lambda x: min(fun_key(r)[1] for r in piece if r.startswith(x + ":"))
            ):
                lines = [fun_key(r)[1] for r in piece if fun_key(r)[0] == f]
                cut = here[f] < whole[f]
                names.append(Path(f).name + (f" (lines {min(lines)}-{max(lines)})" if cut else ""))
            out.append((sum(fun_lines.get(r, 0) for r in piece), ", ".join(names), sorted(here)))
        return out

    def script_lines(n: str) -> int:
        src = by_name[n].get("source", "")
        if not src or args.tag is None or n in reached:
            return 0
        path = args.tag / src
        try:
            return len(path.read_text(encoding="utf-8", errors="replace").splitlines())
        except OSError:
            return 0

    # ------------------------------------------------------------ the queue
    queue: list[dict] = []
    clean: dict[str, list[dict]] = defaultdict(list)  # wave of the replica -> clean-room items
    for wave in QUEUE_WAVES:
        for x in EXTRA:
            if x["wave"] == wave and x.get("first", True):
                queue.append(dict(x, files=[], what=x["what"]))
        for mid, crate, kind, title, _ in MODULES:
            if MODULE_WAVE[mid] != wave or not item_file.get("mod:" + mid):
                continue
            bs = bins("mod:" + mid)
            for i, (n, names, fl) in enumerate(bs, 1):
                label = f"`{mid}`" + (f" ({i} of {len(bs)})" if len(bs) > 1 else "")
                if i == 1:
                    label += f" ({title})"
                queue.append(
                    {
                        "wave": wave,
                        "kind": kind,
                        "crate": crate,
                        "what": f"{label}: {names}",
                        "lines": n,
                        "files": fl,
                        "inputs": "the 3.1 items before it"
                        if wave == "3.1"
                        else f"3.1; {mid}'s programs below",
                        "checks": "unit checks against the unit oracle; then the replays of the"
                        f" first programs that use it ({len(mod_progs[mid])} programs reach"
                        " the module)",
                    }
                )
            if kind == "replica":
                clean[wave].append(
                    {
                        "what": f"`{mid}`'s clean room ({title})",
                        "crate": "larmorx-io"
                        if mid in ("fs.io", "fs.labels")
                        else "larmorx-fs-surf"
                        if mid.startswith(("fs.surf", "fs.deform", "fs.defect"))
                        else "larmorx-fs-gca"
                        if mid == "fs.gca"
                        else "larmorx-fs-core",
                        "lines": mod_total[mid],
                    }
                )
        # the earlier waves' option paths into this wave's modules
        for mid, *_ in MODULES:
            if MODULE_WAVE[mid] != wave:
                continue
            ps = sorted((p for p, w in waits.items() if mid in w), key=lambda p: -waits[p][mid])
            if ps and wave != "3.1":
                top = ", ".join(f"`{p}` ({waits[p][mid]:,})" for p in ps[:6])
                queue.append(
                    {
                        "wave": wave,
                        "kind": "completeness",
                        "crate": "the programs' crates",
                        "what": f"the option paths of {len(ps)} earlier programs into `{mid}`"
                        f" (static reach, lines; most never run by default: each checked when its"
                        f" program's record is re-made): {top}" + (" ..." if len(ps) > 6 else ""),
                        "lines": 0,
                        "files": [],
                        "inputs": f"`{mid}` above",
                        "checks": "synthetic cases for each option path; the programs' records"
                        " re-made",
                    }
                )
        names = [n for n in by_name if PLACE[n][0] == wave]
        order = ORDER.get(wave, [])
        names = [n for n in order if n in names] + [n for n in names if n not in order]
        extra_split = [
            (n, part) for n, parts in SPLIT.items() for part in parts[1:] if part[1] == wave
        ]
        for n in names:
            parts = SPLIT.get(n, [("", wave, "")])
            suffix, _pw, what = parts[0]
            lines = sum(item_file["prog:" + n].values())
            own_script = script_lines(n)
            crate = CRATE[wave] if n != "mri_ca_label" else CRATE["3.3"]
            kind = by_name[n]["kind"]
            if n in ("analyzeto4dfp", "gauss_4dfp", "imgreg_4dfp", "compute_vox2vox"):
                kind, crate = "clean room only", CLEAN[wave]
            if n == "tkregister2_cmdl":
                crate = "a non-commercial package (P1); its clean room in " + CLEAN[wave]
            pieces = bins("prog:" + n) if lines > args.budget * 3 // 2 else []
            label = f"`{n}`" + (f" ({suffix}: {what})" if suffix else "")
            if n in SCRIPT_PARTS:
                sp = SCRIPT_PARTS[n]
                for i, part in enumerate(sp, 1):
                    queue.append(
                        {
                            "wave": wave,
                            "kind": kind,
                            "crate": crate,
                            "what": f"{label} ({i} of {len(sp)}): {part}"
                            + (f" (the script: {own_script:,} lines of {kind})" if i == 1 else ""),
                            "lines": 0,
                            "files": [],
                            "program": n,
                            "inputs": "every wave's programs" if i == 1 else "the parts before it",
                            "checks": "the reference runs end to end (both streams, the six heads),"
                            " replayed from their first step; sMRIPrep's calls"
                            if i == len(sp)
                            else "the recorded command lines of the stages it covers",
                        }
                    )
            elif pieces:
                for i, (k, names_i, fl) in enumerate(pieces, 1):
                    last = i == len(pieces)
                    queue.append(
                        {
                            "wave": wave,
                            "kind": kind,
                            "crate": crate,
                            "what": f"{label} ({i} of {len(pieces)}): {names_i}"
                            + ("; the program assembled" if last else ""),
                            "lines": k,
                            "files": fl,
                            "program": n,
                            "inputs": inputs_of(n, wave) if i == 1 else "the parts before it",
                            "checks": checks_of(n)
                            if last
                            else "unit checks of its functions against the unit oracle",
                        }
                    )
            else:
                queue.append(
                    {
                        "wave": wave,
                        "kind": kind,
                        "crate": crate,
                        "what": label
                        + (f" ({own_script:,} lines of {kind})" if own_script else ""),
                        "lines": lines,
                        "files": sorted(item_file["prog:" + n]),
                        "program": n,
                        "inputs": inputs_of(n, wave),
                        "checks": checks_of(n),
                    }
                )
            if kind != "clean room only":
                clean[wave].append(
                    {
                        "what": f"`{n}`'s clean room",
                        "crate": CLEAN[wave] if n != "mri_ca_label" else CLEAN["3.3"],
                        "lines": lines or own_script,
                    }
                )
        for n, (suffix, _pw, what) in extra_split:
            queue.append(
                {
                    "wave": wave,
                    "kind": by_name[n]["kind"],
                    "crate": CRATE[wave],
                    "what": f"`{n}` ({suffix}: {what})",
                    "lines": 0,
                    "files": [],
                    "program": n,
                    "inputs": inputs_of(n, wave),
                    "checks": checks_of(n),
                }
            )
        for x in EXTRA:
            if x["wave"] == wave and not x.get("first", True):
                queue.append(dict(x, files=[]))
    counters: Counter = Counter()
    for q in queue:
        counters[q["wave"]] += 1
        q["id"] = f"{q['wave']}-{counters[q['wave']]:02d}"
    for wave, items in clean.items():
        for i, c in enumerate(items, 1):
            c["id"] = f"C{wave}-{i:02d}"

    # ------------------------------------------------------------ Markdown
    md: dict[str, str] = {}
    rows = [
        "| Module | Crate (replica) | Kind | Wave | What it holds | Lines in reach | Programs |",
        "|---|---|---|---|---|---:|---:|",
    ]
    for mid, crate, kind, title, _ in MODULES:
        rows.append(
            f"| `{mid}` | `{crate}` | {kind} | {MODULE_WAVE[mid]} | {title} | {mod_total[mid]:,}"
            f" | {len(mod_progs[mid])} |"
        )
    rows.append(
        f"| own files | the wave's crate | program | the program's | each program's own files |"
        f" {mod_total['own']:,} | {len(reached)} |"
    )
    md["modules"] = "\n".join(rows)

    mids = [m[0] for m in MODULES if mod_total[m[0]]]
    head = (
        "| Program | Wave | Group | "
        + " | ".join(f"`{m.split('.', 1)[1]}`" for m in mids)
        + " | own | brought by its item |"
    )
    rows = [head, "|---|---|---|" + "---:|" * (len(mids) + 2)]
    for wave in WAVES:
        for p in sorted(x for x in reached if wave_of[x] == wave):
            g = group_of(by_name[p]["streams"]) if p in by_name else "?"
            cells = [f"{prog_mod[p][m]:,}" if prog_mod[p][m] else "" for m in [*mids, "own"]]
            rows.append(
                f"| `{p}` | {wave} | {g} | "
                + " | ".join(cells)
                + f" | {sum(item_file['prog:' + p].values()):,} |"
            )
    md["map"] = "\n".join(rows)

    rows = [
        "| Wave | Program | Kind | Streams | Group | Replica's crate | Note |",
        "|---|---|---|---|---|---|---|",
    ]
    for wave in WAVES:
        for n in [x for x in by_name if PLACE[x][0] == wave]:
            p = by_name[n]
            crate = CRATE[wave] if n != "mri_ca_label" else CRATE["3.3"]
            if n in ("analyzeto4dfp", "gauss_4dfp", "imgreg_4dfp", "compute_vox2vox"):
                crate = "none (a clean room only)"
            elif n == "tkregister2_cmdl":
                crate = "a non-commercial package (P1)"
            elif crate:
                crate = f"`{crate}`"
            rows.append(
                f"| {wave} | `{n}` | {p['kind']} | {', '.join(p['streams']) or '-'} |"
                f" {group_of(p['streams'])} | {crate} | {PLACE[n][1]} |"
            )
    md["waves"] = "\n".join(rows)

    rows = []
    for wave in QUEUE_WAVES:
        rows += [
            f"### {wave} {WAVE_TITLE[wave]}",
            "",
            "| Item | Kind | What | Crate | Lines | Inputs | Checks |",
            "|---|---|---|---|---:|---|---|",
        ]
        for q in queue:
            if q["wave"] != wave:
                continue
            crate = q["crate"]
            crate = f"`{crate}`" if crate.startswith("larmorx") and " " not in crate else crate
            rows.append(
                f"| {q['id']} | {q['kind']} | {q['what']} | {crate} | {q['lines']:,} |"
                f" {q.get('inputs', '')} | {q.get('checks', '')} |"
            )
        prev = {"3.2": "3.1", "3.3": "3.2", "3.4": "3.3", "3.5": "3.4", "3.6": "3.5"}.get(wave)
        if prev and clean.get(prev):
            rows += [
                "",
                f"Beside them, **the clean rooms of {prev}** (each from the specs its replica item"
                " wrote, by agents that never read FreeSurfer's source):",
                "",
                "| Item | Clean room | Crate | Size of what it follows (lines) |",
                "|---|---|---|---:|",
            ]
            for c in clean[prev]:
                rows.append(f"| {c['id']} | {c['what']} | `{c['crate']}` | {c['lines']:,} |")
        rows.append("")
    for wave in ("3.6", "N", "3.7"):
        if clean.get(wave):
            rows += [
                f"### The clean rooms of {wave}",
                "",
                "| Item | Clean room | Crate | Size of what it follows (lines) |",
                "|---|---|---|---:|",
            ]
            for c in clean[wave]:
                rows.append(f"| {c['id']} | {c['what']} | `{c['crate']}` | {c['lines']:,} |")
            rows.append("")
    md["queue"] = "\n".join(rows).rstrip()

    total: Counter = Counter()
    for q in queue:
        total[q["wave"]] += q["lines"]
    rows = [
        "| Wave | Replica items | Lines brought | Clean-room items |",
        "|---|---:|---:|---:|",
    ]
    for wave in QUEUE_WAVES:
        rows.append(
            f"| {wave} {WAVE_TITLE[wave]} | {counters[wave]} | {total[wave]:,} |"
            f" {len(clean.get(wave, []))} |"
        )
    rows.append(
        f"| all | {sum(counters.values())} | {sum(total.values()):,} |"
        f" {sum(len(v) for v in clean.values())} |"
    )
    md["totals"] = "\n".join(rows)

    for line in problems:
        print("PROBLEM:", line, file=sys.stderr)

    if args.toml:
        out = [
            "# CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS: generated from FreeSurfer 8.2.0's call",
            "# graph (docs/freesurfer/README.md) by validation/investigations/freesurfer-inventory/",
            "# waves.py; what each field means: docs/freesurfer/waves.md. Do not edit by hand.",
            "",
            "schema = 1",
            'freesurfer = "8.2.0"',
            f"budget = {args.budget}",
            "",
        ]
        for mid, crate, kind, title, _ in MODULES:
            out += [
                "[[module]]",
                f"id = {json.dumps(mid)}",
                f"crate = {json.dumps(crate)}",
                f"kind = {json.dumps(kind)}",
                f"wave = {json.dumps(MODULE_WAVE[mid])}",
                f"title = {json.dumps(title)}",
                f"lines = {mod_total[mid]}",
                f"programs = {json.dumps(sorted(mod_progs[mid]))}",
                f"files = {json.dumps(sorted(f for f, m in file_module.items() if m == mid))}",
                "",
            ]
        for n, p in by_name.items():
            wave, note = PLACE[n]
            crate = CRATE[wave] if n != "mri_ca_label" else CRATE["3.3"]
            if n in ("analyzeto4dfp", "gauss_4dfp", "imgreg_4dfp", "compute_vox2vox"):
                crate = "none: a clean room only (P-12)"
            elif n == "tkregister2_cmdl":
                crate = "a non-commercial package (P-11, the user's P1)"
            ccrate = CLEAN[wave] if n != "mri_ca_label" else CLEAN["3.3"]
            ids = [q["id"] for q in queue if q.get("program") == n]
            out += [
                "[[program]]",
                f"name = {json.dumps(n)}",
                f"wave = {json.dumps(wave)}",
                f"group = {json.dumps(group_of(p['streams']))}",
                f"crate = {json.dumps(crate)}",
                f"clean_room_crate = {json.dumps(ccrate)}",
                f"items = {json.dumps(ids)}",
                f"note = {json.dumps(note)}",
                f"runs = {json.dumps(sorted(calls.get(n, [])))}",
                f"item_lines = {sum(item_file['prog:' + n].values())}",
                f"inputs = {json.dumps(inputs_of(n, wave))}",
                f"checks = {json.dumps(checks_of(n))}",
                "[program.reach]",
            ]
            out += [f"{json.dumps(m)} = {k}" for m, k in sorted(prog_mod[n].items())]
            out.append("[program.waits]")
            out += [f"{json.dumps(m)} = {k}" for m, k in sorted(waits.get(n, {}).items())]
            out.append("[program.recorded_steps]")
            out += [f"{json.dumps(r)} = {k}" for r, k in sorted(steps.get(n, {}).items())]
            out.append("")
        for q in queue:
            out += [
                "[[item]]",
                f"id = {json.dumps(q['id'])}",
                f"wave = {json.dumps(q['wave'])}",
                f"kind = {json.dumps(q['kind'])}",
                f"what = {json.dumps(q['what'].replace('`', ''))}",
                f"crate = {json.dumps(q['crate'])}",
                f"lines = {q['lines']}",
                f"files = {json.dumps(q['files'])}",
                f"inputs = {json.dumps(q.get('inputs', ''))}",
                f"checks = {json.dumps(q.get('checks', ''))}",
                "",
            ]
        for wave, items in clean.items():
            for c in items:
                out += [
                    "[[clean_room]]",
                    f"id = {json.dumps(c['id'])}",
                    f"after_wave = {json.dumps(wave)}",
                    f"what = {json.dumps(c['what'].replace('`', ''))}",
                    f"crate = {json.dumps(c['crate'])}",
                    f"lines = {c['lines']}",
                    "",
                ]
        args.toml.write_text("\n".join(out), encoding="utf-8")

    for path in (args.markdown, args.queue):
        if path is None:
            continue
        text = path.read_text(encoding="utf-8")
        for name, body in md.items():
            begin, end = f"<!-- waves.py: {name} -->", f"<!-- waves.py: end {name} -->"
            if begin in text:
                a = text.index(begin) + len(begin)
                b = text.index(end)
                text = text[:a] + "\n" + body + "\n" + text[b:]
        path.write_text(text, encoding="utf-8")
    if not (args.toml or args.markdown or args.queue):
        for name, body in md.items():
            print(f"## {name}\n\n{body}\n")
    return 1 if problems else 0


if __name__ == "__main__":
    raise SystemExit(run_main())
