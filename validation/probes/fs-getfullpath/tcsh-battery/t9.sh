# SPDX-License-Identifier: Apache-2.0
S=$1
cd $S/w3
for a in '[a-c]' '[A-C]' '[a-b]' '[B-a]' '[+-a]'; do echo "== p2 [$a]"; env -i PATH=/usr/bin:/bin HOME=/h LC_ALL=C LANG=C timeout 5 tcsh -f $S/p2.csh "$a" 2>&1; done
for a in '. /' '/ /' '/ .' '/ / /' '. . /' 'x /' '/ x' '. .' '/ /x' '. / .' '/ / .' 'nm[x] /' '. -' '. +' '. *' '. =' '. ==' '. !' '. (' '. )' '. %' '. 0'; do echo "== p12 [$a]"; env -i PATH=/usr/bin:/bin HOME=/h LC_ALL=C timeout 5 tcsh -f $S/p12.csh $a 2>&1; done
