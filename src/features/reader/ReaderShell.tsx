import { useCallback, useEffect, useRef, useState } from "react";

import { commands, toAppError } from "../../bridge/commands";
import {
  type Bookmark,
  type BookSettingsPatch,
  type ReaderLocator,
  type ReaderSession,
  type ReaderSettings,
  type ReaderTocNode,
  readerSessionSchema,
} from "../../bridge/ipc-schemas";
import type { ReaderLocationChangedPayload, ReaderNavigation } from "../../bridge/messages";
import {
  ParentBridge,
  type ParentBridgeOptions,
  type ParentBridgeState,
} from "../../bridge/parent";
import {
  extractSessionToken,
  frameErrorMessage,
  frameErrorRecoverable,
  openErrorMessage,
  openErrorRecoverable,
} from "./reader-errors";
import { chapterLabel, computeProgression, formatPercent } from "./reader-progress";
import { registerTauriWindowClose, type WindowCloseRegistrar } from "./reader-window";
import { buildSpineFrameUrl } from "./session-navigation";

export interface ReaderCommandClient {
  openBook(bookId: string): Promise<ReaderSession>;
  closeBook(sessionId: string): Promise<void>;
  getEffectiveSettings(bookId: string): Promise<ReaderSettings>;
  updateAppSettings(patch: Partial<ReaderSettings>): Promise<ReaderSettings>;
  updateBookSettings(bookId: string, patch: BookSettingsPatch): Promise<ReaderSettings>;
  resetBookSettings(bookId: string): Promise<ReaderSettings>;
  saveReadingState(
    sessionId: string,
    locator: ReaderLocator,
  ): Promise<{ globalProgression: number; savedAt: string }>;
  listBookmarks(bookId: string): Promise<import("../../bridge/ipc-schemas").Bookmark[]>;
  createBookmark(
    sessionId: string,
    locator: ReaderLocator,
  ): Promise<import("../../bridge/ipc-schemas").Bookmark>;
  deleteBookmark(bookId: string, bookmarkId: string): Promise<void>;
}

export interface ReaderBridge {
  start(): void;
  close(): void;
  updateSettings(settings: ReaderSettings): void;
  turnPage(direction: "prev" | "next"): void;
  goTo(locator: ReaderLocator): boolean;
  captureLocator(captureRequestId: number): boolean;
  getState(): ParentBridgeState;
}

export type ParentBridgeFactory = (
  iframe: HTMLIFrameElement,
  options: ParentBridgeOptions,
) => ReaderBridge;

type ReaderPhase = "opening" | "connecting" | "ready" | "open-error" | "frame-error";

interface ReaderShellProps {
  bookId: string;
  commandClient?: ReaderCommandClient;
  bridgeFactory?: ParentBridgeFactory;
  onBack: () => void;
  registerWindowClose?: WindowCloseRegistrar;
}

const defaultClient: ReaderCommandClient = {
  openBook: (bookId) => commands.openBook(bookId),
  closeBook: (sessionId) => commands.closeBook(sessionId),
  getEffectiveSettings: (bookId) => commands.getEffectiveSettings(bookId),
  updateAppSettings: (patch) => commands.updateAppSettings(patch),
  updateBookSettings: (bookId, patch) => commands.updateBookSettings(bookId, patch),
  resetBookSettings: (bookId) => commands.resetBookSettings(bookId),
  saveReadingState: (sessionId, locator) => commands.saveReadingState(sessionId, locator),
  listBookmarks: (bookId) => commands.listBookmarks(bookId),
  createBookmark: (sessionId, locator) => commands.createBookmark(sessionId, locator),
  deleteBookmark: (bookId, bookmarkId) => commands.deleteBookmark(bookId, bookmarkId),
};

const defaultBridgeFactory: ParentBridgeFactory = (iframe, options) =>
  new ParentBridge(iframe, options);

const INTERACTIVE_SELECTOR = "input, textarea, select, button, a[href], [contenteditable='true']";

function isInteractiveTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) {
    return false;
  }
  return (
    (target as HTMLElement).isContentEditable === true ||
    target.closest(INTERACTIVE_SELECTOR) !== null
  );
}

interface TocListProps {
  nodes: ReaderTocNode[];
  onSelect: (node: ReaderTocNode) => void;
  registerFirst?: (element: HTMLButtonElement | null) => void;
}

function TocList({ nodes, onSelect, registerFirst }: TocListProps) {
  return (
    <ul className="reader-toc-list">
      {nodes.map((node, index) => (
        <li key={node.id}>
          <button
            ref={index === 0 ? registerFirst : undefined}
            type="button"
            className="reader-toc-item"
            onClick={() => onSelect(node)}
          >
            {node.label}
          </button>
          {node.children.length > 0 ? <TocList nodes={node.children} onSelect={onSelect} /> : null}
        </li>
      ))}
    </ul>
  );
}

export function ReaderShell({
  bookId,
  commandClient = defaultClient,
  bridgeFactory = defaultBridgeFactory,
  onBack,
  registerWindowClose = registerTauriWindowClose,
}: ReaderShellProps) {
  const [phase, setPhase] = useState<ReaderPhase>("opening");
  const [session, setSession] = useState<ReaderSession | null>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [closing, setClosing] = useState(false);
  const [settings, setSettings] = useState<ReaderSettings>({
    mode: "paged",
    fontScale: 1,
    theme: "day",
  });
  const [spineIndex, setSpineIndex] = useState(0);
  const [navigation, setNavigation] = useState<ReaderNavigation | null>(null);
  const [frameGeneration, setFrameGeneration] = useState(0);
  const [tocOpen, setTocOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [immersive, setImmersive] = useState(false);
  const [controlsVisible, setControlsVisible] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [settingsStatus, setSettingsStatus] = useState<string | null>(null);
  const [chapter, setChapter] = useState("");
  const [readingProgress, setReadingProgress] = useState(0);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [bookmarks, setBookmarks] = useState<Bookmark[]>([]);
  const [bookmarksOpen, setBookmarksOpen] = useState(false);
  const [bookmarkError, setBookmarkError] = useState<string | null>(null);
  const [bookmarkStatus, setBookmarkStatus] = useState<string | null>(null);
  const [hasLocator, setHasLocator] = useState(false);
  const [addingBookmark, setAddingBookmark] = useState(false);
  const pendingCaptureRef = useRef<{
    requestId: number;
    resolve: (locator: ReaderLocator) => void;
  } | null>(null);
  const captureRequestIdRef = useRef(0);
  const settingsGenerationRef = useRef(0);
  const pendingSettingsRef = useRef<ReaderSettings | null>(null);
  const settingsRef = useRef<ReaderSettings>({
    mode: "paged",
    fontScale: 1,
    theme: "day",
  });
  const settingsButtonRef = useRef<HTMLButtonElement | null>(null);
  const bookmarksButtonRef = useRef<HTMLButtonElement | null>(null);
  const bookmarksCloseRef = useRef<HTMLButtonElement | null>(null);
  const bookmarkGenerationRef = useRef(0);
  const [jumpLocator, setJumpLocator] = useState<ReaderLocator | null>(null);
  const settingsCloseRef = useRef<HTMLButtonElement | null>(null);
  const generationRef = useRef(0);
  const latestLocatorRef = useRef<ReaderLocator | null>(null);
  const saveGenerationRef = useRef(0);
  const scheduleSaveRef = useRef<() => void>(() => undefined);
  const saveInFlightPromiseRef = useRef<Promise<void> | null>(null);
  const saveInFlightLocatorRef = useRef<ReaderLocator | null>(null);
  const drainTaskRef = useRef<Promise<void> | null>(null);
  const flushProgressRef = useRef<(timeoutMs?: number) => Promise<void>>(async () => undefined);
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingLocatorRef = useRef<ReaderLocator | null>(null);
  const cancelledRef = useRef(false);
  const mountedRef = useRef(false);
  const sessionRef = useRef<ReaderSession | null>(null);
  const closedRef = useRef(false);
  const bridgeRef = useRef<ReaderBridge | null>(null);
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const tocButtonRef = useRef<HTMLButtonElement | null>(null);
  const tocFirstItemRef = useRef<HTMLButtonElement | null>(null);
  const focusContentRef = useRef(false);
  const headerRef = useRef<HTMLElement | null>(null);
  const hideTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const closeSessionOnce = useCallback(() => {
    if (closedRef.current) {
      return;
    }
    const current = sessionRef.current;
    if (!current) {
      return;
    }
    closedRef.current = true;
    bridgeRef.current?.close();
    bridgeRef.current = null;
    void commandClient.closeBook(current.sessionId).catch(() => undefined);
  }, [commandClient]);

  const drawerOpen = tocOpen || settingsOpen || bookmarksOpen;
  const immersiveRef = useRef(immersive);
  const drawerOpenRef = useRef(drawerOpen);
  immersiveRef.current = immersive;
  drawerOpenRef.current = drawerOpen;

  const clearHideTimer = useCallback(() => {
    if (hideTimerRef.current !== null) {
      clearTimeout(hideTimerRef.current);
      hideTimerRef.current = null;
    }
  }, []);

  const showControls = useCallback(() => {
    clearHideTimer();
    if (immersiveRef.current) {
      setControlsVisible(true);
    }
  }, [clearHideTimer]);

  const scheduleHide = useCallback(() => {
    if (!immersiveRef.current || drawerOpenRef.current) {
      clearHideTimer();
      return;
    }
    clearHideTimer();
    hideTimerRef.current = setTimeout(() => {
      hideTimerRef.current = null;
      if (!mountedRef.current || !immersiveRef.current || drawerOpenRef.current) {
        return;
      }
      const active = document.activeElement;
      const header = headerRef.current;
      if (header && active instanceof Node && header.contains(active)) {
        return;
      }
      setControlsVisible(false);
    }, 900);
  }, [clearHideTimer]);

  const open = useCallback(() => {
    const generation = ++generationRef.current;
    settingsGenerationRef.current += 1;
    cancelledRef.current = false;
    pendingSettingsRef.current = null;
    closeSessionOnce();
    sessionRef.current = null;
    closedRef.current = false;
    setSession(null);
    setErrorCode(null);
    setTocOpen(false);
    setSettingsOpen(false);
    setImmersive(false);
    setControlsVisible(false);
    clearHideTimer();
    setSettingsError(null);
    setSettingsStatus(null);
    setSaveError(null);
    setBookmarks([]);
    setBookmarksOpen(false);
    setBookmarkError(null);
    setBookmarkStatus(null);
    setHasLocator(false);
    setJumpLocator(null);
    setChapter("");
    setReadingProgress(0);
    latestLocatorRef.current = null;
    pendingLocatorRef.current = null;
    saveGenerationRef.current += 1;
    setPhase("opening");
    void commandClient
      .openBook(bookId)
      .then((next) => {
        if (cancelledRef.current || generation !== generationRef.current || !mountedRef.current) {
          closedRef.current = true;
          void commandClient.closeBook(next.sessionId).catch(() => undefined);
          return;
        }
        // 防御性完整校验 openBook 返回值：不依赖单一 token 正则，避免
        // encoded 穿越/query/fragment/反斜杠/NUL 等路径进入 iframe。
        let parsed: ReaderSession;
        try {
          parsed = readerSessionSchema.parse(next);
        } catch {
          closedRef.current = true;
          void commandClient.closeBook(next.sessionId).catch(() => undefined);
          setErrorCode("E_INVALID_FRAME_URL");
          setPhase("frame-error");
          return;
        }
        if (!extractSessionToken(parsed.frameUrl)) {
          closedRef.current = true;
          void commandClient.closeBook(parsed.sessionId).catch(() => undefined);
          setErrorCode("E_INVALID_FRAME_URL");
          setPhase("frame-error");
          return;
        }
        sessionRef.current = parsed;
        setSession(parsed);
        setSettings(parsed.settings);
        settingsRef.current = parsed.settings;
        latestLocatorRef.current = parsed.initialLocator;
        setHasLocator(parsed.initialLocator !== null);
        setChapter(chapterLabel(parsed.toc, parsed.initialLocator?.spineIndex ?? 0));
        setReadingProgress(parsed.initialProgression);
        setSpineIndex(parsed.initialLocator?.spineIndex ?? 0);
        setNavigation(null);
        setFrameGeneration(0);
        setPhase("connecting");
      })
      .catch((error: unknown) => {
        if (cancelledRef.current || generation !== generationRef.current || !mountedRef.current) {
          return;
        }
        setErrorCode(toAppError(error).code);
        setPhase("open-error");
      });
  }, [bookId, clearHideTimer, closeSessionOnce, commandClient]);

  useEffect(() => {
    mountedRef.current = true;
    open();
    return () => {
      mountedRef.current = false;
      cancelledRef.current = true;
      if (saveTimerRef.current !== null) {
        clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
      }
      clearHideTimer();
      bridgeRef.current?.close();
      bridgeRef.current = null;
      const closingSession = sessionRef.current;
      // 先等待进行中的保存与 pending-latest，再关闭旧 session；最多 1 秒。
      void flushProgressRef.current(1_000).finally(() => {
        if (closingSession === null) {
          return;
        }
        if (sessionRef.current === closingSession) {
          closeSessionOnce();
        } else {
          void commandClient.closeBook(closingSession.sessionId).catch(() => undefined);
        }
      });
    };
  }, [clearHideTimer, closeSessionOnce, commandClient, open]);

  const navigateTo = useCallback((index: number, nextNavigation: ReaderNavigation | null) => {
    const current = sessionRef.current;
    if (!current || index < 0 || index >= current.spine.length) {
      return;
    }
    void flushProgressRef.current();
    bridgeRef.current?.close();
    bridgeRef.current = null;
    setSpineIndex(index);
    setNavigation(nextNavigation);
    setFrameGeneration((value) => value + 1);
    setErrorCode(null);
    setPhase("connecting");
  }, []);

  const handleSpineRequest = useCallback(
    (direction: "prev" | "next") => {
      const current = sessionRef.current;
      if (!current) {
        return;
      }
      const target = direction === "next" ? spineIndex + 1 : spineIndex - 1;
      if (target < 0 || target >= current.spine.length) {
        return;
      }
      navigateTo(target, direction === "next" ? { kind: "start" } : { kind: "end" });
    },
    [navigateTo, spineIndex],
  );

  const handleIframeLoad = useCallback(
    (event: React.SyntheticEvent<HTMLIFrameElement>) => {
      if (!mountedRef.current || closedRef.current || bridgeRef.current) {
        return;
      }
      const frame = event.currentTarget;
      if (frame !== iframeRef.current) {
        return;
      }
      const current = sessionRef.current;
      if (!current) {
        return;
      }
      const token = extractSessionToken(current.frameUrl);
      const href = current.spine[spineIndex]?.href;
      const src = token && href ? buildSpineFrameUrl(token, href) : null;
      if (!token || !src) {
        setErrorCode("E_INVALID_FRAME_URL");
        setPhase("frame-error");
        return;
      }
      const scopedJump = jumpLocator;
      const scopedNavigation = scopedJump === null ? navigation : null;
      const scopedSpineIndex = spineIndex;
      try {
        const bridge = bridgeFactory(frame, {
          sessionId: current.sessionId,
          sessionToken: token,
          settings,
          spineIndex: scopedSpineIndex,
          initialLocator: scopedJump ?? (scopedNavigation === null ? current.initialLocator : null),
          initialNavigation: scopedJump === null ? (scopedNavigation ?? undefined) : undefined,
          onReady: () => {
            if (mountedRef.current && !closedRef.current && iframeRef.current === frame) {
              setPhase("ready");
              // ready 后回放当前/待应用设置，避免 waiting 期间修改被丢弃。
              bridge.updateSettings(settingsRef.current);
              pendingSettingsRef.current = null;
              if (focusContentRef.current) {
                focusContentRef.current = false;
                frame.focus();
              }
            }
          },
          onError: (code) => {
            if (mountedRef.current && !closedRef.current && iframeRef.current === frame) {
              setErrorCode(code || "E_READER_ERROR");
              setPhase("frame-error");
            }
          },
          onSpineRequest: (payload) => {
            if (mountedRef.current && !closedRef.current && iframeRef.current === frame) {
              handleSpineRequest(payload.direction);
            }
          },
          onLocationChanged: (payload: ReaderLocationChangedPayload) => {
            if (!mountedRef.current || closedRef.current || iframeRef.current !== frame) {
              return;
            }
            if (payload.reason === "capture") {
              const pending = pendingCaptureRef.current;
              if (pending && pending.requestId === payload.captureRequestId) {
                pendingCaptureRef.current = null;
                pending.resolve(payload.locator);
              }
            }
            latestLocatorRef.current = payload.locator;
            setHasLocator(true);
            const current = sessionRef.current;
            if (current) {
              setChapter(chapterLabel(current.toc, payload.locator.spineIndex));
              setReadingProgress(
                computeProgression(
                  current.chapterWeights,
                  payload.locator.spineIndex,
                  payload.locator.intraSpineProgression,
                ),
              );
            }
            if (payload.reason !== "init") {
              scheduleSaveRef.current();
            }
          },
        });
        bridgeRef.current = bridge;
        bridge.start();
        if (scopedJump !== null) {
          setJumpLocator(null);
        }
      } catch {
        setErrorCode("E_INVALID_STATE");
        setPhase("frame-error");
      }
    },
    [bridgeFactory, handleSpineRequest, jumpLocator, navigation, settings, spineIndex],
  );

  const handleIframeError = useCallback(() => {
    if (mountedRef.current && !closedRef.current) {
      setErrorCode("E_IFRAME_ERROR");
      setPhase("frame-error");
    }
  }, []);

  useEffect(() => {
    if (!session) {
      return;
    }
    const iframe = iframeRef.current;
    if (!iframe) {
      return;
    }
    const onError = () => handleIframeError();
    iframe.addEventListener("error", onError);
    return () => iframe.removeEventListener("error", onError);
  }, [handleIframeError, session]);

  const applyToBridge = useCallback((value: ReaderSettings) => {
    const bridge = bridgeRef.current;
    if (bridge && bridge.getState() === "ready") {
      bridge.updateSettings(value);
      pendingSettingsRef.current = null;
    } else {
      pendingSettingsRef.current = value;
    }
  }, []);

  const saveOne = useCallback(
    async (sessionId: string, locator: ReaderLocator, generation: number): Promise<void> => {
      try {
        const result = await commandClient.saveReadingState(sessionId, locator);
        if (
          generation === saveGenerationRef.current &&
          mountedRef.current &&
          latestLocatorRef.current === locator
        ) {
          setReadingProgress(result.globalProgression);
          setSaveError(null);
        }
      } catch (error) {
        const code = toAppError(error).code;
        if (
          code !== "E_SESSION_INVALID" &&
          mountedRef.current &&
          generation === saveGenerationRef.current
        ) {
          setSaveError("阅读进度未能保存，请重试。");
        }
      }
    },
    [commandClient],
  );

  const drainLatest = useCallback((): Promise<void> => {
    const existing = drainTaskRef.current;
    if (existing) {
      return existing;
    }
    const task = (async () => {
      while (latestLocatorRef.current !== null && sessionRef.current !== null) {
        const inFlight = saveInFlightPromiseRef.current;
        if (inFlight) {
          if (latestLocatorRef.current !== saveInFlightLocatorRef.current) {
            pendingLocatorRef.current = latestLocatorRef.current;
          }
          await inFlight;
          continue;
        }
        const current = sessionRef.current;
        const locator = latestLocatorRef.current;
        const generation = saveGenerationRef.current;
        const task = saveOne(current.sessionId, locator, generation);
        saveInFlightPromiseRef.current = task;
        saveInFlightLocatorRef.current = locator;
        try {
          await task;
        } finally {
          saveInFlightPromiseRef.current = null;
          saveInFlightLocatorRef.current = null;
        }
        const pending = pendingLocatorRef.current;
        pendingLocatorRef.current = null;
        if (pending !== null) {
          latestLocatorRef.current = pending;
        } else if (latestLocatorRef.current === locator) {
          return;
        }
      }
    })();
    drainTaskRef.current = task;
    void task.finally(() => {
      if (drainTaskRef.current === task) {
        drainTaskRef.current = null;
      }
    });
    return task;
  }, [saveOne]);

  const scheduleSave = useCallback(() => {
    if (saveTimerRef.current !== null) {
      clearTimeout(saveTimerRef.current);
    }
    saveTimerRef.current = setTimeout(() => {
      saveTimerRef.current = null;
      void drainLatest();
    }, 1_000);
  }, [drainLatest]);

  const flushProgress = useCallback(
    async (timeoutMs = 1_000): Promise<void> => {
      if (saveTimerRef.current !== null) {
        clearTimeout(saveTimerRef.current);
        saveTimerRef.current = null;
      }
      await Promise.race([
        drainLatest(),
        new Promise<void>((resolve) => {
          setTimeout(resolve, timeoutMs);
        }),
      ]);
    },
    [drainLatest],
  );

  useEffect(() => {
    const handler = () => {
      void drainLatest();
    };
    window.addEventListener("beforeunload", handler);
    return () => {
      window.removeEventListener("beforeunload", handler);
    };
  }, [drainLatest]);

  useEffect(() => {
    const unregister = registerWindowClose(flushProgress);
    return () => {
      unregister();
    };
  }, [flushProgress, registerWindowClose]);

  useEffect(() => {
    scheduleSaveRef.current = scheduleSave;
    flushProgressRef.current = flushProgress;
  }, [flushProgress, scheduleSave]);

  const persistSettings = useCallback(
    (next: ReaderSettings, patch: BookSettingsPatch) => {
      void flushProgress();
      setSettings(next);
      settingsRef.current = next;
      applyToBridge(next);
      setSettingsStatus(null);
      const generation = ++settingsGenerationRef.current;
      void commandClient
        .updateBookSettings(bookId, patch)
        .then((effective) => {
          if (
            generation !== settingsGenerationRef.current ||
            !mountedRef.current ||
            closedRef.current
          ) {
            return;
          }
          setSettings(effective);
          settingsRef.current = effective;
          applyToBridge(effective);
          setSettingsError(null);
        })
        .catch(() => {
          if (generation !== settingsGenerationRef.current || !mountedRef.current) {
            return;
          }
          setSettingsError("设置未能保存，请重试。");
        });
    },
    [applyToBridge, bookId, commandClient, flushProgress],
  );

  const switchMode = useCallback(
    (next: ReaderSettings["mode"]) => {
      if (next === settings.mode) {
        return;
      }
      persistSettings({ ...settings, mode: next }, { mode: next });
    },
    [persistSettings, settings],
  );

  const closeSettings = useCallback(() => {
    setSettingsOpen(false);
    settingsButtonRef.current?.focus();
  }, []);

  useEffect(() => {
    if (settingsOpen) {
      settingsCloseRef.current?.focus();
    }
  }, [settingsOpen]);

  const openSettings = useCallback(() => {
    setTocOpen(false);
    setBookmarksOpen(false);
    setSettingsOpen(true);
    setSettingsError(null);
    setSettingsStatus(null);
    const generation = ++settingsGenerationRef.current;
    void commandClient
      .getEffectiveSettings(bookId)
      .then((effective) => {
        if (
          generation !== settingsGenerationRef.current ||
          !mountedRef.current ||
          closedRef.current
        ) {
          return;
        }
        setSettings(effective);
        settingsRef.current = effective;
        applyToBridge(effective);
      })
      .catch(() => {
        if (generation === settingsGenerationRef.current && mountedRef.current) {
          setSettingsError("本地阅读设置无法读取。");
        }
      });
  }, [applyToBridge, bookId, commandClient]);

  const changeFontScale = useCallback(
    (delta: number) => {
      const next = Math.round((settings.fontScale + delta) * 10) / 10;
      const clamped = Math.min(3, Math.max(0.5, next));
      if (clamped === settings.fontScale) {
        return;
      }
      persistSettings({ ...settings, fontScale: clamped }, { fontScale: clamped });
    },
    [persistSettings, settings],
  );

  const setTheme = useCallback(
    (theme: ReaderSettings["theme"]) => {
      if (theme === settings.theme) {
        return;
      }
      persistSettings({ ...settings, theme }, { theme });
    },
    [persistSettings, settings],
  );

  const saveGlobalDefaults = useCallback(() => {
    setSettingsStatus(null);
    setSettingsError(null);
    void commandClient
      .updateAppSettings({
        mode: settings.mode,
        fontScale: settings.fontScale,
        theme: settings.theme,
      })
      .then(() => {
        if (mountedRef.current) {
          setSettingsStatus("已保存为全局默认");
        }
      })
      .catch(() => {
        if (mountedRef.current) {
          setSettingsError("设置未能保存，请重试。");
        }
      });
  }, [commandClient, settings]);

  const resetBookSettings = useCallback(() => {
    setSettingsStatus(null);
    setSettingsError(null);
    const generation = ++settingsGenerationRef.current;
    void commandClient
      .resetBookSettings(bookId)
      .then((effective) => {
        if (
          generation !== settingsGenerationRef.current ||
          !mountedRef.current ||
          closedRef.current
        ) {
          return;
        }
        setSettings(effective);
        settingsRef.current = effective;
        applyToBridge(effective);
        setSettingsStatus("已恢复为跟随全局");
      })
      .catch(() => {
        if (generation === settingsGenerationRef.current && mountedRef.current) {
          setSettingsError("设置未能保存，请重试。");
        }
      });
  }, [applyToBridge, bookId, commandClient]);

  const closeToc = useCallback((restoreFocus: boolean) => {
    setTocOpen(false);
    if (restoreFocus) {
      tocButtonRef.current?.focus();
    }
  }, []);

  const closeBookmarks = useCallback((restoreFocus = true) => {
    setBookmarksOpen(false);
    if (restoreFocus) {
      bookmarksButtonRef.current?.focus();
    }
  }, []);

  const refreshBookmarks = useCallback(() => {
    const generation = ++bookmarkGenerationRef.current;
    void commandClient
      .listBookmarks(bookId)
      .then((items) => {
        if (generation === bookmarkGenerationRef.current && mountedRef.current) {
          setBookmarks(items);
        }
      })
      .catch(() => {
        if (generation === bookmarkGenerationRef.current && mountedRef.current) {
          setBookmarkError("书签未能加载，请重试。");
        }
      });
  }, [bookId, commandClient]);

  const openBookmarks = useCallback(() => {
    setTocOpen(false);
    setSettingsOpen(false);
    setBookmarksOpen(true);
    setBookmarkError(null);
    setBookmarkStatus(null);
    refreshBookmarks();
  }, [refreshBookmarks]);

  const captureLocator = useCallback((): Promise<ReaderLocator | null> => {
    const bridge = bridgeRef.current;
    if (bridge?.getState() !== "ready") {
      return Promise.resolve(null);
    }
    return new Promise((resolve) => {
      const requestId = ++captureRequestIdRef.current;
      const timer = setTimeout(() => {
        if (pendingCaptureRef.current?.requestId === requestId) {
          pendingCaptureRef.current = null;
        }
        resolve(null);
      }, 500);
      pendingCaptureRef.current = {
        requestId,
        resolve: (locator) => {
          clearTimeout(timer);
          resolve(locator);
        },
      };
      if (!bridge.captureLocator(requestId)) {
        clearTimeout(timer);
        if (pendingCaptureRef.current?.requestId === requestId) {
          pendingCaptureRef.current = null;
        }
        resolve(null);
      }
    });
  }, []);

  const addBookmark = useCallback(async () => {
    if (addingBookmark) {
      return;
    }
    const current = sessionRef.current;
    if (!current) {
      return;
    }
    setAddingBookmark(true);
    setBookmarkError(null);
    setBookmarkStatus(null);
    try {
      const fresh = await captureLocator();
      if (fresh === null) {
        if (mountedRef.current) {
          setBookmarkError("书签未能添加，请重试。");
        }
        return;
      }
      await commandClient.createBookmark(current.sessionId, fresh);
      if (!mountedRef.current) {
        return;
      }
      setBookmarkStatus("书签已添加");
      refreshBookmarks();
    } catch {
      if (mountedRef.current) {
        setBookmarkError("书签未能添加，请重试。");
      }
    } finally {
      if (mountedRef.current) {
        setAddingBookmark(false);
      }
    }
  }, [addingBookmark, captureLocator, commandClient, refreshBookmarks]);

  const removeBookmark = useCallback(
    (bookmarkId: string) => {
      setBookmarkError(null);
      void commandClient
        .deleteBookmark(bookId, bookmarkId)
        .then(() => {
          if (mountedRef.current) {
            setBookmarks((items) => items.filter((item) => item.id !== bookmarkId));
          }
        })
        .catch(() => {
          if (mountedRef.current) {
            setBookmarkError("书签未能删除，请重试。");
          }
        });
    },
    [bookId, commandClient],
  );

  const jumpToBookmark = useCallback(
    (bookmark: Bookmark) => {
      if (!sessionRef.current) {
        return;
      }
      setBookmarksOpen(false);
      const bridge = bridgeRef.current;
      if (bookmark.locator.spineIndex === spineIndex && bridge && bridge.getState() === "ready") {
        if (bridge.goTo(bookmark.locator)) {
          return;
        }
      }
      setJumpLocator(bookmark.locator);
      navigateTo(bookmark.locator.spineIndex, null);
    },
    [navigateTo, spineIndex],
  );

  useEffect(() => {
    if (bookmarksOpen) {
      bookmarksCloseRef.current?.focus();
    }
  }, [bookmarksOpen]);

  const handleTocSelect = useCallback(
    (node: ReaderTocNode) => {
      const current = sessionRef.current;
      if (!current || node.spineIndex >= current.spine.length) {
        closeToc(true);
        return;
      }
      focusContentRef.current = true;
      setTocOpen(false);
      navigateTo(
        node.spineIndex,
        node.fragment ? { kind: "fragment", fragment: node.fragment } : { kind: "start" },
      );
    },
    [closeToc, navigateTo],
  );

  useEffect(() => {
    if (tocOpen) {
      tocFirstItemRef.current?.focus();
    }
  }, [tocOpen]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (settingsOpen) {
          event.preventDefault();
          closeSettings();
          return;
        }
        if (bookmarksOpen) {
          event.preventDefault();
          closeBookmarks();
          return;
        }
        if (tocOpen) {
          event.preventDefault();
          closeToc(true);
          return;
        }
        if (immersive) {
          event.preventDefault();
          setImmersive(false);
          setControlsVisible(false);
          return;
        }
      }
      if (settingsOpen || tocOpen || event.defaultPrevented) {
        return;
      }
      if (event.ctrlKey || event.metaKey || event.altKey) {
        return;
      }
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
        return;
      }
      if (isInteractiveTarget(event.target)) {
        return;
      }
      event.preventDefault();
      bridgeRef.current?.turnPage(event.key === "ArrowLeft" ? "prev" : "next");
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [bookmarksOpen, closeBookmarks, closeSettings, closeToc, immersive, settingsOpen, tocOpen]);

  useEffect(() => {
    if (!immersive) {
      clearHideTimer();
      return;
    }
    if (drawerOpen) {
      // 抽屉打开时保持 header 可见，不自动隐藏。
      clearHideTimer();
      setControlsVisible(true);
      return;
    }
    scheduleHide();
    return clearHideTimer;
  }, [clearHideTimer, drawerOpen, immersive, scheduleHide]);

  const handleBack = useCallback(() => {
    if (closing) {
      return;
    }
    setClosing(true);
    setTocOpen(false);
    void (async () => {
      await flushProgress(1_000);
      latestLocatorRef.current = null;
      closeSessionOnce();
      onBack();
    })();
  }, [closing, closeSessionOnce, flushProgress, onBack]);

  const handleRetry = useCallback(() => {
    if (!closing) {
      open();
    }
  }, [closing, open]);

  const token = session ? extractSessionToken(session.frameUrl) : null;
  const sectionHref = session?.spine[spineIndex]?.href ?? null;
  const frameSrc = token && sectionHref ? buildSpineFrameUrl(token, sectionHref) : null;
  const showFrame = session !== null && phase !== "open-error";
  const errorMessage =
    phase === "open-error" ? openErrorMessage(errorCode) : frameErrorMessage(errorCode);
  const errorRecoverable =
    phase === "open-error" ? openErrorRecoverable(errorCode) : frameErrorRecoverable(errorCode);

  const controlsHidden = immersive && !controlsVisible;
  return (
    <div className={`reader-shell${immersive ? " reader-immersive" : ""}`}>
      {controlsHidden ? (
        <button
          type="button"
          className="reader-hot-zone"
          onMouseEnter={showControls}
          onFocus={showControls}
          onClick={showControls}
        >
          显示阅读控件
        </button>
      ) : null}

      {!controlsHidden ? (
        // biome-ignore lint/a11y/noStaticElementInteractions: header 仅用于指针/焦点可见性，不提供交互语义
        <header
          ref={headerRef}
          className="reader-header"
          onMouseEnter={showControls}
          onMouseLeave={scheduleHide}
          onFocus={showControls}
          onBlur={(event) => {
            const next = event.relatedTarget;
            if (
              !(next instanceof Node) ||
              (headerRef.current && !headerRef.current.contains(next))
            ) {
              scheduleHide();
            }
          }}
        >
          <button type="button" onClick={handleBack} disabled={closing}>
            返回书库
          </button>
          <h2>{session?.title ?? "阅读器"}</h2>
          <span className="reader-position" role="status" aria-label="阅读位置">
            {chapter ? `${chapter} · ${formatPercent(readingProgress)}` : ""}
          </span>
          <div className="reader-controls">
            <button
              ref={tocButtonRef}
              type="button"
              aria-expanded={tocOpen}
              aria-controls="reader-toc"
              aria-label={tocOpen ? "关闭目录" : "打开目录"}
              onClick={() => {
                if (tocOpen) {
                  closeToc(true);
                } else {
                  setSettingsOpen(false);
                  setBookmarksOpen(false);
                  setTocOpen(true);
                }
              }}
            >
              目录
            </button>
            <button
              ref={bookmarksButtonRef}
              type="button"
              aria-label={bookmarksOpen ? "关闭书签" : "打开书签"}
              onClick={() => {
                if (bookmarksOpen) {
                  closeBookmarks();
                } else {
                  openBookmarks();
                }
              }}
            >
              书签
            </button>
            <fieldset className="reader-mode-group" aria-label="阅读模式">
              <button
                type="button"
                aria-pressed={settings.mode === "paged"}
                onClick={() => switchMode("paged")}
              >
                分页
              </button>
              <button
                type="button"
                aria-pressed={settings.mode === "scrolling"}
                onClick={() => switchMode("scrolling")}
              >
                连续滚动
              </button>
            </fieldset>
            <button
              ref={settingsButtonRef}
              type="button"
              aria-label="阅读设置"
              onClick={openSettings}
            >
              设置
            </button>
            <button
              type="button"
              onClick={() => {
                if (immersive) {
                  setImmersive(false);
                  setControlsVisible(false);
                } else {
                  setImmersive(true);
                  setControlsVisible(false);
                }
              }}
            >
              {immersive ? "退出沉浸式" : "沉浸式"}
            </button>
          </div>
        </header>
      ) : null}

      {phase === "opening" ? (
        <p className="reader-status" role="status">
          正在打开书籍…
        </p>
      ) : null}
      {phase === "connecting" ? (
        <p className="reader-status" role="status">
          正在准备阅读内容…
        </p>
      ) : null}
      {closing ? (
        <p className="reader-status" role="status">
          正在返回书库…
        </p>
      ) : null}
      {saveError ? (
        <p className="reader-status" role="status">
          {saveError}
        </p>
      ) : null}
      {settingsError && !settingsOpen ? (
        <p className="reader-status" role="status">
          {settingsError}
        </p>
      ) : null}

      {showFrame && frameSrc ? (
        <div className="reader-stage">
          <iframe
            key={`${spineIndex}-${frameGeneration}`}
            ref={iframeRef}
            src={frameSrc}
            sandbox="allow-scripts"
            referrerPolicy="no-referrer"
            title={`《${session.title}》阅读内容`}
            onLoad={handleIframeLoad}
            onMouseEnter={scheduleHide}
            style={{ visibility: phase === "ready" ? "visible" : "hidden" }}
          />
        </div>
      ) : null}

      {tocOpen && session ? (
        <div className="reader-toc-overlay">
          <button
            type="button"
            className="reader-toc-backdrop"
            aria-label="关闭目录遮罩"
            onClick={() => closeToc(true)}
          />
          <aside
            id="reader-toc"
            className="reader-toc"
            role="dialog"
            aria-modal="true"
            aria-label="目录"
          >
            <div className="reader-toc-header">
              <h3>目录</h3>
              <button type="button" onClick={() => closeToc(true)}>
                关闭目录
              </button>
            </div>
            {session.toc.length === 0 ? (
              <p>本书没有可用目录。</p>
            ) : (
              <nav aria-label="目录">
                <TocList
                  nodes={session.toc}
                  onSelect={handleTocSelect}
                  registerFirst={(element) => {
                    tocFirstItemRef.current = element;
                  }}
                />
              </nav>
            )}
          </aside>
        </div>
      ) : null}

      {bookmarksOpen && session ? (
        <div className="reader-bookmarks-overlay">
          <button
            type="button"
            className="reader-bookmarks-backdrop"
            aria-label="关闭书签遮罩"
            onClick={() => closeBookmarks()}
          />
          <aside
            id="reader-bookmarks"
            className="reader-bookmarks"
            role="dialog"
            aria-modal="true"
            aria-label="书签"
          >
            <div className="reader-bookmarks-header">
              <h3>书签</h3>
              <button ref={bookmarksCloseRef} type="button" onClick={() => closeBookmarks()}>
                关闭书签
              </button>
            </div>
            <button
              type="button"
              onClick={() => void addBookmark()}
              disabled={!hasLocator || addingBookmark}
            >
              添加书签
            </button>
            {bookmarkStatus ? (
              <p className="reader-settings-message" role="status">
                {bookmarkStatus}
              </p>
            ) : null}
            {bookmarkError ? (
              <p className="reader-settings-message" role="status">
                {bookmarkError}
              </p>
            ) : null}
            {bookmarks.length === 0 ? (
              <p>当前书籍还没有书签。</p>
            ) : (
              <ul className="reader-bookmarks-list">
                {bookmarks.map((bookmark) => (
                  <li key={bookmark.id}>
                    <button
                      type="button"
                      className="reader-bookmark-jump"
                      onClick={() => jumpToBookmark(bookmark)}
                    >
                      <span className="reader-bookmark-chapter">
                        {chapterLabel(session.toc, bookmark.locator.spineIndex)}
                      </span>
                      <span className="reader-bookmark-excerpt">
                        {bookmark.excerpt ?? "无摘录"}
                      </span>
                      <time dateTime={bookmark.createdAt}>
                        {new Date(bookmark.createdAt).toLocaleString("zh-CN")}
                      </time>
                    </button>
                    <button
                      type="button"
                      aria-label={`删除书签 ${bookmark.locator.spineIndex + 1}`}
                      onClick={() => removeBookmark(bookmark.id)}
                    >
                      删除
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </aside>
        </div>
      ) : null}

      {settingsOpen && session ? (
        <div className="reader-settings-overlay">
          <button
            type="button"
            className="reader-settings-backdrop"
            aria-label="关闭阅读设置"
            onClick={closeSettings}
          />
          <aside
            id="reader-settings"
            className="reader-settings"
            role="dialog"
            aria-modal="true"
            aria-label="阅读设置"
          >
            <div className="reader-settings-header">
              <h3>阅读设置</h3>
              <button ref={settingsCloseRef} type="button" onClick={closeSettings}>
                关闭设置
              </button>
            </div>
            <p>字号：{Math.round(settings.fontScale * 100)}%</p>
            <div className="reader-settings-row">
              <button
                type="button"
                aria-label="减小字号"
                disabled={settings.fontScale <= 0.5}
                onClick={() => changeFontScale(-0.1)}
              >
                A-
              </button>
              <button
                type="button"
                aria-label="增大字号"
                disabled={settings.fontScale >= 3}
                onClick={() => changeFontScale(0.1)}
              >
                A+
              </button>
            </div>
            <fieldset className="reader-theme-group" aria-label="阅读主题">
              <button
                type="button"
                aria-pressed={settings.theme === "day"}
                onClick={() => setTheme("day")}
              >
                白天
              </button>
              <button
                type="button"
                aria-pressed={settings.theme === "night"}
                onClick={() => setTheme("night")}
              >
                黑夜
              </button>
            </fieldset>
            <button type="button" onClick={saveGlobalDefaults}>
              保存为全局默认
            </button>
            <button type="button" onClick={resetBookSettings}>
              恢复本书跟随全局
            </button>
            {settingsError ? (
              <p className="reader-settings-message" role="status">
                {settingsError}
              </p>
            ) : null}
            {settingsStatus ? (
              <p className="reader-settings-message" role="status">
                {settingsStatus}
              </p>
            ) : null}
          </aside>
        </div>
      ) : null}

      {phase === "open-error" || phase === "frame-error" ? (
        <div className="reader-error" role="alert">
          <p>{errorMessage}</p>
          <div className="reader-error-actions">
            {errorRecoverable ? (
              <button type="button" onClick={handleRetry}>
                重新加载阅读页
              </button>
            ) : null}
            <button type="button" onClick={handleBack}>
              返回书库
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
