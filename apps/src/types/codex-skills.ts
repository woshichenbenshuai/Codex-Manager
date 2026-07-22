export type CodexSkillSource = "user" | "system";

export interface CodexSkillSummary {
  directoryName: string;
  name: string;
  description: string;
  source: CodexSkillSource;
  deletable: boolean;
  valid: boolean;
  error: string | null;
}

export interface CodexSkillsInventory {
  codexHome: string;
  skillsRoot: string;
  items: CodexSkillSummary[];
  warnings: string[];
}

export interface CodexSkillMarketplaceSummary {
  name: string;
  sourceType: string;
  source: string | null;
}

export interface CodexSkillMarketplaceSkill {
  name: string;
  description: string;
}

export interface CodexSkillMarketplacePlugin {
  pluginId: string;
  name: string;
  marketplaceName: string;
  version: string;
  installed: boolean;
  enabled: boolean;
  description: string;
  author: string;
  category: string;
  skills: CodexSkillMarketplaceSkill[];
}

export interface CodexSkillMarketplaceInventory {
  cliAvailable: boolean;
  codexHome: string;
  marketplaces: CodexSkillMarketplaceSummary[];
  plugins: CodexSkillMarketplacePlugin[];
  warnings: string[];
}
