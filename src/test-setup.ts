import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// React 19 的 act 需要显式启用测试环境标记；RTL 的 render/act 会在该标记下运行。
const reactTestEnvironment = globalThis as unknown as {
  IS_REACT_ACT_ENVIRONMENT?: boolean;
};
reactTestEnvironment.IS_REACT_ACT_ENVIRONMENT = true;

afterEach(() => {
  cleanup();
});
