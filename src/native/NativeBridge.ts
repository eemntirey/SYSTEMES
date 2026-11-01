// Browser-safe compatibility facade for the SYSTEMES native host.
// Native processes are owned by the WPF host, never spawned from renderer code.

export interface NativeResponse<T = unknown> {
  result?: T;
  error?: string;
  id?: string;
}

type WebViewMessage = {
  postMessage(message: unknown): void;
  addEventListener(type: 'message', listener: (event: MessageEvent<NativeResponse>) => void): void;
  removeEventListener(type: 'message', listener: (event: MessageEvent<NativeResponse>) => void): void;
};

declare global {
  interface Window {
    chrome?: { webview?: WebViewMessage };
  }
}

export class NativeBridge {
  private connected = false;
  private readonly pending = new Map<string, {
    resolve: (value: NativeResponse) => void;
    reject: (reason: Error) => void;
  }>();

  private readonly onMessage = (event: MessageEvent<NativeResponse>) => {
    const response = event.data;
    if (!response?.id) return;
    const pending = this.pending.get(response.id);
    if (!pending) return;
    this.pending.delete(response.id);
    if (response.error) pending.reject(new Error(response.error));
    else pending.resolve(response);
  };

  async connect(): Promise<void> {
    const transport = this.transport();
    if (!transport) {
      throw new Error('SYSTEMES Native Host indisponible : démarrez SYSTEMES depuis son application Windows.');
    }
    if (!this.connected) {
      transport.addEventListener('message', this.onMessage);
      this.connected = true;
    }
  }

  async send(method: string, params: Record<string, unknown> = {}): Promise<NativeResponse> {
    await this.connect();
    const transport = this.transport();
    if (!transport) throw new Error('Le canal natif WebView2 est indisponible.');

    const id = globalThis.crypto?.randomUUID?.()
      ?? `systemes-${Date.now()}-${Math.random().toString(16).slice(2)}`;

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      try {
        transport.postMessage({ id, method, params });
      } catch (error) {
        this.pending.delete(id);
        reject(error instanceof Error ? error : new Error(String(error)));
      }
    });
  }

  disconnect(): void {
    const transport = this.transport();
    if (this.connected && transport) {
      transport.removeEventListener('message', this.onMessage);
    }
    this.connected = false;
    for (const pending of this.pending.values()) {
      pending.reject(new Error('Connexion au moteur natif interrompue.'));
    }
    this.pending.clear();
  }

  private transport(): WebViewMessage | null {
    if (typeof window === 'undefined') return null;
    return window.chrome?.webview ?? null;
  }
}

export const nativeBridge = new NativeBridge();
