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
    timeout: ReturnType<typeof setTimeout>;
  }>();

  private readonly onMessage = (event: MessageEvent<NativeResponse>) => {
    const response = event.data;
    if (!response?.id) return;
    const pending = this.pending.get(response.id);
    if (!pending) return;
    this.pending.delete(response.id);
    clearTimeout(pending.timeout);
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
      const timeout = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Délai dépassé pour la requête native « ${method} » (15 s).`));
      }, 15_000);
      this.pending.set(id, { resolve, reject, timeout });
      try {
        transport.postMessage({ id, method, params });
      } catch (error) {
        this.pending.delete(id);
        clearTimeout(timeout);
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
      clearTimeout(pending.timeout);
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
