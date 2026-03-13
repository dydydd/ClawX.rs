// Frontend Integration Fix
// Add this to the appropriate frontend stores

import { invokeIpc } from '@/lib/api-client';

// ============================================
// Provider Management
// ============================================

export async function fetchProviderSnapshot(): Promise<ProviderSnapshot> {
  const [accounts, statuses, vendors, defaultInfo] = await Promise.all([
    invokeIpc<ProviderAccount[]>('list_provider_accounts'),
    invokeIpc<ProviderWithKeyInfo[]>('list_providers'), // Legacy
    invokeIpc<ProviderVendorInfo[]>('list_provider_vendors'),
    invokeIpc<{ accountId: string | null }>('get_default_provider_account'),
  ]);

  return {
    accounts,
    statuses,
    vendors,
    defaultAccountId: defaultInfo.accountId,
  };
}

// ============================================
// API Key Management
// ============================================

export async function setProviderApiKey(providerId: string, apiKey: string): Promise<void> {
  await invokeIpc<void>('set_provider_api_key', providerId, apiKey);
}

export async function getProviderApiKey(providerId: string): Promise<string | null> {
  return await invokeIpc<string | null>('get_provider_api_key', providerId);
}

export async function hasProviderApiKey(providerId: string): Promise<boolean> {
  return await invokeIpc<boolean>('has_provider_api_key', providerId);
}

export async function deleteProviderApiKey(providerId: string): Promise<void> {
  await invokeIpc<void>('delete_provider_api_key', providerId);
}

// ============================================
// OAuth Flow
// ============================================

export async function startOAuthFlow(
  provider: string,
  options?: { region?: string; accountId?: string; label?: string }
): Promise<void> {
  await invokeIpc<void>('oauth_start', provider, options?.region, options?.accountId, options?.label);
}

export async function cancelOAuthFlow(): Promise<void> {
  await invokeIpc<void>('oauth_cancel');
}

export async function submitOAuthCode(code: string): Promise<boolean> {
  return await invokeIpc<boolean>('oauth_submit_code', code);
}

export async function getOAuthStatus(): Promise<OAuthStatus> {
  return await invokeIpc<OAuthStatus>('oauth_get_status');
}

// ============================================
// Settings Management
// ============================================

export async function getSetting<T = unknown>(key: string): Promise<T | undefined> {
  return await invokeIpc<T | undefined>('get_setting', key);
}

export async function setSetting(key: string, value: unknown): Promise<void> {
  await invokeIpc<void>('set_setting', key, value);
}

export async function setManySettings(patch: Record<string, unknown>): Promise<void> {
  await invokeIpc<void>('set_many_settings', patch);
}

export async function getAllSettings(): Promise<Record<string, unknown>> {
  return await invokeIpc<Record<string, unknown>>('get_all_settings');
}

export async function resetSettings(): Promise<void> {
  await invokeIpc<void>('reset_settings');
}

// ============================================
// Skills Management
// ============================================

export async function searchSkills(query: string): Promise<Skill[]> {
  return await invokeIpc<Skill[]>('search_skills', query);
}

export async function installSkill(skillId: string): Promise<void> {
  await invokeIpc<void>('install_skill', skillId);
}

export async function uninstallSkill(skillId: string): Promise<void> {
  await invokeIpc<void>('uninstall_skill', skillId);
}

export async function getSkillConfig(skillId: string): Promise<SkillConfig | null> {
  return await invokeIpc<SkillConfig | null>('get_skill_config', skillId);
}

export async function updateSkillConfig(skillId: string, config: Partial<SkillConfig>): Promise<void> {
  await invokeIpc<void>('update_skill_config', skillId, config);
}

// ============================================
// Channels Management
// ============================================

export async function listAllChannels(): Promise<Channel[]> {
  return await invokeIpc<Channel[]>('list_all_channels');
}

export async function createChannel(channel: Channel): Promise<void> {
  await invokeIpc<void>('create_channel', channel);
}

export async function updateChannelConfig(channelId: string, config: Record<string, unknown>): Promise<void> {
  await invokeIpc<void>('update_channel_config', channelId, config);
}

export async function startWhatsAppLogin(): Promise<void> {
  await invokeIpc<void>('start_whatsapp_login');
}

export async function getWhatsAppLoginStatus(): Promise<WhatsAppLoginStatus> {
  return await invokeIpc<WhatsAppLoginStatus>('get_whatsapp_login_status');
}

export async function logoutWhatsApp(): Promise<void> {
  await invokeIpc<void>('logout_whatsapp');
}

// ============================================
// Type Definitions
// ============================================

interface ProviderSnapshot {
  accounts: ProviderAccount[];
  statuses: ProviderWithKeyInfo[];
  vendors: ProviderVendorInfo[];
  defaultAccountId: string | null;
}

interface ProviderAccount {
  id: string;
  vendorId: string;
  label?: string;
  authMode: 'api_key' | 'oauth_device' | 'oauth_browser' | 'local';
  baseUrl?: string;
  apiProtocol?: string;
  model?: string;
  fallbackModels?: string[];
  fallbackAccountIds?: string[];
  enabled: boolean;
  isDefault?: boolean;
  metadata?: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
}

interface ProviderVendorInfo {
  id: string;
  name: string;
  type: string;
  authMode: string;
  baseUrl?: string;
  supportsMultipleAccounts: boolean;
}

interface ProviderWithKeyInfo {
  id: string;
  type: string;
  hasKey: boolean;
}

interface OAuthStatus {
  type: 'idle' | 'device_flow' | 'browser_flow' | 'completed' | 'error';
  provider?: string;
  verification_uri?: string;
  user_code?: string;
  auth_url?: string;
  account_id?: string;
  message?: string;
}

interface Skill {
  id: string;
  name: string;
  description?: string;
  version?: string;
  author?: string;
}

interface SkillConfig {
  apiKey?: string;
  env?: Record<string, string>;
}

interface Channel {
  id: string;
  type: string;
  enabled: boolean;
  config: Record<string, unknown>;
  status: 'connected' | 'connecting' | 'disconnected' | 'error';
}

interface WhatsAppLoginStatus {
  state: 'idle' | 'awaiting_qr' | 'connecting' | 'connected' | 'error';
  qrCode?: {
    base64: string;
    raw: string;
    timestamp: number;
  };
}