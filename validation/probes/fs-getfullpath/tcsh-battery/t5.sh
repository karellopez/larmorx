# SPDX-License-Identifier: Apache-2.0
S=$1
for a in 'zz*' 'a*' '[B]' '=1' '=0' '~' '{a,b}' 'x{' '{}'; do echo "== p6 [$a]"; env -i PATH=/usr/bin:/bin HOME=/h timeout 5 tcsh -f $S/p6.csh "$a" 2>&1; done
for a in 'a{}b' '{}' '{' '}' 'x/{}' '{}x'; do echo "== p7 [$a]"; env -i PATH=/usr/bin:/bin HOME=/h timeout 5 tcsh -f $S/p7.csh "$a" 2>&1; done
