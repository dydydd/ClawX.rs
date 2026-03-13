/**
 * Tauri API Wrapper
 *
 * Unified API layer that abstracts away the difference between Electron and Tauri IPC.
 * Detects the runtime environment and uses the appropriate IPC mechanism.
 */

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

/**
 * Check if running in Tauri environment
 */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}

/**
 * Check if running in Electron environment
 */
export function isElectron(): boolean {
  return typeof window !== 'undefined' && 'electron' in window && window.electron !== undefined;
}

/**
 * Invoke an IPC command (works in both Tauri and Electron)
 *
 * @param channel - The IPC channel/command name (e.g., 'gateway:status')
 * @param args - Arguments to pass to the command
 * @returns Promise resolving to the command result
 */
export async function invokeIpc<T>(channel: string, ...args: unknown[]): Promise<T> {
  if (isTauri()) {
    // Convert Electron IPC channel to Tauri command format
    // e.g., 'gateway:status' -> 'gateway_status'
    const command = channel.replace(':', '_');

    // Tauri expects a single object parameter
    const params = args.length > 0 ? args[0] : {};

    return await invoke<T>(command, params as Record<string, unknown>);
  }

  if (isElectron()) {
    // Use Electron IPC
    return await window.electron.ipcRenderer.invoke(channel, ...args) as T;
  }

  throw new Error('Neither Tauri nor Electron runtime detected');
}

/**
 * Listen to an IPC event (works in both Tauri and Electron)
 *
 * @param event - The event name (e.g., 'gateway:status-changed')
 * @param callback - Callback function to handle the event
 * @returns Promise resolving to an unlisten function
 */
export async function onIpcEvent<T>(
  event: string,
  callback: (payload: T) => void
): Promise<() => void> {
  if (isTauri()) {
    const unlisten = await listen<T>(event, (event) => {
      callback(event.payload);
    });
    return unlisten;
  }

  if (isElectron()) {
    const handler = (_event: unknown, payload: T) => {
      callback(payload);
    };
    window.electron.ipcRenderer.on(event, handler as (...args: unknown[]) => void);
    return () => {
      window.electron.ipcRenderer.off(event, handler as (...args: unknown[]) => void);
    };
  }

  throw new Error('Neither Tauri nor Electron runtime detected');
}

/**
 * Open an external URL in the default browser
 *
 * @param url - The URL to open
 */
export async function openExternal(url: string): Promise<void> {
  if (isTauri()) {
    await invokeIpc('shell:open_external', { url });
  } else if (isElectron()) {
    await window.electron.openExternal(url);
  } else {
    throw new Error('Neither Tauri nor Electron runtime detected');
  }
}