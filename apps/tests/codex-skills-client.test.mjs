import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";
import ts from "../node_modules/typescript/lib/typescript.js";

const appsRoot = path.resolve(import.meta.dirname, "..");
const sourcePath = path.join(
  appsRoot,
  "src",
  "lib",
  "api",
  "codex-skills-client.ts",
);

async function loadClientModule() {
  const source = await fs.readFile(sourcePath, "utf8");
  const compiled = ts.transpileModule(source, {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
    },
    fileName: sourcePath,
  });
  const tempDir = await fs.mkdtemp(
    path.join(os.tmpdir(), "codexmanager-codex-skills-client-"),
  );
  const tempFile = path.join(tempDir, "codex-skills-client.mjs");
  await fs.writeFile(
    path.join(tempDir, "transport.mjs"),
    "export async function invoke(command, params, options) { globalThis.__codexSkillsInvokeCalls ??= []; globalThis.__codexSkillsInvokeCalls.push({ command, params, options }); return globalThis.__codexSkillsInvokeResult ?? {}; }\nexport function withAddr(value = {}) { return value; }\n",
    "utf8",
  );
  await fs.writeFile(
    tempFile,
    compiled.outputText.replace("./transport", "./transport.mjs"),
    "utf8",
  );
  return import(pathToFileURL(tempFile).href);
}

const client = await loadClientModule();

test("normalizeCodexSkillsInventory accepts camelCase and snake_case payloads", () => {
  const inventory = client.normalizeCodexSkillsInventory({
    codex_home: "/srv/codex",
    skillsRoot: "/srv/codex/skills",
    warnings: [" warning ", null],
    items: [
      {
        directory_name: "user-skill",
        name: " User Skill ",
        description: " description ",
        source: "user",
        deletable: true,
        valid: true,
      },
      {
        directoryName: ".system/system-skill",
        name: "System Skill",
        source: "system",
        deletable: false,
        valid: true,
      },
      { name: "missing-directory" },
    ],
  });

  assert.equal(inventory.codexHome, "/srv/codex");
  assert.equal(inventory.skillsRoot, "/srv/codex/skills");
  assert.deepEqual(inventory.warnings, ["warning"]);
  assert.equal(inventory.items.length, 2);
  assert.deepEqual(inventory.items[0], {
    directoryName: "user-skill",
    name: "User Skill",
    description: "description",
    source: "user",
    deletable: true,
    valid: true,
    error: null,
  });
  assert.equal(inventory.items[1].source, "system");
  assert.equal(inventory.items[1].deletable, false);
});

test("normalizeCodexSkillMarketplaceInventory filters malformed entries", () => {
  const inventory = client.normalizeCodexSkillMarketplaceInventory({
    cli_available: true,
    codex_home: "/srv/codex",
    marketplaces: [
      {
        name: " role-specific-plugins ",
        source_type: "git",
        source: " https://github.com/openai/role-specific-plugins.git ",
      },
      { source: "missing-name" },
    ],
    plugins: [
      {
        plugin_id: "product-design@role-specific-plugins",
        name: " Product Design ",
        marketplace_name: "role-specific-plugins",
        version: "0.1.50",
        installed: true,
        enabled: true,
        description: " Design workflows ",
        author: " OpenAI ",
        category: " Creativity ",
        skills: [
          { name: " audit ", description: " Review a flow " },
          { description: "missing name" },
        ],
      },
      { name: "missing-plugin-id" },
    ],
    warnings: [" warning ", null],
  });

  assert.equal(inventory.cliAvailable, true);
  assert.equal(inventory.codexHome, "/srv/codex");
  assert.deepEqual(inventory.marketplaces, [
    {
      name: "role-specific-plugins",
      sourceType: "git",
      source: "https://github.com/openai/role-specific-plugins.git",
    },
  ]);
  assert.equal(inventory.plugins.length, 1);
  assert.deepEqual(inventory.plugins[0], {
    pluginId: "product-design@role-specific-plugins",
    name: "Product Design",
    marketplaceName: "role-specific-plugins",
    version: "0.1.50",
    installed: true,
    enabled: true,
    description: "Design workflows",
    author: "OpenAI",
    category: "Creativity",
    skills: [{ name: "audit", description: "Review a flow" }],
  });
  assert.deepEqual(inventory.warnings, ["warning"]);
});

test("marketplace client methods keep command names and RPC parameters aligned", async () => {
  globalThis.__codexSkillsInvokeCalls = [];
  globalThis.__codexSkillsInvokeResult = {
    cliAvailable: true,
    codexHome: "/srv/codex",
    marketplaces: [],
    plugins: [],
    warnings: [],
  };

  await client.codexSkillsClient.listMarketplace("/srv/codex");
  await client.codexSkillsClient.addMarketplace({
    source: "openai/role-specific-plugins",
    refName: "main",
    codexHome: "/srv/codex",
  });
  await client.codexSkillsClient.refreshMarketplace({
    marketplaceName: "role-specific-plugins",
    codexHome: "/srv/codex",
  });
  await client.codexSkillsClient.installMarketplacePlugin({
    pluginId: "product-design@role-specific-plugins",
    codexHome: "/srv/codex",
  });

  const marketplaceOptions = {
    timeoutMs: client.CODEX_SKILLS_LONG_OPERATION_TIMEOUT_MS,
    retries: 0,
  };
  assert.equal(client.CODEX_SKILLS_LONG_OPERATION_TIMEOUT_MS, 600_000);

  assert.deepEqual(globalThis.__codexSkillsInvokeCalls, [
    {
      command: "service_codex_skills_marketplace_list",
      params: { codexHome: "/srv/codex" },
      options: marketplaceOptions,
    },
    {
      command: "service_codex_skills_marketplace_add",
      params: {
        source: "openai/role-specific-plugins",
        refName: "main",
        codexHome: "/srv/codex",
      },
      options: marketplaceOptions,
    },
    {
      command: "service_codex_skills_marketplace_refresh",
      params: {
        marketplaceName: "role-specific-plugins",
        codexHome: "/srv/codex",
      },
      options: marketplaceOptions,
    },
    {
      command: "service_codex_skills_marketplace_plugin_install",
      params: {
        pluginId: "product-design@role-specific-plugins",
        codexHome: "/srv/codex",
      },
      options: marketplaceOptions,
    },
  ]);
});

test("Skill file mutations use a long non-retrying request", async () => {
  globalThis.__codexSkillsInvokeCalls = [];
  globalThis.__codexSkillsInvokeResult = {
    codexHome: "/srv/codex",
    skillsRoot: "/srv/codex/skills",
    items: [],
    warnings: [],
  };

  await client.codexSkillsClient.list("/srv/codex");
  await client.codexSkillsClient.installZip({
    fileName: "skill.zip",
    archiveBase64: "eA==",
    codexHome: "/srv/codex",
  });
  await client.codexSkillsClient.importDirectory({
    sourcePath: "/opt/skills/example",
    codexHome: "/srv/codex",
  });
  await client.codexSkillsClient.delete({
    directoryName: "example",
    codexHome: "/srv/codex",
  });

  const longOptions = {
    timeoutMs: client.CODEX_SKILLS_LONG_OPERATION_TIMEOUT_MS,
    retries: 0,
  };
  assert.deepEqual(
    globalThis.__codexSkillsInvokeCalls.map(({ command, options }) => ({
      command,
      options,
    })),
    [
      { command: "service_codex_skills_list", options: undefined },
      { command: "service_codex_skills_install_zip", options: longOptions },
      {
        command: "service_codex_skills_import_directory",
        options: longOptions,
      },
      { command: "service_codex_skills_delete", options: longOptions },
    ],
  );
});

test("marketplace install confirmation stays nested and shows the real source", async () => {
  const source = await fs.readFile(
    path.join(appsRoot, "src", "app", "skills", "marketplace-dialog.tsx"),
    "utf8",
  );
  const confirmIndex = source.indexOf("<ConfirmDialog");
  const outerDialogCloseIndex = source.lastIndexOf("</Dialog>");

  assert.ok(confirmIndex > 0, "install confirmation is rendered");
  assert.ok(
    confirmIndex < outerDialogCloseIndex,
    "install confirmation remains inside the outer Dialog React tree",
  );
  assert.match(
    source,
    /if \(!nextOpen && anyMutationPending\) return;[\s\S]*?disablePointerDismissal=\{anyMutationPending\}/,
  );
  assert.match(source, /marketplaceSource=\{/);
  assert.match(source, /来源：\{source\}/);
});
