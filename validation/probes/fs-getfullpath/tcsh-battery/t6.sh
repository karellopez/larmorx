# SPDX-License-Identifier: Apache-2.0
S=$1; W=$2
for a in "$W/zz*" "$W/nm[x]" "nm[x]" "$W/nm[x]/q" "x[y]"; do echo "== p6 [$a]"; env -i PATH=/usr/bin:/bin HOME=/h timeout 5 tcsh -f $S/p6.csh "$a" 2>&1; done
printf '%s\n' '#!/bin/tcsh -f' 'set b = `pwd`' 'echo "n=$#b"' 'echo $b' > $S/p8.csh
cd "$W/nm[x]"; echo "== p8 in nm[x]"; env -i PATH=/usr/bin:/bin HOME=/h timeout 5 tcsh -f $S/p8.csh 2>&1
