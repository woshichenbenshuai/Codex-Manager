import type {
  CodexSkillMarketplaceInventory,
  CodexSkillMarketplacePlugin,
  CodexSkillMarketplaceSkill,
  CodexSkillMarketplaceSummary,
  CodexSkillSource,
  CodexSkillSummary,
  CodexSkillsInventory,
} from "@/types";
import { invoke, withAddr } from "./transport";

export const CODEX_SKILLS_QUERY_KEY = ["codex-skills", "inventory"] as const;
export const CODEX_SKILLS_MARKETPLACE_QUERY_KEY = [
  "codex-skills",
  "marketplace",
] as const;
export const MAX_CODEX_SKILL_ZIP_BYTES = 16 * 1024 * 1024;
export const CODEX_SKILLS_LONG_OPERATION_TIMEOUT_MS = 10 * 60 * 1000;

function asObject(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function asString(value: unknown): string {
  return typeof value === "string" ? value.trim() : "";
}

function asBoolean(value: unknown): boolean {
  return value === true;
}

function asNullableString(value: unknown): string | null {
  return asString(value) || null;
}

function normalizeSource(value: unknown): CodexSkillSource {
  return asString(value).toLowerCase() === "system" ? "system" : "user";
}

function normalizeSkill(payload: unknown): CodexSkillSummary | null {
  const source = asObject(payload);
  const directoryName = asString(source.directoryName ?? source.directory_name);
  if (!directoryName) return null;
  return {
    directoryName,
    name: asString(source.name) || directoryName,
    description: asString(source.description),
    source: normalizeSource(source.source),
    deletable: asBoolean(source.deletable),
    valid: asBoolean(source.valid),
    error: asString(source.error) || null,
  };
}

export function normalizeCodexSkillsInventory(
  payload: unknown,
): CodexSkillsInventory {
  const source = asObject(payload);
  return {
    codexHome: asString(source.codexHome ?? source.codex_home),
    skillsRoot: asString(source.skillsRoot ?? source.skills_root),
    items: (Array.isArray(source.items) ? source.items : [])
      .map(normalizeSkill)
      .filter((item): item is CodexSkillSummary => Boolean(item)),
    warnings: (Array.isArray(source.warnings) ? source.warnings : [])
      .map(asString)
      .filter(Boolean),
  };
}

function normalizeMarketplaceSummary(
  payload: unknown,
): CodexSkillMarketplaceSummary | null {
  const source = asObject(payload);
  const name = asString(source.name);
  if (!name) return null;
  return {
    name,
    sourceType: asString(source.sourceType ?? source.source_type),
    source: asNullableString(source.source),
  };
}

function normalizeMarketplaceSkill(
  payload: unknown,
): CodexSkillMarketplaceSkill | null {
  const source = asObject(payload);
  const name = asString(source.name);
  if (!name) return null;
  return {
    name,
    description: asString(source.description),
  };
}

function normalizeMarketplacePlugin(
  payload: unknown,
): CodexSkillMarketplacePlugin | null {
  const source = asObject(payload);
  const pluginId = asString(source.pluginId ?? source.plugin_id);
  if (!pluginId) return null;
  return {
    pluginId,
    name: asString(source.name) || pluginId,
    marketplaceName: asString(
      source.marketplaceName ?? source.marketplace_name,
    ),
    version: asString(source.version),
    installed: asBoolean(source.installed),
    enabled: asBoolean(source.enabled),
    description: asString(source.description),
    author: asString(source.author),
    category: asString(source.category),
    skills: (Array.isArray(source.skills) ? source.skills : [])
      .map(normalizeMarketplaceSkill)
      .filter((item): item is CodexSkillMarketplaceSkill => Boolean(item)),
  };
}

export function normalizeCodexSkillMarketplaceInventory(
  payload: unknown,
): CodexSkillMarketplaceInventory {
  const source = asObject(payload);
  return {
    cliAvailable: asBoolean(source.cliAvailable ?? source.cli_available),
    codexHome: asString(source.codexHome ?? source.codex_home),
    marketplaces: (Array.isArray(source.marketplaces)
      ? source.marketplaces
      : []
    )
      .map(normalizeMarketplaceSummary)
      .filter((item): item is CodexSkillMarketplaceSummary => Boolean(item)),
    plugins: (Array.isArray(source.plugins) ? source.plugins : [])
      .map(normalizeMarketplacePlugin)
      .filter((item): item is CodexSkillMarketplacePlugin => Boolean(item)),
    warnings: (Array.isArray(source.warnings) ? source.warnings : [])
      .map(asString)
      .filter(Boolean),
  };
}

async function invokeInventory(
  command: string,
  params: Record<string, unknown> = {},
): Promise<CodexSkillsInventory> {
  const isMutation = command !== "service_codex_skills_list";
  const result = await invoke<unknown>(
    command,
    withAddr(params),
    isMutation
      ? {
          timeoutMs: CODEX_SKILLS_LONG_OPERATION_TIMEOUT_MS,
          // File mutations may complete on the service after the browser stops waiting. Retrying
          // would repeat an install/import/delete and turn a successful first attempt into an error.
          retries: 0,
        }
      : undefined,
  );
  return normalizeCodexSkillsInventory(result);
}

async function invokeMarketplace(
  command: string,
  params: Record<string, unknown> = {},
): Promise<CodexSkillMarketplaceInventory> {
  const result = await invoke<unknown>(command, withAddr(params), {
    timeoutMs: CODEX_SKILLS_LONG_OPERATION_TIMEOUT_MS,
    // A timed-out mutation may still be running on the service host. Retrying it would enqueue a
    // duplicate Marketplace operation, so React Query owns any user-visible retry instead.
    retries: 0,
  });
  return normalizeCodexSkillMarketplaceInventory(result);
}

export const codexSkillsClient = {
  list(codexHome?: string | null): Promise<CodexSkillsInventory> {
    return invokeInventory("service_codex_skills_list", {
      codexHome: codexHome || null,
    });
  },

  installZip(params: {
    fileName: string;
    archiveBase64: string;
    codexHome?: string | null;
  }): Promise<CodexSkillsInventory> {
    return invokeInventory("service_codex_skills_install_zip", {
      fileName: params.fileName,
      archiveBase64: params.archiveBase64,
      codexHome: params.codexHome || null,
    });
  },

  importDirectory(params: {
    sourcePath: string;
    codexHome?: string | null;
  }): Promise<CodexSkillsInventory> {
    return invokeInventory("service_codex_skills_import_directory", {
      sourcePath: params.sourcePath,
      codexHome: params.codexHome || null,
    });
  },

  delete(params: {
    directoryName: string;
    codexHome?: string | null;
  }): Promise<CodexSkillsInventory> {
    return invokeInventory("service_codex_skills_delete", {
      directoryName: params.directoryName,
      codexHome: params.codexHome || null,
    });
  },

  listMarketplace(
    codexHome?: string | null,
  ): Promise<CodexSkillMarketplaceInventory> {
    return invokeMarketplace("service_codex_skills_marketplace_list", {
      codexHome: codexHome || null,
    });
  },

  addMarketplace(params: {
    source: string;
    refName?: string | null;
    codexHome?: string | null;
  }): Promise<CodexSkillMarketplaceInventory> {
    return invokeMarketplace("service_codex_skills_marketplace_add", {
      source: params.source,
      refName: params.refName || null,
      codexHome: params.codexHome || null,
    });
  },

  refreshMarketplace(params?: {
    marketplaceName?: string | null;
    codexHome?: string | null;
  }): Promise<CodexSkillMarketplaceInventory> {
    return invokeMarketplace("service_codex_skills_marketplace_refresh", {
      marketplaceName: params?.marketplaceName || null,
      codexHome: params?.codexHome || null,
    });
  },

  installMarketplacePlugin(params: {
    pluginId: string;
    codexHome?: string | null;
  }): Promise<CodexSkillMarketplaceInventory> {
    return invokeMarketplace(
      "service_codex_skills_marketplace_plugin_install",
      {
        pluginId: params.pluginId,
        codexHome: params.codexHome || null,
      },
    );
  },
};
