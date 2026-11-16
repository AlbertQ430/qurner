//! 基于章节权重的全局阅读进度计算。

/// 计算全局进度：前缀权重 + 当前章节权重 * 章内进度，结果夹紧到 0..=1，
/// 并四舍五入到最多四位小数。权重全为 0 或越界时返回 0。
#[must_use]
pub fn compute_global_progression(
    chapter_weights: &[u32],
    spine_index: usize,
    intra_spine_progression: f64,
) -> f64 {
    if chapter_weights.is_empty() || spine_index >= chapter_weights.len() {
        return 0.0;
    }
    let total: u64 = chapter_weights
        .iter()
        .map(|weight| u64::from(*weight))
        .sum();
    if total == 0 {
        return 0.0;
    }
    let prefix: u64 = chapter_weights[..spine_index]
        .iter()
        .map(|weight| u64::from(*weight))
        .sum();
    let within = if intra_spine_progression.is_finite() {
        intra_spine_progression.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let value = (prefix as f64 + f64::from(chapter_weights[spine_index]) * within) / total as f64;
    (value.clamp(0.0, 1.0) * 10_000.0).round() / 10_000.0
}
