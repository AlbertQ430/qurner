/** 阅读正文文本索引与 Locator 文本搜索（不依赖布局与 Tauri）。 */

export interface TextIndexEntry {
  node: Text;
  /** 规范化文本（\\r\\n → \\n）中的起始 UTF-16 偏移。 */
  start: number;
  /** 规范化文本中的结束偏移（不含）。 */
  end: number;
  /** 节点原始文本，用于把规范化偏移映射回 DOM 偏移。 */
  original: string;
}

export interface TextIndex {
  text: string;
  entries: TextIndexEntry[];
}

export interface TextPoint {
  entry: TextIndexEntry;
  sourceOffset: number;
  textOffset: number;
}

export interface QuoteContext {
  exact: string;
  prefix: string;
  suffix: string;
}

const EXCLUDED_TAGS = new Set(["SCRIPT", "STYLE", "NOSCRIPT", "TEMPLATE"]);

/** 按 DOM 顺序建立正文文本索引；排除脚本/样式等非渲染内容。 */
export function buildTextIndex(root: Node): TextIndex {
  const walker =
    root.ownerDocument?.createTreeWalker(root, NodeFilter.SHOW_TEXT) ??
    document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  const entries: TextIndexEntry[] = [];
  let text = "";
  let node = walker.nextNode();
  while (node) {
    const parent = node.parentElement;
    if (!parent || !EXCLUDED_TAGS.has(parent.tagName)) {
      const original = node.nodeValue ?? "";
      const normalized = normalizeNewlines(original);
      if (normalized.length > 0) {
        entries.push({
          node: node as Text,
          start: text.length,
          end: text.length + normalized.length,
          original,
        });
        text += normalized;
      }
    }
    node = walker.nextNode();
  }
  return { text, entries };
}

function normalizeNewlines(value: string): string {
  return value.replace(/\r\n?/g, "\n");
}

/** 把规范化偏移映射回节点原始文本偏移。 */
export function normalizedOffsetToSource(entry: TextIndexEntry, normalizedOffset: number): number {
  let normalized = 0;
  let source = 0;
  const original = entry.original;
  while (source < original.length && normalized < normalizedOffset) {
    if (original[source] === "\r") {
      source += original[source + 1] === "\n" ? 2 : 1;
      normalized += 1;
    } else {
      source += 1;
      normalized += 1;
    }
  }
  return source;
}

/** 源文本偏移映射到规范化文本偏移。 */
export function sourceOffsetToNormalized(entry: TextIndexEntry, sourceOffset: number): number {
  let source = 0;
  let normalized = 0;
  while (source < sourceOffset && source < entry.original.length) {
    if (entry.original[source] === "\r") {
      source += entry.original[source + 1] === "\n" ? 2 : 1;
    } else {
      source += 1;
    }
    normalized += 1;
  }
  return Math.min(normalized, entry.end - entry.start);
}

/** 规范化文本偏移映射到 `(Text, sourceOffset)`。 */
export function locateOffset(index: TextIndex, offset: number): TextPoint | null {
  if (index.entries.length === 0) {
    return null;
  }
  const clamped = Math.max(0, Math.min(offset, index.text.length));
  for (const entry of index.entries) {
    if (clamped >= entry.start && clamped <= entry.end) {
      return {
        entry,
        sourceOffset: normalizedOffsetToSource(entry, clamped - entry.start),
        textOffset: clamped,
      };
    }
  }
  const last = index.entries[index.entries.length - 1];
  if (!last) {
    return null;
  }
  return {
    entry: last,
    sourceOffset: last.original.length,
    textOffset: index.text.length,
  };
}

/** 从文本偏移捕获 quote（exact 32 / prefix 32 / suffix 32 UTF-16 code unit）。 */
export function captureQuote(index: TextIndex, textOffset: number): QuoteContext {
  const start = Math.max(0, Math.min(textOffset, index.text.length));
  return {
    exact: index.text.slice(start, start + 32),
    prefix: index.text.slice(Math.max(0, start - 32), start),
    suffix: index.text.slice(start + 32, start + 64),
  };
}

/** 恢复 Locator 到文本点；无匹配返回 null。 */
export function restoreLocator(
  index: TextIndex,
  locator: {
    textOffset: number;
    intraSpineProgression: number;
    quote: QuoteContext;
  },
): TextPoint | null {
  if (index.text.length === 0) {
    return null;
  }
  const exact = locator.quote.exact;
  if (locator.textOffset <= index.text.length) {
    const point = locateOffset(index, locator.textOffset);
    const textAt = index.text.slice(locator.textOffset, locator.textOffset + exact.length);
    if (point && (exact.length === 0 || textAt === exact)) {
      return point;
    }
  }
  if (exact.length === 0) {
    return null;
  }

  const matches: number[] = [];
  let from = 0;
  while (from <= index.text.length - exact.length) {
    const found = index.text.indexOf(exact, from);
    if (found === -1) {
      break;
    }
    matches.push(found);
    from = found + 1;
  }
  if (matches.length === 0) {
    return null;
  }

  const scored = matches.map((position) => {
    const prefix = locator.quote.prefix;
    const suffix = locator.quote.suffix;
    const prefixMatches =
      prefix.length > 0 &&
      index.text.slice(Math.max(0, position - prefix.length), position) === prefix;
    const suffixMatches =
      suffix.length > 0 &&
      index.text.slice(position + exact.length, position + exact.length + suffix.length) === suffix;
    const score = (prefixMatches ? 2 : 0) + (suffixMatches ? 1 : 0);
    const distance = Math.abs(position / index.text.length - locator.intraSpineProgression);
    return { position, score, distance };
  });
  scored.sort((left, right) => right.score - left.score || left.distance - right.distance);
  const best = scored[0];
  return best ? locateOffset(index, best.position) : null;
}
