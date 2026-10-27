# SPDX-License-Identifier: Apache-2.0
"""The data panel's images (larmorx-testdata ``panel.toml``) that the suites use directly.

fMRIPrep's own calls on the fourteen oracle runs (:mod:`~larmorx_validation.parity.fmriprep_nodes`)
reach each subject's T1w, BOLD runs and field maps as fMRIPrep processes them. The panel adds
images fMRIPrep never gives an ANTs program (T2w, FLAIR, the raw MP2RAGE, a 600-volume
multiband series) and difficulties its calls do not isolate (a lesion mask, 6 mm slices). The
suites' ``panel-images`` cases run the programs on these files directly: one case per distinct
difficulty, not one per image. Each constant is a ``{in:<catalog path>}`` placeholder (fetched
with ``python -m larmorx_testdata fetch --tier full --tag panel``).
"""

from __future__ import annotations

from dataclasses import dataclass

#: The category of the cases on the panel's images.
CATEGORY = "panel-images"
#: Their case id prefix.
PREFIX = "panel/images/"


@dataclass(frozen=True)
class PanelImage:
    """One image of the panel: its catalog path and what makes it hard."""

    path: str
    dataset: str
    subject: str
    what: str

    @property
    def arg(self) -> str:
        """The ``{in:...}`` placeholder of the suites' arguments."""
        return "{in:" + self.path + "}"

    def __str__(self) -> str:
        return self.arg


_ON = "openneuro"

MP2RAGE_7T = PanelImage(
    f"{_ON}/ds005454/sub-16/anat/sub-16_acq-mp2rage_T1w.nii.gz",
    "ds005454",
    "sub-16",
    "7 T MP2RAGE uniform image as reconstructed (0.8 mm, 224 x 272 x 288, its noisy background "
    "kept), the panel's largest anatomical image",
)
MP2RAGE_7T_DENOISED = PanelImage(
    f"{_ON}/ds005454/sub-16/anat/sub-16_acq-denoised_T1w.nii.gz",
    "ds005454",
    "sub-16",
    "7 T MP2RAGE uniform image, background denoised (0.8 mm)",
)
UNIT1_3T = PanelImage(
    f"{_ON}/ds004958/sub-fsm64ps/anat/sub-fsm64ps_acq-mp2rage_UNIT1.nii.gz",
    "ds004958",
    "sub-fsm64ps",
    "3 T MP2RAGE UNIT1 of a 68-year-old (1 mm, its salt-and-pepper background)",
)
T2W_OLDER = PanelImage(
    f"{_ON}/ds004958/sub-fsm64ps/anat/sub-fsm64ps_acq-t2spaceVnav_T2w.nii.gz",
    "ds004958",
    "sub-fsm64ps",
    "3 T T2-SPACE of a 68-year-old (1 mm, defaced with MiDeFace)",
)
MPRAGE_15T = PanelImage(
    f"{_ON}/ds002422/sub-02/anat/sub-02_T1w.nii",
    "ds002422",
    "sub-02",
    "1.5 T MPRAGE of a 75-year-old (float32, PSR storage, uncompressed)",
)
CHILD_T1W = PanelImage(
    f"{_ON}/ds000256/sub-CTS203/anat/sub-CTS203_T1w.nii.gz",
    "ds000256",
    "sub-CTS203",
    "a 7.5-year-old's low-quality 0.8 mm T1w",
)
CHILD_T2W = PanelImage(
    f"{_ON}/ds000256/sub-CTS203/anat/sub-CTS203_T2w.nii.gz",
    "ds000256",
    "sub-CTS203",
    "a 7.5-year-old's 0.8 mm T2w",
)
TODDLER_T1W = PanelImage(
    f"{_ON}/ds000228/sub-pixar008/anat/sub-pixar008_T1w.nii.gz",
    "ds000228",
    "sub-pixar008",
    "a 3.5-year-old's T1w (small head, immature contrast, oblique field of view)",
)
FLAIR_OLDER = PanelImage(
    f"{_ON}/ds003592/sub-280/ses-1/anat/sub-280_ses-1_FLAIR.nii.gz",
    "ds003592",
    "sub-280",
    "an 82-year-old's FLAIR (0.42 x 0.42 x 3 mm, 44 slices)",
)
TUMOUR_T1W = PanelImage(
    f"{_ON}/ds005003/sub-RS001/ses-1/anat/sub-RS001_ses-1_T1w.nii.gz",
    "ds005003",
    "sub-RS001",
    "a T1w with a 70 mL right frontal oligodendroglioma",
)
TUMOUR_T2W = PanelImage(
    f"{_ON}/ds005003/sub-RS001/ses-1/anat/sub-RS001_ses-1_T2w.nii.gz",
    "ds005003",
    "sub-RS001",
    "the tumour's T2w: 5 mm slices 6 mm apart (0.72 x 0.72 x 6 mm)",
)
MOTION_T1W = PanelImage(
    f"{_ON}/ds004173/sub-603309/anat/sub-603309_acq-headmotion2_T1w.nii.gz",
    "ds004173",
    "sub-603309",
    "a 64.6-year-old's MPRAGE with strong instructed head motion (rated 3)",
)
STROKE_T2W = PanelImage(
    f"{_ON}/ds004884/sub-M2079/ses-217/anat/sub-M2079_ses-217_acq-spc3_run-4_T2w.nii.gz",
    "ds004884",
    "sub-M2079",
    "a chronic left-hemisphere stroke (176 mL) on its T2w",
)
STROKE_LESION = PanelImage(
    f"{_ON}/ds004884/derivatives/lesion_masks/sub-M2079/ses-217/anat/"
    "sub-M2079_ses-217_acq-spc3_run-4_T2w_desc-lesion_mask.nii.gz",
    "ds004884",
    "sub-M2079",
    "the stroke's manual lesion mask, drawn on the T2w (uint8)",
)
STROKE_T1W = PanelImage(
    f"{_ON}/ds004884/sub-M2079/ses-217/anat/sub-M2079_ses-217_acq-tfl3p2_run-3_T1w.nii.gz",
    "ds004884",
    "sub-M2079",
    "the stroke's T1w",
)
BOLD_MB8 = PanelImage(
    f"{_ON}/ds004958/sub-fsm64ps/func/sub-fsm64ps_task-rest_bold.nii.gz",
    "ds004958",
    "sub-fsm64ps",
    "multiband-8 BOLD: 600 volumes of 104 x 104 x 72 (2 mm, uint16, TR 0.8 s)",
)
BOLD_MB4_PHILIPS = PanelImage(
    f"{_ON}/ds002603/sub-01/func/sub-01_task-wm_acq-Mb4Mm3Tr550_bold.nii.gz",
    "ds002603",
    "sub-01",
    "Philips multiband-4 BOLD: 588 volumes of 80 x 80 x 36 (float32, TR 0.55 s)",
)
BOLD_7T = PanelImage(
    f"{_ON}/ds005454/sub-16/func/sub-16_task-rest_bold.nii.gz",
    "ds005454",
    "sub-16",
    "7 T multiband-4 BOLD: 280 volumes of 160 x 160 x 96 (1.4 mm)",
)

ALL = (
    MP2RAGE_7T,
    MP2RAGE_7T_DENOISED,
    UNIT1_3T,
    T2W_OLDER,
    MPRAGE_15T,
    CHILD_T1W,
    CHILD_T2W,
    TODDLER_T1W,
    FLAIR_OLDER,
    TUMOUR_T1W,
    TUMOUR_T2W,
    MOTION_T1W,
    STROKE_T2W,
    STROKE_LESION,
    STROKE_T1W,
    BOLD_MB8,
    BOLD_MB4_PHILIPS,
    BOLD_7T,
)


def describe(text: str, *images: PanelImage) -> str:
    """A case's description: ``text``, then each image's dataset and difficulty."""
    named = "; ".join(f"{i.dataset} {i.subject}: {i.what}" for i in images)
    return f"{text} ({named})" if named else text


def ants_case(program: str, cid: str, description: str, args, images, **kw):
    """An :class:`~larmorx_validation.parity.ants_programs.AntsCase` of the panel tier on
    panel images: ``args`` with :class:`PanelImage` values turned into their placeholders,
    ``images`` the ones the description names."""
    from larmorx_validation.parity.ants_programs import AntsCase

    return AntsCase(
        PREFIX + cid,
        CATEGORY,
        describe(description, *images),
        program,
        tuple(a.arg if isinstance(a, PanelImage) else a for a in args),
        tier="panel",
        **kw,
    )


__all__ = ["ALL", "CATEGORY", "PREFIX", "PanelImage", "ants_case", "describe"]
