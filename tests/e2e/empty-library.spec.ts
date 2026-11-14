import { expect, test } from "@playwright/test";

test("空书库页面显示 Qurner 标题、书库区域与导入入口", async ({ page }) => {
  await page.addInitScript(() => {
    (
      window as unknown as {
        __TAURI_INTERNALS__?: {
          invoke: (command: string) => Promise<unknown>;
        };
      }
    ).__TAURI_INTERNALS__ = {
      invoke: async (command: string) => (command === "listBooks" ? [] : null),
    };
  });
  await page.goto("/");

  await expect(page).toHaveTitle("Qurner");
  await expect(page.getByRole("heading", { level: 1, name: "Qurner" })).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "书库" })).toBeVisible();
  await expect(page.getByText("书库还是空的")).toBeVisible();
  await expect(page.getByRole("button", { name: "导入 EPUB" })).toHaveCount(2);
});
