// SPDX-License-Identifier: Apache-2.0
// fs-convert's init hook (build.sh puts it after mri_convert's last declaration in main): main's
// locals that the program leaves unset until an option sets them get marked values, so that the
// dump hook prints them as U (unset). CLOSED TO FREESURFER CLEAN-ROOM IMPLEMENTERS (it names
// mri_convert's variables).
probe::unset_i(reorder_vals, 3); probe::unset_i(shift_dim, 3); probe::unset_i(reorder4_vals, 4);
probe::unset_s(resample_type);
probe::unset_f(&in_i_size, 1); probe::unset_f(&in_j_size, 1); probe::unset_f(&in_k_size, 1);
probe::unset_f(&out_i_size, 1); probe::unset_f(&out_j_size, 1); probe::unset_f(&out_k_size, 1);
probe::unset_f(in_i_directions, 3); probe::unset_f(in_j_directions, 3); probe::unset_f(in_k_directions, 3);
probe::unset_f(out_i_directions, 3); probe::unset_f(out_j_directions, 3); probe::unset_f(out_k_directions, 3);
probe::unset_f(voxel_size, 3);
probe::unset_s(in_orientation_string); probe::unset_s(out_orientation_string);
probe::unset_f(in_center, 3); probe::unset_f(out_center, 3); probe::unset_f(delta_in_center, 3);
probe::unset_s(out_data_type_string);
probe::unset_i(&out_n_i, 1); probe::unset_i(&out_n_j, 1); probe::unset_i(&out_n_k, 1);
probe::unset_s(out_type_string); probe::unset_s(template_type_string); probe::unset_s(reslice_like_name);
probe::unset_i(frames, 2000);
probe::unset_i(&SubSampStart, 1); probe::unset_i(&SubSampDelta, 1); probe::unset_i(&SubSampEnd, 1);
probe::unset_f(downsample_factor, 3);
probe::unset_s(in_name_only); probe::unset_s(transform_fname);
probe::unset_i(&smooth_parcellation_count, 1);
probe::unset_s(in_like_name); probe::unset_s(out_like_name);
probe::unset_i(&in_n_i, 1); probe::unset_i(&in_n_j, 1); probe::unset_i(&in_n_k, 1);
probe::unset_i(&n_erode_seg, 1); probe::unset_i(&n_dil_seg, 1);
probe::unset_s(FirstDicomFile); probe::unset_s(color_file_name); probe::unset_s(devxfm_subject);
probe::unset_i(&rescale_voxel_c, 1); probe::unset_i(&rescale_voxel_r, 1); probe::unset_i(&rescale_voxel_s, 1);
probe::unset_s(LeftRightMirrorHemi); probe::unset_s(AutoAlignFile);
