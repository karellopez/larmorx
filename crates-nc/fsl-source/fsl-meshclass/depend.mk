selfintersection.o: selfintersection.cpp meshclass.h mesh.h point.h \
 mpoint.h triangle.h pt_special.h profile.h
mesh.o: mesh.cpp mesh.h point.h triangle.h mpoint.h pt_special.h
pt_special.o: pt_special.cpp pt_special.h mesh.h point.h mpoint.h \
 triangle.h
mpoint.o: mpoint.cpp mpoint.h point.h triangle.h
profile.o: profile.cpp profile.h
drawmesh.o: drawmesh.cpp meshclass.h mesh.h point.h mpoint.h triangle.h \
 pt_special.h profile.h
triangle.o: triangle.cpp triangle.h point.h mpoint.h
point.o: point.cpp point.h
