# SPDX-License-Identifier: Apache-2.0
S=$1
cd $S/w4
for a in '[a-c]' '[A-C]' '[b-b]' '[c-a]' '[0-9]' '[Z-a]' '[z-A]' '[a-Z]' '[_-a]' '[A-z]' '[^a-c]'; do echo "== p2 [$a]"; env -i PATH=/usr/bin:/bin HOME=/h LC_ALL=C LANG=C timeout 5 tcsh -f $S/p2.csh "$a" 2>&1 | tr '\n' ' '; echo; done
for a in 'dd*' 'zz*' 'd?1' 'dd1' ; do echo "== p13 [$a]"; env -i PATH=/usr/bin:/bin HOME=/h LC_ALL=C LANG=C timeout 5 tcsh -f $S/p13.csh "$a" 2>&1; done
