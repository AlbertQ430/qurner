import { IframeBridge } from "../bridge/iframe";
import { createDomReaderEngine, ENGINE_VERSION } from "./engine";

const script = document.currentScript instanceof HTMLScriptElement ? document.currentScript : null;

function bootstrap(): void {
  if (!script) {
    return;
  }
  const sessionId = script.dataset.sessionId ?? "";
  if (sessionId.length === 0) {
    return;
  }
  const spineIndex = Number(script.dataset.spineIndex);
  const spineHref = script.dataset.spineHref ?? "";
  if (!Number.isInteger(spineIndex) || spineIndex < 0 || spineHref.length === 0) {
    // 非 spine 文档不启用阅读引擎。
    return;
  }

  let token = "";
  try {
    // 生产 URL 的 host 即协议令牌；测试以 <token>.test 形式模拟。
    token = new URL(script.src).hostname.split(".")[0] ?? "";
  } catch {
    return;
  }
  if (token.length === 0) {
    return;
  }

  let parentOrigin = "";
  if (document.referrer.length > 0) {
    try {
      parentOrigin = new URL(document.referrer).origin;
    } catch {
      parentOrigin = "";
    }
  }
  // referrer 为空（例如 packaged Qurner.app 的 iframe referrerPolicy=no-referrer）
  // 时不退出：由 IframeBridge 从父窗口的合法 init 学习父 origin 并校验后续消息。

  try {
    const engine = createDomReaderEngine(document, spineIndex, spineHref);
    new IframeBridge(
      {
        sessionId,
        sessionToken: token,
        parentOrigin,
        engineVersion: ENGINE_VERSION,
      },
      { engine },
    );
  } catch {
    // 配置非法时不发送任何消息。
  }
}

bootstrap();
