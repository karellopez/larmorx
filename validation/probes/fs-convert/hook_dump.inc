// SPDX-License-Identifier: Apache-2.0
// fs-convert's dump hook (build.sh puts it before mri_convert's check of a GDF image stem, after
// its option loop, its checks and the parse-only exit): every variable the options set, then the
// end of the process (the parent reads the line after its marker). CLOSED TO FREESURFER
// CLEAN-ROOM IMPLEMENTERS (it names mri_convert's variables and globals).
#define DI(x) probe::di(#x, (long long)(x))
#define DIA(x, n) probe::dia(#x, x, n)
#define DF(x) probe::df(#x, x)
#define DFA(x, n) probe::dfa(#x, x, n)
#define DS(x) probe::ds(#x, x)
#define DP(x) probe::dp(#x, x)
#define DM(x) probe::dm(#x, x)
#define DE(x) probe::de(x)
if (probe::dump_on) {
  probe::d.clear();
  DI(outside_val); DI(reorder_flag); DIA(reorder_vals, 3); DI(reorder4_flag); DIA(reorder4_vals, 4);
  DI(shiftdim_flag); DIA(shift_dim, 3); DF(invert_val); DS(in_name); DS(out_name);
  DI(conform_flag); DI(ConfKeepDC); DI(conform_width_256_flag); DI(conform_min); DF(conform_size);
  DS(AutoAlignFile); DM(AutoAlign); DI(nochange_flag); DI(store_orig_ras2vox_flag); DI(parse_only_flag);
  DI(in_info_flag); DI(out_info_flag); DI(template_info_flag); DI(in_stats_flag); DI(out_stats_flag);
  DI(read_only_flag); DI(no_write_flag); DI(in_matrix_flag); DI(out_matrix_flag); DI(force_ras_good);
  DI(sphinx_flag); DS(transform_fname); DI(transform_flag); DI(invert_transform_flag);
  DS(out_like_name); DI(out_like_flag); DI(crop_flag); DIA(crop_center, 3); DIA(crop_size, 3);
  DS(devxfm_subject); DI(DevXFM); DI(UpsampleFlag); DI(UpsampleFactor);
  DI(in_i_size_flag); DF(in_i_size); DI(in_j_size_flag); DF(in_j_size); DI(in_k_size_flag); DF(in_k_size);
  DI(out_i_size_flag); DF(out_i_size); DI(out_j_size_flag); DF(out_j_size); DI(out_k_size_flag); DF(out_k_size);
  DI(in_i_direction_flag); DFA(in_i_directions, 3); DI(in_j_direction_flag); DFA(in_j_directions, 3);
  DI(in_k_direction_flag); DFA(in_k_directions, 3); DI(out_i_direction_flag); DFA(out_i_directions, 3);
  DI(out_j_direction_flag); DFA(out_j_directions, 3); DI(out_k_direction_flag); DFA(out_k_directions, 3);
  DS(colortablefile); DI(in_orientation_flag); DS(in_orientation_string);
  DI(out_orientation_flag); DS(out_orientation_string);
  DI(in_center_flag); DFA(in_center, 3); DI(delta_in_center_flag); DFA(delta_in_center, 3);
  DI(out_center_flag); DFA(out_center, 3);
  DI(out_n_i_flag); DI(out_n_i); DI(out_n_j_flag); DI(out_n_j); DI(out_n_k_flag); DI(out_n_k);
  DI(voxel_size_flag); DFA(voxel_size, 3); DI(downsample_flag); DI(downsampleold_flag);
  DI(downsample2_flag); DFA(downsample_factor, 3);
  DI(in_n_i_flag); DI(in_n_i); DI(in_n_j_flag); DI(in_n_j); DI(in_n_k_flag); DI(in_n_k);
  DI(antialias_flag); DI(nthreads); DF(fwhm); DF(gstd); DI(reduce);
  DI(in_tr_flag); DF(in_tr); DI(in_ti_flag); DF(in_ti); DI(in_te_flag); DF(in_te);
  DI(in_flip_angle_flag); DF(in_flip_angle); DI(out_data_type); DS(out_data_type_string);
  DF(rescale_factor); DF(scale_factor); DF(out_scale_factor);
  DI(rescale_voxel); DI(rescale_voxel_c); DI(rescale_voxel_r); DI(rescale_voxel_s);
  DS(resample_type); DI(resample_type_val);
  DI(force_in_type_flag); DI(forced_in_type); DS(in_type_string);
  DI(force_out_type_flag); DI(forced_out_type); DS(out_type_string);
  DI(force_template_type_flag); DI(forced_template_type); DS(template_type_string);
  DS(subject_name); DS(gdf_image_stem); DI(reslice_like_flag); DS(reslice_like_name);
  DI(frame_flag); DI(mid_frame_flag); DI(nframes); DIA(frames, nframes);
  DI(erode_seg_flag); DI(n_erode_seg); DI(dil_seg_flag); DI(n_dil_seg); DS(dil_seg_mask);
  DI(SliceBias); DF(SliceBiasAlpha);
  DI(subsample_flag); DI(SubSampStart); DI(SubSampDelta); DI(SubSampEnd);
  DI(in_like_flag); DS(in_like_name); DI(FirstDicom); DS(FirstDicomFile);
  DI(roi_flag); DI(fill_parcellation_flag); DI(zero_outlines_flag);
  DI(smooth_parcellation_flag); DI(smooth_parcellation_count); DI(color_file_flag); DS(color_file_name);
  DI(translate_labels_flag); DI(no_scale_flag); DI(nthframe);
  DI(LeftRightReverse); DI(LeftRightReversePix); DI(LeftRightMirrorFlag); DI(LeftRightKeepFlag);
  DS(LeftRightMirrorHemi); DI(LeftRightSwapLabel); DM(lhswap); DM(rhswap);
  DI(FlipCols); DI(SliceReverse); DI(ascii_flag); DI(InStatTableFlag); DI(OutStatTableFlag);
  DI(zero_ge_z_offset_flag); DI(nskip); DI(ndrop);
  DS(in_name_only); DI(out_volume_type);
  DI(debug); DF(upper_thresh); DI(upper_thresh_flag); DI(ncutends); DI(cutends_flag);
  DI(slice_crop_flag); DI(slice_crop_start); DI(slice_crop_stop); DI(SplitFrames); DI(DeleteCMDs);
  DI(DoNewTransformFname); DS(NewTransformFname);
  DI(UseDCM2NIIX); DI(DCM2NIIX_createBIDS); DP(DCM2NIIX_outdir); DI(DCM2NIIX_no_ForceStackSameSeries);
  DP(DCM2NIIX_INFO_DUMP); DP(DCM2NIIX_DICOM_FLIST); DI(DCM2NIIX_OnlySingleFile); DP(DCM2NIIX_OPTS);
  DI(DoSiemensAsciiDump); DI(DoSiemensAsciiAltDump); DI(UseDICOMRead2);
  DP(SDCMStatusFile); DP(SDCMListFile); DI(N_Zero_Pad_Output); DI(N_Zero_Pad_Input);
  DI(MRIIO_Strip_Pound); DI(Gdiag_no); DI(Gdiag); DI(SliceResElTag1); DI(SliceResElTag2);
  DI(AutoSliceResElTag); DI(DICOMReadFirstDicomFile[0]);
  DE("FS_LOAD_DWI"); DE("FS_RESCALE_DICOM"); DE("FS_DESIRED_BVEC_SPACE"); DE("FS_ANALYZE_NO_RESCALE");
  DE("BFILE_LITTLE_ENDIAN"); DE("FS_dcmGetDWIParamsSiemens_VoxelSpace"); DE("NSLICES_OVERRIDE");
  DE("NCOLS_OVERRIDE"); DE("NROWS_OVERRIDE"); DE("FS_MOSAIC_FIX_NOASCII");
  probe::finish_dump();
}
#undef DI
#undef DIA
#undef DF
#undef DFA
#undef DS
#undef DP
#undef DM
#undef DE
