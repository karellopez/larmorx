# FreeSurfer 8.2.0: the inventory

**Closed to FreeSurfer clean-room implementers.** Everything in this folder is written from
FreeSurfer's source (`<workspace>/tags/freesurfer-8.2.0`, the scripts and Python of the installed
package) by replica-side agents (`docs/porting-guide.md` §1). A clean-room implementer of any
FreeSurfer program must not open, search or list the files of this folder; what it needs comes
through `specs/<program>.md`, written by the spec writers of each wave.

This is step 3.0 of [the FreeSurfer plan](../freesurfer-plan.md) ("3.0 Preparation", item 3): the
programs, the code each needs, the third-party audit, the data files, variation, the waves and the
porting cards. It grows relay by relay; the state of each part is below.

| File | What it holds | State |
|---|---|---|
| [programs.md](programs.md) | every program `recon-all` 8.2.0 runs in its two streams (v8, the default, and `-no-v8`), and the FreeSurfer programs fMRIPrep 25.2.6 and sMRIPrep 0.19.2 run outside it, each with the options used and the step that runs it; the corrections this brings to the plan | relay 1, from the script; to be checked against the recordings when they exist |
| [programs.toml](programs.toml) | the same, one entry per program, machine-readable (the base the cards and the queue are built from) | relay 1 |
| [code.md](code.md) | per program: its source files, the shared library functions it reaches, their size; the foundations | started by relay 1 (the build graph's first pass); relay 2 continues |
| `third-party.md` | the licence and origin of every file in those call graphs | later |
| `data-files.md` | every data file each program reads, its licence, source, size and checksum | later |
| `variation.md` | thread counts, seeds, floating-point settings | later |
| `waves.md`, `queue.md`, `cards/` | the waves checked, the layout of `crates-freesurfer/`, the order of work, one porting card per program | later |

The tool behind the command lines: `validation/investigations/freesurfer-inventory/extract_commands.py`
(it prints FreeSurfer's script lines, so it is closed too).
