// SYSTEMES Native Bridge — WebView2 host transport.
// No browser fallback is allowed to fabricate operating-system results.

type NativeReply<T = unknown> = {
  id?: string;
  result?: T;
  error?: string;
};

type WebViewBridge = {
  postMessage: (message: unknown) => void;
  addEventListener: (type: 'message', listener: (event: MessageEvent<NativeReply>) => void) => void;
  removeEventListener: (type: 'message', listener: (event: MessageEvent<NativeReply>) => void) => void;
};

declare global {
  interface Window {
    chrome?: { webview?: WebViewBridge };
  }
}

const getTransport = (): WebViewBridge | null => {
  if (typeof window === 'undefined') return null;
  return window.chrome?.webview ?? null;
};

const request = <T>(method: string, params: Record<string, unknown> = {}): Promise<NativeReply<T>> => {
  const transport = getTransport();
  if (!transport) {
    return Promise.reject(new Error('SYSTEMES Native Host indisponible : ouvrez l’application Windows SYSTEMES.'));
  }

  const id = globalThis.crypto?.randomUUID?.() ?? `req-${Date.now()}-${Math.random().toString(16).slice(2)}`;

  return new Promise((resolve, reject) => {
    const onMessage = (event: MessageEvent<NativeReply<T>>) => {
      const response = event.data;
      if (!response || response.id !== id) return;
      transport.removeEventListener('message', onMessage as (event: MessageEvent<NativeReply>) => void);
      if (response.error) reject(new Error(response.error));
      else resolve(response);
    };

    transport.addEventListener('message', onMessage as (event: MessageEvent<NativeReply>) => void);
    try {
      transport.postMessage({ id, method, params });
    } catch (error) {
      transport.removeEventListener('message', onMessage as (event: MessageEvent<NativeReply>) => void);
      reject(error instanceof Error ? error : new Error(String(error)));
    }
  });
};

export const NativeBridge = {
  async readHardware() {
    const response = await request('hardware/read');
    return response.result;
  },

  async readDevices() {
    const response = await request('devices/read');
    return response.result;
  },

  async readProcesses() {
    const response = await request('processes/read');
    return response.result;
  },

  async killProcess(pid: number) {
    if (!Number.isInteger(pid) || pid <= 0) throw new Error('PID invalide.');
    const response = await request('processes/kill', { pid });
    return response.result;
  },

  async readFile(path: string) {
    const response = await request('files/read', { path });
    return response.result;
  },

  async createFile(path: string, content = '') {
    const response = await request('files/create', { path, content });
    return response.result;
  },

  async deleteFile(path: string) {
    const response = await request('files/delete', { path });
    return response.result;
  },

  async checkWindowsUpdate() {
    const response = await request('update/check');
    return response.result;
  },

  async fileOperate(action: 'read' | 'write' | 'delete' | 'rename', path: string, content?: string) {
    switch (action) {
      case 'read': return this.readFile(path);
      case 'write': return this.createFile(path, content ?? '');
      case 'delete': return this.deleteFile(path);
      case 'rename':
        throw new Error('Le renommage natif n’est pas encore exposé par NativeServer.');
    }
  },
};
