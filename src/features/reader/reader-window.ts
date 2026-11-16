/** 窗口关闭注册器；测试可注入 fake。 */
export type WindowCloseRegistrar = (flush: () => Promise<void>) => () => void;

/** 正常窗口关闭时阻止一次默认关闭，最多等待 1 秒保存后销毁窗口。 */
export function registerTauriWindowClose(flush: () => Promise<void>): () => void {
  let unlisten: (() => void) | undefined;
  let disposed = false;
  let closing = false;
  void (async () => {
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      const window = getCurrentWindow();
      const handler = await window.onCloseRequested(async (event) => {
        if (closing) {
          return;
        }
        closing = true;
        event.preventDefault();
        try {
          await Promise.race([
            flush(),
            new Promise<void>((resolve) => {
              setTimeout(resolve, 1_000);
            }),
          ]);
        } finally {
          try {
            await window.destroy();
          } catch {
            // 窗口已关闭。
          }
        }
      });
      if (disposed) {
        handler();
      } else {
        unlisten = handler;
      }
    } catch {
      // 非 Tauri 环境（例如 Vitest）不注册。
    }
  })();
  return () => {
    disposed = true;
    unlisten?.();
  };
}
