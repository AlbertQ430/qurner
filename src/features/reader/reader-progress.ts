import type { ReaderTocNode } from "../../bridge/ipc-schemas";

/** 查找与 spineIndex 匹配的目录标签；无匹配时回退为“第 N 节”。 */
export function chapterLabel(toc: ReaderTocNode[], spineIndex: number): string {
  const find = (nodes: ReaderTocNode[]): string | null => {
    for (const node of nodes) {
      if (node.spineIndex === spineIndex) {
        return node.label;
      }
      const child = find(node.children);
      if (child !== null) {
        return child;
      }
    }
    return null;
  };
  return find(toc) ?? `第 ${spineIndex + 1} 节`;
}

/** 与 Rust 相同的章节权重全局进度，最多四位小数。 */
export function computeProgression(
  chapterWeights: number[],
  spineIndex: number,
  intraSpineProgression: number,
): number {
  if (chapterWeights.length === 0 || spineIndex >= chapterWeights.length) {
    return 0;
  }
  const total = chapterWeights.reduce((sum, weight) => sum + weight, 0);
  if (total <= 0) {
    return 0;
  }
  const prefix = chapterWeights.slice(0, spineIndex).reduce((sum, weight) => sum + weight, 0);
  const within = Number.isFinite(intraSpineProgression)
    ? Math.min(1, Math.max(0, intraSpineProgression))
    : 0;
  const value = (prefix + chapterWeights[spineIndex] * within) / total;
  return Math.round(Math.min(1, Math.max(0, value)) * 10_000) / 10_000;
}

/** 一位小数百分比。 */
export function formatPercent(value: number): string {
  return `${(value * 100).toFixed(1)}%`;
}
