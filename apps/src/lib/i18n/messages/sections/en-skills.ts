"use client";

import type { MessageCatalog } from "../types";

export const EN_SKILLS_MESSAGES: MessageCatalog = {
  "Skills 管理": "Skills",
  "扫描并管理 codexmanager-service 主机上的 Codex Skills。":
    "Scan and manage Codex Skills on the codexmanager-service host.",
  "系统内置 · 只读": "Built-in · read-only",
  用户安装: "User installed",
  系统只读: "System read-only",
  配置无效: "Invalid manifest",
  展开描述: "Show description",
  收起描述: "Collapse description",
  "由 Codex 管理": "Managed by Codex",
  不可安全删除: "Cannot be safely deleted",
  "Skill ZIP 已安装": "Skill ZIP installed",
  "Skill 目录已导入": "Skill directory imported",
  "Skill 已删除": "Skill deleted",
  "请选择 ZIP 文件": "Choose a ZIP file",
  "ZIP 文件不能超过 {size}": "The ZIP file cannot exceed {size}",
  请输入服务主机上的绝对路径: "Enter an absolute path on the service host",
  导入已有目录: "Import directory",
  "安装 ZIP": "Install ZIP",
  服务主机文件系统: "Service host filesystem",
  "这里的安装、导入和删除都发生在 codexmanager-service 所在主机，不是浏览器所在设备。":
    "Install, import, and delete operations happen on the codexmanager-service host, not on the device running this browser.",
  "搜索名称、描述或目录": "Search name, description, or directory",
  "当前无法读取 Skills": "Skills are currently unavailable",
  "请确认管理 RPC 可用并已连接 codexmanager-service。":
    "Make sure management RPC is available and codexmanager-service is connected.",
  "Skills 加载失败": "Failed to load Skills",
  "没有匹配的 Skill": "No matching Skill",
  "尚未发现 Skill": "No Skills found",
  "请调整搜索条件。": "Adjust the search criteria.",
  "可以安装一个 ZIP，或从服务主机导入已有目录。":
    "Install a ZIP or import an existing directory from the service host.",
  "导入已有 Skill 目录": "Import an existing Skill directory",
  "输入 codexmanager-service 主机上的绝对路径。目录根部必须包含 SKILL.md。":
    "Enter an absolute path on the codexmanager-service host. The directory root must contain SKILL.md.",
  服务主机绝对路径: "Absolute service-host path",
  导入: "Import",
  "例如 /opt/codex-skills/my-skill": "For example, /opt/codex-skills/my-skill",
  "删除 Skill": "Delete Skill",
  "将从服务主机永久删除“{name}”目录。此操作不可撤销。":
    "Permanently delete the “{name}” directory from the service host. This cannot be undone.",
  确认删除: "Delete",
  "Skills 市场": "Skills Marketplace",
  "Codex Skills 市场": "Codex Skills Marketplace",
  "通过 Codex 原生 Marketplace 安装完整插件，只展示包含标准 SKILL.md 的插件。":
    "Install complete plugins through the native Codex Marketplace. Only plugins containing standard SKILL.md files are shown.",
  "包含 {count} 个 Codex Skills": "Contains {count} Codex Skills",
  "收起 Skill 清单": "Collapse Skill list",
  "查看全部 {count} 个 Skills": "View all {count} Skills",
  "已由 Codex 安装": "Installed by Codex",
  "已安装（未启用）": "Installed (disabled)",
  已安装但未启用: "Installed but disabled",
  安装完整插件: "Install complete plugin",
  "Codex Marketplace 已导入": "Codex Marketplace imported",
  "导入 Marketplace 失败": "Failed to import Marketplace",
  "Marketplace 已刷新": "Marketplace refreshed",
  "刷新 Marketplace 失败": "Failed to refresh Marketplace",
  "插件已安装，新建 Codex 会话后生效":
    "Plugin installed. It will be active in a new Codex session.",
  安装插件失败: "Failed to install plugin",
  "请输入 GitHub 仓库": "Enter a GitHub repository",
  "GitHub 仓库，例如 openai/role-specific-plugins":
    "GitHub repository, for example openai/role-specific-plugins",
  "GitHub Marketplace 仓库": "GitHub Marketplace repository",
  "分支或标签（可选）": "Branch or tag (optional)",
  "例如 main": "For example, main",
  导入市场: "Import market",
  已连接市场: "Connected markets",
  "搜索插件、市场或 Skill": "Search plugins, markets, or Skills",
  "{count} 个兼容插件": "{count} compatible plugins",
  刷新市场: "Refresh markets",
  "Marketplace 加载失败": "Failed to load Marketplace",
  "当前无法读取 Skills 市场": "Skills Marketplace is currently unavailable",
  "当前 Codex CLI 不支持 Skills 市场":
    "The current Codex CLI does not support the Skills Marketplace",
  "请在 codexmanager-service 主机安装或升级支持 plugin 命令的 Codex CLI。":
    "Install or upgrade to a Codex CLI with plugin command support on the codexmanager-service host.",
  "没有匹配的 Marketplace 插件": "No matching Marketplace plugins",
  "没有发现兼容的 Codex 插件": "No compatible Codex plugins found",
  "导入 GitHub Marketplace；不含 Codex 插件清单或标准 SKILL.md 的插件会被忽略。":
    "Import a GitHub Marketplace. Plugins without a Codex manifest or standard SKILL.md files are ignored.",
  "安装完整 Codex 插件": "Install complete Codex plugin",
  "将安装“{name}”完整插件（市场：{marketplace}；来源：{source}），其中包含 {count} 个 Skills，也可能包含 MCP、Hooks、Apps 或脚本。仅在信任来源时继续。":
    "Install the complete “{name}” plugin (market: {marketplace}; source: {source}). It contains {count} Skills and may also include MCP servers, hooks, apps, or scripts. Continue only if you trust the source.",
  确认安装插件: "Install plugin",
};
