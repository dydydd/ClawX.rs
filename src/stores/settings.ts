/**
 * Settings State Store
 * Manages application settings using Tauri IPC commands
 */
import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import i18n from '@/i18n';
import { invokeIpc } from '@/lib/api-client';

type Theme = 'light' | 'dark' | 'system';
type UpdateChannel = 'stable' | 'beta' | 'dev';

interface SettingsState {
  // General
  theme: Theme;
  language: string;
  startMinimized: boolean;
  launchAtStartup: boolean;
  telemetryEnabled: boolean;

  // Gateway
  gatewayAutoStart: boolean;
  gatewayPort: number;
  proxyEnabled: boolean;
  proxyServer: string;
  proxyHttpServer: string;
  proxyHttpsServer: string;
  proxyAllServer: string;
  proxyBypassRules: string;

  // Update
  updateChannel: UpdateChannel;
  autoCheckUpdate: boolean;
  autoDownloadUpdate: boolean;

  // UI State
  sidebarCollapsed: boolean;
  devModeUnlocked: boolean;

  // Setup
  setupComplete: boolean;

  // Actions
  init: () => Promise<void>;
  setTheme: (theme: Theme) => void;
  setLanguage: (language: string) => void;
  setStartMinimized: (value: boolean) => void;
  setLaunchAtStartup: (value: boolean) => void;
  setTelemetryEnabled: (value: boolean) => void;
  setGatewayAutoStart: (value: boolean) => void;
  setGatewayPort: (port: number) => void;
  setProxyEnabled: (value: boolean) => void;
  setProxyServer: (value: string) => void;
  setProxyHttpServer: (value: string) => void;
  setProxyHttpsServer: (value: string) => void;
  setProxyAllServer: (value: string) => void;
  setProxyBypassRules: (value: string) => void;
  setUpdateChannel: (channel: UpdateChannel) => void;
  setAutoCheckUpdate: (value: boolean) => void;
  setAutoDownloadUpdate: (value: boolean) => void;
  setSidebarCollapsed: (value: boolean) => void;
  setDevModeUnlocked: (value: boolean) => void;
  markSetupComplete: () => void;
  resetSettings: () => void;
}

const defaultSettings = {
  theme: 'system' as Theme,
  language: (() => {
    const lang = navigator.language.toLowerCase();
    if (lang.startsWith('zh')) return 'zh';
    if (lang.startsWith('ja')) return 'ja';
    return 'en';
  })(),
  startMinimized: false,
  launchAtStartup: false,
  telemetryEnabled: true,
  gatewayAutoStart: true,
  gatewayPort: 18789,
  proxyEnabled: false,
  proxyServer: '',
  proxyHttpServer: '',
  proxyHttpsServer: '',
  proxyAllServer: '',
  proxyBypassRules: '<local>;localhost;127.0.0.1;::1',
  updateChannel: 'stable' as UpdateChannel,
  autoCheckUpdate: true,
  autoDownloadUpdate: false,
  sidebarCollapsed: false,
  devModeUnlocked: false,
  setupComplete: false,
};

export const useSettingsStore = create<SettingsState>()(
  persist(
    (set) => ({
      ...defaultSettings,

      init: async () => {
        try {
          const settings = await invokeIpc<Record<string, unknown>>('get_all_settings');
          // Explicitly handle setupComplete to ensure it's correctly synced from backend
          const setupComplete = settings.setupComplete;
          set((state) => ({ ...state, ...settings, setupComplete: setupComplete === true }));
          if (settings.language) {
            i18n.changeLanguage(settings.language as string);
            // Also update the tray menu language
            void invokeIpc('update_tray_language_cmd', { language: settings.language }).catch(() => {});
          }
        } catch {
          // Keep renderer-persisted settings as a fallback when the main
          // process store is not reachable.
        }
      },

      setTheme: (theme) => {
        set({ theme });
        void invokeIpc('set_setting', { key: 'theme', value: theme }).catch(() => {});
      },
      setLanguage: (language) => {
        i18n.changeLanguage(language);
        set({ language });
        // Update tray menu language in the backend
        void invokeIpc('update_tray_language_cmd', { language }).catch(() => {});
        void invokeIpc('set_setting', { key: 'language', value: language }).catch(() => {});
      },
      setStartMinimized: (startMinimized) => {
        set({ startMinimized });
        void invokeIpc('set_setting', { key: 'startMinimized', value: startMinimized }).catch(() => {});
      },
      setLaunchAtStartup: (launchAtStartup) => {
        set({ launchAtStartup });
        void invokeIpc('set_setting', { key: 'launchAtStartup', value: launchAtStartup }).catch(() => {});
      },
      setTelemetryEnabled: (telemetryEnabled) => {
        set({ telemetryEnabled });
        void invokeIpc('set_setting', { key: 'telemetryEnabled', value: telemetryEnabled }).catch(() => {});
      },
      setGatewayAutoStart: (gatewayAutoStart) => {
        set({ gatewayAutoStart });
        void invokeIpc('set_setting', { key: 'gatewayAutoStart', value: gatewayAutoStart }).catch(() => {});
      },
      setGatewayPort: (gatewayPort) => {
        set({ gatewayPort });
        void invokeIpc('set_setting', { key: 'gatewayPort', value: gatewayPort }).catch(() => {});
      },
      setProxyEnabled: (proxyEnabled) => {
        set({ proxyEnabled });
        void invokeIpc('set_setting', { key: 'proxyEnabled', value: proxyEnabled }).catch(() => {});
      },
      setProxyServer: (proxyServer) => {
        set({ proxyServer });
        void invokeIpc('set_setting', { key: 'proxyServer', value: proxyServer }).catch(() => {});
      },
      setProxyHttpServer: (proxyHttpServer) => {
        set({ proxyHttpServer });
        void invokeIpc('set_setting', { key: 'proxyHttpServer', value: proxyHttpServer }).catch(() => {});
      },
      setProxyHttpsServer: (proxyHttpsServer) => {
        set({ proxyHttpsServer });
        void invokeIpc('set_setting', { key: 'proxyHttpsServer', value: proxyHttpsServer }).catch(() => {});
      },
      setProxyAllServer: (proxyAllServer) => {
        set({ proxyAllServer });
        void invokeIpc('set_setting', { key: 'proxyAllServer', value: proxyAllServer }).catch(() => {});
      },
      setProxyBypassRules: (proxyBypassRules) => {
        set({ proxyBypassRules });
        void invokeIpc('set_setting', { key: 'proxyBypassRules', value: proxyBypassRules }).catch(() => {});
      },
      setUpdateChannel: (updateChannel) => {
        set({ updateChannel });
        void invokeIpc('set_setting', { key: 'updateChannel', value: updateChannel }).catch(() => {});
      },
      setAutoCheckUpdate: (autoCheckUpdate) => {
        set({ autoCheckUpdate });
        void invokeIpc('set_setting', { key: 'autoCheckUpdate', value: autoCheckUpdate }).catch(() => {});
      },
      setAutoDownloadUpdate: (autoDownloadUpdate) => {
        set({ autoDownloadUpdate });
        void invokeIpc('set_setting', { key: 'autoDownloadUpdate', value: autoDownloadUpdate }).catch(() => {});
      },
      setSidebarCollapsed: (sidebarCollapsed) => {
        set({ sidebarCollapsed });
        void invokeIpc('set_setting', { key: 'sidebarCollapsed', value: sidebarCollapsed }).catch(() => {});
      },
      setDevModeUnlocked: (devModeUnlocked) => {
        set({ devModeUnlocked });
        void invokeIpc('set_setting', { key: 'devModeUnlocked', value: devModeUnlocked }).catch(() => {});
      },
      markSetupComplete: () => {
        set({ setupComplete: true });
        void invokeIpc('set_setting', { key: 'setupComplete', value: true }).catch(() => {});
      },
      resetSettings: async () => {
        try {
          const settings = await invokeIpc<Record<string, unknown>>('reset_settings');
          set((state) => ({ ...state, ...settings }));
        } catch {
          set(defaultSettings);
        }
      },
    }),
    {
      name: 'clawx-settings',
    }
  )
);
