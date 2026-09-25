import { createHash, randomBytes } from "node:crypto";
import { createServer } from "node:http";

import { expect, test } from "@playwright/test";

const baseUrl = process.env.MCP_E2E_BASE_URL ?? "http://127.0.0.1:3000";
const username = process.env.MCP_E2E_USERNAME;
const password = process.env.MCP_E2E_PASSWORD;

test("OAuth login, consent, callback, and token exchange", async ({
  page,
  request,
}) => {
  test.skip(
    !username || !password,
    "Set MCP_E2E_USERNAME and MCP_E2E_PASSWORD to run the OAuth flow",
  );

  let resolveCallback!: (url: URL) => void;
  const callbackReceived = new Promise<URL>((resolve) => {
    resolveCallback = resolve;
  });
  let redirectUri = "";
  const callbackServer = createServer((incoming, response) => {
    response.writeHead(200, { "content-type": "text/plain; charset=utf-8" });
    response.end("Authorization complete. You can close this window.");
    resolveCallback(new URL(incoming.url ?? "/", redirectUri));
  });

  await new Promise<void>((resolve, reject) => {
    callbackServer.once("error", reject);
    callbackServer.listen(0, "127.0.0.1", resolve);
  });

  try {
    const address = callbackServer.address();
    if (!address || typeof address === "string") {
      throw new Error("Could not determine the OAuth callback port");
    }
    redirectUri = `http://127.0.0.1:${address.port}/callback`;

    const registration = await request.post(`${baseUrl}/oauth/register`, {
      data: {
        client_name: "Flashy Playwright test",
        redirect_uris: [redirectUri],
        token_endpoint_auth_method: "none",
        grant_types: ["authorization_code", "refresh_token"],
        response_types: ["code"],
      },
    });
    expect(registration.status()).toBe(201);
    const { client_id: clientId } = await registration.json();

    const verifier = randomBytes(32).toString("base64url");
    const challenge = createHash("sha256")
      .update(verifier)
      .digest("base64url");
    const state = randomBytes(16).toString("base64url");
    const authorizeUrl = new URL("/oauth/authorize", baseUrl);
    authorizeUrl.search = new URLSearchParams({
      response_type: "code",
      client_id: clientId,
      redirect_uri: redirectUri,
      resource: `${baseUrl}/mcp`,
      scope: "flashy:read",
      state,
      code_challenge: challenge,
      code_challenge_method: "S256",
    }).toString();

    await page.goto(authorizeUrl.toString());
    await page.getByLabel("Username").fill(username!);
    await page.getByLabel("Password").fill(password!);

    const consentResponsePromise = page.waitForResponse((response) =>
      response.url().includes("/oauth/consent/"),
    );
    await page.getByRole("button", { name: "Login" }).click();
    const consentResponse = await consentResponsePromise;

    await expect(
      page.getByRole("heading", { name: "Allow read access?" }),
    ).toBeVisible();
    expect(consentResponse.headers()["content-security-policy"]).toContain(
      new URL(redirectUri).origin,
    );

    await page.getByRole("button", { name: "Allow" }).click();
    const callbackUrl = await callbackReceived;
    expect(callbackUrl.searchParams.get("state")).toBe(state);
    const code = callbackUrl.searchParams.get("code");
    expect(code).toBeTruthy();

    const tokenResponse = await request.post(`${baseUrl}/oauth/token`, {
      form: {
        grant_type: "authorization_code",
        client_id: clientId,
        code: code!,
        redirect_uri: redirectUri,
        code_verifier: verifier,
        resource: `${baseUrl}/mcp`,
      },
    });
    expect(tokenResponse.status()).toBe(200);
    await expect(tokenResponse.json()).resolves.toMatchObject({
      token_type: "Bearer",
      scope: "flashy:read",
    });
  } finally {
    await new Promise<void>((resolve, reject) => {
      callbackServer.close((error) => (error ? reject(error) : resolve()));
    });
  }
});
