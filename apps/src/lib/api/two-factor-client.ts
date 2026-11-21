import { invoke, withAddr } from "./transport";

export interface TwoFactorStatus { enabled: boolean; recoveryCodesRemaining: number }
export interface TwoFactorSetup { secret: string; otpauthUri: string; setupToken: string }
export interface TwoFactorConfirmation { recoveryCodes: string[] }

export const twoFactorClient = {
  status: () => invoke<TwoFactorStatus>("service_account_manager_two_factor_status", withAddr()),
  setup: (password: string) => invoke<TwoFactorSetup>("service_account_manager_two_factor_setup", withAddr({ password })),
  confirm: (password: string, setupToken: string, code: string) => invoke<TwoFactorConfirmation>("service_account_manager_two_factor_confirm", withAddr({ password, setupToken, code })),
  disable: (password: string, code: string) => invoke<void>("service_account_manager_two_factor_disable", withAddr({ password, code })),
};
