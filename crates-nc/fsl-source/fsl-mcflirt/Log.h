 /*  Log.h

    Mark Woolrich, FMRIB Image Analysis Group

    Copyright (C) 1999-2000 University of Oxford  */

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

#include <iostream>
#include <fstream>
#include <string>
#include "armawrap/newmat.h"

namespace UTILS{

#if !defined(__Log_h)
#define __Log_h

  class Log
  {
  public:
    static Log& getInstance();
    ~Log() { delete logger; }

    int establishDir(const std::string& name);
    void setDir(const std::string& name);
    void setLogFile(const std::string& name) {logfilename = name;}
    const std::string& getDir() const { return dir; }

    void out(const std::string& p_fname, const NEWMAT::Matrix& p_mat, bool p_addMatrixString = true);
    void out(const std::string& p_fname, const NEWMAT::RowVector& p_mat);
    void out(const std::string& p_fname, const NEWMAT::ColumnVector& p_mat);
    void in(const std::string& p_fname, NEWMAT::Matrix& p_mat);
    void in(const std::string& p_fname, NEWMAT::Matrix& p_mat, const int numRows, const int numWaves);
    void in(const std::string& p_fname, NEWMAT::RowVector& p_mat);
    void in(const std::string& p_fname, NEWMAT::ColumnVector& p_mat);

    std::ofstream& str() { return logfileout; }

  private:
    Log() {}

    const Log& operator=(Log&);
    Log(Log&);

    static Log* logger;
    std::string dir;
    std::ofstream logfileout;
    std::string logfilename;
  };

  inline void Log::out(const std::string& p_fname, const NEWMAT::Matrix& p_mat, bool p_addMatrixString)
  {
    std::ofstream out;
    out.open((dir + "/" + p_fname).c_str(), std::ios::out);

    if(p_addMatrixString)
      out << "/Matrix" << std::endl;

    out << p_mat;
    out.close();
  }

  inline void Log::out(const std::string& p_fname, const NEWMAT::ColumnVector& p_mat)
  {
    std::ofstream out;
    out.open((dir + "/" + p_fname).c_str(), std::ios::out);
    out << p_mat;
    out.close();
  }

  inline void Log::out(const std::string& p_fname, const NEWMAT::RowVector& p_mat)
  {
    std::ofstream out;
    out.open((dir + "/" + p_fname).c_str(), std::ios::out);
    out << p_mat;
    out.close();
  }

  inline void Log::in(const std::string& p_fname, NEWMAT::Matrix& p_mat)
  {
    std::ifstream in;
    in.open((dir + "/" + p_fname).c_str(), std::ios::in);

    if(!in)
      {
        std::cerr << "Unable to open " << p_fname << std::endl;
        throw;
      }

    int numWaves = 0;
    int numPoints = 0;

    std::string str;

    while(true)
      {
        in >> str;
        if(str == "/Matrix")
          break;
        else if(str == "/NumWaves")
          {
            in >> numWaves;
          }
        else if(str == "/NumPoints" || str == "/NumContrasts")
          {
            in >> numPoints;
          }
      }

    if(numWaves != 0)
      {
        p_mat.ReSize(numPoints, numWaves);
      }
    else
      {
        numWaves = p_mat.Ncols();
        numPoints = p_mat.Nrows();
      }

    for(int i = 1; i <= numPoints; i++)
      {
        for(int j = 1; j <= numWaves; j++)
          {
            in >> p_mat(i,j);
          }
	  }

    in.close();
  }

  inline void Log::in(const std::string& p_fname, NEWMAT::Matrix& p_mat, const int numWaves, const int numPoints)
  {
    std::ifstream in;
    in.open((dir + "/" + p_fname).c_str(), std::ios::in);

    if(!in)
      {
        std::cerr << "Unable to open " << p_fname << std::endl;
        throw;
      }

    p_mat.ReSize(numWaves, numPoints);

    for(int i = 1; i <= numPoints; i++)
      {
        for(int j = 1; j <= numWaves; j++)
          {
            in >> p_mat(i,j);
          }
	  }

    in.close();
  }

  inline void Log::in(const std::string& p_fname, NEWMAT::ColumnVector& p_mat)
  {
    int size = p_mat.Nrows();

    std::ifstream in;
    in.open((dir + "/" + p_fname).c_str(), std::ios::in);

    if(!in)
      {
        std::string err("Unable to open ");
        err =  err +  p_fname;
        throw NEWMAT::Exception(err.c_str());
      }

    for(int i = 1; i <= size; i++)
      {
        in >> p_mat(i);
      }

    in.close();
  }

  inline void Log::in(const std::string& p_fname, NEWMAT::RowVector& p_mat)
  {
    int size = p_mat.Ncols();

    std::ifstream in;
    in.open((dir + "/" + p_fname).c_str(), std::ios::in);

    if(!in)
      {
        std::string err("Unable to open ");
        err =  err +  p_fname;
        throw NEWMAT::Exception(err.c_str());
      }

    for(int i = 1; i <= size; i++)
      {
        in >> p_mat(i);
      }

    in.close();
  }

  inline Log& Log::getInstance(){
    if(logger == NULL)
      logger = new Log();

    return *logger;
  }

#endif

}
