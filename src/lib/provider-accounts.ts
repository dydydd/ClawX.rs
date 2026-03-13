import { invokeIpc } from '@/lib/api-client';
import type {
  ProviderAccount,
  ProviderType,
  ProviderVendorInfo,
  ProviderWithKeyInfo,
} from '@/lib/providers';

export interface ProviderSnapshot {
  accounts: ProviderAccount[];
  statuses: ProviderWithKeyInfo[];
  vendors: ProviderVendorInfo[];
  defaultAccountId: string | null;
}

export interface ProviderListItem {
  account: ProviderAccount;
  vendor?: ProviderVendorInfo;
  status?: ProviderWithKeyInfo;
}

export async function fetchProviderSnapshot(): Promise<ProviderSnapshot> {
  const [accounts, vendors, defaultAccountId] = await Promise.all([
    invokeIpc<ProviderAccount[]>('list_provider_accounts').catch(() => []),
    invokeIpc<ProviderVendorInfo[]>('list_provider_vendors').catch(() => []),
    invokeIpc<string | null>('get_default_provider_account').catch(() => null),
  ]);

  // Build statuses array with key info
  const statuses: ProviderWithKeyInfo[] = await Promise.all(
    (accounts || []).map(async (account) => {
      const hasKey = await invokeIpc<boolean>('has_provider_api_key', account.id).catch(() => false);
      const keyMasked = hasKey ? await invokeIpc<string>('get_provider_api_key_masked', account.id).catch(() => '') : '';

      return {
        id: account.id,
        type: account.vendorId,
        name: account.label || account.vendorId,
        hasKey,
        keyMasked,
        baseUrl: account.baseUrl,
        model: account.model,
        fallbackModels: account.fallbackModels,
        fallbackProviderIds: account.fallbackAccountIds,
        enabled: account.enabled,
        createdAt: account.createdAt,
        updatedAt: account.updatedAt,
      };
    })
  );

  return {
    accounts: Array.isArray(accounts) ? accounts : [],
    statuses: Array.isArray(statuses) ? statuses : [],
    vendors: Array.isArray(vendors) ? vendors : [],
    defaultAccountId: defaultAccountId ?? null,
  };
}

export function hasConfiguredCredentials(
  account: ProviderAccount,
  status?: ProviderWithKeyInfo,
): boolean {
  if (account.authMode === 'oauth_device' || account.authMode === 'oauth_browser' || account.authMode === 'local') {
    return true;
  }
  return status?.hasKey ?? false;
}

export function pickPreferredAccount(
  accounts: ProviderAccount[],
  defaultAccountId: string | null,
  vendorId: ProviderType | string,
  statusMap: Map<string, ProviderWithKeyInfo>,
): ProviderAccount | null {
  const sameVendor = accounts.filter((account) => account.vendorId === vendorId);
  if (sameVendor.length === 0) return null;

  return (
    (defaultAccountId ? sameVendor.find((account) => account.id === defaultAccountId) : undefined)
    || sameVendor.find((account) => hasConfiguredCredentials(account, statusMap.get(account.id)))
    || sameVendor[0]
  );
}

export function buildProviderAccountId(
  vendorId: ProviderType,
  existingAccountId: string | null,
  vendors: ProviderVendorInfo[],
): string {
  if (existingAccountId) {
    return existingAccountId;
  }

  const safeVendors = vendors || [];
  const vendor = safeVendors.find((candidate) => candidate.id === vendorId);
  return vendor?.supportsMultipleAccounts ? `${vendorId}-${crypto.randomUUID()}` : vendorId;
}

export function legacyProviderToAccount(provider: ProviderWithKeyInfo): ProviderAccount {
  return {
    id: provider.id,
    vendorId: provider.type,
    label: provider.name,
    authMode: provider.type === 'ollama' ? 'local' : 'api_key',
    baseUrl: provider.baseUrl,
    model: provider.model,
    fallbackModels: provider.fallbackModels,
    fallbackAccountIds: provider.fallbackProviderIds,
    enabled: provider.enabled,
    isDefault: false,
    createdAt: provider.createdAt,
    updatedAt: provider.updatedAt,
  };
}

export function buildProviderListItems(
  accounts: ProviderAccount[],
  statuses: ProviderWithKeyInfo[],
  vendors: ProviderVendorInfo[],
  defaultAccountId: string | null,
): ProviderListItem[] {
  const safeAccounts = accounts || [];
  const safeStatuses = statuses || [];
  const safeVendors = vendors || [];

  const vendorMap = new Map(safeVendors.map((vendor) => [vendor.id, vendor]));
  const statusMap = new Map(safeStatuses.map((status) => [status.id, status]));

  if (safeAccounts.length > 0) {
    return safeAccounts
      .map((account) => ({
        account,
        vendor: vendorMap.get(account.vendorId),
        status: statusMap.get(account.id),
      }))
      .sort((left, right) => {
        if (left.account.id === defaultAccountId) return -1;
        if (right.account.id === defaultAccountId) return 1;
        return right.account.updatedAt.localeCompare(left.account.updatedAt);
      });
  }

  return safeStatuses.map((status) => ({
    account: legacyProviderToAccount(status),
    vendor: vendorMap.get(status.type),
    status,
  }));
}
