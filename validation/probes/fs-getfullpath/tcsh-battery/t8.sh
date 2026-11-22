# SPDX-License-Identifier: Apache-2.0
S=$1; W=$2
cd "$W/nm[x]"
for cmd in 'pwd' '/bin/pwd' '/bin/echo $cwd:q' "/bin/echo $W/nm[x]" "/bin/echo $W/nm[[]x]" 'which pwd'; do
printf '%s\n' '#!/bin/tcsh -f' "set b = \`$cmd\`" 'echo "n=$#b"' 'echo "[$b]"' > $S/p11.csh
echo "== [$cmd]"; env -i PATH=/usr/bin:/bin HOME=/h timeout 5 tcsh -f $S/p11.csh 2>&1; done
cd "$W"
printf '%s\n' '#!/bin/tcsh -f' "set b = \`/bin/echo $W/nm[x]\`" 'echo "n=$#b"' 'echo "[$b]"' > $S/p11.csh
echo "== from w3"; env -i PATH=/usr/bin:/bin HOME=/h timeout 5 tcsh -f $S/p11.csh 2>&1
