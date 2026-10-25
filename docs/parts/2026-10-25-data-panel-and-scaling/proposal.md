# Proposal: the larmorx data panel (for approval, nothing downloaded yet)

*2026-10-24. Research only: OpenNeuro snapshot listings, `dataset_description.json`, `participants.tsv`,
JSON sidecars and the datasets' own MRIQC tables. No image data was downloaded. Drafts of the catalog
entries are in `catalog-draft/`.*

## In short

Twelve datasets, one subject each, 1.5 GB in total. Five are the datasets we already ran fMRIPrep
25.2.6 on (their files are already in the test-data cache). Seven add what those five lack: 1.5 T,
multiband 8, children, older adults, a brain tumour, strong head motion, multi-echo and a 3 T MP2RAGE.
Together they cover every criterion in the brief. The thin spots: 1.5 T and multiband exist only on
Siemens, the one lesion is a tumour (no stroke, no lesion mask), multi-echo is in one dataset (see
"What the panel does not cover"). Only 628 MB would be new downloads. Every licence allows redistribution: eight CC0, two PDDL, one CC-BY-4.0 and one CC-BY-SA.

| # | Dataset (subject) | What it adds | Licence | Size |
|---|---|---|---|---:|
| 1 | ds000005 (sub-01), **fMRIPrep run** | fMRIPrep's own test subject: 3 T single-band BOLD, no field map, no slice timing, thick 4 mm slices | PDDL-1.0 | 100 MB |
| 2 | ds001600 (sub-1), **fMRIPrep run** | Siemens field maps of every kind (phase difference, two-phase, reversed phase encoding with mismatched grids); 5-volume runs | CC-BY-SA | 16 MB |
| 3 | ds005040 (sub-002), **fMRIPrep run** | **Philips**, reversed-phase-encoding field map, ascending slice order, strongly scaled integers | CC0-1.0 | 24 MB |
| 4 | ds005752 (sub-ON01016), **fMRIPrep run** | **GE**, direct field map in Hz (not linked to the BOLD), GE IR-FSPGR T1w | CC0-1.0 | 60 MB |
| 5 | ds005454 (sub-16), **fMRIPrep run** | **7 T**: 0.8 mm MP2RAGE, multiband 4 at 1.4 mm, 280 volumes, phase difference linked by B0FieldIdentifier | CC-BY-4.0 | 655 MB |
| 6 | ds004958 (sub-fsm02cd), new | **Multiband 8** (TR 0.8 s, **563 volumes**, sbref), spin-echo field-map pair *and* phase difference on one subject, **3 T MP2RAGE**, 4-echo MPRAGE, T2w; FreeSurfer's own maintenance dataset | CC0-1.0 | 177 MB |
| 7 | ds002422 (sub-02), new | **1.5 T** (Siemens Avanto), **75-year-old**, phase-difference maps, a task run that covers only a 79 mm slab | CC0-1.0 | 133 MB |
| 8 | ds000256 (sub-CTS203), new | **7.5-year-old child**, three runs with **graded head motion** (mean FD 0.15, 1.60, 4.97 mm), low-SNR T1w, T2w | PDDL-1.0 | 88 MB |
| 9 | ds000228 (sub-pixar008), extended | **3.5-year-old child** (T1w already in the catalog) plus her BOLD, **descending** slice order | CC0-1.0 | 35 MB |
| 10 | ds003592 (sub-280), extended | **82-year-old** (T1w already in the catalog) plus **multi-echo** BOLD (3 echoes) and a 0.8×0.8×3 mm FLAIR | CC0-1.0 | 139 MB |
| 11 | ds005003 (sub-RS001), new | **Brain tumour** (70 mL frontal glioma), T1w, thick-slice T2w, rest and motor BOLD | CC0-1.0 | 38 MB |
| 12 | ds004173 (sub-603309), new | The **same T1w scanned three times**: still, slight motion, strong motion (64-year-old) | CC0-1.0 | 35 MB |
| | **Total** | | | **1,499 MB** (628 MB new) |

Optional 13th, if a second multiband vendor matters more than the size: **ds002603** (Philips, the
same task single-band and multiband 4 at TR 0.55 s, each with its own field map, plus a Philips phase
difference; CC0; 247 MB).

**Recommendation:** approve the twelve. Then: fetch and hash the new files, check the defacing of every
anatomical image before anything reaches the site, run the fMRIPrep 25.2.6 oracle on the seven datasets
not run yet, and tag all panel files `panel` in the catalog.

---

## How the panel covers the criteria

`x` = covered by that dataset (numbers as in the table above).

| Criterion | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1.5 T | | | | | | | x | | | | | |
| 3 T | x | x | x | x | | x | | x | x | x | x | x |
| 7 T | | | | | x | | | | | | | |
| Siemens / GE / Philips | S¹ | S | **P** | **G** | S | S | S | S | S | ?² | S | S |
| Single-band BOLD | x | x | x | x | | | x | x | x | x | x | |
| Multiband BOLD | | | | | MB4 | **MB8** | | | | | | |
| Multi-echo BOLD | | | | | | | | | | x | | |
| Long run (≥ 280 vol.) / short run (≤ 63 vol.) | 240 | **5** | **63** | 200 | **280** | **563** | | 164 | 168 | 200 | | |
| Slice timing missing | x | | | | | | | | | | | |
| Ascending / descending slices | | | asc | | | | | | **desc** | | | |
| Interleaved (even-first / odd-first) | | odd | | even | MB | MB | odd | odd | | even | odd | |
| TR (s) | 2 | 3³ | 2 | 3 | 1.5 | **0.8** | 2.1, 3.56 | 2.5 | 2 | 3 | 2.6 | |
| Reversed-phase-encoding field map | | x⁴ | x | | | x⁵ | | | | | | |
| Phase-difference field map | | x | | | x | x⁵ | x | x⁵ | | | | |
| Direct field map | | | | x⁵ | | | | | | | | |
| No field map | x | | | | | | | | x | x | x | |
| T1w MPRAGE (or TFE/FSPGR) | x | x | TFE | FSPGR | | x | x | x | x | x | x | x |
| MP2RAGE | | | | | 7 T | **3 T** | | | | | | |
| T2w / FLAIR | in-plane T2 | | | | | T2w | | T2w | | FLAIR | T2w | |
| Anisotropic voxels | BOLD 4 mm | | | | | | slab | | | FLAIR 3 mm | T2w 5+1 mm | |
| Children | | | | | | | | 7.5 y | **3.5 y** | | | |
| Older adults | | | | | | | 75 y | | | **82 y** | | 64.6 y |
| Lesion | | | | | | | | | | | **tumour** | |
| Strong head motion | | | | | | | | **x** (BOLD) | | | | **x** (T1w) |
| Low SNR / artefacts | | | | | B1, noise | | 1.5 T | T1w SNR 6.6 | | | | motion |
| fMRIPrep end-to-end | x | x | x | x | x | x | x | x | x | x | x | anat only |

1. ds000005 has no per-image sidecars; the original study scanned on a Siemens 3 T (not recorded in the files).
2. ds003592's sidecars record no manufacturer or field strength; sub-280 is from the Toronto (York) site; the vendor is to be confirmed when the files are fetched (NIfTI header extension) or from the data descriptor.
3. TR of the acq-v4 run; the PEPOLAR target has TR 4 s.
4. ds001600's pair has mismatched grids (80×80×60 at 3 mm vs 64×64×44 at 3.75×3.75×4 mm), a hard case on purpose.
5. Present but **not linked** to the BOLD (no `IntendedFor`, no `B0FieldIdentifier`): fMRIPrep will not use it; larmorx's field-map tools can still be run on it directly.

## What the panel does not cover (or covers thinly)

- **1.5 T on GE or Philips.** The only clean 1.5 T BOLD with complete metadata is Siemens (ds002422).
  The GE 1.5 T candidates (ds003459, ds002236, ds003500; Signa) were rejected: ds003459's README says
  the images were converted from Analyze files "as orientation files were not present", so the
  geometry may be wrong, and its `SliceTiming` reaches the TR (3.0 s for TR 3 s), which is invalid.
  The only Philips 1.5 T BOLD (ds002614) is one subject without slice timing or phase-encoding data.
- **A stroke lesion, and a lesion mask.** The panel has one tumour, without a mask. Rejected: the
  tumour masks of BTC_preop (ds001226) are misfiled (the folder `sub-PAT26` holds a file named
  `sub-PAT31_…`, stored as an object of the post-operative dataset ds002080); the Aphasia Recovery
  Cohort (ds004884, CC0, chronic stroke, manual lesion masks) spreads each subject's T1w, T2w, BOLD and
  mask over different sessions. It is the best stroke option if the user wants one (about 60 MB for
  one subject).
- **GE or Philips multiband, multi-echo on more than one vendor, GE phase difference.** Optional
  extras: ds002603 (Philips multiband, above) and ds005012 (GE DISCOVERY MR750 HyperBand MB6, TR 0.8 s,
  linked reversed-phase-encoding pair, adolescents; CC0; but 315 MB per run).
- **Multi-echo with multiband.** Not in the panel (many Siemens candidates exist, e.g. ds006926,
  ds008765, all over 150 MB per echo set).
- **Defacing cannot be checked before download for two datasets.** ds002422 (and the optional
  ds002603) state nothing about defacing. Every anatomical image must be checked after the fetch; if
  one is not defaced, the site shows only brain-masked views of it. All the other new datasets state
  defacing (MiDeFace, pydeface or OpenNeuro's "affirmed defaced"), and the cached T1w images of datasets
  1–5, 9 and 10 were checked here on a mid-sagittal view: all are defaced.
- **Lesion masks, manual segmentations or other ground truth** are not part of the panel.

## The datasets in detail

Common to all: OpenNeuro, version-pinned snapshot, S3 object-version URLs. Sizes are the snapshot
listing's, decimal MB. "Used by" names the larmorx tools and fMRIPrep steps that read each file.

### 1. ds000005 "Mixed-gambles task", snapshot 00001 (git b094fc2f), sub-01 — already in the catalog, fMRIPrep run
- Files: T1w, in-plane T2, three BOLD runs (64×64×34×240, 3.125×3.125×4 mm, TR 2 s), top-level
  `task-mixedgamblestask_bold.json` (RepetitionTime only: no SliceTiming, no PhaseEncodingDirection). 100.4 MB.
- Licence: `"License"` gives the PDDL 1.0 text → **PDDL-1.0**. Defaced (checked on the cached T1w).
- Citation: Tom SM, Fox CR, Trepel C, Poldrack RA (2007) Science 315:515; OpenNeuro ds000005.
- Used by: every tool as the reference case; 3dTshift is *not* run (no SliceTiming), SDC is skipped.

### 2. ds001600 "Example fieldmaps", 1.0.0 (git b36f484f), sub-1 — catalog, fMRIPrep run
- Evidence: `MagneticFieldStrength 3`, `Manufacturer Siemens`, `Prisma_fit`; phasediff with
  `EchoTime1 0.004`, `EchoTime2 0.00646`, `IntendedFor`; BOLD acq-v4 `RepetitionTime 3`, 44 slices
  interleaved odd-first. Runs of 5 volumes. 41 files, 16.2 MB.
- Licence: `"License": "CC-BY-SA"` (no version) → **CC-BY-SA**: redistribution allowed; figures made
  from it that are adaptations must carry attribution and the same licence (see Licences below).
- Used by: sdcflows-like phase-difference, two-phase and PEPOLAR estimators; short-run edge cases.

### 3. ds005040 "Political Moralization", 1.2.0 (git 0a447e59), sub-002 — catalog, fMRIPrep run
- Evidence: `Philips`, `Achieva dStream`, 3 T; BOLD `RepetitionTime 2`, `ParallelReductionFactorInPlane 2`
  (SENSE), `PhaseEncodingDirection j`, `TotalReadoutTime 0.0138`, 40 slices **ascending**; field maps
  `dir-A`/`dir-P` gradient-echo EPI with `IntendedFor`. 24.0 MB.
- Licence: `"License": "CC0"` → **CC0-1.0**; OpenNeuro "affirmed defaced" (and checked).
- Citation: Cohen MS, Cao Q, Decety J. Political Moralization (PMM). OpenNeuro ds005040 v1.2.0, doi:10.18112/openneuro.ds005040.v1.2.0.

### 4. ds005752 "NIMH Healthy Research Volunteer Dataset", 2.1.0 (git f6c800b5), sub-ON01016 — catalog, fMRIPrep run
- Evidence: `GE`, `DISCOVERY MR750`, 3 T; field map `Units "Hz"`, no `IntendedFor`; BOLD `RepetitionTime 3`,
  48 slices interleaved even-first, `PhaseEncodingDirection j`, `TotalReadoutTime 0.0262`; a
  reversed-blip run (`dir-reverse`); IR-FSPGR T1w (`InversionTime 0.4`). 60.0 MB.
- Licence: CC0 (`dataset_description.json` and the LICENSE file) → **CC0-1.0**; affirmed defaced.
- Citation: Nugent AC, Thomas AG, Mahoney M, et al. (2022) The NIMH intramural healthy volunteer dataset: a comprehensive MEG, MRI, and behavioral resource. Sci Data 9:518, doi:10.1038/s41597-022-01623-9; OpenNeuro ds005752 v2.1.0.

### 5. ds005454 "fMRIPrep boot camp 7T training dataset", 1.0.0 (git cdd33694), sub-16 — catalog, fMRIPrep run
- Evidence: `MagneticFieldStrength 7`, `MAGNETOM Terra.X`; BOLD `MultibandAccelerationFactor 4`,
  `ParallelReductionFactorInPlane 3`, `RepetitionTime 1.5`, 96 slices / 24 slice times, `B0FieldSource
  "gre16"`; phasediff `B0FieldIdentifier "gre16"`; MP2RAGE UNI 0.8 mm. 654.8 MB (the BOLD is 607 MB).
- Licence: `dataset_description.json` says CC0 but the shipped LICENSE is CC BY 4.0 → recorded as
  **CC-BY-4.0** (attribution required). Affirmed defaced.
- Used by: high-resolution and 7 T B1-inhomogeneity cases for N4, brain extraction, registration; the
  B0FieldIdentifier link.

### 6. ds004958 "The FreeSurfer Maintenance Dataset", 1.0.0 (git 43b3eaec), sub-fsm02cd (25 y, F) — new
- Why: the HCP/ABCD-style acquisition the panel lacked, and the dataset FreeSurfer's developers use
  for their own maintenance (the larmorx-freesurfer port will need exactly this).
- Evidence: `Siemens`, `Prisma_fit`, 3 T; rest BOLD `PulseSequenceDetails …hcp_mbep2d_bold`,
  `MultibandAccelerationFactor 8`, `RepetitionTime 0.8`, 72 slices / 9 slice times,
  `PhaseEncodingDirection j-`, `TotalReadoutTime 0.0597`, `dcmmeta_shape [104, 104, 72, 563]`; sbref;
  `acq-se_dir-AP/PA_epi` (`hcp_mbep2d_se`, j-/j, same readout); `acq-gre_phasediff` (`EchoTime1 0.00492`,
  `EchoTime2 0.00738`); none of the field maps has `IntendedFor` or `B0FieldIdentifier`.
  Anatomy: MP2RAGE UNIT1 (TI 0.7/2.5 s, 1 mm), MPRAGE with navigators, 4 echoes (echo-1 magnitude and
  the RMS image taken), T2-SPACE with navigators.
- Files proposed (24, 176.8 MB): rest BOLD 114.3 MB + sbref, both field-map sets, echo-1 magnitude T1w,
  T1wRMS, UNIT1, T2w, sidecars, `participants.tsv`. Only the echo-1 magnitude is taken as T1w because
  fMRIPrep collects every `part-mag` T1w and would otherwise average four echoes as four runs.
- Licence: `"License": "CC0"` → **CC0-1.0**. Defacing: README "All anatomical data was defaced with
  mideface"; OpenNeuro affirmed defaced.
- Citation: Greve DN, Fischl B. The FreeSurfer Maintenance Dataset. OpenNeuro ds004958 v1.0.0,
  doi:10.18112/openneuro.ds004958.v1.0.0 (no HowToAcknowledge; references FsmData wiki, MiDeFace, fsm2bids).
- Caveat: the README says "50-repetition resting state data", the sidecar says 563 volumes
  (sub-fsm79bf: 600); to be confirmed from the header at fetch. The other subjects' BOLD files are
  ~700 MB (probably stored as float), this one 114 MB.

### 7. ds002422 "fMRI: resting state and arithmetic task", 1.1.0 (git 4eadb371), sub-02 (75 y, M) — new
- Evidence: top-level `T1w.json` `MagneticFieldStrength 1.5`, `Siemens`, `Avanto`, MPRAGE (`TR 1.9`,
  `InversionTime 1.1`); rest BOLD `RepetitionTime 3.56`, 36 slices of 3.6 mm (spacing 3.78), slice
  times up to 2.99 s (acquisition shorter than the TR), interleaved odd-first, `PhaseEncodingDirection j-`,
  `TotalReadoutTime 0.0284`; task BOLD `RepetitionTime 2.1`, 24 slices of 3 mm (spacing 3.3): a 79 mm
  slab; `run-01_phasediff` (`EchoTime1 0.00476`, `EchoTime2 0.00952`) `IntendedFor` both task runs,
  `run-02_phasediff` `IntendedFor` the rest run. Ages 20–75 in `participants.tsv`.
- Files (18, 132.7 MB): T1w `.nii` (uncompressed, 46.1 MB: probably float32), rest + both task runs,
  both field-map sets (uncompressed; magnitudes without sidecars, phasediff sidecars minimal).
- Licence: `"License": "CC0"` → **CC0-1.0**. Defacing: **not stated** (no README; not affirmed): check at fetch.
- Citation: Panikratova Y, Tomyshev A, Pechenkova E, Vlasova R. fMRI: resting state and arithmetic task.
  OpenNeuro ds002422 v1.1.0, doi:10.18112/openneuro.ds002422.v1.1.0 ("This dataset was provided by
  Ya. Panikratova, A. Tomyshev, E. Pechenkova, and R. Vlasova").

### 8. ds000256 "Behavioral interventions for reducing head motion during MRI scans in children", 00002 (git f93025ab), sub-CTS203 (7.5 y) — new
- Evidence: `Siemens`, `TrioTim`, 3 T; BOLD `RepetitionTime 2.5`, 32 slices interleaved odd-first,
  `PhaseEncodingDirection j-`, `TotalReadoutTime 0.0372`, 164 volumes per run. The dataset's MRIQC
  table (`derivatives/mriqc/bold.csv`) gives this child's runs a mean FD of 0.15 (restbaseline), 0.21,
  0.70, 1.47, 1.60 (restfixationfixed), 2.12 and 4.97 mm (restfixationnofeedback); the three taken span
  the range. T1w and T2w at 0.8 mm (`T1_.8x.8x.8_224Slices`, `T2_…tse_vfl`); MRIQC T1w SNR 6.6, CNR 1.57.
  Phase difference with **`EchoTime1 0.00765` > `EchoTime2 0.00519`** (magnitude1 TE 7.65 ms,
  magnitude2 5.19 ms) and no `IntendedFor`.
- Files (23, 87.9 MB) including the inherited top-level task sidecars and the two MRIQC tables (evidence).
- Licence: `"PDDL (http://opendatacommons.org/licenses/pddl/)"` → **PDDL-1.0**. Defacing: README
  "Pydeface was used on all anatomical images".
- Citation: Greene DJ, Koller JM, Hampton JM, et al. (2018) Behavioral interventions for reducing head
  motion during MRI scans in children. NeuroImage 171:234–245
  (https://www.sciencedirect.com/science/article/pii/S1053811918300235, the dataset's reference); OpenNeuro ds000256.
- Used by: head-motion correction, boldref, 3dAutomask and confounds over a controlled motion range
  (same child, scanner and protocol); brain extraction and segmentation on a low-SNR child T1w.

### 9. ds000228 "MRI data of 3–12 year old children and adults during viewing of a short animated film", 1.1.1 (git 11410ae3), sub-pixar008 (3.5 y, F) — extend
- Already in the catalog: her T1w. Added: the BOLD (168 volumes, 28.8 MB) and its sidecars. Top-level
  `task-pixar_bold.json`: `Siemens`, `TrioTim`, `RepetitionTime 2`, `PhaseEncodingDirection j-`,
  `EffectiveEchoSpacing 0.0005`; subject sidecar: 32 slices in **descending** order. MRIQC (the
  dataset's `bold.csv`): mean FD 0.37 mm, tSNR 66.8. (Children with far more motion exist here:
  sub-pixar061, 5.6 y, mean FD 1.83 mm; ds000256 covers motion.)
- Licence **CC0-1.0**; defaced (checked on the cached T1w).
- Citation: Richardson H, Lisandrelli G, Riobueno-Naylor A, Saxe R (2018) Nat Commun 9:1027.

### 10. ds003592 "Neurocognitive aging data release…", 1.0.13 (git b751b53f), sub-280 (82 y, F) — extend
- Already in the catalog: her T1w. Added: session 1's three echoes (`EchoTime 0.014`, `0.02996`,
  `0.04592`; `RepetitionTime 3`; 43 slices interleaved even-first; 200 volumes per the README for this
  site) and the FLAIR (0.8×0.8×3 mm per the README). 8 files, 129.4 MB. The sidecars have no
  `Manufacturer`, `MagneticFieldStrength` or `PhaseEncodingDirection` (so no SDC of any kind).
- Licence **CC0-1.0**; affirmed defaced (and checked on the cached T1w).
- Citation: Spreng RN, Setton R, Alter U, et al. (2022) Neurocognitive aging data release with
  behavioral, structural and multi-echo functional MRI measures. Sci Data 9 (2022),
  doi:10.1038/s41597-022-01231-7; OpenNeuro ds003592 v1.0.13.

### 11. ds005003 "Retrospective Task/Rest fMRI Data from Tumor Patients", 2.0.0 (git 143e1d3b), sub-RS001 — new
- Evidence: `patient_demographics.csv`: "RS 001, 31, M, …, Rght frontal lobe, 70, Oligodendroglioma".
  `Siemens`, `TrioTim`, 3 T; T1w MPRAGE (`TR 1.9`, `TI 0.9`); T2w `TRA T2 BLADE`, `SliceThickness 5`,
  `SpacingBetweenSlices 6`, 320 matrix; rest and motor BOLD `RepetitionTime 2.6`, 36 slices of 4 mm,
  interleaved odd-first, `ep2d_pace`, `PhaseEncodingDirection j-`, `TotalReadoutTime 0.0435`.
  10 files, 37.7 MB.
- Why this patient: of the 42 patients, most have minimal sidecars (no SliceTiming or phase encoding,
  e.g. RS007, RS009, RS019, RS028); RS001 has complete metadata and a large tumour.
- Licence `"CC0"` → **CC0-1.0**; affirmed defaced.
- Citation: Leuthardt EC, Shimony JS, Snyder AZ, Dierker D, Park KY. OpenNeuro ds005003 v2.0.0,
  doi:10.18112/openneuro.ds005003.v2.0.0; the authors ask to cite doi:10.1016/j.nic.2017.06.011 and
  doi:10.1371/journal.pone.0236423 and to name accession ds005003.
- Used by: brain extraction, Atropos segmentation and template registration with a mass lesion; BOLD-to-T1w registration.

### 12. ds004173 "Movement-related artefacts (MR-ART)", 1.0.2 (git fb45b349), sub-603309 (64.6 y, F) — new
- Evidence: `Siemens`, `Prisma`, 3 T, MPRAGE `TR 2.3`, `TI 0.9`; `acq-standard`, `acq-headmotion1`,
  `acq-headmotion2` T1w; `derivatives/scores.tsv` rates them 1, 2, 3 (good, medium, bad). 35 of 148
  subjects have exactly this 1/2/3 grading; this is the oldest of them. 9 files, 34.7 MB.
- Licence `"CC0"` → **CC0-1.0**; affirmed defaced (README: "anonymized").
- Citation: Nárai Á, Hermann P, Auer T, et al. (2022) Movement-related artefacts (MR-ART) dataset of
  matched motion-corrupted and clean structural MRI brain scans. Sci Data 9:630,
  doi:10.1038/s41597-022-01694-8; OpenNeuro ds004173 v1.0.2.
- Used by: every anatomical tool on matched clean and corrupted input; fMRIPrep `--anat-only` would
  merge the three T1w (its multi-T1w template path).

### Optional: ds002603 "MB-EPI-comparison", 1.0.0 (git 4e3c33fc), sub-01 — 16 files, 246.8 MB
- Evidence: `Philips`, 3 T (no model name); `acq-SeqMm3Tr2000` (single-band, TR 2) and
  `acq-Mb4Mm3Tr550` (`MultibandAccelerationFactor 4`, TR 0.55, 36 slices / 9 times, ascending within
  band); per-run reversed-PE EPI with `IntendedFor`; `acq-Mm2_phasediff` (`EchoTime1 0.003`,
  `EchoTime2 0.008`, magnitude1 only) `IntendedFor` all four runs, so two field maps claim the same run.
  Licence `"CC0"`; defacing not stated. Citation: Snoek L, Beemsterboer T, Scholte HS. MB-EPI-comparison.
  OpenNeuro ds002603 v1.0.0, doi:10.18112/openneuro.ds002603.v1.0.0.

## Licences

- CC0-1.0 (8) and PDDL-1.0 (ds000005, ds000256): no conditions; PDDL's acknowledgement requests are
  not binding, but the site should cite every dataset anyway.
- **CC-BY-4.0 (ds005454):** attribution on every page that shows it (already the catalog's reading).
- **CC-BY-SA (ds001600, version unspecified):** redistribution allowed; images derived from it that
  the site shows (e.g. a field-map view) are adaptations and should carry attribution and a
  CC-BY-SA notice. Worth one line in the site's evidence pages.

## Data quirks found while searching (worth keeping)

- ds000256: phase-difference sidecars of several children (CTS200, CTS203) give `EchoTime1` > `EchoTime2`; others (CTS209) the usual order.
- ds001226 (BTC_preop): tumour masks are misfiled (folder `sub-PATxx` holds another patient's mask, stored as an object of ds002080); masks for PAT01–PAT05 seem absent.
- ds003459 (GE 1.5 T): `SliceTiming` reaches the TR; README warns that orientation was not available at conversion.
- ds004958: README "50-repetition resting state" vs sidecar shape 563 volumes.
- ds005003: most patients' sidecars lack SliceTiming and phase encoding; RS007's T1w is from a Skyra and its BOLD from a Trio in the same session.
- ds003999 (stroke rehabilitation): T1w sidecars carry an `IntendedFor` pointing at BOLD runs.
- OpenNeuro listings can return, for a JSON file, the S3 object of *another* path with the same content (git-annex deduplication), e.g. ds003592 sub-280's sidecars point at sub-301's objects. Same bytes (checked), but the catalog should list the snapshot URL first for such files (the drafts do).

## How the candidates were found

- A metadata sweep of all 1,182 public OpenNeuro MRI datasets (latest snapshot; first subject's
  directory listing and one BOLD/T1w sidecar each; `scripts/sweep.py`, results `meta/sweep.jsonl`,
  summary `meta/analysis.txt`). Only 11 datasets record 1.5 T; 91 record multiband ≥ 6.
- Targeted reads of candidates: READMEs, `participants.tsv`, the datasets' own MRIQC tables (to rank
  motion and quality without downloading images), sidecars of the chosen subjects.
- Rejected and why: ds001226/ds002080 (mask misfiling, defacing not stated), ds004884 ARC (sessions
  scattered; kept as the stroke alternative), ds003999 (stroke rehab, no masks), ds003459/ds002236/
  ds003500 (GE 1.5 T, orientation and slice-timing problems), ds002614 (Philips 1.5 T, one subject,
  minimal metadata), ds005597 (subset of ds005003), ds006736/ds003452 (in the catalog; their roles —
  clean linked spin-echo PEPOLAR, 650-volume benchmark — are partly taken by ds004958 and stay
  available outside the panel).

## After approval (steps, not done)

1. Fetch the 628 MB of new files; fill in the SHA-256 and verify sizes (`python -m larmorx_testdata`);
   write `docs/sources/<collection>.md` for the five new collections and update the two extended ones.
2. Defacing check of every anatomical image (mid-sagittal and 3D view, kept private) before any site use.
3. Confirm from the headers: ds004958's volume count and data type, ds002422's T1w data type,
   ds003592's vendor.
4. fMRIPrep 25.2.6 oracle runs (`scripts/run_fmriprep_oracle.sh`, `--fs-no-reconall` as before) for
   ds004958, ds002422, ds000256, ds000228, ds003592, ds005003 (BOLD) and ds004173 (`--anat-only`):
   about 1 h each on this machine, longer for ds004958 (563 MB8 volumes).
5. Add the tag `panel` to the catalog vocabulary and to every panel file (existing ones listed in the
   `*.panel-additions.toml` comments).
