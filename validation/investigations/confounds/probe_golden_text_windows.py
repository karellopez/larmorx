# SPDX-License-Identifier: Apache-2.0
"""The golden case cli.nipype.run's text normalisation on this platform: the lines its runs print
(nipype's inputs and outputs: plain paths, lists of paths through repr, which doubles a Windows
backslash, and the cwd as os.getcwd() gives it), normalised by the case's code before and after
2026-11-13's fix, against the lines recorded on Linux. Run with python -I and an optional
folder (a fresh temporary one by default); windows_paths.sh runs it on a Windows machine."""

import contextlib
import os
import sys
import tempfile
from pathlib import Path

LINUX = (
    "mask_files = ['<inputs>/conf_bold_mask.nii.gz']\n"
    "realigned_file = <inputs>/conf_bold.nii.gz\n"
    "components_file = <cwd>/components_file.txt\n"
    "in_vfs = ['<inputs>/conf_gm.nii.gz', '<inputs>/conf_wm.nii.gz', '<inputs>/conf_csf.nii.gz']\n"
    "out_masks = ['<inputs>/conf_csf.nii.gz', '<cwd>/acompcor_wm.nii.gz', '<cwd>/acompcor_wmcsf.nii.gz']\n"
)


def old(t, folder, here, inputs):
    for d, name in ((here, "<cwd>"), (str(folder), "<cwd>"), (str(inputs), "<inputs>")):
        t = t.replace(d, name)
    return t.replace("\\", "/")


def new(t, folder, here, inputs):
    for d, name in (
        (here, "<cwd>"),
        (str(folder), "<cwd>"),
        (os.path.realpath(folder), "<cwd>"),
        (str(inputs), "<inputs>"),
        (os.path.realpath(inputs), "<inputs>"),
    ):
        t = t.replace(d.replace("\\", "\\\\"), name).replace(d, name)
    return t.replace("\\\\", "/").replace("\\", "/")


base = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(tempfile.mkdtemp())
inputs = base / "golden-inputs0"
folder = base / "work" / "fix" / "acompcor"
inputs.mkdir(parents=True, exist_ok=True)
folder.mkdir(parents=True, exist_ok=True)
with contextlib.chdir(folder):
    here = os.getcwd()


def i(n: str) -> str:
    return str(Path(os.path.realpath(inputs)) / n)  # nipype resolves its File inputs


def c(n: str) -> str:
    return os.path.join(here, n)


printed = (
    f"mask_files = {[i('conf_bold_mask.nii.gz')]}\n"
    f"realigned_file = {i('conf_bold.nii.gz')}\n"
    f"components_file = {c('components_file.txt')}\n"
    f"in_vfs = {[i('conf_gm.nii.gz'), i('conf_wm.nii.gz'), i('conf_csf.nii.gz')]}\n"
    f"out_masks = {[i('conf_csf.nii.gz'), c('acompcor_wm.nii.gz'), c('acompcor_wmcsf.nii.gz')]}\n"
)
print("folder", folder, "| cwd", here)
for name, fn in (("old", old), ("new", new)):
    got = fn(printed, folder, here, inputs)
    print(name, "IDENTICAL to Linux's lines" if got == LINUX else "DIFFERS:\n" + got)
