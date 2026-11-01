// Phase 1-2 Integration: NativeBridge contract for SYSTEMES Native Core
// Used by OSContext, settings, hardware, and process modules when native engine is running.

export interface NativeResponse<T = any> {
  success: boolean;
  data?: T;
  error?: string;
  requiresPrivilege?: boolean;
}

export interface INativeBridge {
  getWindowsInfo: () => Promise<NativeResponse<{ version: string; build: string; isWindows11: boolean; edition: string }>>;
  getHardwareInfo: () => Promise<NativeResponse<{ cpu: string; ramUsedBytes: number; ramTotalBytes: number; gpu: string; diskFreeBytes: number }>>;
  getDevices: () => Promise<NativeResponse<{ name: string; status: string; driverVersion: string }[]>>;
  getProcesses: () => Promise<NativeResponse<{ name: string; pid: number; cpuPercent: number; workingSetBytes: number }[]>>;
  requestPrivilege: (operation: string, reason: string) => Promise<NativeResponse<{ granted: boolean }>>;
}

export const nativeBridgeDirect: INativeBridge | null = (typeof window !== 'undefined' && (window as any).nativeBridge) ? (window as any).nativeBridge : null;
