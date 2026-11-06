// Browser-safe stub for the SYSTEMES direct native bridge.
//
// The native engine (SystemesNative.exe) is owned and spawned by the WPF host
// (SYSTEMES.WindowsHost), never by the renderer. Inside the WebView2 renderer
// there is no Node runtime, so process spawning (child_process / process /
// Buffer) is unavailable here. This module keeps the public API surface used by
// NativeBridge.ts, but fails fast with a clear message instead of pretending to
// launch .NET from the browser bundle.

export interface NativeResponse {
  result?: unknown
  error?: string
  id?: string
}

const UNAVAILABLE =
  "Mode direct indisponible dans le bundle navigateur : le moteur natif est " +
  "démarré par l'hôte Windows SYSTEMES (WebView2). Aucun process .NET n'est " +
  "lancé depuis le renderer."

export class NativeBridgeDirect {
  async start(): Promise<void> {
    throw new Error(UNAVAILABLE)
  }

  async send(_method: string, _params?: unknown): Promise<NativeResponse> {
    throw new Error(UNAVAILABLE)
  }

  disconnect(): void {
    // Rien à nettoyer : aucun process n'est détenu côté renderer.
  }
}

export const nativeDirect = new NativeBridgeDirect()
