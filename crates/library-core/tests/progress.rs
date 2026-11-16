//! 章节权重全局进度计算。

use library_core::compute_global_progression;

#[test]
fn computes_prefix_and_intra_chapter_progress() {
    let weights = [1, 1, 2];
    assert_eq!(compute_global_progression(&weights, 0, 0.0), 0.0);
    assert_eq!(compute_global_progression(&weights, 0, 0.5), 0.125);
    assert_eq!(compute_global_progression(&weights, 1, 1.0), 0.5);
    assert_eq!(compute_global_progression(&weights, 2, 0.5), 0.75);
    assert_eq!(compute_global_progression(&weights, 2, 1.0), 1.0);
}

#[test]
fn clamps_and_handles_invalid_inputs() {
    let weights = [1, 3];
    assert_eq!(compute_global_progression(&weights, 1, 2.0), 1.0);
    assert_eq!(compute_global_progression(&weights, 1, -1.0), 0.25);
    assert_eq!(compute_global_progression(&weights, 1, f64::NAN), 0.25);
    assert_eq!(compute_global_progression(&[], 0, 0.5), 0.0);
    assert_eq!(compute_global_progression(&weights, 3, 0.5), 0.0);
    assert_eq!(compute_global_progression(&[0, 0], 1, 0.5), 0.0);
    assert_eq!(compute_global_progression(&[1, 1, 1], 0, 1.0 / 3.0), 0.1111);
}
