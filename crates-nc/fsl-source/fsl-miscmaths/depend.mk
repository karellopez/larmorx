optimise.o: optimise.cc optimise.h miscmaths.h kernel.h
kernel.o: kernel.cc kernel.h miscmaths.h
miscmaths.o: miscmaths.cc miscmaths.h kernel.h miscprob.h
cspline.o: cspline.cc miscmaths.h kernel.h cspline.h
sparsefn.o: sparsefn.cc sparse_matrix.h sparsefn.h miscmaths.h kernel.h
bfmatrix.o: bfmatrix.cpp miscmaths.h kernel.h bfmatrix.h SpMat.h cg.h \
 bicg.h
nonlin.o: nonlin.cpp bfmatrix.h SpMat.h cg.h bicg.h miscmaths.h kernel.h \
 nonlin.h Simplex.h
Simplex.o: Simplex.cpp miscmaths.h kernel.h nonlin.h bfmatrix.h SpMat.h \
 cg.h bicg.h Simplex.h
SpMatMatrices.o: SpMatMatrices.cpp SpMat.h cg.h bicg.h miscmaths.h \
 kernel.h SpMatMatrices.h
sparse_matrix.o: sparse_matrix.cc sparse_matrix.h miscmaths.h kernel.h
kertest.o: kertest.cc kernel.h
minimize.o: minimize.cc miscmaths.h kernel.h minimize.h
t2z.o: t2z.cc t2z.h base2z.h
rungekutta.o: rungekutta.cc rungekutta.h
miscprob.o: miscprob.cc miscprob.h miscmaths.h kernel.h
base2z.o: base2z.cc base2z.h
histogram.o: histogram.cc miscmaths.h kernel.h histogram.h
f2z.o: f2z.cc f2z.h base2z.h
