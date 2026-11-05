// Browser-safe compatibility facade for the SYSTEMES native host.
// Native processes are owned by the WPF host, never spawned from renderer code.

export type NativeErrorCode =
  | 'permission-required' | 'unsupported' | 'host-unavailable' | 'timeout'
  | 'not-found' | 'invalid-argument' | 'io-error' | 'native-error';

export interface NativeResponse<T = unknown> {
  result?: T;
  error?: string;
  errorCode?: NativeErrorCode;
  retryable?: boolean;
  operation?: string;
  id?: string;
}

export type NativeBridgeState = 'checking' | 'ready' | 'unavailable' | 'error';

export class NativeOperationError extends Error {
  readonly code: NativeErrorCode;
  readonly retryable: boolean;
  readonly operation?: string;

  constructor(message: string, code: NativeErrorCode = 'native-error', retryable = false, operation?: string) {
    super(message);
    this.name = 'NativeOperationError';
    this.code = code;
    this.retryable = retryable;
    this.operation = operation;
  }
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

/** Méthodes exposées par NativeServer.cs — toute autre méthode est refusée côté client. */
export const ALLOWED_NATIVE_METHODS = new Set([
  'system/ping',
  'system/capabilities',
  'system/permissions/read',
  'terminal/execute',
  'security/read',
  'system/open-settings',
  'system/power/read',
  'system/power/set',
  'hardware/read',
  'memory/read',
  'processes/read',
  'processes/kill',
  'files/locations',
  'files/list',
  'files/read',
  'files/read-content',
  'files/write',
  'files/create-directory',
  'files/create',
  'files/recycle-bin/open',
  'files/delete',
  'files/permissions/read',
  'files/rename',
  'files/copy',
  'files/move',
  'devices/read',
  'services/read',
  'services/control',
  'audio/read',
  'audio/set-volume',
  'audio/set-mute',
  'audio/devices',
  'display/read',
  'storage/volumes',
  'apps/installed',
  'power/read',
  'network/read',
  'network/wifi/scan',
  'network/wifi/status',
  'network/wifi/connect',
  'network/wifi/disconnect',
  'network/wifi/adapter-status',
  'network/wifi/set-enabled',
  'bluetooth/set-enabled',
  'bluetooth/status',
  'bluetooth/scan',
  'bluetooth/pair',
  'update/check',
])

const MAX_PENDING_REQUESTS = 32
const MAX_PARAMS_JSON_CHARS = 512 * 1024
const MAX_METHOD_CHARS = 128
const DEFAULT_TIMEOUT_MS = 15_000

const METHOD_TIMEOUT_MS: Record<string, number> = {
  'system/ping': 5_000,
  'files/list': 30_000,
  'files/read-content': 30_000,
  'files/write': 30_000,
  'files/create': 20_000,
  'files/delete': 20_000,
  'files/rename': 20_000,
  'network/wifi/scan': 30_000,
  'network/wifi/connect': 45_000,
  'bluetooth/scan': 30_000,
  'bluetooth/pair': 45_000,
  'terminal/execute': 60_000,
  'update/check': 180_000,
  'processes/read': 20_000,
  'storage/volumes': 20_000,
  'apps/installed': 25_000,
  'devices/read': 25_000,
}

const METHOD_PATTERN = /^[a-z][a-z0-9/_-]{0,126}[a-z0-9]$/

function assertValidMethod(method: string): void {
  if (!method || method.length > MAX_METHOD_CHARS) {
    throw new Error('Méthode IPC invalide ou trop longue.')
  }
  if (!METHOD_PATTERN.test(method)) {
    throw new Error(`Format de méthode IPC invalide : « ${method} ».`)
  }
  if (!ALLOWED_NATIVE_METHODS.has(method)) {
    throw new Error(`Méthode native non autorisée côté client : « ${method} ».`)
  }
}

function assertValidParams(params: Record<string, unknown>): void {
  if (params === null || typeof params !== 'object' || Array.isArray(params)) {
    throw new Error('Les paramètres IPC doivent être un objet JSON.')
  }
  let encoded: string
  try {
    encoded = JSON.stringify(params)
  } catch {
    throw new Error('Les paramètres IPC ne sont pas sérialisables en JSON.')
  }
  if (encoded.length > MAX_PARAMS_JSON_CHARS) {
    throw new Error(`Les paramètres IPC dépassent la limite de ${MAX_PARAMS_JSON_CHARS} caractères.`)
  }
}

export class NativeBridge {
  private connected = false
  private state: NativeBridgeState = 'checking'
  private readonly stateListeners = new Set<(state: NativeBridgeState) => void>()

  get connectionState(): NativeBridgeState {
    return this.state
  }

  subscribe(listener: (state: NativeBridgeState) => void): () => void {
    this.stateListeners.add(listener)
    listener(this.state)
    return () => this.stateListeners.delete(listener)
  }

  private setState(state: NativeBridgeState): void {
    if (this.state === state) return
    this.state = state
    for (const listener of this.stateListeners) listener(state)
  }
  private readonly pending = new Map<string, {
    resolve: (value: NativeResponse) => void
    reject: (reason: Error) => void
    timeout: ReturnType<typeof setTimeout>
  }>()

  private readonly onMessage = (event: MessageEvent<NativeResponse>) => {
    const response = event.data
    if (!response || typeof response !== 'object' || typeof response.id !== 'string' || !response.id) {
      return
    }
    const pending = this.pending.get(response.id)
    if (!pending) return
    this.pending.delete(response.id)
    clearTimeout(pending.timeout)
    if (typeof response.error === 'string' && response.error) {
      pending.reject(new NativeOperationError(
        response.error,
        response.errorCode ?? 'native-error',
        response.retryable ?? false,
        response.operation,
      ))
    } else {
      if (response.result && typeof response.result === 'object'
        && (response.result as { status?: unknown }).status === 'ready') this.setState('ready')
      pending.resolve(response)
    }
  }

  async connect(): Promise<void> {
    const transport = this.transport()
    if (!transport) {
      this.setState('unavailable')
      throw new NativeOperationError(
        'SYSTEMES Native Host indisponible : démarrez SYSTEMES depuis son application Windows.',
        'host-unavailable',
        true,
      )
    }
    if (!this.connected) {
      this.setState('checking')
      transport.addEventListener('message', this.onMessage)
      this.connected = true
    }
  }

  async send(method: string, params: Record<string, unknown> = {}): Promise<NativeResponse> {
    assertValidMethod(method)
    assertValidParams(params)

    if (this.pending.size >= MAX_PENDING_REQUESTS) {
      throw new Error(`Trop de requêtes natives en cours (max ${MAX_PENDING_REQUESTS}). Réessayez dans un instant.`)
    }

    await this.connect()
    const transport = this.transport()
    if (!transport) throw new Error('Le canal natif WebView2 est indisponible.')

    const id = globalThis.crypto?.randomUUID?.()
      ?? `systemes-${Date.now()}-${Math.random().toString(16).slice(2)}`

    if (this.pending.has(id)) {
      throw new Error('Collision d\u2019identifiant IPC \u2014 réessayez.')
    }

    const timeoutMs = METHOD_TIMEOUT_MS[method] ?? DEFAULT_TIMEOUT_MS

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this.pending.delete(id)
        this.setState('error')
        reject(new NativeOperationError(`Délai dépassé pour la requête native « ${method} » (${Math.round(timeoutMs / 1000)} s).`, 'timeout', true, method))
      }, timeoutMs)
      this.pending.set(id, { resolve, reject, timeout })
      try {
        transport.postMessage({ id, method, params })
      } catch (error) {
        this.pending.delete(id)
        clearTimeout(timeout)
        this.setState('error')
        reject(error instanceof Error ? error : new NativeOperationError(String(error), 'io-error', true, method))
      }
    })
  }

  disconnect(): void {
    const transport = this.transport()
    if (this.connected && transport) {
      transport.removeEventListener('message', this.onMessage)
    }
    this.connected = false
    this.setState('unavailable')
    for (const pending of this.pending.values()) {
      clearTimeout(pending.timeout)
      pending.reject(new Error('Connexion au moteur natif interrompue.'))
    }
    this.pending.clear()
  }

  get pendingCount(): number {
    return this.pending.size
  }

  private transport(): WebViewMessage | null {
    if (typeof window === 'undefined') return null
    return window.chrome?.webview ?? null
  }
}

export const nativeBridge = new NativeBridge()
