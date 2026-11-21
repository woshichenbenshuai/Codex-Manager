import { expect, test } from "@playwright/test";

test("administrator can bind and disable an optional authenticator without changing the login mode", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 1000 });
  const settings = {
    locale: "zh-CN", theme: "tech", appearancePreset: "classic", codexCliGuideDismissed: true,
    webAuthMode: "password", webAccessPasswordConfigured: true, distributionEnabled: false,
    serviceAddr: "localhost:48760", updateAutoCheck: false,
  };
  let enabled = false;
  const writes: Array<{ method: string; params: Record<string, unknown> }> = [];
  const secret = "JBSWY3DPEHPK3PXPJBSWY3DP";
  await page.route("**/api/runtime**", (route) => route.fulfill({ json: { mode: "web-gateway", rpcBaseUrl: "/api/rpc", canManageService: false, canSelfUpdate: false, canCloseToTray: false, canOpenLocalDir: false } }));
  await page.route("**/api/rpc**", async (route) => {
    const { id, method, params = {} } = route.request().postDataJSON();
    const ok = (result: unknown) => route.fulfill({ json: { jsonrpc: "2.0", id, result } });
    if (method.startsWith("accountManager/twoFactor/") && !method.endsWith("/status")) writes.push({ method, params });
    if (method === "appSettings/get") return ok(settings);
    if (method === "initialize") return ok({ userAgent: "codex_cli_rs/test", codexHome: "C:/Test", platformFamily: "windows", platformOs: "windows" });
    if (method === "accountManager/session/current") return ok({ mode: "password", currentUser: null, role: "system_admin", permissions: ["system:admin"], distributionEnabled: false });
    if (method === "accountManager/twoFactor/status") return ok({ enabled, recoveryCodesRemaining: enabled ? 8 : 0 });
    if (method === "accountManager/twoFactor/setup") return ok({ secret, setupToken: "pending-setup", otpauthUri: `otpauth://totp/CodexManager:Admin?secret=${secret}&issuer=CodexManager` });
    if (method === "accountManager/twoFactor/confirm") {
      if (params.code !== "123456") return route.fulfill({ json: { jsonrpc: "2.0", id, error: { code: -32000, message: "动态验证码错误或已使用" } } });
      enabled = true;
      return ok({ recoveryCodes: ["recovery-one", "recovery-two"] });
    }
    if (method === "accountManager/twoFactor/disable") { enabled = false; return ok({ ok: true }); }
    return ok({});
  });
  await page.goto("/settings/");
  await page.getByRole("button", { name: "访问控制", exact: true }).click();
  const panel = page.getByTestId("two-factor-panel");
  await expect(panel.getByRole("button", { name: "绑定验证器", exact: true })).toBeDisabled();
  await panel.getByLabel("当前登录密码", { exact: true }).fill("existing-password");
  await panel.getByRole("button", { name: "绑定验证器", exact: true }).click();
  await expect(panel.getByRole("img", { name: "验证器绑定二维码" })).toBeVisible();
  await expect(panel.getByLabel("验证器密钥", { exact: true })).toHaveValue(secret);
  expect(enabled).toBe(false);
  await panel.getByLabel("6 位动态验证码", { exact: true }).fill("111111");
  await panel.getByRole("button", { name: "验证并启用 2FA" }).click();
  await expect(panel.getByRole("alert")).toContainText("动态验证码错误");
  expect(enabled).toBe(false);
  await panel.getByLabel("6 位动态验证码", { exact: true }).fill("123456");
  await panel.getByRole("button", { name: "验证并启用 2FA" }).click();
  await expect(panel.getByTestId("two-factor-recovery-codes")).toContainText("recovery-one");
  await page.screenshot({ path: test.info().outputPath("two-factor-recovery.png"), animations: "disabled" });
  const storage = await page.evaluate(() => JSON.stringify({ ...localStorage, ...sessionStorage }));
  expect(storage).not.toContain(secret);
  expect(storage).not.toContain("recovery-one");
  await page.getByRole("dialog", { name: "访问控制" }).getByRole("button", { name: "取消", exact: true }).click();
  await page.getByRole("button", { name: "访问控制", exact: true }).click();
  await expect(panel.getByTestId("two-factor-recovery-codes")).toHaveCount(0);
  await panel.getByLabel("当前登录密码", { exact: true }).fill("existing-password");
  await panel.getByLabel("动态验证码或恢复码", { exact: true }).fill("recovery-two");
  await panel.getByRole("button", { name: "验证并关闭 2FA" }).click();
  await expect(panel.getByRole("button", { name: "绑定验证器", exact: true })).toBeVisible();
  expect(enabled).toBe(false);
  expect(writes.map((w) => w.method)).toEqual(["accountManager/twoFactor/setup", "accountManager/twoFactor/confirm", "accountManager/twoFactor/confirm", "accountManager/twoFactor/disable"]);
  expect(writes[1].params.setupToken).toBe("pending-setup");
  expect(settings.webAuthMode).toBe("password");
});
