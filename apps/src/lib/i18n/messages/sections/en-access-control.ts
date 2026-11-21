import type { MessageCatalog } from "../types";

export const EN_ACCESS_CONTROL_MESSAGES: MessageCatalog = {
  "两步验证（2FA）": "Two-factor authentication (2FA)",
  "已启用。管理登录需要密码和动态验证码。": "Enabled. Administrator login requires a password and an authenticator code.",
  "可选启用验证器，为现有管理登录增加保护。": "Optionally add an authenticator to protect your existing administrator login.",
  "请先设置并保存登录密码，再绑定验证器。": "Set and save a login password before binding an authenticator.",
  "绑定成功，请保存恢复码": "Authenticator enabled. Save your recovery codes.",
  "恢复码仅显示一次，每条只能使用一次。遗失验证器时可代替动态验证码登录。": "Recovery codes are shown once and can each be used once. Use one instead of an authenticator code if you lose your device.",
  "已复制恢复码": "Recovery codes copied",
  "复制失败，请手动选择并保存恢复码。": "Copy failed. Select and save the recovery codes manually.",
  "复制恢复码": "Copy recovery codes",
  "当前登录密码": "Current login password",
  "使用验证器扫描二维码，或手动输入下面的密钥。": "Scan the QR code with your authenticator, or enter the secret below.",
  "验证器绑定二维码": "Authenticator setup QR code",
  "验证器密钥": "Authenticator secret",
  "6 位动态验证码": "6-digit authenticator code",
  "动态验证码或恢复码": "Authenticator or recovery code",
  "验证并启用 2FA": "Verify and enable 2FA",
  "取消绑定": "Cancel setup",
  "已关闭两步验证": "Two-factor authentication disabled",
  "验证并关闭 2FA": "Verify and disable 2FA",
  "绑定验证器": "Bind authenticator",
  "剩余恢复码：{count}": "Recovery codes remaining: {count}",

  访问方式: "Access mode",
  不启用: "Disabled",
  账号系统已启用: "Account system enabled",
  "账号系统未初始化，首次打开 Web 登录页时会创建管理员":
    "Account system is not initialized. The first Web login will create the administrator.",
  "适合个人或小团队：所有访问者共用同一个访问密码。":
    "Suitable for individuals or small teams: all visitors share one access password.",
  "适合多人使用：管理员维护成员账号，成员按归属钱包消费额度。":
    "Suitable for multi-user usage: administrators manage member accounts, and members consume quota from assigned wallets.",
  "公开访问不会拦截 Web 管理页，请只在本机可信环境使用。":
    "Public access does not protect the Web admin UI. Use it only on a trusted local machine.",
  "留空保存时会保留当前访问密码。":
    "Leave blank to keep the current access password.",
  "首次启用访问密码模式必须填写密码。":
    "A password is required the first time password access is enabled.",
  "请先启用账号系统，再开启额度分发。":
    "Enable the account system before turning on quota distribution.",
  "启用后平台 Key 需要归属到成员钱包":
    "After enabling, platform keys must be assigned to member wallets.",
  "已进入账号计费模式。为避免权限归属错乱和账务断层，不能从界面关闭账号系统或额度分发。":
    "Account billing mode is active. To avoid broken ownership and accounting gaps, the account system and quota distribution cannot be disabled from the UI.",
  "已进入账号计费模式，不能从界面关闭账号系统。":
    "Account billing mode is active, so the account system cannot be disabled from the UI.",
  "已进入账号计费模式，不能从界面关闭额度分发。":
    "Account billing mode is active, so quota distribution cannot be disabled from the UI.",
  锁定原因: "Lock reason",
};
