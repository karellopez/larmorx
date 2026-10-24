newimagefns.o: newimagefns.cc newimagefns.h newimage.h positerators.h \
 lazy.h complexvolume.h imfft.h
generalio.o: generalio.cc newimageio.h newimage.h positerators.h lazy.h \
 complexvolume.h
imfft.o: imfft.cc imfft.h newimage.h positerators.h lazy.h \
 complexvolume.h
costfns.o: costfns.cc costfns.h newimageall.h edt.h newimage.h \
 positerators.h lazy.h newimageio.h complexvolume.h newimagefns.h imfft.h
edt.o: edt.cc newimagefns.h newimage.h positerators.h lazy.h \
 complexvolume.h imfft.h newimageio.h edt.h
lazy.o: lazy.cc lazy.h
newimage.o: newimage.cc newimage.h positerators.h lazy.h
test_main.o: test/test_main.cc
complexvolume.o: complexvolume.cc newimagefns.h newimage.h positerators.h \
 lazy.h complexvolume.h imfft.h
