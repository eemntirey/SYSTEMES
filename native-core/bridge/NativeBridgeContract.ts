export interface SYSTEMES_NativeBridge {
  endpoint: string; // e.g., ws://localhost:9876 or ipc
  operations: {
    getWindowsInfo: () => Promise<{ version: string; build: string; isWindows11: boolean }>;
    getHardwareInfo: () => Promise<{ cpu: string; ramUsed: number; ramTotal: number; diskFree: number }>;
    getDevices: () => Promise<{ name: string; status: string }[]>;
    getProcesses: () => Promise<{ name: string; pid: number; cpu: number }[]>;
    requestPrivilege: (op: string, reason: string) => Promise<{ granted: boolean; message: string }>;
  };
  rules: {
    noAdminPermanent: true;
    noFakeResults: true;
    confirmWindowsBeforeClaim: true;
    logAllSensitiveOps: true;
  };
}
