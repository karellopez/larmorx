# SPDX-License-Identifier: Apache-2.0
S=$1
for a in 'zz*' 'a*' '=1' '~' '{a,b}' 'x{' 'nm[x]'; do echo "== p9 [$a]"; env -i PATH=/usr/bin:/bin HOME=/h timeout 5 tcsh -f $S/p9.csh "$a" 2>&1; echo "== p10 [$a]"; env -i PATH=/usr/bin:/bin HOME=/h timeout 5 tcsh -f $S/p10.csh "$a" 2>&1; done
