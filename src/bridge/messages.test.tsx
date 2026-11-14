import { describe, expect, it } from "vitest";

import {
  createReaderMessage,
  MAX_TEXT_OFFSET,
  PROTOCOL_VERSION,
  parseReaderMessage,
} from "./messages";

const SESSION_ID = "123e4567-e89b-42d3-a456-426614174000";
const TOKEN = "a".repeat(32);

function locator(overrides: Record<string, unknown> = {}) {
  return {
    version: 1 as const,
    spineIndex: 0,
    spineHref: "OEBPS/chapter1.xhtml",
    textOffset: 0,
    intraSpineProgression: 0,
    quote: { exact: "", prefix: "", suffix: "" },
    ...overrides,
  };
}

function validInit(overrides: Record<string, unknown> = {}) {
  return {
    protocolVersion: PROTOCOL_VERSION,
    sessionId: SESSION_ID,
    sessionToken: TOKEN,
    type: "reader:init",
    payload: { mode: "paged", initialLocator: null },
    ...overrides,
  };
}

function pageState(overrides: Record<string, unknown> = {}) {
  return {
    pageIndex: 0,
    pageCount: 1,
    atStart: true,
    atEnd: false,
    ...overrides,
  };
}

describe("reader message schema", () => {
  it("accepts init, ready, turn-page and error", () => {
    const init = createReaderMessage("reader:init", SESSION_ID, TOKEN, {
      mode: "scrolling",
      initialLocator: locator({ textOffset: 5 }),
    });
    expect(parseReaderMessage(init)).toEqual(init);

    const ready = createReaderMessage("reader:ready", SESSION_ID, TOKEN, {
      engineVersion: "0.2.0",
    });
    expect(parseReaderMessage(ready)).toEqual(ready);

    const turnPage = createReaderMessage("reader:turn-page", SESSION_ID, TOKEN, {
      direction: "next",
    });
    expect(parseReaderMessage(turnPage)).toEqual(turnPage);

    const error = createReaderMessage("reader:error", SESSION_ID, TOKEN, {
      code: "E_LOCATOR_NOT_FOUND",
      message: "无法定位到原文本位置。",
    });
    expect(parseReaderMessage(error)).toEqual(error);
  });

  it("accepts go-to, update-settings and capture-locator", () => {
    for (const message of [
      {
        ...validInit(),
        type: "reader:go-to",
        payload: { locator: locator({ textOffset: 12 }) },
      },
      {
        ...validInit(),
        type: "reader:update-settings",
        payload: { mode: "scrolling" },
      },
      {
        ...validInit(),
        type: "reader:capture-locator",
        payload: {},
      },
    ]) {
      expect(parseReaderMessage(message)).toEqual(message);
    }
  });

  it("accepts location-changed, page-changed and spine-request", () => {
    const location = {
      ...validInit(),
      type: "reader:location-changed",
      payload: {
        reason: "turn",
        locator: locator({ textOffset: 12 }),
        mode: "paged",
        ...pageState({ pageIndex: 1, pageCount: 3, atStart: false }),
      },
    };
    expect(parseReaderMessage(location)).toEqual(location);

    const page = {
      ...validInit(),
      type: "reader:page-changed",
      payload: pageState({ pageIndex: 2, pageCount: 3, atEnd: true }),
    };
    expect(parseReaderMessage(page)).toEqual(page);

    const spine = {
      ...validInit(),
      type: "reader:spine-request",
      payload: { direction: "next", locator: locator({ textOffset: 99 }) },
    };
    expect(parseReaderMessage(spine)).toEqual(spine);
  });

  it("rejects unknown fields and wrong versions", () => {
    expect(parseReaderMessage({ ...validInit(), extra: true })).toBeNull();
    expect(
      parseReaderMessage({
        ...validInit(),
        payload: { mode: "paged", initialLocator: null, extra: true },
      }),
    ).toBeNull();
    expect(parseReaderMessage({ ...validInit(), protocolVersion: 2 })).toBeNull();
    expect(parseReaderMessage({ ...validInit(), type: "reader:unknown" })).toBeNull();
  });

  it("rejects malformed session identifiers", () => {
    expect(parseReaderMessage({ ...validInit(), sessionId: "" })).toBeNull();
    expect(parseReaderMessage({ ...validInit(), sessionId: "session-1" })).toBeNull();
    expect(parseReaderMessage({ ...validInit(), sessionToken: "XYZ" })).toBeNull();
    expect(parseReaderMessage({ ...validInit(), sessionToken: "A".repeat(32) })).toBeNull();
  });

  it("rejects invalid locators", () => {
    for (const invalid of [
      locator({ version: 2 }),
      locator({ spineHref: "../chapter.xhtml" }),
      locator({ spineHref: "/abs.xhtml" }),
      locator({ spineHref: "" }),
      locator({ textOffset: MAX_TEXT_OFFSET + 1 }),
      locator({ textOffset: -1 }),
      locator({ intraSpineProgression: 1.5 }),
      locator({ quote: { exact: "a".repeat(129), prefix: "", suffix: "" } }),
      locator({ quote: { exact: "😀".repeat(65), prefix: "", suffix: "" } }),
    ]) {
      expect(
        parseReaderMessage({
          ...validInit(),
          type: "reader:go-to",
          payload: { locator: invalid },
        }),
      ).toBeNull();
    }
  });

  it("rejects invalid page state and directions", () => {
    expect(
      parseReaderMessage({
        ...validInit(),
        type: "reader:page-changed",
        payload: pageState({ pageIndex: 3, pageCount: 2 }),
      }),
    ).toBeNull();
    expect(
      parseReaderMessage({
        ...validInit(),
        type: "reader:page-changed",
        payload: pageState({ pageIndex: 2, pageCount: 2 }),
      }),
    ).toBeNull();
    expect(
      parseReaderMessage({
        ...validInit(),
        type: "reader:page-changed",
        payload: pageState({ pageIndex: -1 }),
      }),
    ).toBeNull();
    // 滚动模式的 0/0 状态仍合法。
    expect(
      parseReaderMessage({
        ...validInit(),
        type: "reader:page-changed",
        payload: pageState({ pageIndex: 0, pageCount: 0, atStart: true, atEnd: false }),
      }),
    ).not.toBeNull();
    expect(
      parseReaderMessage({
        ...validInit(),
        type: "reader:turn-page",
        payload: { direction: "sideways" },
      }),
    ).toBeNull();
    expect(
      parseReaderMessage({
        ...validInit(),
        type: "reader:location-changed",
        payload: {
          reason: "unknown",
          locator: locator(),
          mode: "paged",
          ...pageState(),
        },
      }),
    ).toBeNull();
  });
});
