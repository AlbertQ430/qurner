import { commands } from "../../bridge/commands";
import type { BookSort, BookSummary, SortDirection } from "../../bridge/ipc-schemas";

export interface LibraryListInput {
  query?: string;
  sort: BookSort;
  direction: SortDirection;
}

export interface LibraryCommandClient {
  listBooks(input: LibraryListInput): Promise<BookSummary[]>;
  softRemoveBook(bookId: string): Promise<void>;
}

export const tauriLibraryClient: LibraryCommandClient = {
  listBooks: (input) => commands.listBooks(input),
  softRemoveBook: (bookId) => commands.softRemoveBook(bookId),
};
