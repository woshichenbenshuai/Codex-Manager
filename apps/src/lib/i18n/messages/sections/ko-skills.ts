"use client";

import type { MessageCatalog } from "../types";

export const KO_SKILLS_MESSAGES: MessageCatalog = {
  "Skills 管理": "Skills 관리",
  "扫描并管理 codexmanager-service 主机上的 Codex Skills。":
    "codexmanager-service 호스트의 Codex Skills를 검색하고 관리합니다.",
  "系统内置 · 只读": "시스템 내장 · 읽기 전용",
  用户安装: "사용자 설치",
  系统只读: "시스템 읽기 전용",
  配置无效: "잘못된 매니페스트",
  展开描述: "설명 펼치기",
  收起描述: "설명 접기",
  "由 Codex 管理": "Codex에서 관리",
  不可安全删除: "안전하게 삭제할 수 없음",
  "Skill ZIP 已安装": "Skill ZIP을 설치했습니다",
  "Skill 目录已导入": "Skill 디렉터리를 가져왔습니다",
  "Skill 已删除": "Skill을 삭제했습니다",
  "请选择 ZIP 文件": "ZIP 파일을 선택하세요",
  "ZIP 文件不能超过 {size}": "ZIP 파일은 {size}를 초과할 수 없습니다",
  请输入服务主机上的绝对路径: "서비스 호스트의 절대 경로를 입력하세요",
  导入已有目录: "기존 디렉터리 가져오기",
  "安装 ZIP": "ZIP 설치",
  服务主机文件系统: "서비스 호스트 파일 시스템",
  "这里的安装、导入和删除都发生在 codexmanager-service 所在主机，不是浏览器所在设备。":
    "설치, 가져오기, 삭제는 브라우저 장치가 아닌 codexmanager-service 호스트에서 실행됩니다.",
  "搜索名称、描述或目录": "이름, 설명 또는 디렉터리 검색",
  "当前无法读取 Skills": "현재 Skills를 읽을 수 없습니다",
  "请确认管理 RPC 可用并已连接 codexmanager-service。":
    "관리 RPC를 사용할 수 있고 codexmanager-service가 연결되어 있는지 확인하세요.",
  "Skills 加载失败": "Skills를 불러오지 못했습니다",
  "没有匹配的 Skill": "일치하는 Skill 없음",
  "尚未发现 Skill": "발견된 Skill 없음",
  "请调整搜索条件。": "검색 조건을 조정하세요.",
  "可以安装一个 ZIP，或从服务主机导入已有目录。":
    "ZIP을 설치하거나 서비스 호스트의 기존 디렉터리를 가져올 수 있습니다.",
  "导入已有 Skill 目录": "기존 Skill 디렉터리 가져오기",
  "输入 codexmanager-service 主机上的绝对路径。目录根部必须包含 SKILL.md。":
    "codexmanager-service 호스트의 절대 경로를 입력하세요. 디렉터리 루트에 SKILL.md가 있어야 합니다.",
  服务主机绝对路径: "서비스 호스트 절대 경로",
  导入: "가져오기",
  "例如 /opt/codex-skills/my-skill": "예: /opt/codex-skills/my-skill",
  "删除 Skill": "Skill 삭제",
  "将从服务主机永久删除“{name}”目录。此操作不可撤销。":
    "서비스 호스트에서 ‘{name}’ 디렉터리를 영구 삭제합니다. 이 작업은 되돌릴 수 없습니다.",
  确认删除: "삭제",
  "Skills 市场": "Skills 마켓",
  "Codex Skills 市场": "Codex Skills 마켓",
  "通过 Codex 原生 Marketplace 安装完整插件，只展示包含标准 SKILL.md 的插件。":
    "Codex 기본 Marketplace를 통해 전체 플러그인을 설치합니다. 표준 SKILL.md가 포함된 플러그인만 표시합니다.",
  "包含 {count} 个 Codex Skills": "Codex Skills {count}개 포함",
  "收起 Skill 清单": "Skill 목록 접기",
  "查看全部 {count} 个 Skills": "Skills {count}개 모두 보기",
  "已由 Codex 安装": "Codex에서 설치됨",
  "已安装（未启用）": "설치됨(비활성화)",
  已安装但未启用: "설치되었지만 비활성화됨",
  安装完整插件: "전체 플러그인 설치",
  "Codex Marketplace 已导入": "Codex Marketplace를 가져왔습니다",
  "导入 Marketplace 失败": "Marketplace를 가져오지 못했습니다",
  "Marketplace 已刷新": "Marketplace를 새로 고쳤습니다",
  "刷新 Marketplace 失败": "Marketplace를 새로 고치지 못했습니다",
  "插件已安装，新建 Codex 会话后生效":
    "플러그인을 설치했습니다. 새 Codex 세션에서 적용됩니다.",
  安装插件失败: "플러그인을 설치하지 못했습니다",
  "请输入 GitHub 仓库": "GitHub 저장소를 입력하세요",
  "GitHub 仓库，例如 openai/role-specific-plugins":
    "GitHub 저장소(예: openai/role-specific-plugins)",
  "GitHub Marketplace 仓库": "GitHub Marketplace 저장소",
  "分支或标签（可选）": "브랜치 또는 태그(선택 사항)",
  "例如 main": "예: main",
  导入市场: "마켓 가져오기",
  已连接市场: "연결된 마켓",
  "搜索插件、市场或 Skill": "플러그인, 마켓 또는 Skill 검색",
  "{count} 个兼容插件": "호환 플러그인 {count}개",
  刷新市场: "마켓 새로 고침",
  "Marketplace 加载失败": "Marketplace를 불러오지 못했습니다",
  "当前无法读取 Skills 市场": "현재 Skills Marketplace를 읽을 수 없습니다",
  "当前 Codex CLI 不支持 Skills 市场":
    "현재 Codex CLI는 Skills 마켓을 지원하지 않습니다",
  "请在 codexmanager-service 主机安装或升级支持 plugin 命令的 Codex CLI。":
    "codexmanager-service 호스트에서 plugin 명령을 지원하는 Codex CLI를 설치하거나 업그레이드하세요.",
  "没有匹配的 Marketplace 插件": "일치하는 Marketplace 플러그인 없음",
  "没有发现兼容的 Codex 插件": "호환되는 Codex 플러그인을 찾지 못했습니다",
  "导入 GitHub Marketplace；不含 Codex 插件清单或标准 SKILL.md 的插件会被忽略。":
    "GitHub Marketplace를 가져오세요. Codex 매니페스트 또는 표준 SKILL.md가 없는 플러그인은 무시됩니다.",
  "安装完整 Codex 插件": "전체 Codex 플러그인 설치",
  "将安装“{name}”完整插件（市场：{marketplace}；来源：{source}），其中包含 {count} 个 Skills，也可能包含 MCP、Hooks、Apps 或脚本。仅在信任来源时继续。":
    "전체 ‘{name}’ 플러그인을 설치합니다(마켓: {marketplace}, 출처: {source}). Skills {count}개가 포함되어 있으며 MCP 서버, 훅, 앱 또는 스크립트도 포함될 수 있습니다. 출처를 신뢰하는 경우에만 계속하세요.",
  确认安装插件: "플러그인 설치",
};
