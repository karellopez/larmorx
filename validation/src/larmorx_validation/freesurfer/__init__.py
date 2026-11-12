# SPDX-License-Identifier: Apache-2.0
"""FreeSurfer's reference runs: the recordings, replaying one step, comparing FreeSurfer's files.

- :mod:`.recording`: a recorded recon-all run (``<workspace>/oracles/freesurfer-8.2.0/runs/
  <run>/``, made by ``validation/investigations/freesurfer-oracle/``): its steps, the subject
  directory before each, recon-all's environment;
- :mod:`.replay`: the subject directory rebuilt before a step, a program run there (FreeSurfer's
  own through the oracle-output cache, or LARMORX's), its files compared with the recorded ones;
- :mod:`.files`: FreeSurfer's files compared with their provenance (command line, time, user,
  machine, paths) left out, and what differs measured;
- :mod:`.package`: the installation's identity and the launchers the cache keys runs by.

The command: ``python -m larmorx_validation freesurfer-replay`` (``steps``, ``rebuild``, ``run``,
``compare``). How a recording is made and what it holds:
``validation/investigations/freesurfer-oracle/README.md``.

Written without FreeSurfer's source: the recordings, the files themselves, nibabel's readers and
black-box runs.
"""
