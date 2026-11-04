#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# FSL's side of the first -S/-B/-A/-A2 probes (in parallel; WHO=fsl or rep)
P=$(cd "$(dirname "$0")" && pwd)
TD=${LARMORX_TESTDATA_CACHE:-$HOME/.cache/larmorx-testdata/data}/openneuro
ADULT=$TD/ds000005/sub-01/anat/sub-01_T1w.nii.gz
C7=$TD/ds000256/sub-CTS203/anat/sub-CTS203_T1w.nii.gz
C7T2=$TD/ds000256/sub-CTS203/anat/sub-CTS203_T2w.nii.gz
export WHO=${WHO:-fsl}
$P/bet.sh S-adult-d $ADULT -S -d &
$P/bet.sh S-adult-mv $ADULT -S -m -v &
$P/bet.sh S-adult-n $ADULT -S -n &
$P/bet.sh B-adult-d $ADULT -B -d &
$P/bet.sh B-adult-s $ADULT -B -s -m &
$P/bet.sh A-adult-d $ADULT -A -d &
T2=$C7T2 $P/bet.sh A2-child7-d $C7 -A2 T2 -d &
wait
echo "FSL-BATCH-DONE $(date +%T)"
