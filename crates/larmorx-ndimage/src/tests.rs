// SPDX-License-Identifier: Apache-2.0
//! Unit tests of the filters, morphology and labelling on small hand-checked arrays (SciPy's
//! values on these inputs; the parity checks against SciPy itself are in
//! `validation/investigations/sdcflows-fieldmap-b/`).

use crate::*;

#[test]
fn median_and_extremes_of_a_line() {
    let a = [5.0f32, 1.0, 4.0, 2.0, 3.0];
    let l = Layout::c(&[5]);
    let fp = Footprint::full(&[3]);
    // scipy.ndimage.median_filter([5, 1, 4, 2, 3], 3) (reflect) = [5, 4, 2, 3, 3]
    let m = rank_filter::<f32, f32>(&a, &l, 1, &fp, Extend::Reflect, 0.0, &[0]).unwrap();
    assert_eq!(m, [5.0, 4.0, 2.0, 3.0, 3.0]);
    // minimum_filter1d(..., 3) = [1, 1, 1, 2, 2]
    let lo = min_or_max_filter1d::<f32, f32>(&a, &l, 3, 0, Extend::Reflect, 0.0, 0, true).unwrap();
    assert_eq!(lo, [1.0, 1.0, 1.0, 2.0, 2.0]);
    // maximum over the footprint = [5, 5, 4, 4, 3]
    let hi = min_or_max_filter::<f32, f32>(&a, &l, &fp, None, Extend::Reflect, 0.0, &[0], false)
        .unwrap();
    assert_eq!(hi, [5.0, 5.0, 4.0, 4.0, 3.0]);
    // constant mode with cval 9: maximum = [9, 5, 4, 4, 9]
    let c = min_or_max_filter::<f32, f32>(&a, &l, &fp, None, Extend::Constant, 9.0, &[0], false)
        .unwrap();
    assert_eq!(c, [9.0, 5.0, 4.0, 4.0, 9.0]);
}

#[test]
fn grey_dilation_with_a_structure() {
    // maximum of x + [1, 2, 1] around each element of [0, 1, 0, 0] (reflect): [2, 3, 2, 2]
    let a = [0.0f64, 1.0, 0.0, 0.0];
    let l = Layout::c(&[4]);
    let fp = Footprint::full(&[3]);
    let st = [1.0, 2.0, 1.0];
    let d =
        min_or_max_filter::<f64, f64>(&a, &l, &fp, Some(&st), Extend::Reflect, 0.0, &[0], false)
            .unwrap();
    assert_eq!(d, [2.0, 3.0, 2.0, 2.0]);
}

#[test]
fn binary_erosion_and_dilation() {
    // a 5 x 5 square of ones inside 7 x 7
    let mut a = vec![0u8; 49];
    for r in 1..6 {
        for c in 1..6 {
            a[r * 7 + c] = 1;
        }
    }
    let l = Layout::c(&[7, 7]);
    let st = generate_binary_structure(2, 1);
    let args = BinaryErosion {
        structure: &st,
        origins: &[0, 0],
        iterations: 1,
        border_value: false,
        invert: false,
        brute_force: false,
    };
    let e = binary_erosion::<u8, u8>(&a, &l, args, None).unwrap();
    assert_eq!(e.iter().map(|&v| v as usize).sum::<usize>(), 9);
    let e2 = binary_erosion::<u8, u8>(
        &a,
        &l,
        BinaryErosion {
            iterations: 2,
            ..args
        },
        None,
    )
    .unwrap();
    assert_eq!(e2.iter().map(|&v| v as usize).sum::<usize>(), 1);
    let e2b = binary_erosion::<u8, u8>(
        &a,
        &l,
        BinaryErosion {
            iterations: 2,
            brute_force: true,
            ..args
        },
        None,
    )
    .unwrap();
    assert_eq!(e2, e2b);
    let (rev, org) = dilation_arguments(&st, &[0, 0]);
    let d = binary_erosion::<u8, u8>(
        &e,
        &l,
        BinaryErosion {
            structure: &rev,
            origins: &org,
            invert: true,
            ..args
        },
        None,
    )
    .unwrap();
    // the dilated 3 x 3 square is a 5 x 5 square without its corners
    assert_eq!(d.iter().map(|&v| v as usize).sum::<usize>(), 21);
}

#[test]
fn structures() {
    let s = generate_binary_structure(2, 1);
    assert_eq!(
        s.mask,
        [false, true, false, true, true, true, false, true, false]
    );
    let it = iterate_structure(&s, 2);
    assert_eq!(it.shape, [5, 5]);
    assert_eq!(it.count(), 13);
    assert_eq!(generate_binary_structure(3, 3).count(), 27);
}

#[test]
fn labels_follow_the_first_element_in_c_order() {
    // 0 1 0 1
    // 1 1 0 1
    // 0 0 0 0
    // 1 0 1 1
    let a = [0u8, 1, 0, 1, 1, 1, 0, 1, 0, 0, 0, 0, 1, 0, 1, 1];
    let l = Layout::c(&[4, 4]);
    let (lab, n) = label(&a, &l, &generate_binary_structure(2, 1)).unwrap();
    assert_eq!(n, 4);
    assert_eq!(
        lab,
        Labels::I32(vec![0, 1, 0, 2, 1, 1, 0, 2, 0, 0, 0, 0, 3, 0, 4, 4])
    );
    // the same values in Fortran order are labelled by C order of the index, not of memory
    let mut f = vec![0u8; 16];
    for r in 0..4 {
        for c in 0..4 {
            f[c * 4 + r] = a[r * 4 + c];
        }
    }
    let (lab_f, _) = label(&f, &Layout::f(&[4, 4]), &generate_binary_structure(2, 1)).unwrap();
    assert_eq!(lab, lab_f);
    let Labels::I32(v) = &lab else { unreachable!() };
    let sums = sum_labels_bincount(&a, &l, v, &l, &[0, 1, 2, 3, 4, 9]).unwrap();
    assert_eq!(sums, [0.0, 3.0, 2.0, 1.0, 2.0, 0.0]);
}

#[test]
fn one_dimensional_rank_filter() {
    // the same medians as the n-dimensional filter on a full window
    let a = [5.0f64, 1.0, 4.0, 2.0, 3.0, 9.0, 0.5];
    for mode in [
        Extend::Reflect,
        Extend::Nearest,
        Extend::Mirror,
        Extend::Wrap,
        Extend::Constant,
    ] {
        let nd = rank_filter::<f64, f64>(
            &a,
            &Layout::c(&[7]),
            1,
            &Footprint::full(&[3]),
            mode,
            2.0,
            &[0],
        )
        .unwrap();
        assert_eq!(rank_filter_1d(&a, 1, 3, mode, 2.0, 0), nd, "{mode:?}");
    }
}
