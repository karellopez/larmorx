# SPDX-License-Identifier: Apache-2.0
"""nipype's command line for the interfaces larmorx has ported: ``larmorx nipype run``.

``nipypecli run <module> <Interface> [inputs]`` (nipype 1.12.0, ``nipype/scripts/cli.py``,
``scripts/utils.py``, ``scripts/instance.py``, ``utils/nipype_cmd.py``; Apache-2.0, the nipype
developers) runs one nipype interface from the shell: ``--list`` lists a module's interfaces, the
interface's name alone prints its help (an argparse parser built from its input traits: the
mandatory inputs positional in name order, the others ``--name``), and with inputs it sets them,
prints them, runs the interface in the current folder and prints its outputs. ``larmorx nipype
run`` (and :func:`run`) is that command for the modules of fMRIPrep's confound estimation, whose
interfaces larmorx runs without nipype: ``nipype.algorithms.confounds``,
``niworkflows.interfaces.{confounds, images, morphology, nibabel, utility, bold, patches}`` and
``fmriprep.interfaces.confounds`` (niworkflows 1.15.4, fMRIPrep 25.2.7). The same arguments,
listing, help, printed inputs and outputs, files, messages and exit statuses; nipype's tracebacks
as their first and last lines. ``docs/api/nipype-run.md`` has the details, the interfaces larmorx
runs and the defects of nipype's command line it corrects (PIPE-81, PIPE-82; ``reproduce`` gives
nipype's behaviour).
"""

from larmorx.nipype._run import MODULES, list_interfaces, ported_interfaces, run

__all__ = ["MODULES", "list_interfaces", "ported_interfaces", "run"]
