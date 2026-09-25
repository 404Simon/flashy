import { expect, test } from "@playwright/test";

test("login page renders the real Flashy authentication flow", async ({ page }) => {
  await page.goto("http://localhost:3000/login");

  await expect(page).toHaveTitle("Flashy - AI Flashcards from Slides");
  await expect(page.getByRole("heading", { name: "Login" })).toBeVisible();
  await expect(page.getByLabel("Username")).toBeVisible();
  await expect(page.getByLabel("Password")).toBeVisible();
  await expect(page.getByRole("button", { name: "Login" })).toBeVisible();
});

test("OAuth login continuation is retained without rendering it as a URL", async ({
  page,
}) => {
  const requestId = "opaque_request-123";
  await page.goto(
    `http://localhost:3000/login?oauth_request=${requestId}`,
  );

  await expect(page).toHaveURL(new RegExp(`oauth_request=${requestId}$`));
  await expect(page.locator("body")).not.toContainText(requestId);
});
