import { useState } from "react";

import type { BookSummary } from "../../bridge/ipc-schemas";

interface BookCoverProps {
  book: BookSummary;
  /** 网格卡内按可用高度缩放的封面宽度（保持 2:3）；缺省为 100%。 */
  coverWidth?: number;
}

export function BookCover({ book, coverWidth }: BookCoverProps) {
  const [failed, setFailed] = useState(false);
  const usable = !failed && book.coverUrl !== null && book.coverUrl.startsWith("cover://");

  const sizeStyle = coverWidth === undefined ? undefined : { width: coverWidth };

  if (!usable) {
    return (
      <div className="book-cover book-cover-placeholder" aria-hidden="true" style={sizeStyle} />
    );
  }

  return (
    <img
      className="book-cover"
      src={book.coverUrl ?? undefined}
      alt={`《${book.title}》封面`}
      onError={() => setFailed(true)}
      style={sizeStyle}
    />
  );
}
