//     fslmerge.cc concatenate AVW files into a single output
//     Steve Smith, David Flitney, Stuart Clare and Matthew Webster, FMRIB Image Analysis Group
//     Copyright (C) 2000-2019 University of Oxford
/*  Part of FSL - FMRIB's Software Library
    http://www.fmrib.ox.ac.uk/fsl
    fsl@fmrib.ox.ac.uk

    Developed at FMRIB (Oxford Centre for Functional Magnetic Resonance
    Imaging of the Brain), Department of Clinical Neurology, Oxford
    University, Oxford, UK


    LICENCE

    FMRIB Software Library, Release 6.0 (c) 2018, The University of
    Oxford (the "Software")

    The Software remains the property of the Oxford University Innovation
    ("the University").

    The Software is distributed "AS IS" under this Licence solely for
    non-commercial use in the hope that it will be useful, but in order
    that the University as a charitable foundation protects its assets for
    the benefit of its educational and research purposes, the University
    makes clear that no condition is made or to be implied, nor is any
    warranty given or to be implied, as to the accuracy of the Software,
    or that it will be suitable for any particular purpose or for use
    under any specific conditions. Furthermore, the University disclaims
    all responsibility for the use which is made of the Software. It
    further disclaims any liability for the outcomes arising from using
    the Software.

    The Licensee agrees to indemnify the University and hold the
    University harmless from and against any and all claims, damages and
    liabilities asserted by third parties (including claims for
    negligence) which arise directly or indirectly from the use of the
    Software or the sale of any products based on the Software.

    No part of the Software may be reproduced, modified, transmitted or
    transferred in any form or by any means, electronic or mechanical,
    without the express permission of the University. The permission of
    the University is not required if the said reproduction, modification,
    transmission or transference is done without financial return, the
    conditions of this Licence are imposed upon the receiver of the
    product, and all original and amended source code is included in any
    transmitted product. You may be held legally responsible for any
    copyright infringement that is caused or encouraged by your failure to
    abide by these terms and conditions.

    You are not permitted under this Licence to use this Software
    commercially. Use for which any financial return is received shall be
    defined as commercial use, and includes (1) integration of all or part
    of the source code or the Software into a product for sale or license
    by or on behalf of Licensee to third parties or (2) use of the
    Software or any derivative of it for research with the final aim of
    developing software products for sale or license to a third party or
    (3) use of the Software or any derivative of it for research with the
    final aim of developing non-software products for sale or license to a
    third party, or (4) use of the Software to provide any service to an
    external organisation for which payment is received. If you are
    interested in using the Software commercially, please contact Oxford
    University Innovation ("OUI"), the technology transfer company of the
    University, to negotiate a licence. Contact details are:
    fsl@innovation.ox.ac.uk quoting Reference Project 9564, FSL.*/
#define EXPOSE_TREACHEROUS

#include <string>
#include "NewNifti/NewNifti.h"
#include "miscmaths/miscmaths.h"
#include "newimage/newimageall.h"
#include "newimage/fmribmain.h"

using namespace std;
using namespace NiftiIO;
using namespace NEWIMAGE;
using namespace MISCMATHS;

int print_usage(const string& progname)
{
  cout << endl;
  cout << "Usage: fslmerge <-x/y/z/t/a/tr> <output> <file1 file2 .......> [tr value in seconds]" << endl;
  cout << "     -t : concatenate images in time" << endl;
  cout << "     -x : concatenate images in the x direction"  << endl;
  cout << "     -y : concatenate images in the y direction"  << endl;
  cout << "     -z : concatenate images in the z direction" << endl;
  cout << "     -a : auto-choose: single slices -> volume, volumes -> 4D (time series)"  << endl;
  cout << "     -tr : concatenate images in time and set the output image tr to the final option value"  << endl;
  cout << "     -n <N> : only use volume <N> from each input file (first volume is 0 not 1)" << endl;
  return 1;
}

template <class T>
int fmrib_main(int argc, char *argv[])
{
  volume4D<T> input_volume;
  int direction(-1),xoffset=0,yoffset=0,zoffset=0,toffset=0;
  unsigned int outputPosition(!strcmp(argv[1], "-n") ? 3 : 2 );
  unsigned int firstInputPosition(outputPosition+1);

  NiftiHeader header(loadHeader(return_validimagefilename(string(argv[firstInputPosition]))));
  float newTR(header.pixdim[4]);
  int64_t specificVolume(-1);

  if (!strcmp(argv[1], "-t"))       direction=0;
  else if (!strcmp(argv[1], "-x"))  direction=1;
  else if (!strcmp(argv[1], "-y"))  direction=2;
  else if (!strcmp(argv[1], "-z"))  direction=3;
  else if (!strcmp(argv[1], "-a"))  direction=4;
  else if (!strcmp(argv[1], "-n")) {
    direction=0;
    specificVolume=atof(argv[2]);
  }
  else if (!strcmp(argv[1], "-tr")) {
    direction=0;
    newTR=atof(argv[--argc]);
  }
  if ( direction < 0 )
    return print_usage(string(argv[0]));
  if ( specificVolume >= 0 )
    header.dim[4]=1;
  int xdimtot(std::max(header.dim[1],(int64_t)1)), ydimtot(std::max(header.dim[2],(int64_t)1)), zdimtot(std::max(header.dim[3],(int64_t)1)), tdimtot(std::max(header.dim[4],(int64_t)1));

  //Automatic mode direction logic
  if(direction==4)
  {
    direction = 0;
    if ( zdimtot < 2 && tdimtot < 2 )
      direction=3;
  }
  //Calculate output image dimensions
  for( int vol = firstInputPosition+1; vol < argc; vol++ )
  {
    header=loadHeader(return_validimagefilename(string(argv[vol])));
    if ( specificVolume >= 0 )
      header.dim[4]=1;
    if (direction==0) tdimtot+=std::max(header.dim[4],(int64_t)1);
    if (direction==1) xdimtot+=std::max(header.dim[1],(int64_t)1);
    if (direction==2) ydimtot+=std::max(header.dim[2],(int64_t)1);
    if (direction==3) zdimtot+=std::max(header.dim[3],(int64_t)1);
  }
  volume4D<T> output_volume(xdimtot,ydimtot,zdimtot,tdimtot);
  read_volume4D(input_volume,string(argv[firstInputPosition]));
  output_volume.copyproperties(input_volume);
  //Copy data into output image
  for(int vol = firstInputPosition; vol < argc; vol++) {
    if (vol>firstInputPosition) {
      read_volume4D(input_volume,string(argv[vol]));
      // sanity check on valid orientation info (it should be consistent)
      if ((output_volume.sform_code()!=NIFTI_XFORM_UNKNOWN) || (output_volume.qform_code()!=NIFTI_XFORM_UNKNOWN)) {
	      if ((input_volume.sform_code()!=NIFTI_XFORM_UNKNOWN) || (input_volume.qform_code()!=NIFTI_XFORM_UNKNOWN)) {
	        float rms = rms_deviation(output_volume.newimagevox2mm_mat(),input_volume.newimagevox2mm_mat());
	        if ( rms > 0.5 && direction == 0 ) {   // arbitrary 0.5mm rms diff threshold - maybe too sensitive?
	          cerr << endl << "WARNING:: Inconsistent orientations for individual images when attempting to merge." <<endl;
	          cerr <<"          Merge will use voxel-based orientation which is probably incorrect - *PLEASE CHECK*!" <<endl<<endl;
	        }
	      }
      }
    }
    bool dimerror(false);
    if (direction == 0 && (input_volume.xsize() != xdimtot || input_volume.ysize() != ydimtot || input_volume.zsize() != zdimtot)) dimerror=true;
    if (direction == 1 && (input_volume.ysize() != ydimtot || input_volume.zsize() != zdimtot || input_volume.tsize() != tdimtot)) dimerror=true;
    if (direction == 2 && (input_volume.xsize() != xdimtot || input_volume.zsize() != zdimtot || input_volume.tsize() != tdimtot)) dimerror=true;
    if (direction == 3 && (input_volume.xsize() != xdimtot || input_volume.ysize() != ydimtot || input_volume.tsize() != tdimtot)) dimerror=true;
    if ( dimerror ) {
      cerr << "Error in size-match along non-concatenated dimension for input file: " << string(argv[vol]) << endl;
      return 1;
    }
    if ( specificVolume >= 0 )
      input_volume=volume<T>(input_volume[specificVolume]);
    for(int t=0;t<input_volume.tsize();t++)
      for(int k=0;k<input_volume.zsize();k++)
        for(int j=0;j<input_volume.ysize();j++)
          for(int i=0;i<input_volume.xsize();i++)
            output_volume.value(i+xoffset,j+yoffset,k+zoffset,t+toffset)=input_volume.value(i,j,k,t);
    if (direction==0)  toffset+=input_volume.tsize();
    if (direction==1)  xoffset+=input_volume.xsize();
    if (direction==2)  yoffset+=input_volume.ysize();
    if (direction==3)  zoffset+=input_volume.zsize();
  }
  output_volume.setTR(newTR);
  save_volume4D(output_volume,string(argv[outputPosition]));
  return 0;
}


int main(int argc,char *argv[])
{
  if (argc < 4)
    return print_usage(string(argv[0]));
  unsigned int firstInputPosition(!strcmp(argv[1], "-n") ? 4 : 3 );
  return call_fmrib_main(dtype(string(argv[firstInputPosition])),argc,argv);
}
