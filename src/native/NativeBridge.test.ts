import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NativeBridge, ALLOWED_NATIVE_METHODS } from './NativeBridge';

type Listener = (event: MessageEvent) => void;

describe('NativeBridge WebView2 transport', () => {
  let listeners: Listener[];
  let postMessage: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    listeners = [];
    postMessage = vi.fn();
    Object.defineProperty(window, 'chrome', {
      configurable: true,
      value: {
        webview: {
          postMessage,
          addEventListener: vi.fn((_type: string, listener: Listener) => listeners.push(listener)),
          removeEventListener: vi.fn((_type: string, listener: Listener) => {
            listeners = listeners.filter((candidate) => candidate !== listener);
          }),
        },
      },
    });
  });

  afterEach(() => {
    vi.useRealTimers();
    delete (window as Window & { chrome?: unknown }).chrome;
  });

  it('sends a request and resolves only its matching response', async () => {
    const bridge = new NativeBridge();
    const pending = bridge.send('hardware/read');
    await Promise.resolve();

    expect(postMessage).toHaveBeenCalledTimes(1);
    const request = postMessage.mock.calls[0][0] as { id: string; method: string };
    expect(request.method).toBe('hardware/read');

    listeners[0]({ data: { id: 'unrelated', result: 'ignored' } } as MessageEvent);
    listeners[0]({ data: { id: request.id, result: { cpuName: 'Test CPU' } } } as MessageEvent);

    await expect(pending).resolves.toMatchObject({ result: { cpuName: 'Test CPU' }, id: request.id });
    bridge.disconnect();
  });

  it('publishes bridge readiness after a successful native handshake', async () => {
    const bridge = new NativeBridge();
    const states: string[] = [];
    const unsubscribe = bridge.subscribe((state) => states.push(state));
    const pending = bridge.send('system/ping');
    await Promise.resolve();
    const request = postMessage.mock.calls[0][0] as { id: string };
    listeners[0]({ data: { id: request.id, result: { status: 'ready' } } } as MessageEvent);
    await expect(pending).resolves.toMatchObject({ result: { status: 'ready' } });
    expect(bridge.connectionState).toBe('ready');
    expect(states).toContain('checking');
    expect(states).toContain('ready');
    unsubscribe();
    bridge.disconnect();
  });

  it('preserves structured permission errors from the native engine', async () => {
    const bridge = new NativeBridge();
    const pending = bridge.send('files/write', { path: 'C:/protected/file.txt', content: 'x' });
    await Promise.resolve();
    const request = postMessage.mock.calls[0][0] as { id: string };
    listeners[0]({ data: {
      id: request.id,
      error: 'Access denied',
      errorCode: 'permission-required',
      retryable: false,
      operation: 'files/write',
    } } as MessageEvent);
    await expect(pending).rejects.toMatchObject({
      name: 'NativeOperationError',
      code: 'permission-required',
      retryable: false,
      operation: 'files/write',
    });
    bridge.disconnect();
  });

  it('rejects a native protocol error', async () => {
    const bridge = new NativeBridge();
    const pending = bridge.send('files/list', { path: 'Z:/missing' });
    await Promise.resolve();
    const request = postMessage.mock.calls[0][0] as { id: string };

    listeners[0]({ data: { id: request.id, error: 'Access denied' } } as MessageEvent);

    await expect(pending).rejects.toThrow('Access denied');
    bridge.disconnect();
  });

  it('rejects requests that exceed the deadline', async () => {
    vi.useFakeTimers();
    const bridge = new NativeBridge();
    const pending = bridge.send('hardware/read');
    await Promise.resolve();
    const assertion = expect(pending).rejects.toThrow('Délai dépassé');

    await vi.advanceTimersByTimeAsync(15_000);
    await assertion;
    bridge.disconnect();
  });

  it('rejects all pending requests when disconnected', async () => {
    const bridge = new NativeBridge();
    const pending = bridge.send('devices/read');
    await Promise.resolve();
    const assertion = expect(pending).rejects.toThrow('Connexion au moteur natif interrompue.');

    bridge.disconnect();
    await assertion;
  });

  it('fails explicitly when the WebView2 host is unavailable', async () => {
    delete (window as Window & { chrome?: unknown }).chrome;
    const bridge = new NativeBridge();

    await expect(bridge.send('hardware/read')).rejects.toThrow('Native Host indisponible');
  });

  it('rejects methods not on the client allowlist', async () => {
    const bridge = new NativeBridge();
    await expect(bridge.send('evil/pwn')).rejects.toThrow('non autorisée');
    expect(postMessage).not.toHaveBeenCalled();
    bridge.disconnect();
  });

  it('rejects invalid method format', async () => {
    const bridge = new NativeBridge();
    await expect(bridge.send('../escape')).rejects.toThrow('Format de méthode');
    await expect(bridge.send('')).rejects.toThrow('invalide');
    expect(postMessage).not.toHaveBeenCalled();
    bridge.disconnect();
  });

  it('rejects non-object params', async () => {
    const bridge = new NativeBridge();
    await expect(bridge.send('hardware/read', null as unknown as Record<string, unknown>)).rejects.toThrow('objet JSON');
    expect(postMessage).not.toHaveBeenCalled();
    bridge.disconnect();
  });

  it('ignores malformed responses without id', async () => {
    const bridge = new NativeBridge();
    const pending = bridge.send('system/ping');
    await Promise.resolve();
    const request = postMessage.mock.calls[0][0] as { id: string };

    listeners[0]({ data: { result: 'no-id' } } as MessageEvent);
    listeners[0]({ data: null } as MessageEvent);
    listeners[0]({ data: { id: request.id, result: { status: 'ready' } } } as MessageEvent);

    await expect(pending).resolves.toMatchObject({ result: { status: 'ready' } });
    bridge.disconnect();
  });

  it('exports a non-empty allowlist used by the server surface', () => {
    expect(ALLOWED_NATIVE_METHODS.has('system/ping')).toBe(true);
    expect(ALLOWED_NATIVE_METHODS.has('files/delete')).toBe(true);
    expect(ALLOWED_NATIVE_METHODS.has('hardware/read')).toBe(true);
    expect(ALLOWED_NATIVE_METHODS.size).toBeGreaterThan(20);
  });
});
