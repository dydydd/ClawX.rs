/**
 * Provider State Store
 * Manages AI provider configurations using Tauri IPC commands
 */
import { create } from 'zustand';
import type {
  ProviderAccount,
  ProviderConfig,
  ProviderVendorInfo,
  ProviderWithKeyInfo,
} from '@/lib/providers';
import { invokeIpc } from '@/lib/api-client';
import {
  fetchProviderSnapshot,
} from '@/lib/provider-accounts';

// Re-export types for consumers that imported from here
export type {
  ProviderAccount,
  ProviderConfig,
  ProviderVendorInfo,
  ProviderWithKeyInfo,
} from '@/lib/providers';
export type { ProviderSnapshot } from '@/lib/provider-accounts';

interface ProviderState {
  statuses: ProviderWithKeyInfo[];
  accounts: ProviderAccount[];
  vendors: ProviderVendorInfo[];
  defaultAccountId: string | null;
  loading: boolean;
  error: string | null;

  // Actions
  refreshProviderSnapshot: () => Promise<void>;
  createAccount: (account: ProviderAccount, apiKey?: string) => Promise<void>;
  removeAccount: (accountId: string) => Promise<void>;
  validateAccountApiKey: (
    accountId: string,
    apiKey: string,
    options?: { baseUrl?: string; apiProtocol?: ProviderAccount['apiProtocol'] }
  ) => Promise<{ valid: boolean; error?: string }>;
  getAccountApiKey: (accountId: string) => Promise<string | null>;

  // Legacy compatibility aliases
  fetchProviders: () => Promise<void>;
  addProvider: (config: Omit<ProviderConfig, 'createdAt' | 'updatedAt'>, apiKey?: string) => Promise<void>;
  addAccount: (account: ProviderAccount, apiKey?: string) => Promise<void>;
  updateProvider: (providerId: string, updates: Partial<ProviderConfig>, apiKey?: string) => Promise<void>;
  updateAccount: (accountId: string, updates: Partial<ProviderAccount>, apiKey?: string) => Promise<void>;
  deleteProvider: (providerId: string) => Promise<void>;
  deleteAccount: (accountId: string) => Promise<void>;
  setApiKey: (providerId: string, apiKey: string) => Promise<void>;
  updateProviderWithKey: (
    providerId: string,
    updates: Partial<ProviderConfig>,
    apiKey?: string
  ) => Promise<void>;
  deleteApiKey: (providerId: string) => Promise<void>;
  setDefaultProvider: (providerId: string) => Promise<void>;
  setDefaultAccount: (accountId: string) => Promise<void>;
  validateApiKey: (
    providerId: string,
    apiKey: string,
    options?: { baseUrl?: string; apiProtocol?: ProviderAccount['apiProtocol'] }
  ) => Promise<{ valid: boolean; error?: string }>;
  getApiKey: (providerId: string) => Promise<string | null>;
}

export const useProviderStore = create<ProviderState>((set, get) => ({
  statuses: [],
  accounts: [],
  vendors: [],
  defaultAccountId: null,
  loading: false,
  error: null,

  refreshProviderSnapshot: async () => {
    set({ loading: true, error: null });

    try {
      const snapshot = await fetchProviderSnapshot();

      set({
        statuses: snapshot.statuses,
        accounts: snapshot.accounts,
        vendors: snapshot.vendors,
        defaultAccountId: snapshot.defaultAccountId,
        loading: false
      });
    } catch (error) {
      set({ error: String(error), loading: false });
    }
  },

  fetchProviders: async () => get().refreshProviderSnapshot(),

  addProvider: async (config, apiKey) => {
    try {
      const fullConfig: ProviderConfig = {
        ...config,
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      };

      // Convert ProviderConfig to ProviderAccount
      const account: ProviderAccount = {
        id: fullConfig.id,
        vendorId: fullConfig.type,
        label: fullConfig.name,
        authMode: fullConfig.type === 'ollama' ? 'local' : 'api_key',
        baseUrl: fullConfig.baseUrl,
        model: fullConfig.model,
        fallbackModels: fullConfig.fallbackModels,
        fallbackAccountIds: fullConfig.fallbackProviderIds,
        enabled: fullConfig.enabled,
        isDefault: false,
        createdAt: fullConfig.createdAt,
        updatedAt: fullConfig.updatedAt,
      };

      await invokeIpc('create_provider_account', { account, apiKey });

      // Refresh the list
      await get().refreshProviderSnapshot();
    } catch (error) {
      console.error('Failed to add provider:', error);
      throw error;
    }
  },

  createAccount: async (account, apiKey) => {
    try {
      await invokeIpc('create_provider_account', { account, apiKey });
      await get().refreshProviderSnapshot();
    } catch (error) {
      console.error('Failed to add account:', error);
      throw error;
    }
  },

  addAccount: async (account, apiKey) => get().createAccount(account, apiKey),

  updateProvider: async (providerId, updates, apiKey) => {
    try {
      const updateData: Record<string, unknown> = {
        label: updates.name,
        baseUrl: updates.baseUrl,
        model: updates.model,
        fallbackModels: updates.fallbackModels,
        fallbackAccountIds: updates.fallbackProviderIds,
        enabled: updates.enabled,
      };

      await invokeIpc('update_provider_account', { id: providerId, updates: updateData });

      if (apiKey) {
        await invokeIpc('set_provider_api_key', { providerId, apiKey });
      }

      // Refresh the list
      await get().refreshProviderSnapshot();
    } catch (error) {
      console.error('Failed to update provider:', error);
      throw error;
    }
  },

  updateAccount: async (accountId, updates, apiKey) => {
    try {
      await invokeIpc('update_provider_account', { id: accountId, updates });

      if (apiKey) {
        await invokeIpc('set_provider_api_key', { providerId: accountId, apiKey });
      }

      await get().refreshProviderSnapshot();
    } catch (error) {
      console.error('Failed to update account:', error);
      throw error;
    }
  },

  deleteProvider: async (providerId) => {
    try {
      await invokeIpc('delete_provider_account', { id: providerId });

      // Refresh the list
      await get().refreshProviderSnapshot();
    } catch (error) {
      console.error('Failed to delete provider:', error);
      throw error;
    }
  },

  removeAccount: async (accountId) => {
    try {
      await invokeIpc('delete_provider_account', { id: accountId });
      await get().refreshProviderSnapshot();
    } catch (error) {
      console.error('Failed to delete account:', error);
      throw error;
    }
  },

  deleteAccount: async (accountId) => get().removeAccount(accountId),

  setApiKey: async (providerId, apiKey) => {
    try {
      await invokeIpc('set_provider_api_key', { providerId, apiKey });

      // Refresh the list
      await get().refreshProviderSnapshot();
    } catch (error) {
      console.error('Failed to set API key:', error);
      throw error;
    }
  },

  updateProviderWithKey: async (providerId, updates, apiKey) => {
    try {
      await invokeIpc('update_provider_account', { id: providerId, updates });

      if (apiKey) {
        await invokeIpc('set_provider_api_key', { providerId, apiKey });
      }

      await get().refreshProviderSnapshot();
    } catch (error) {
      console.error('Failed to update provider with key:', error);
      throw error;
    }
  },

  deleteApiKey: async (providerId) => {
    try {
      await invokeIpc('delete_provider_api_key', { providerId });

      // Refresh the list
      await get().refreshProviderSnapshot();
    } catch (error) {
      console.error('Failed to delete API key:', error);
      throw error;
    }
  },

  setDefaultProvider: async (providerId) => {
    try {
      await invokeIpc('set_default_provider_account', { id: providerId });
      set({ defaultAccountId: providerId });
    } catch (error) {
      console.error('Failed to set default provider:', error);
      throw error;
    }
  },

  setDefaultAccount: async (accountId) => {
    try {
      await invokeIpc('set_default_provider_account', { id: accountId });
      set({ defaultAccountId: accountId });
    } catch (error) {
      console.error('Failed to set default account:', error);
      throw error;
    }
  },

  validateAccountApiKey: async (providerId, apiKey, options) => {
    // Validation would need to be implemented on backend
    // For now, just return valid
    return { valid: true };
  },

  validateApiKey: async (providerId, apiKey, options) => get().validateAccountApiKey(providerId, apiKey, options),

  getAccountApiKey: async (providerId) => {
    try {
      const apiKey = await invokeIpc<string | null>('get_provider_api_key', providerId);
      return apiKey;
    } catch {
      return null;
    }
  },

  getApiKey: async (providerId) => get().getAccountApiKey(providerId),
}));