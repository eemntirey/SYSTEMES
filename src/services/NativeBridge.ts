// SYSTEMES Native Bridge — Phase 7 Integration
// Communique avec le moteur natif C#/.NET via protocole local
// Aucun résultat simulé présenté comme réel

const API_BASE = 'http://localhost:5000'

export const NativeBridge = {
  async readHardware() {
    try {
      const res = await fetch(`${API_BASE}/hardware`)
      return await res.json()
    } catch {
      return { error: 'Moteur natif non accessible — données réelles non disponibles', source: 'Windows WMI/Win32' }
    }
  },

  async readDisk() {
    try {
      const res = await fetch(`${API_BASE}/disk`)
      return await res.json()
    } catch {
      return { error: 'Moteur natif non accessible', source: 'Windows WMI' }
    }
  },

  async fileOperate(action: 'read' | 'write' | 'delete' | 'rename', path: string, content?: string) {
    try {
      const res = await fetch(`${API_BASE}/file/operate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ action, path, content })
      })
      return await res.json()
    } catch {
      return { error: 'Opération système refusée ou non autorisée', action, path }
    }
  },

  async checkWindowsUpdate() {
    try {
      const res = await fetch(`${API_BASE}/update/check`)
      return await res.json()
    } catch {
      return { available: false, note: 'Windows Update non accessible sans autorisation' }
    }
  },
}
