# SPDX-License-Identifier: Apache-2.0
"""tedana's multi-echo tools: ``lx.tedana``.

- :func:`t2smap`: tedana's ``t2smap`` (tedana 26.0.3): the T2* and S0 maps of multi-echo fMRI
  (a log-linear or a monoexponential fit of the echoes' decay), the adaptive mask of each voxel's
  good echoes, the fit's quality (RMSE map and table) and the optimally combined series (Posse et
  al. 1999's T2*-weighted combination, or PAID); fMRIPrep's multi-echo step.

tedana is LGPL-2.1: its bit-exact replica is in the ``larmorx-gpl`` package (the GNU GPL version
3, as the LGPL's section 3 allows), a separate program larmorx runs and exchanges files with
(``docs/licensing.md``); its clean-room original, written from ``specs/t2smap.md``, is in this
package (``crates/larmorx-tedana``). ``implementation="auto"`` runs the replica when
``larmorx-gpl`` is installed and the clean-room original otherwise.
"""

from larmorx.tedana.t2smap import T2smapResult, t2smap

__all__ = ["T2smapResult", "t2smap"]
