import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  BookSettingsPatch,
  ReaderSession,
  ReaderSettings,
  ReaderSettingsPatch,
} from "../../bridge/ipc-schemas";
import type { ParentBridgeOptions, ParentBridgeState } from "../../bridge/parent";
import { type ReaderBridge, type ReaderCommandClient, ReaderShell } from "./ReaderShell";
import type { WindowCloseRegistrar } from "./reader-window";

const SESSION_ID = "123e4567-e89b-42d3-a456-426614174000";
const TOKEN = "a".repeat(32);
const FRAME_URL = `epub://${TOKEN}/OEBPS/chapter1.xhtml`;

function session(overrides: Partial<ReaderSession> = {}): ReaderSession {
  return {
    sessionId: SESSION_ID,
    bookId: "book-1",
    title: "示例书",
    frameUrl: FRAME_URL,
    spine: [
      { index: 0, href: "OEBPS/chapter1.xhtml" },
      { index: 1, href: "OEBPS/chapter2.xhtml" },
    ],
    chapterWeights: [1, 1],
    toc: [
      {
        id: "c1",
        label: "第一章",
        spineIndex: 0,
        children: [
          {
            id: "s1",
            label: "第一节",
            spineIndex: 0,
            fragment: "s1",
            children: [],
          },
        ],
      },
      {
        id: "c2",
        label: "第二章",
        spineIndex: 1,
        fragment: "s2",
        children: [],
      },
    ],
    initialLocator: {
      version: 1,
      spineIndex: 0,
      spineHref: "OEBPS/chapter1.xhtml",
      textOffset: 12,
      intraSpineProgression: 0.3,
      quote: { exact: "片段", prefix: "", suffix: "" },
    },
    initialProgression: 0.3,
    settings: { mode: "scrolling", fontScale: 1, theme: "day" },
    ...overrides,
  };
}

class FakeClient implements ReaderCommandClient {
  openCalls: string[] = [];
  closeCalls: string[] = [];
  bookSettingsCalls: Array<{ bookId: string; patch: unknown }> = [];
  appSettingsCalls: unknown[] = [];
  resetCalls: string[] = [];
  failSettings = false;
  failSave = false;
  deferSave = false;
  saveCalls: Array<{
    sessionId: string;
    spineIndex: number;
    intraSpineProgression: number;
  }> = [];
  pendingSaves: Array<{
    resolve: (value: { globalProgression: number; savedAt: string }) => void;
    reject: (error: unknown) => void;
  }> = [];
  deferSettings = false;
  pendingSettings: Array<{
    resolve: (value: ReaderSettings) => void;
    reject: (error: unknown) => void;
  }> = [];
  effectiveSettings: ReaderSettings = {
    mode: "paged",
    fontScale: 1,
    theme: "day",
  };
  closeError: unknown = null;
  defer = false;
  results: Array<ReaderSession | { error: unknown }> = [];
  pending: Array<{
    resolve: (value: ReaderSession) => void;
    reject: (error: unknown) => void;
  }> = [];

  openBook(bookId: string): Promise<ReaderSession> {
    this.openCalls.push(bookId);
    if (this.defer) {
      return new Promise((resolve, reject) => {
        this.pending.push({ resolve, reject });
      });
    }
    const next = this.results.shift();
    if (!next) {
      return Promise.reject(new Error("missing open result"));
    }
    if ("error" in next) {
      return Promise.reject(next.error);
    }
    return Promise.resolve(next);
  }

  async closeBook(sessionId: string): Promise<void> {
    this.closeCalls.push(sessionId);

    if (this.closeError !== null) {
      throw this.closeError;
    }
  }

  async getEffectiveSettings(): Promise<ReaderSettings> {
    return { ...this.effectiveSettings };
  }

  async updateAppSettings(patch: ReaderSettingsPatch): Promise<ReaderSettings> {
    if (this.failSettings) {
      throw { code: "E_STORAGE", message: "raw" };
    }
    this.appSettingsCalls.push(patch);
    return {
      mode: patch.mode ?? this.effectiveSettings.mode,
      fontScale: patch.fontScale ?? this.effectiveSettings.fontScale,
      theme: patch.theme ?? this.effectiveSettings.theme,
    };
  }

  async updateBookSettings(bookId: string, patch: BookSettingsPatch): Promise<ReaderSettings> {
    if (this.failSettings) {
      throw { code: "E_STORAGE", message: "raw" };
    }
    this.bookSettingsCalls.push({ bookId, patch });
    if (this.deferSettings) {
      return new Promise((resolve, reject) => {
        this.pendingSettings.push({ resolve, reject });
      });
    }
    const next: ReaderSettings = { ...this.effectiveSettings };
    if (patch.mode !== undefined && patch.mode !== null) {
      next.mode = patch.mode;
    }
    if (patch.fontScale !== undefined && patch.fontScale !== null) {
      next.fontScale = patch.fontScale;
    }
    if (patch.theme !== undefined && patch.theme !== null) {
      next.theme = patch.theme;
    }
    this.effectiveSettings = next;
    return next;
  }

  async saveReadingState(
    sessionId: string,
    locator: {
      spineIndex: number;
      intraSpineProgression: number;
    },
  ): Promise<{ globalProgression: number; savedAt: string }> {
    this.saveCalls.push({
      sessionId,
      spineIndex: locator.spineIndex,
      intraSpineProgression: locator.intraSpineProgression,
    });
    if (this.failSave) {
      throw { code: "E_STORAGE", message: "raw" };
    }
    if (this.deferSave) {
      return new Promise((resolve, reject) => {
        this.pendingSaves.push({ resolve, reject });
      });
    }
    return {
      globalProgression: 0.5,
      savedAt: "2026-11-17T00:00:00Z",
    };
  }

  bookmarks: import("../../bridge/ipc-schemas").Bookmark[] = [];
  createBookmarkCalls: Array<{ sessionId: string; locator: unknown }> = [];
  deleteBookmarkCalls: Array<{ bookId: string; bookmarkId: string }> = [];
  failBookmarkLoad = false;
  failBookmarkCreate = false;
  failBookmarkDelete = false;

  async listBookmarks(): Promise<import("../../bridge/ipc-schemas").Bookmark[]> {
    if (this.failBookmarkLoad) {
      throw { code: "E_STORAGE", message: "raw" };
    }
    return [...this.bookmarks];
  }

  async createBookmark(
    sessionId: string,
    locator: unknown,
  ): Promise<import("../../bridge/ipc-schemas").Bookmark> {
    if (this.failBookmarkCreate) {
      throw { code: "E_STORAGE", message: "raw" };
    }
    this.createBookmarkCalls.push({ sessionId, locator });
    const bookmark = {
      id: `123e4567-e89b-42d3-a456-42661417400${this.bookmarks.length}`,
      bookId: "book-1",
      locator: locator as never,
      excerpt: null,
      createdAt: "2026-11-18T00:00:00Z",
    };
    this.bookmarks.push(bookmark);
    return bookmark;
  }

  async deleteBookmark(bookId: string, bookmarkId: string): Promise<void> {
    if (this.failBookmarkDelete) {
      throw { code: "E_STORAGE", message: "raw" };
    }
    this.deleteBookmarkCalls.push({ bookId, bookmarkId });
    this.bookmarks = this.bookmarks.filter((item) => item.id !== bookmarkId);
  }

  async resetBookSettings(bookId: string): Promise<ReaderSettings> {
    if (this.failSettings) {
      throw { code: "E_STORAGE", message: "raw" };
    }
    this.resetCalls.push(bookId);
    this.effectiveSettings = { mode: "paged", fontScale: 1, theme: "day" };
    return { ...this.effectiveSettings };
  }
}

class FakeBridge implements ReaderBridge {
  started = false;
  closed = false;
  closeCount = 0;
  state: ParentBridgeState = "waiting";

  constructor(
    public readonly iframe: HTMLIFrameElement,
    public readonly options: ParentBridgeOptions,
  ) {}

  start(): void {
    this.started = true;
    this.state = "ready";
  }

  close(): void {
    this.closed = true;
    this.closeCount += 1;
  }

  settingsCalls: Array<{ mode: string; fontScale: number; theme: string }> = [];
  goToCalls: unknown[] = [];
  goToResult = true;
  captureCalls = 0;
  captureRequestIds: number[] = [];
  captureResult: unknown = null;
  turnCalls: string[] = [];

  updateSettings(settings: { mode: string; fontScale: number; theme: string }): void {
    this.settingsCalls.push(settings);
  }

  turnPage(direction: string): void {
    this.turnCalls.push(direction);
  }

  goTo(locator: unknown): boolean {
    this.goToCalls.push(locator);
    return this.goToResult;
  }

  captureLocator(requestId: number): boolean {
    this.captureCalls += 1;
    this.captureRequestIds.push(requestId);
    if (this.captureResult !== null) {
      this.options.onLocationChanged?.({
        reason: "capture",
        captureRequestId: requestId,
        locator: this.captureResult,
        mode: "paged",
        pageIndex: 0,
        pageCount: 1,
        atStart: true,
        atEnd: true,
        sessionId: "unused",
      } as never);
    }
    return true;
  }

  getState(): ParentBridgeState {
    return this.state;
  }
}

interface Harness {
  client: FakeClient;
  bridges: FakeBridge[];
  onBack: ReturnType<typeof vi.fn>;
  rerender: (bookId: string) => void;
  unmount: () => void;
}

function renderShell(
  client: FakeClient,
  bookId = "book-1",
  options: { registerWindowClose?: WindowCloseRegistrar } = {},
): Harness {
  const bridges: FakeBridge[] = [];
  const factory = (iframe: HTMLIFrameElement, options: ParentBridgeOptions) => {
    const bridge = new FakeBridge(iframe, options);
    bridges.push(bridge);
    return bridge;
  };
  const onBack = vi.fn();
  const view = render(
    <ReaderShell
      bookId={bookId}
      commandClient={client}
      bridgeFactory={factory}
      registerWindowClose={options.registerWindowClose}
      onBack={onBack}
    />,
  );
  return {
    client,
    bridges,
    onBack,
    rerender: (nextBookId: string) => {
      view.rerender(
        <ReaderShell
          bookId={nextBookId}
          commandClient={client}
          bridgeFactory={factory}
          registerWindowClose={options.registerWindowClose}
          onBack={onBack}
        />,
      );
    },
    unmount: view.unmount,
  };
}

async function waitForIframe(): Promise<HTMLIFrameElement> {
  return (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
}

beforeEach(() => {
  vi.useRealTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("ReaderShell", () => {
  it("opens a book, configures a sandboxed iframe and starts the bridge", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);

    expect(screen.getByText("正在打开书籍…")).toBeInTheDocument();
    const iframe = await waitForIframe();
    expect(iframe.getAttribute("src")).toBe(FRAME_URL);
    expect(iframe.getAttribute("sandbox")).toBe("allow-scripts");
    expect(iframe.getAttribute("referrerpolicy")).toBe("no-referrer");
    expect(screen.getByText("正在准备阅读内容…")).toBeInTheDocument();

    fireEvent.load(iframe);
    expect(harness.bridges).toHaveLength(1);
    const bridge = harness.bridges[0];
    expect(bridge?.started).toBe(true);
    expect(bridge?.options.sessionId).toBe(SESSION_ID);
    expect(bridge?.options.sessionToken).toBe(TOKEN);
    expect(bridge?.options.spineIndex).toBe(0);
    expect(bridge?.options.settings.mode).toBe("scrolling");
    expect(bridge?.options.initialLocator).toEqual(session().initialLocator);

    fireEvent.load(iframe);
    expect(harness.bridges).toHaveLength(1);

    await act(async () => {
      bridge?.options.onReady?.("0.1.0");
    });
    await waitFor(() => {
      expect(iframe.style.visibility).toBe("visible");
    });
    expect(screen.queryByText("正在准备阅读内容…")).not.toBeInTheDocument();
  });

  it("shows fixed open errors without backend messages and without retry for file errors", async () => {
    for (const [code, message] of [
      ["E_BOOK_NOT_FOUND", "该书已不在书库。"],
      ["E_BOOK_UNAVAILABLE", "原始文件已被移动、删除或无法读取。"],
      ["E_SOURCE_CHANGED", "文件内容与书库记录不一致，需要重新导入。"],
    ]) {
      const { unmount } = render(
        <ReaderShell
          bookId="book-1"
          commandClient={new FakeClient()}
          bridgeFactory={() => {
            throw new Error("unused");
          }}
          onBack={() => undefined}
        />,
      );
      unmount();
      const client = new FakeClient();
      client.results.push({ error: { code, message: "raw /Users/secret" } });
      const harness = renderShell(client);
      await screen.findByText(message);
      expect(screen.queryByRole("button", { name: "重新加载阅读页" })).not.toBeInTheDocument();
      expect(
        within(screen.getByRole("alert")).getByRole("button", {
          name: "返回书库",
        }),
      ).toBeInTheDocument();
      expect(document.body.textContent).not.toContain("raw");
      expect(document.body.textContent).not.toContain("/Users/secret");
      expect(client.closeCalls).toHaveLength(0);
      harness.unmount();
    }
  });

  it("allows retry for storage errors", async () => {
    const client = new FakeClient();
    client.results.push({ error: { code: "E_STORAGE", message: "raw" } });
    renderShell(client);
    await screen.findByText("本地阅读数据无法读取。");
    expect(screen.getByRole("button", { name: "重新加载阅读页" })).toBeInTheDocument();
  });

  it("reports iframe error events", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    const iframe = await waitForIframe();
    fireEvent(iframe, new Event("error"));
    await screen.findByText("阅读内容加载失败，请重试。");
    expect(screen.getByRole("button", { name: "重新加载阅读页" })).toBeInTheDocument();
  });

  it("reports handshake and iframe errors and retries with a new session and bridge", async () => {
    const client = new FakeClient();
    client.results.push(session(), session({ sessionId: "223e4567-e89b-42d3-a456-426614174000" }));
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    await act(async () => {
      harness.bridges[0]?.options.onError?.("E_HANDSHAKE_TIMEOUT", "raw");
    });
    await screen.findByText("阅读内容加载超时，请重试。");
    expect(screen.getByRole("button", { name: "重新加载阅读页" })).toBeInTheDocument();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "重新加载阅读页" }));
    });
    await waitFor(() => {
      expect(client.openCalls).toHaveLength(2);
    });
    expect(client.closeCalls).toEqual([SESSION_ID]);
    expect(harness.bridges[0]?.closed).toBe(true);
    const nextIframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    fireEvent.load(nextIframe);
    expect(harness.bridges).toHaveLength(2);
  });

  it("rejects unsafe frame urls and closes the created session", async () => {
    for (const frameUrl of [
      "epub://bad",
      `epub://${"a".repeat(32)}/OEBPS/%2e%2e%2fchapter.xhtml`,
      `epub://${"a".repeat(32)}/OEBPS/chapter.xhtml?x=1`,
      `epub://${"a".repeat(32)}/OEBPS/chapter.xhtml#frag`,
      `epub://${"a".repeat(32)}/OEBPS/chapter%5c.xhtml`,
      `epub://${"a".repeat(32)}/OEBPS/chapter%00.xhtml`,
    ]) {
      const client = new FakeClient();
      client.results.push(session({ frameUrl }));
      const harness = renderShell(client);
      await screen.findByText("阅读内容地址无效，请返回书库后重试。");
      expect(client.closeCalls).toEqual([SESSION_ID]);
      expect(screen.queryByTitle("《示例书》阅读内容")).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "重新加载阅读页" })).not.toBeInTheDocument();
      harness.unmount();
    }
  });

  it("loads the section declared by a non-first initial locator", async () => {
    const client = new FakeClient();
    const locator = {
      version: 1 as const,
      spineIndex: 1,
      spineHref: "OEBPS/chapter2.xhtml",
      textOffset: 3,
      intraSpineProgression: 0.2,
      quote: { exact: "第二章", prefix: "", suffix: "" },
    };
    client.results.push(session({ initialLocator: locator }));
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    expect(iframe.getAttribute("src")).toContain("chapter2.xhtml");
    fireEvent.load(iframe);
    expect(harness.bridges[0]?.options.initialLocator).toEqual(locator);
    expect(harness.bridges[0]?.options.spineIndex).toBe(1);
    expect(harness.bridges[0]?.options.initialNavigation).toBeUndefined();
  });

  it("opens a nested toc and navigates to a fragment section", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);

    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    const dialog = await screen.findByRole("dialog", { name: "目录" });
    expect(within(dialog).getByRole("navigation", { name: "目录" })).toBeInTheDocument();
    expect(within(dialog).getByText("第一章")).toBeInTheDocument();
    expect(within(dialog).getByText("第一节")).toBeInTheDocument();

    fireEvent.click(within(dialog).getByText("第二章"));
    await act(async () => {});
    expect(screen.queryByRole("dialog", { name: "目录" })).not.toBeInTheDocument();
    expect(harness.bridges[0]?.closed).toBe(true);
    const nextIframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    expect(nextIframe.getAttribute("src")).toContain("chapter2.xhtml");
    fireEvent.load(nextIframe);
    expect(harness.bridges[1]?.options.initialNavigation).toEqual({
      kind: "fragment",
      fragment: "s2",
    });
    expect(harness.bridges[1]?.options.initialLocator).toBeNull();
  });

  it("rebuilds the iframe for a same-chapter fragment and focuses content", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);

    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    const dialog = await screen.findByRole("dialog", { name: "目录" });
    fireEvent.click(within(dialog).getByText("第一节"));
    await act(async () => {});
    expect(screen.queryByRole("dialog", { name: "目录" })).not.toBeInTheDocument();
    expect(harness.bridges[0]?.closed).toBe(true);
    const nextIframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    expect(nextIframe.getAttribute("src")).toContain("chapter1.xhtml");
    fireEvent.load(nextIframe);
    expect(harness.bridges[1]?.options.initialNavigation).toEqual({
      kind: "fragment",
      fragment: "s1",
    });
    await act(async () => {
      harness.bridges[1]?.options.onReady?.("0.1.0");
    });
    expect(document.activeElement).toBe(nextIframe);
  });

  it("passes the logical spine index to bridges and never closes the session on section changes", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    expect(harness.bridges[0]?.options.spineIndex).toBe(0);

    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    const dialog = await screen.findByRole("dialog", { name: "目录" });
    fireEvent.click(within(dialog).getByText("第二章"));
    await act(async () => {});
    const nextIframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    fireEvent.load(nextIframe);
    expect(harness.bridges[1]?.options.spineIndex).toBe(1);
    expect(client.openCalls).toHaveLength(1);
    expect(client.closeCalls).toHaveLength(0);
  });

  it("navigates to a chapter without fragment using start", async () => {
    const client = new FakeClient();
    client.results.push(
      session({
        toc: [{ id: "c3", label: "第三章", spineIndex: 1, children: [] }],
      }),
    );
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    const dialog = await screen.findByRole("dialog", { name: "目录" });
    fireEvent.click(within(dialog).getByText("第三章"));
    await act(async () => {});
    const nextIframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    fireEvent.load(nextIframe);
    expect(harness.bridges[1]?.options.initialNavigation).toEqual({ kind: "start" });
  });

  it("ignores stale bridge callbacks after a section change", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    const dialog = await screen.findByRole("dialog", { name: "目录" });
    fireEvent.click(within(dialog).getByText("第二章"));
    await act(async () => {});
    const nextIframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    fireEvent.load(nextIframe);
    const before = nextIframe.getAttribute("src");

    await act(async () => {
      harness.bridges[0]?.options.onReady?.("0.1.0");
      harness.bridges[0]?.options.onSpineRequest?.({
        direction: "next",
        locator: {
          version: 1,
          spineIndex: 0,
          spineHref: "OEBPS/chapter1.xhtml",
          textOffset: 0,
          intraSpineProgression: 0,
          quote: { exact: "", prefix: "", suffix: "" },
        },
      });
    });
    expect(harness.bridges).toHaveLength(2);
    expect((screen.getByTitle("《示例书》阅读内容") as HTMLIFrameElement).getAttribute("src")).toBe(
      before,
    );
  });

  it("shows an empty toc without changing the frame", async () => {
    const client = new FakeClient();
    client.results.push(session({ toc: [] }));
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    const src = iframe.getAttribute("src");

    const tocButton = screen.getByRole("button", { name: "打开目录" });
    fireEvent.click(tocButton);
    const dialog = await screen.findByRole("dialog", { name: "目录" });
    expect(within(dialog).getByText("本书没有可用目录。")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "关闭目录" })).toBeInTheDocument();
    expect(tocButton).toHaveAttribute("aria-label", "关闭目录");
    expect(iframe.getAttribute("src")).toBe(src);
    expect(harness.bridges).toHaveLength(1);
    expect(client.closeCalls).toHaveLength(0);
  });

  it("closes the toc via backdrop or close button without navigation", async () => {
    for (const closeName of ["关闭目录遮罩", "关闭目录"]) {
      const client = new FakeClient();
      client.results.push(session());
      const harness = renderShell(client);
      const iframe = await waitForIframe();
      fireEvent.load(iframe);
      const src = iframe.getAttribute("src");
      const tocButton = screen.getByRole("button", { name: "打开目录" });
      fireEvent.click(tocButton);
      await screen.findByRole("dialog", { name: "目录" });
      const target =
        closeName === "关闭目录"
          ? within(screen.getByRole("dialog", { name: "目录" })).getByRole("button", {
              name: "关闭目录",
            })
          : screen.getByRole("button", { name: "关闭目录遮罩" });
      fireEvent.click(target);
      await waitFor(() => {
        expect(screen.queryByRole("dialog", { name: "目录" })).not.toBeInTheDocument();
      });
      expect(iframe.getAttribute("src")).toBe(src);
      expect(harness.bridges).toHaveLength(1);
      expect(client.closeCalls).toHaveLength(0);
      expect(document.activeElement).toBe(tocButton);
      harness.unmount();
    }
  });

  it("renders nested toc entries as parent/child DOM structure", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    await waitForIframe();
    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    const dialog = await screen.findByRole("dialog", { name: "目录" });
    const nav = within(dialog).getByRole("navigation", { name: "目录" });
    const topItems = nav.querySelectorAll(":scope > ul > li");
    expect(topItems).toHaveLength(2);
    const first = topItems[0] as HTMLElement;
    expect(first.querySelector(":scope > button")?.textContent).toBe("第一章");
    const childItems = first.querySelectorAll(":scope > ul > li");
    expect(childItems).toHaveLength(1);
    expect(childItems[0]?.querySelector(":scope > button")?.textContent).toBe("第一节");
    expect((topItems[1] as HTMLElement).querySelector(":scope > button")?.textContent).toBe(
      "第二章",
    );
  });

  it("closes the toc without navigation and restores focus", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    const before = iframe.getAttribute("src");

    const tocButton = screen.getByRole("button", { name: "打开目录" });
    fireEvent.click(tocButton);
    expect(screen.getByRole("dialog", { name: "目录" })).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "目录" })).not.toBeInTheDocument();
    });
    expect(iframe.getAttribute("src")).toBe(before);
    expect(harness.bridges).toHaveLength(1);
    expect(document.activeElement).toBe(tocButton);
  });

  it("carries the temporary mode into the next section", async () => {
    const client = new FakeClient();
    client.results.push(session({ settings: { mode: "paged", fontScale: 1, theme: "day" } }));
    const harness = renderShell(client);
    const initialIframe = await waitForIframe();
    fireEvent.load(initialIframe);
    fireEvent.click(screen.getByRole("button", { name: "连续滚动" }));
    expect(screen.getByRole("button", { name: "连续滚动" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    const dialog = await screen.findByRole("dialog", { name: "目录" });
    fireEvent.click(within(dialog).getByText("第二章"));
    await act(async () => {});
    const nextIframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    fireEvent.load(nextIframe);
    expect(harness.bridges[1]?.options.settings.mode).toBe("scrolling");
    expect(client.closeCalls).toHaveLength(0);
  });

  it("switches modes and does not page while the toc is open", async () => {
    const client = new FakeClient();
    client.results.push(session({ settings: { mode: "paged", fontScale: 1, theme: "day" } }));
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);

    fireEvent.click(screen.getByRole("button", { name: "连续滚动" }));
    expect(harness.bridges[0]?.settingsCalls[0]?.mode).toBe("scrolling");
    expect(screen.getByRole("button", { name: "连续滚动" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    fireEvent.keyDown(window, { key: "ArrowRight" });
    expect(harness.bridges[0]?.turnCalls).toEqual(["next"]);

    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    await screen.findByRole("dialog", { name: "目录" });
    fireEvent.keyDown(window, { key: "ArrowRight" });
    expect(harness.bridges[0]?.turnCalls).toEqual(["next"]);
  });

  it("navigates adjacent sections on spine requests and stops at boundaries", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);

    await act(async () => {
      harness.bridges[0]?.options.onSpineRequest?.({
        direction: "prev",
        locator: {
          version: 1,
          spineIndex: 0,
          spineHref: "OEBPS/chapter1.xhtml",
          textOffset: 0,
          intraSpineProgression: 0,
          quote: { exact: "", prefix: "", suffix: "" },
        },
      });
    });
    expect(harness.bridges).toHaveLength(1);

    await act(async () => {
      harness.bridges[0]?.options.onSpineRequest?.({
        direction: "next",
        locator: {
          version: 1,
          spineIndex: 0,
          spineHref: "OEBPS/chapter1.xhtml",
          textOffset: 0,
          intraSpineProgression: 0,
          quote: { exact: "", prefix: "", suffix: "" },
        },
      });
    });
    const nextIframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    expect(nextIframe.getAttribute("src")).toContain("chapter2.xhtml");
    fireEvent.load(nextIframe);
    expect(harness.bridges[1]?.options.initialNavigation).toEqual({ kind: "start" });

    await act(async () => {
      harness.bridges[1]?.options.onSpineRequest?.({
        direction: "next",
        locator: {
          version: 1,
          spineIndex: 1,
          spineHref: "OEBPS/chapter2.xhtml",
          textOffset: 0,
          intraSpineProgression: 0,
          quote: { exact: "", prefix: "", suffix: "" },
        },
      });
    });
    expect(harness.bridges).toHaveLength(2);
  });

  it("navigates both directions between logical spine items sharing one href", async () => {
    const client = new FakeClient();
    client.results.push(
      session({
        spine: [
          { index: 0, href: "OEBPS/chapter1.xhtml" },
          { index: 1, href: "OEBPS/chapter1.xhtml" },
        ],
        chapterWeights: [1, 1],
        initialLocator: null,
        toc: [
          { id: "a", label: "上部", spineIndex: 0, children: [] },
          {
            id: "b",
            label: "下部",
            spineIndex: 1,
            fragment: "s2",
            children: [],
          },
        ],
      }),
    );
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    expect(harness.bridges[0]?.options.spineIndex).toBe(0);

    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    const dialog = await screen.findByRole("dialog", { name: "目录" });
    fireEvent.click(within(dialog).getByText("下部"));
    await act(async () => {});
    const secondIframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    fireEvent.load(secondIframe);
    expect(harness.bridges[1]?.options.spineIndex).toBe(1);
    expect(harness.bridges[1]?.options.initialNavigation).toEqual({
      kind: "fragment",
      fragment: "s2",
    });
    expect(secondIframe.getAttribute("src")).toContain("chapter1.xhtml");

    await act(async () => {
      harness.bridges[1]?.options.onSpineRequest?.({
        direction: "prev",
        locator: {
          version: 1,
          spineIndex: 1,
          spineHref: "OEBPS/chapter1.xhtml",
          textOffset: 0,
          intraSpineProgression: 0,
          quote: { exact: "", prefix: "", suffix: "" },
        },
      });
    });
    const thirdIframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    fireEvent.load(thirdIframe);
    expect(harness.bridges[2]?.options.spineIndex).toBe(0);
    expect(harness.bridges[2]?.options.initialNavigation).toEqual({
      kind: "end",
    });
    expect(thirdIframe.getAttribute("src")).toContain("chapter1.xhtml");
    expect(client.openCalls).toHaveLength(1);
    expect(client.closeCalls).toHaveLength(0);
  });

  it("opens settings and changes font scale in 0.1 steps", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);

    fireEvent.click(screen.getByRole("button", { name: "阅读设置" }));
    const dialog = await screen.findByRole("dialog", { name: "阅读设置" });
    expect(within(dialog).getByText("字号：100%")).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "增大字号" }));
    await act(async () => {});
    expect(harness.bridges[0]?.settingsCalls.at(-1)?.fontScale).toBe(1.1);
    expect(client.bookSettingsCalls[0]).toEqual({
      bookId: "book-1",
      patch: { fontScale: 1.1 },
    });
    expect(within(dialog).getByText("字号：110%")).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole("button", { name: "减小字号" }));
    await act(async () => {});
    expect(within(dialog).getByText("字号：100%")).toBeInTheDocument();

    client.effectiveSettings = { mode: "paged", fontScale: 0.5, theme: "day" };
    fireEvent.click(screen.getByRole("button", { name: "关闭设置" }));
    fireEvent.click(screen.getByRole("button", { name: "阅读设置" }));
    const reopened = await screen.findByRole("dialog", { name: "阅读设置" });
    expect(within(reopened).getByRole("button", { name: "减小字号" })).toBeDisabled();
  });

  it("changes theme and persists book overrides", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "阅读设置" }));
    const dialog = await screen.findByRole("dialog", { name: "阅读设置" });
    fireEvent.click(within(dialog).getByRole("button", { name: "黑夜" }));
    await act(async () => {});
    expect(harness.bridges[0]?.settingsCalls.at(-1)?.theme).toBe("night");
    expect(client.bookSettingsCalls[0]?.patch).toEqual({ theme: "night" });
    expect(within(dialog).getByRole("button", { name: "黑夜" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
  });

  it("saves global defaults and resets the book", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "阅读设置" }));
    const dialog = await screen.findByRole("dialog", { name: "阅读设置" });
    fireEvent.click(within(dialog).getByRole("button", { name: "保存为全局默认" }));
    await act(async () => {});
    expect(client.appSettingsCalls[0]).toEqual({
      mode: "paged",
      fontScale: 1,
      theme: "day",
    });
    expect(within(dialog).getByText("已保存为全局默认")).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button", { name: "恢复本书跟随全局" }));
    await act(async () => {});
    expect(client.resetCalls).toEqual(["book-1"]);
    expect(within(dialog).getByText("已恢复为跟随全局")).toBeInTheDocument();
  });

  it("shows a fixed write failure and keeps the session", async () => {
    const client = new FakeClient();
    client.failSettings = true;
    client.results.push(session());
    const harness = renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "阅读设置" }));
    const dialog = await screen.findByRole("dialog", { name: "阅读设置" });
    fireEvent.click(within(dialog).getByRole("button", { name: "增大字号" }));
    await act(async () => {});
    expect(within(dialog).getByText("设置未能保存，请重试。")).toBeInTheDocument();
    expect(screen.getByRole("dialog", { name: "阅读设置" })).toBeInTheDocument();
    expect(client.closeCalls).toHaveLength(0);
    expect(harness.onBack).not.toHaveBeenCalled();
  });

  it("keeps settings and toc mutually exclusive and closes settings first on Esc", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    fireEvent.load(await waitForIframe());

    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    await screen.findByRole("dialog", { name: "目录" });
    fireEvent.click(screen.getByRole("button", { name: "阅读设置" }));
    await screen.findByRole("dialog", { name: "阅读设置" });
    expect(screen.queryByRole("dialog", { name: "目录" })).not.toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "阅读设置" })).not.toBeInTheDocument();
    });
  });

  it("hides controls in immersive mode and Esc closes settings before immersive", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    fireEvent.load(await waitForIframe());
    expect(screen.getByRole("button", { name: "返回书库" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "沉浸式" }));
    expect(screen.queryByRole("button", { name: "返回书库" })).not.toBeInTheDocument();
    const hotZone = screen.getByRole("button", { name: "显示阅读控件" });
    expect(hotZone).toBeInTheDocument();

    fireEvent.focus(hotZone);
    expect(screen.getByRole("button", { name: "返回书库" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "阅读设置" }));
    await screen.findByRole("dialog", { name: "阅读设置" });

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "阅读设置" })).not.toBeInTheDocument();
    });
    expect(screen.queryByRole("button", { name: "显示阅读控件" })).not.toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    expect(screen.getByRole("button", { name: "返回书库" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "显示阅读控件" })).not.toBeInTheDocument();
  });

  it("ignores settings responses from the previous book after switching", async () => {
    const client = new FakeClient();
    client.deferSettings = true;
    client.results.push(
      session({ settings: { mode: "paged", fontScale: 1, theme: "day" } }),
      session({
        bookId: "book-2",
        sessionId: "223e4567-e89b-42d3-a456-426614174000",
        settings: { mode: "paged", fontScale: 1, theme: "day" },
      }),
    );
    const first = renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "连续滚动" }));
    await act(async () => {});
    expect(client.pendingSettings).toHaveLength(1);
    first.unmount();
    await waitFor(() => {
      expect(client.closeCalls).toEqual([SESSION_ID]);
    });

    const second = renderShell(client, "book-2");
    await waitForIframe();
    client.deferSettings = false;
    await act(async () => {
      client.pendingSettings[0]?.resolve({
        mode: "scrolling",
        fontScale: 1.5,
        theme: "night",
      });
    });
    expect(screen.getByRole("button", { name: "分页" })).toHaveAttribute("aria-pressed", "true");
    second.unmount();
    await waitFor(() => {
      expect(client.closeCalls).toHaveLength(2);
    });
  });

  it("applies the latest settings after the iframe becomes ready", async () => {
    const client = new FakeClient();
    client.results.push(session({ settings: { mode: "paged", fontScale: 1, theme: "day" } }));
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    const bridge = harness.bridges[0];
    if (bridge) {
      bridge.state = "waiting";
    }
    expect(bridge?.getState()).toBe("waiting");

    fireEvent.click(screen.getByRole("button", { name: "连续滚动" }));
    await act(async () => {});
    expect(bridge?.settingsCalls).toHaveLength(0);

    await act(async () => {
      if (bridge) {
        bridge.state = "ready";
        bridge.options.onReady?.("0.1.0");
      }
    });
    expect(bridge?.settingsCalls.at(-1)?.mode).toBe("scrolling");
  });

  it("shows the write failure outside the settings drawer", async () => {
    const client = new FakeClient();
    client.failSettings = true;
    client.results.push(session({ settings: { mode: "paged", fontScale: 1, theme: "day" } }));
    renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "连续滚动" }));
    await act(async () => {});
    expect(screen.getByText("设置未能保存，请重试。")).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "阅读设置" })).not.toBeInTheDocument();
  });

  it("moves focus into the settings drawer and restores it on close", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    fireEvent.load(await waitForIframe());
    const settingsButton = screen.getByRole("button", { name: "阅读设置" });
    fireEvent.click(settingsButton);
    const dialog = await screen.findByRole("dialog", { name: "阅读设置" });
    expect(dialog.contains(document.activeElement)).toBe(true);
    fireEvent.click(within(dialog).getByRole("button", { name: "关闭设置" }));
    expect(document.activeElement).toBe(settingsButton);
  });

  it("shows initial chapter label and percentage from the restored locator", async () => {
    const client = new FakeClient();
    client.results.push(
      session({
        initialLocator: {
          version: 1,
          spineIndex: 1,
          spineHref: "OEBPS/chapter2.xhtml",
          textOffset: 0,
          intraSpineProgression: 0.4,
          quote: { exact: "", prefix: "", suffix: "" },
        },
        initialProgression: 0.4,
      }),
    );
    renderShell(client);
    await waitForIframe();
    expect(screen.getByLabelText("阅读位置")).toHaveTextContent("第二章 · 40.0%");
  });

  it("shows the start for a book without saved progress", async () => {
    const client = new FakeClient();
    client.results.push(session({ initialLocator: null, initialProgression: 0 }));
    renderShell(client);
    await waitForIframe();
    expect(screen.getByLabelText("阅读位置")).toHaveTextContent("第一章 · 0.0%");
  });

  it("debounces turn locations and saves only the latest", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    const bridge = harness.bridges[0];
    vi.useFakeTimers();
    expect(bridge).toBeDefined();
    await act(async () => {
      bridge?.options.onLocationChanged?.({
        reason: "init",
        locator: {
          version: 1,
          spineIndex: 0,
          spineHref: "OEBPS/chapter1.xhtml",
          textOffset: 0,
          intraSpineProgression: 0,
          quote: { exact: "", prefix: "", suffix: "" },
        },
      } as never);
    });
    await act(async () => {
      vi.advanceTimersByTime(1_100);
    });
    expect(client.saveCalls).toHaveLength(0);

    const locator = (intra: number, offset: number) => ({
      version: 1 as const,
      spineIndex: 0,
      spineHref: "OEBPS/chapter1.xhtml",
      textOffset: offset,
      intraSpineProgression: intra,
      quote: { exact: "", prefix: "", suffix: "" },
    });
    await act(async () => {
      bridge?.options.onLocationChanged?.({
        reason: "turn",
        locator: locator(0.2, 10),
      } as never);
      bridge?.options.onLocationChanged?.({
        reason: "turn",
        locator: locator(0.6, 30),
      } as never);
      vi.advanceTimersByTime(999);
    });
    expect(client.saveCalls).toHaveLength(0);
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(client.saveCalls).toHaveLength(1);
    expect(client.saveCalls[0]?.intraSpineProgression).toBe(0.6);
  });

  it("keeps one in-flight save and flushes the pending latest", async () => {
    const client = new FakeClient();
    client.deferSave = true;
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    const bridge = harness.bridges[0];
    vi.useFakeTimers();
    const locator = (intra: number) => ({
      version: 1 as const,
      spineIndex: 0,
      spineHref: "OEBPS/chapter1.xhtml",
      textOffset: 0,
      intraSpineProgression: intra,
      quote: { exact: "", prefix: "", suffix: "" },
    });
    await act(async () => {
      bridge?.options.onLocationChanged?.({
        reason: "turn",
        locator: locator(0.3),
      } as never);
      vi.advanceTimersByTime(1_000);
    });
    expect(client.saveCalls).toHaveLength(1);

    await act(async () => {
      bridge?.options.onLocationChanged?.({
        reason: "turn",
        locator: locator(0.8),
      } as never);
      vi.advanceTimersByTime(1_000);
    });
    expect(client.saveCalls).toHaveLength(1);
    expect(client.pendingSaves).toHaveLength(1);

    await act(async () => {
      client.pendingSaves[0]?.resolve({
        globalProgression: 0.3,
        savedAt: "2026-11-17T00:00:00Z",
      });
      await Promise.resolve();
    });
    await act(async () => {});
    expect(client.saveCalls).toHaveLength(2);
    expect(client.saveCalls[1]?.intraSpineProgression).toBe(0.8);
  });

  it("flushes before settings, navigation, back and unmount", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    fireEvent.click(screen.getByRole("button", { name: "分页" }));
    await act(async () => {});
    expect(client.saveCalls.length).toBeGreaterThanOrEqual(1);

    const beforeNavigation = client.saveCalls.length;
    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    const dialog = await screen.findByRole("dialog", { name: "目录" });
    fireEvent.click(within(dialog).getByText("第二章"));
    await act(async () => {});
    expect(client.saveCalls.length).toBeGreaterThan(beforeNavigation);

    const beforeBack = client.saveCalls.length;
    fireEvent.click(screen.getByRole("button", { name: "返回书库" }));
    await act(async () => {});
    expect(client.saveCalls.length).toBeGreaterThan(beforeBack);
    expect(harness.onBack).toHaveBeenCalledTimes(1);
  });

  it("shows a fixed progress failure message and retries on next change", async () => {
    const client = new FakeClient();
    client.failSave = true;
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    const bridge = harness.bridges[0];
    vi.useFakeTimers();
    const locator = {
      version: 1 as const,
      spineIndex: 0,
      spineHref: "OEBPS/chapter1.xhtml",
      textOffset: 0,
      intraSpineProgression: 0.5,
      quote: { exact: "", prefix: "", suffix: "" },
    };
    await act(async () => {
      bridge?.options.onLocationChanged?.({
        reason: "turn",
        locator,
      } as never);
      vi.advanceTimersByTime(1_000);
    });
    expect(screen.getByText("阅读进度未能保存，请重试。")).toBeInTheDocument();
    expect(client.closeCalls).toHaveLength(0);

    client.failSave = false;
    await act(async () => {
      bridge?.options.onLocationChanged?.({
        reason: "turn",
        locator,
      } as never);
      vi.advanceTimersByTime(1_000);
    });
    expect(screen.queryByText("阅读进度未能保存，请重试。")).not.toBeInTheDocument();
    expect(client.saveCalls).toHaveLength(2);
  });

  it("waits for the in-flight save before closing on back", async () => {
    const client = new FakeClient();
    client.deferSave = true;
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    const bridge = harness.bridges[0];
    vi.useFakeTimers();
    await act(async () => {
      bridge?.options.onLocationChanged?.({
        reason: "turn",
        locator: {
          version: 1,
          spineIndex: 0,
          spineHref: "OEBPS/chapter1.xhtml",
          textOffset: 0,
          intraSpineProgression: 0.4,
          quote: { exact: "", prefix: "", suffix: "" },
        },
      } as never);
      vi.advanceTimersByTime(1_000);
    });
    expect(client.saveCalls).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "返回书库" }));
    await act(async () => {});
    expect(client.closeCalls).toHaveLength(0);
    expect(harness.onBack).not.toHaveBeenCalled();

    await act(async () => {
      client.pendingSaves[0]?.resolve({
        globalProgression: 0.4,
        savedAt: "2026-11-17T00:00:00Z",
      });
    });
    await act(async () => {});
    expect(client.closeCalls).toHaveLength(1);
    expect(harness.onBack).toHaveBeenCalledTimes(1);
  });

  it("closes after the flush timeout even if saving never resolves", async () => {
    const client = new FakeClient();
    client.deferSave = true;
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    const bridge = harness.bridges[0];
    vi.useFakeTimers();
    await act(async () => {
      bridge?.options.onLocationChanged?.({
        reason: "turn",
        locator: {
          version: 1,
          spineIndex: 0,
          spineHref: "OEBPS/chapter1.xhtml",
          textOffset: 0,
          intraSpineProgression: 0.4,
          quote: { exact: "", prefix: "", suffix: "" },
        },
      } as never);
      vi.advanceTimersByTime(1_000);
    });
    expect(client.saveCalls).toHaveLength(1);

    fireEvent.click(screen.getByRole("button", { name: "返回书库" }));
    await act(async () => {
      vi.advanceTimersByTime(1_000);
    });
    await act(async () => {});
    expect(client.closeCalls).toHaveLength(1);
    expect(harness.onBack).toHaveBeenCalledTimes(1);
  });

  it("does not regress progress for a stale save response", async () => {
    const client = new FakeClient();
    client.deferSave = true;
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    const bridge = harness.bridges[0];
    const locator = (intra: number) => ({
      version: 1 as const,
      spineIndex: 0,
      spineHref: "OEBPS/chapter1.xhtml",
      textOffset: 0,
      intraSpineProgression: intra,
      quote: { exact: "", prefix: "", suffix: "" },
    });
    vi.useFakeTimers();
    await act(async () => {
      bridge?.options.onLocationChanged?.({
        reason: "turn",
        locator: locator(0.2),
      } as never);
      vi.advanceTimersByTime(1_000);
    });
    await act(async () => {
      bridge?.options.onLocationChanged?.({
        reason: "turn",
        locator: locator(0.6),
      } as never);
      vi.advanceTimersByTime(1_000);
    });
    expect(client.saveCalls).toHaveLength(1);

    await act(async () => {
      client.pendingSaves[0]?.resolve({
        globalProgression: 0.1,
        savedAt: "2026-11-17T00:00:00Z",
      });
      await Promise.resolve();
    });
    await act(async () => {});
    const position = screen.getByLabelText("阅读位置").textContent ?? "";
    expect(position).not.toContain("10.0%");
    expect(position).toContain("30.0%");
  });

  it("registers a window close handler and only flushes once per trigger", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const flushes: Array<() => Promise<void>> = [];
    const unregister = vi.fn();
    const harness = renderShell(client, "book-1", {
      registerWindowClose: (flush) => {
        flushes.push(flush);
        return unregister;
      },
    });
    fireEvent.load(await waitForIframe());
    expect(flushes).toHaveLength(1);
    await act(async () => {
      await flushes[0]?.();
    });
    expect(client.saveCalls).toHaveLength(1);
    harness.unmount();
    expect(unregister).toHaveBeenCalledTimes(1);
  });

  it("shows an empty bookmark list without reloading the iframe", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    const src = iframe.getAttribute("src");

    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    const dialog = await screen.findByRole("dialog", { name: "书签" });
    expect(within(dialog).getByText("当前书籍还没有书签。")).toBeInTheDocument();
    expect(iframe.getAttribute("src")).toBe(src);
    expect(harness.bridges).toHaveLength(1);
    expect(client.closeCalls).toHaveLength(0);
  });

  it("captures the current locator when adding a bookmark", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    const dialog = await screen.findByRole("dialog", { name: "书签" });
    const freshLocator = {
      version: 1 as const,
      spineIndex: 0,
      spineHref: "OEBPS/chapter1.xhtml",
      textOffset: 40,
      intraSpineProgression: 0.55,
      quote: { exact: "最新位置", prefix: "", suffix: "" },
    };
    const bridge = harness.bridges[0];
    if (!bridge) {
      throw new Error("bridge missing");
    }
    bridge.captureResult = freshLocator;
    fireEvent.click(within(dialog).getByRole("button", { name: "添加书签" }));
    await waitFor(() => {
      expect(client.createBookmarkCalls).toHaveLength(1);
    });
    expect(harness.bridges[0]?.captureCalls).toBe(1);
    expect(client.createBookmarkCalls[0]?.sessionId).toBe(SESSION_ID);
    expect(client.createBookmarkCalls[0]?.locator).toEqual(freshLocator);
    expect(within(dialog).getByText("书签已添加")).toBeInTheDocument();
    expect(within(dialog).getByText("第一章")).toBeInTheDocument();
    expect(within(dialog).getByText("无摘录")).toBeInTheDocument();
  });

  it("does not create when capture times out and ignores a late capture", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    const dialog = await screen.findByRole("dialog", { name: "书签" });
    const bridge = harness.bridges[0];
    if (!bridge) {
      throw new Error("bridge missing");
    }
    vi.useFakeTimers();
    fireEvent.click(within(dialog).getByRole("button", { name: "添加书签" }));
    await act(async () => {
      vi.advanceTimersByTime(500);
    });
    await act(async () => {});
    expect(client.createBookmarkCalls).toHaveLength(0);
    expect(within(dialog).getByText("书签未能添加，请重试。")).toBeInTheDocument();

    // 迟到的旧 capture 不得消费新的添加请求。
    await act(async () => {
      bridge.options.onLocationChanged?.({
        reason: "capture",
        captureRequestId: bridge.captureRequestIds[0],
        locator: {
          version: 1,
          spineIndex: 0,
          spineHref: "OEBPS/chapter1.xhtml",
          textOffset: 99,
          intraSpineProgression: 0.9,
          quote: { exact: "迟到", prefix: "", suffix: "" },
        },
        mode: "paged",
        pageIndex: 0,
        pageCount: 1,
        atStart: true,
        atEnd: true,
        sessionId: "unused",
      } as never);
    });
    expect(client.createBookmarkCalls).toHaveLength(0);

    const fresh = {
      version: 1 as const,
      spineIndex: 0,
      spineHref: "OEBPS/chapter1.xhtml",
      textOffset: 5,
      intraSpineProgression: 0.2,
      quote: { exact: "新位置", prefix: "", suffix: "" },
    };
    bridge.captureResult = fresh;
    fireEvent.click(within(dialog).getByRole("button", { name: "添加书签" }));
    await act(async () => {
      vi.advanceTimersByTime(500);
    });
    await act(async () => {});
    expect(client.createBookmarkCalls).toHaveLength(1);
    expect(client.createBookmarkCalls[0]?.locator).toEqual(fresh);
  });

  it("drops a late capture response that arrives while a new request waits", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    const dialog = await screen.findByRole("dialog", { name: "书签" });
    const bridge = harness.bridges[0];
    if (!bridge) {
      throw new Error("bridge missing");
    }
    vi.useFakeTimers();
    // 请求 A 超时，不创建。
    fireEvent.click(within(dialog).getByRole("button", { name: "添加书签" }));
    await act(async () => {
      vi.advanceTimersByTime(500);
    });
    await act(async () => {});
    expect(client.createBookmarkCalls).toHaveLength(0);
    const requestA = bridge.captureRequestIds[0];

    // 请求 B 等待中，A 的迟到响应到达：ID 不匹配，直接丢弃。
    fireEvent.click(within(dialog).getByRole("button", { name: "添加书签" }));
    await act(async () => {});
    expect(bridge.captureRequestIds).toHaveLength(2);
    const requestB = bridge.captureRequestIds[1];
    await act(async () => {
      bridge.options.onLocationChanged?.({
        reason: "capture",
        captureRequestId: requestA,
        locator: {
          version: 1,
          spineIndex: 0,
          spineHref: "OEBPS/chapter1.xhtml",
          textOffset: 99,
          intraSpineProgression: 0.9,
          quote: { exact: "旧", prefix: "", suffix: "" },
        },
        mode: "paged",
        pageIndex: 0,
        pageCount: 1,
        atStart: true,
        atEnd: true,
        sessionId: "unused",
      } as never);
    });
    await act(async () => {});
    expect(client.createBookmarkCalls).toHaveLength(0);

    // B 的匹配响应完成创建。
    const freshB = {
      version: 1 as const,
      spineIndex: 0,
      spineHref: "OEBPS/chapter1.xhtml",
      textOffset: 7,
      intraSpineProgression: 0.3,
      quote: { exact: "新", prefix: "", suffix: "" },
    };
    await act(async () => {
      bridge.options.onLocationChanged?.({
        reason: "capture",
        captureRequestId: requestB,
        locator: freshB,
        mode: "paged",
        pageIndex: 0,
        pageCount: 1,
        atStart: true,
        atEnd: true,
        sessionId: "unused",
      } as never);
    });
    await act(async () => {});
    expect(client.createBookmarkCalls).toHaveLength(1);
    expect(client.createBookmarkCalls[0]?.locator).toEqual(freshB);
  });

  it("lists multiple bookmarks, deletes one and keeps failures visible", async () => {
    const client = new FakeClient();
    client.results.push(session());
    client.bookmarks = [
      {
        id: "223e4567-e89b-42d3-a456-426614174000",
        bookId: "book-1",
        locator: session().initialLocator as never,
        excerpt: "摘录一",
        createdAt: "2026-11-18T00:00:00Z",
      },
      {
        id: "323e4567-e89b-42d3-a456-426614174000",
        bookId: "book-1",
        locator: {
          version: 1 as const,
          spineIndex: 1,
          spineHref: "OEBPS/chapter2.xhtml",
          textOffset: 0,
          intraSpineProgression: 0,
          quote: { exact: "", prefix: "", suffix: "" },
        },
        excerpt: null,
        createdAt: "2026-11-18T00:00:01Z",
      },
    ];
    const harness = renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    const dialog = await screen.findByRole("dialog", { name: "书签" });
    expect(within(dialog).getByText("摘录一")).toBeInTheDocument();
    expect(within(dialog).getByText("无摘录")).toBeInTheDocument();
    expect(within(dialog).getAllByRole("listitem")).toHaveLength(2);

    fireEvent.click(within(dialog).getByRole("button", { name: "删除书签 1" }));
    await act(async () => {});
    expect(client.deleteBookmarkCalls).toEqual([
      {
        bookId: "book-1",
        bookmarkId: "223e4567-e89b-42d3-a456-426614174000",
      },
    ]);
    expect(within(dialog).getAllByRole("listitem")).toHaveLength(1);

    client.failBookmarkDelete = true;
    fireEvent.click(within(dialog).getByRole("button", { name: "删除书签 2" }));
    await act(async () => {});
    expect(within(dialog).getByText("书签未能删除，请重试。")).toBeInTheDocument();
    expect(within(dialog).getAllByRole("listitem")).toHaveLength(1);
    expect(harness.onBack).not.toHaveBeenCalled();
  });

  it("shows fixed load and create failures", async () => {
    const client = new FakeClient();
    client.results.push(session());
    client.failBookmarkLoad = true;
    renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    const dialog = await screen.findByRole("dialog", { name: "书签" });
    await act(async () => {});
    expect(within(dialog).getByText("书签未能加载，请重试。")).toBeInTheDocument();

    client.failBookmarkLoad = false;
    client.failBookmarkCreate = true;
    fireEvent.click(within(dialog).getByRole("button", { name: "添加书签" }));
    await waitFor(
      () => {
        expect(within(dialog).getByText("书签未能添加，请重试。")).toBeInTheDocument();
      },
      { timeout: 2_000 },
    );
  });

  it("jumps to a same-chapter bookmark with go-to", async () => {
    const client = new FakeClient();
    client.results.push(session());
    client.bookmarks = [
      {
        id: "223e4567-e89b-42d3-a456-426614174000",
        bookId: "book-1",
        locator: session().initialLocator as never,
        excerpt: "同章",
        createdAt: "2026-11-18T00:00:00Z",
      },
    ];
    const harness = renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    const dialog = await screen.findByRole("dialog", { name: "书签" });
    fireEvent.click(within(dialog).getByText("同章"));
    await act(async () => {});
    expect(harness.bridges[0]?.goToCalls).toHaveLength(1);
    expect(screen.queryByRole("dialog", { name: "书签" })).not.toBeInTheDocument();
  });

  it("jumps to a cross-chapter bookmark by rebuilding the target iframe", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const targetLocator = {
      version: 1 as const,
      spineIndex: 1,
      spineHref: "OEBPS/chapter2.xhtml",
      textOffset: 5,
      intraSpineProgression: 0.4,
      quote: { exact: "第二章", prefix: "", suffix: "" },
    };
    client.bookmarks = [
      {
        id: "223e4567-e89b-42d3-a456-426614174000",
        bookId: "book-1",
        locator: targetLocator,
        excerpt: "跨章",
        createdAt: "2026-11-18T00:00:00Z",
      },
    ];
    const harness = renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    const dialog = await screen.findByRole("dialog", { name: "书签" });
    fireEvent.click(within(dialog).getByText("跨章"));
    await act(async () => {});
    const iframe = (await screen.findByTitle("《示例书》阅读内容")) as HTMLIFrameElement;
    expect(iframe.getAttribute("src")).toContain("chapter2.xhtml");
    fireEvent.load(iframe);
    expect(harness.bridges[1]?.options.initialLocator).toEqual(targetLocator);
    expect(harness.bridges[1]?.options.initialNavigation).toBeUndefined();
    expect(client.closeCalls).toHaveLength(0);
    expect(client.openCalls).toHaveLength(1);
  });

  it("keeps bookmarks mutually exclusive with toc and settings and orders Esc", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    fireEvent.load(await waitForIframe());

    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    await screen.findByRole("dialog", { name: "书签" });
    fireEvent.click(screen.getByRole("button", { name: "阅读设置" }));
    await screen.findByRole("dialog", { name: "阅读设置" });
    expect(screen.queryByRole("dialog", { name: "书签" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    await screen.findByRole("dialog", { name: "书签" });
    expect(screen.queryByRole("dialog", { name: "阅读设置" })).not.toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "书签" })).not.toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    await screen.findByRole("dialog", { name: "书签" });
    fireEvent.click(screen.getByRole("button", { name: "打开目录" }));
    await screen.findByRole("dialog", { name: "目录" });
    expect(screen.queryByRole("dialog", { name: "书签" })).not.toBeInTheDocument();
    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() => {
      expect(screen.queryByRole("dialog", { name: "目录" })).not.toBeInTheDocument();
    });
  });

  it("opens the bookmark drawer from immersive controls", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    fireEvent.load(await waitForIframe());
    fireEvent.click(screen.getByRole("button", { name: "沉浸式" }));
    const hotZone = screen.getByRole("button", { name: "显示阅读控件" });
    await act(async () => {
      hotZone.focus();
    });
    fireEvent.click(screen.getByRole("button", { name: "打开书签" }));
    expect(await screen.findByRole("dialog", { name: "书签" })).toBeInTheDocument();
  });

  it("moves focus into the bookmark drawer and restores it on close", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    fireEvent.load(await waitForIframe());
    const button = screen.getByRole("button", { name: "打开书签" });
    fireEvent.click(button);
    const dialog = await screen.findByRole("dialog", { name: "书签" });
    expect(dialog.contains(document.activeElement)).toBe(true);
    fireEvent.click(within(dialog).getByRole("button", { name: "关闭书签" }));
    expect(document.activeElement).toBe(button);
  });

  it("auto-hides immersive controls after the pointer leaves the header", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    fireEvent.load(await waitForIframe());
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "沉浸式" }));
    expect(screen.getByRole("button", { name: "显示阅读控件" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "返回书库" })).not.toBeInTheDocument();

    fireEvent.mouseEnter(screen.getByRole("button", { name: "显示阅读控件" }));
    const header = screen.getByRole("banner");
    expect(screen.getByRole("button", { name: "返回书库" })).toBeInTheDocument();

    fireEvent.mouseLeave(header);
    await act(async () => {
      vi.advanceTimersByTime(899);
    });
    expect(screen.getByRole("button", { name: "返回书库" })).toBeInTheDocument();
    await act(async () => {
      vi.advanceTimersByTime(1);
    });
    expect(screen.queryByRole("button", { name: "返回书库" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "显示阅读控件" })).toBeInTheDocument();
  });

  it("keeps immersive controls while a drawer is open or header focus remains", async () => {
    const client = new FakeClient();
    client.results.push(session());
    renderShell(client);
    fireEvent.load(await waitForIframe());
    vi.useFakeTimers();
    fireEvent.click(screen.getByRole("button", { name: "沉浸式" }));
    fireEvent.mouseEnter(screen.getByRole("button", { name: "显示阅读控件" }));
    const header = screen.getByRole("banner");

    // header 焦点存在：鼠标离开 + 延迟后仍显示。
    const tocButton = screen.getByRole("button", { name: "打开目录" });
    tocButton.focus();
    fireEvent.mouseLeave(header);
    await act(async () => {
      vi.advanceTimersByTime(1_000);
    });
    expect(screen.getByRole("button", { name: "返回书库" })).toBeInTheDocument();

    // 抽屉打开：不自动隐藏。
    fireEvent.click(tocButton);
    expect(screen.getByRole("dialog", { name: "目录" })).toBeInTheDocument();
    fireEvent.mouseLeave(header);
    await act(async () => {
      vi.advanceTimersByTime(1_000);
    });
    expect(screen.getByRole("button", { name: "返回书库" })).toBeInTheDocument();
  });

  it("closes the session exactly once on back, even when close fails", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    await waitForIframe();
    client.closeError = { code: "E_SESSION_INVALID", message: "raw" };
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "返回书库" }));
    });
    await waitFor(() => {
      expect(harness.onBack).toHaveBeenCalledTimes(1);
    });
    expect(client.closeCalls).toHaveLength(1);
    harness.unmount();
  });

  it("back without a session does not call closeBook", async () => {
    const client = new FakeClient();
    client.results.push({ error: { code: "E_BOOK_NOT_FOUND", message: "raw" } });
    const harness = renderShell(client);
    await screen.findByText("该书已不在书库。");
    await act(async () => {
      fireEvent.click(
        within(screen.getByRole("alert")).getByRole("button", {
          name: "返回书库",
        }),
      );
    });
    expect(harness.onBack).toHaveBeenCalledTimes(1);
    expect(client.closeCalls).toHaveLength(0);
  });

  it("ignores iframe load after the session has been closed", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "返回书库" }));
    });
    expect(client.closeCalls).toHaveLength(1);
    fireEvent.load(iframe);
    expect(harness.bridges).toHaveLength(0);
    expect(client.closeCalls).toHaveLength(1);
  });

  it("closes session and bridge exactly once on unmount", async () => {
    const client = new FakeClient();
    client.results.push(session());
    const harness = renderShell(client);
    const iframe = await waitForIframe();
    fireEvent.load(iframe);
    harness.unmount();
    await waitFor(() => {
      expect(client.closeCalls).toHaveLength(1);
    });
    expect(harness.bridges[0]?.closeCount).toBe(1);
    expect(client.saveCalls.length).toBeGreaterThanOrEqual(1);
  });

  it("closes a session that resolves after unmount without rendering", async () => {
    const client = new FakeClient();
    client.defer = true;
    const harness = renderShell(client);
    await waitFor(() => {
      expect(client.openCalls).toHaveLength(1);
    });
    harness.unmount();
    client.pending[0]?.resolve(session());
    await waitFor(() => {
      expect(client.closeCalls).toEqual([SESSION_ID]);
    });
  });

  it("closes the old session after flushing when switching books", async () => {
    const client = new FakeClient();
    client.results.push(session(), {
      ...session({
        bookId: "book-2",
        sessionId: "323e4567-e89b-42d3-a456-426614174000",
        frameUrl: `epub://${"b".repeat(32)}/OEBPS/chapter2.xhtml`,
        spine: [{ index: 0, href: "OEBPS/chapter2.xhtml" }],
        chapterWeights: [1],
        toc: [],
        initialLocator: null,
        initialProgression: 0,
      }),
    });
    const first = renderShell(client);
    fireEvent.load(await waitForIframe());
    first.unmount();
    await waitFor(() => {
      expect(client.closeCalls).toEqual([SESSION_ID]);
    });

    const second = renderShell(client, "book-2");
    const iframe = await waitForIframe();
    expect(iframe.getAttribute("src")).toContain("chapter2.xhtml");
    await waitFor(() => {
      expect(client.openCalls).toEqual(["book-1", "book-2"]);
    });
    second.unmount();
    await waitFor(() => {
      expect(client.closeCalls).toHaveLength(2);
    });
  });

  it("does not expose frame url or token in DOM text, logs or storage", async () => {
    const localMap = new Map<string, string>();
    const sessionMap = new Map<string, string>();
    const makeStorage = (map: Map<string, string>): Storage => ({
      get length() {
        return map.size;
      },
      clear: () => map.clear(),
      getItem: (key: string) => map.get(key) ?? null,
      key: (index: number) => Array.from(map.keys())[index] ?? null,
      removeItem: (key: string) => {
        map.delete(key);
      },
      setItem: (key: string, value: string) => {
        map.set(key, value);
      },
    });
    vi.stubGlobal("localStorage", makeStorage(localMap));
    vi.stubGlobal("sessionStorage", makeStorage(sessionMap));
    const consoleLog = vi.spyOn(console, "log");
    const consoleError = vi.spyOn(console, "error");
    const consoleWarn = vi.spyOn(console, "warn");
    try {
      const client = new FakeClient();
      client.results.push(session());
      const harness = renderShell(client);
      const iframe = await waitForIframe();
      fireEvent.load(iframe);
      await act(async () => {
        harness.bridges[0]?.options.onReady?.("0.1.0");
      });
      await waitFor(() => {
        expect(iframe.style.visibility).toBe("visible");
      });

      const text = document.body.textContent ?? "";
      expect(text).not.toContain(TOKEN);
      expect(text).not.toContain(FRAME_URL);
      expect(localMap.size).toBe(0);
      expect(sessionMap.size).toBe(0);
      for (const calls of [
        consoleLog.mock.calls,
        consoleError.mock.calls,
        consoleWarn.mock.calls,
      ]) {
        expect(JSON.stringify(calls)).not.toContain(TOKEN);
      }
      harness.unmount();
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
