import { expect, test } from "@playwright/test";

test("空书库页面显示 Qurner 标题与中文提示", async ({ page }) => {
  await page.goto("/");

  await expect(page).toHaveTitle("Qurner");
  await expect(page.getByRole("heading", { level: 1, name: "Qurner" })).toBeVisible();
  await expect(page.getByRole("heading", { level: 2, name: "书库还是空的" })).toBeVisible();
  await expect(page.getByText("稍后可以通过导入添加书籍。")).toBeVisible();
  await expect(page.getByRole("button")).toHaveCount(0);
});
