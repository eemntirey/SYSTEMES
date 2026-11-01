import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NativeBridge } from './NativeBridge';

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

    expect(postMessage).toHaveBeenCalledTimes(1);
    const request = postMessage.mock.calls[0][0] as { id: string; method: string };
    expect(request.method).toBe('hardware/read');

    listeners[0]({ data: { id: 'unrelated', result: 'ignored' } } as MessageEvent);
    listeners[0]({ data: { id: request.id, result: { cpuName: 'Test CPU' } } } as MessageEvent);

    await expect(pending).resolves.toMatchObject({ result: { cpuName: 'Test CPU' }, id: request.id });
    bridge.disconnect();
  });

  it('rejects a native protocol error', async () => {
    const bridge = new NativeBridge();
    const pending = bridge.send('files/read', { path: 'Z:/missing.txt' });
    const request = postMessage.mock.calls[0][0] as { id: string };

    listeners[0]({ data: { id: request.id, error: 'Access denied' } } as MessageEvent);

    await expect(pending).rejects.toThrow('Access denied');
    bridge.disconnect();
  });

  it('rejects requests that exceed the 15 second deadline', async () => {
    vi.useFakeTimers();
    const bridge = new NativeBridge();
    const pending = bridge.send('hardware/read');
    const assertion = expect(pending).rejects.toThrow('Délai dépassé');

    await vi.advanceTimersByTimeAsync(15_000);
    await assertion;
    bridge.disconnect();
  });

  it('rejects all pending requests when disconnected', async () => {
    const bridge = new NativeBridge();
    const pending = bridge.send('devices/read');
    const assertion = expect(pending).rejects.toThrow('Connexion au moteur natif interrompue.');

    bridge.disconnect();
    await assertion;
  });

  it('fails explicitly when the WebView2 host is unavailable', async () => {
    delete (window as Window & { chrome?: unknown }).chrome;
    const bridge = new NativeBridge();

    await expect(bridge.send('hardware/read')).rejects.toThrow('Native Host indisponible');
  });
});
