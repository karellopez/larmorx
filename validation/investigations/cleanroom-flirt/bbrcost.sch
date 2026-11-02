setscale 1 force
setoption costfunction bbr
setoption bbrstep 1
setrow UU 1 0 0 0  0 1 0 0  0 0 1 0  0 0 0 1
clear U
measurecost 6 UU 0 0 0 0 0 0 rel
print U
setoption bbrstep 2
clear U
measurecost 6 UU 0 0 0 0 0 0 rel
print U
setoption bbrstep 200
clear U
measurecost 6 UU 0 0 0 0 0 0 rel
print U
setoption bbrstep 201
clear U
measurecost 6 UU 0 0 0 0 0 0 rel
print U
