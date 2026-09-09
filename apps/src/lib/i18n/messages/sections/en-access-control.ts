import type { MessageCatalog } from "../types";

export const EN_ACCESS_CONTROL_MESSAGES: MessageCatalog = {
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
  "6 位动态验证码": "6-digit authenticator code",
  绑定时间: "Bound at",
  "绑定挑战已失效，请重新开始": "The setup challenge has expired. Start again.",
  绑定验证器: "Set up authenticator",
  查看并吊销当前账号的其他登录会话: "View and revoke other sessions for this account",
  待迁移的旧密码模式: "Legacy password mode pending migration",
  当前管理员密码: "Current administrator password",
  当前设备: "Current device",
  当前设备会话: "Account sessions",
  "当前未启用账号登录，Web 管理页处于公开状态":
    "Account login is disabled, so the Web management UI is publicly accessible.",
  "当前运行环境暂不支持读取或保存访问控制设置。":
    "This runtime cannot read or save access-control settings.",
  吊销: "Revoke",
  更换验证器: "Replace authenticator",
  关闭验证器: "Disable authenticator",
  "关闭验证器会吊销当前账号会话，确定继续？":
    "Disabling the authenticator revokes this account's sessions. Continue?",
  "管理员必须启用，成员可选启用": "Required for administrators and optional for members",
  管理员验证器已启用: "Administrator authenticator enabled",
  "管理员账号已创建，请立即完成验证器绑定":
    "Administrator account created. Complete authenticator setup now.",
  会话已吊销: "Session revoked",
  "将密钥添加到验证器应用，然后输入当前显示的 6 位验证码。":
    "Add the key to your authenticator app, then enter its current 6-digit code.",
  旧访问密码仅用于一次性迁移: "The legacy access password is only used for one-time migration",
  其他设备: "Other device",
  "请将密钥添加到验证器应用；密钥仅在本次绑定流程显示。":
    "Add this key to your authenticator app. It is shown only during this setup.",
  请输入当前管理员密码: "Enter the current administrator password",
  确认绑定: "Confirm setup",
  "统一管理 Web 账号登录和团队额度分发。旧访问密码仅用于一次性迁移。":
    "Manage Web account login and team quota distribution. The legacy access password is only used for one-time migration.",
  完成验证器绑定: "Complete authenticator setup",
  未绑定: "Not configured",
  验证器: "Authenticator",
  验证器绑定失败: "Authenticator setup failed",
  验证器二次验证: "Authenticator two-step verification",
  验证器已关闭: "Authenticator disabled",
  验证器已启用: "Authenticator enabled",
  "验证器已重置，目标账号的旧会话已失效":
    "Authenticator reset. All existing sessions for the target account are invalid.",
  验证器重置失败: "Failed to reset authenticator",
  "验证中...": "Verifying...",
  已绑定: "Configured",
  已启用动态验证码: "Dynamic codes enabled",
  用于授权创建管理员并发起验证器绑定:
    "Authorizes administrator creation and starts authenticator setup",
  暂无会话: "No sessions",
  "重置该账号验证器并吊销其全部会话？":
    "Reset this account's authenticator and revoke all of its sessions?",
  重置验证器: "Reset authenticator",
};
