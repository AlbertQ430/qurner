import { useState } from "react";

import type { BookSummary } from "../../bridge/ipc-schemas";

interface BookCoverProps {
  book: BookSummary;
}

export function BookCover({ book }: BookCoverProps) {
  const [failed, setFailed] = useState(false);
  const usable = !failed && book.coverUrl !== null && book.coverUrl.startsWith("cover://");

  if (!usable) {
    return <div className="book-cover book-cover-placeholder" aria-hidden="true" />;
  }

  return (
    <img
      className="book-cover"
      src={book.coverUrl ?? undefined}
      alt={`《${book.title}》封面`}
      onError={() => setFailed(true)}
    />
  );
}
