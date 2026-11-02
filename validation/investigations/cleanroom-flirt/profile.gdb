# SPDX-License-Identifier: Apache-2.0
# The gdb commands of profile.sh: stop on SIGUSR1, print every thread's stack, go on.
set pagination off
set print thread-events off
set confirm off
handle SIGUSR1 stop print nopass
run
while 1
  thread apply all bt 40
  continue
end
