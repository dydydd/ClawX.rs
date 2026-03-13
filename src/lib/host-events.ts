import { createHostEventSource } from './host-api';
import { listen } from '@tauri-apps/api/event';

let eventSource: EventSource | null = null;

const HOST_EVENT_TO_IPC_CHANNEL: Record<string, string> = {
  'gateway:status': 'gateway:status',
  'gateway:error': 'gateway:error',
  'gateway:notification': 'gateway:notification',
  'gateway:chat-message': 'gateway:chat-message',
  'gateway:channel-status': 'gateway:channel-status',
  'gateway:exit': 'gateway:exit',
  'oauth:code': 'oauth:code',
  'oauth:success': 'oauth:success',
  'oauth:error': 'oauth:error',
  'channel:whatsapp-qr': 'channel:whatsapp-qr',
  'channel:whatsapp-success': 'channel:whatsapp-success',
  'channel:whatsapp-error': 'channel:whatsapp-error',
};

function getEventSource(): EventSource {
  if (!eventSource) {
    eventSource = createHostEventSource();
  }
  return eventSource;
}

function allowSseFallback(): boolean {
  try {
    return window.localStorage.getItem('clawx:allow-sse-fallback') === '1';
  } catch {
    return false;
  }
}

export function subscribeHostEvent<T = unknown>(
  eventName: string,
  handler: (payload: T) => void,
): () => void {
  const ipcChannel = HOST_EVENT_TO_IPC_CHANNEL[eventName];
  if (ipcChannel) {
    // Tauri event listener
    let unlisten: (() => void) | null = null;
    listen<T>(ipcChannel, (event) => {
      handler(event.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      if (unlisten) unlisten();
    };
  }

  if (!allowSseFallback()) {
    console.warn(`[host-events] no IPC mapping for event "${eventName}", SSE fallback disabled`);
    return () => {};
  }

  const source = getEventSource();
  const listener = (event: Event) => {
    const payload = JSON.parse((event as MessageEvent).data) as T;
    handler(payload);
  };
  source.addEventListener(eventName, listener);
  return () => {
    source.removeEventListener(eventName, listener);
  };
}
