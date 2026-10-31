/**
 * SYSTEMES Phase 1 — Native Bridge (React ↔ .NET Native Core)
 * Règles de sécurité : pas de privilège permanent, pas de faux résultat,
 * confirmation Windows obligatoire, journalisation.
 */
export interface NativeSystemResponse {
  success: boolean;
  data?: Record<string, unknown>;
  error?: string;
  requiresPrivilege?: boolean;
}

export interface INativeBridge {
  getWindowsInfo: () => Promise<NativeSystemResponse>;
  getHardwareInfo: () => Promise<NativeSystemResponse>;
  getDevices: () => Promise<NativeSystemResponse>;
  getProcesses: () => Promise<NativeSystemResponse>;
  requestPrivilege: (op: string, reason: string) => Promise<NativeSystemResponse>;
}

// Connexion locale sécurisée — Phase 1 : protocole IPC via localhost sécurisé
// Ou WebView2 / service privé selon déploiement le plus sûr.
export const nativeBridge: INativeBridge = {
  getWindowsInfo: async () => {
    try {
      // Phase 1 : connexion réelle au moteur .NET
      const res = await fetch('http://localhost:9876/api/windows-info', { method: 'GET' });
      if (!res.ok) throw new Error('Native core unreachable');
      const json = await res.json();
      return { success: true, data: json };
    } catch {
      return { success: false, error: 'Moteur natif non disponible. Vérifiez SYSTEMES.Native.Core.', requiresPrivilege: false };
    }
  },
  getHardwareInfo: async () => {
    try {
      const res = await fetch('http://localhost:9876/api/hardware', { method: 'GET' });
      if (!res.ok) throw new Error('Native core unreachable');
      const json = await res.json();
      return { success: true, data: json };
    } catch {
      return { success: false, error: 'Moteur natif indisponible.', requiresPrivilege: false };
    }
  },
  getDevices: async () => {
    try {
      const res = await fetch('http://localhost:9876/api/devices', { method: 'GET' });
      if (!res.ok) throw new Error('Native core unreachable');
      const json = await res.json();
      return { success: true, data: json };
    } catch {
      return { success: false, error: 'Moteur natif indisponible.', requiresPrivilege: false };
    }
  },
  getProcesses: async () => {
    try {
      const res = await fetch('http://localhost:9876/api/processes', { method: 'GET' });
      if (!res.ok) throw new Error('Native core unreachable');
      const json = await res.json();
      return { success: true, data: json };
    } catch {
      return { success: false, error: 'Moteur natif indisponible.', requiresPrivilege: false };
    }
  },
  requestPrivilege: async (op: string, reason: string) => {
    // Phase 8 : aucune commande arbitraire sans validation
    console.log(`[SYSTEMES SECURITY] Demande de privilège : ${op} — raison : ${reason}`);
    try {
      const res = await fetch('http://localhost:9876/api/privilege', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ operation: op, reason }),
      });
      if (!res.ok) throw new Error('Privilege denied');
      return { success: true, data: { granted: true } };
    } catch {
      return { success: false, error: 'Privilège refusé ou non disponible.', requiresPrivilege: true };
    }
  },
};
