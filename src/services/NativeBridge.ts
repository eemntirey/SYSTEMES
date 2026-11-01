// Typed native operations built on the single WebView2 transport.
import { nativeBridge as webViewBridge } from '../native/NativeBridge';

export type WindowsSettingsPage =
  | 'display' | 'night-light' | 'accessibility-display' | 'sound' | 'notifications'
  | 'quiet-hours' | 'battery' | 'power' | 'bluetooth' | 'printers'
  | 'network-status' | 'network-ethernet' | 'network-vpn' | 'airplane-mode'
  | 'mobile-hotspot' | 'data-usage' | 'proxy' | 'taskbar' | 'lock-screen'
  | 'optional-features' | 'default-apps' | 'accounts' | 'sign-in-options'
  | 'accounts-sync' | 'work-access' | 'other-users' | 'date-time' | 'region'
  | 'language' | 'gaming' | 'game-mode' | 'graphics' | 'accessibility-magnifier'
  | 'accessibility-color-filters' | 'accessibility-captions' | 'accessibility-keyboard'
  | 'accessibility-mouse' | 'accessibility-speech' | 'windows-security'
  | 'device-encryption' | 'privacy' | 'privacy-diagnostics' | 'privacy-activity'
  | 'privacy-location' | 'privacy-camera' | 'privacy-microphone' | 'privacy-notifications'
  | 'windows-update' | 'windows-update-hours' | 'windows-update-optional'
  | 'windows-insider' | 'about' | 'pen-touch' | 'mobile-devices' | 'advanced-system' | 'device-manager';

type NativeReply<T = unknown> = {
  id?: string;
  result?: T;
  error?: string;
};

const request = async <T>(method: string, params: Record<string, unknown> = {}): Promise<NativeReply<T>> => {
  const response = await webViewBridge.send(method, params) as NativeReply<T>;
  if (response.error) throw new Error(response.error);
  return response;
};

export const NativeBridge = {
  async readHardware() {
    const response = await request('hardware/read');
    return response.result;
  },

  async readSecurityContext() {
    const response = await request<{
      userName: string;
      machineName: string;
      userSid?: string;
      authenticationType?: string;
      isAdministrator: boolean;
      isAuthenticated: boolean;
      isSystem: boolean;
      isGuest: boolean;
      impersonationLevel: string;
      groups: { sid: string; name: string }[];
      groupCount: number;
      uacEnabled: boolean | null;
      consentPromptBehaviorAdmin: number | null;
      secureDesktopEnabled: boolean | null;
      source: string;
      note: string;
    }>('security/read');
    if (!response.result) throw new Error('Windows n’a retourné aucun contexte de sécurité.');
    return response.result;
  },

  async readMemory() {
    const response = await request<{
      totalPhysicalBytes: number;
      availablePhysicalBytes: number;
      usedPhysicalBytes: number;
      memoryLoadPercent: number;
      totalPageFileBytes: number;
      availablePageFileBytes: number;
      totalVirtualBytes: number;
      availableVirtualBytes: number;
      sampleSource: string;
      sampledAtUtc: string;
    }>('memory/read');
    if (!response.result) throw new Error('Le moteur natif n’a retourné aucune mesure mémoire.');
    return response.result;
  },

  // ===== Audio (Core Audio réel) =====
  async readAudio() {
    const response = await request('audio/read');
    return response.result as {
      volume?: number;
      isMuted?: boolean;
      deviceName?: string;
      deviceId?: string;
      channels?: number;
      error?: string;
    };
  },

  async setAudioVolume(percent: number) {
    if (!Number.isFinite(percent) || percent < 0 || percent > 100) throw new Error('Volume invalide (0–100 attendu).');
    const response = await request('audio/set-volume', { percent: Math.round(percent) });
    return response.result;
  },

  async setAudioMuted(muted: boolean) {
    const response = await request('audio/set-mute', { muted });
    return response.result;
  },

  async listAudioDevices() {
    const response = await request('audio/devices');
    return response.result as { devices?: { id: string; name: string; flow: string }[]; error?: string };
  },

  async readDevices() {
    const response = await request('devices/read');
    return response.result;
  },

  async readServices() {
    const response = await request<{ services: { name: string; displayName: string; state: string; startMode: string; account: string; processId: number; executablePath: string }[]; count: number; source: string }>('services/read');
    if (!response.result) throw new Error('Windows n’a retourné aucun inventaire de services.');
    return response.result;
  },

  async controlService(name: string, action: 'start' | 'stop') {
    if (!/^[A-Za-z0-9_.-]{1,128}$/.test(name)) throw new Error('Nom de service invalide.');
    const response = await request('services/control', { name, action });
    return response.result;
  },

  async readProcesses() {
    const response = await request('processes/read');
    return response.result;
  },

  async killProcess(pid: number, expectedName: string, expectedStartTimeUtcTicks: string) {
    if (!Number.isInteger(pid) || pid <= 0) throw new Error('PID invalide.');
    if (!expectedName.trim()) throw new Error('Le nom du processus est obligatoire pour vérifier le PID.');
    if (!/^\d+$/.test(expectedStartTimeUtcTicks)) throw new Error('L’heure de démarrage du processus est requise pour vérifier le PID.');
    const response = await request('processes/kill', { pid, name: expectedName, startTimeUtcTicks: expectedStartTimeUtcTicks });
    return response.result;
  },

  async readFile(path: string) {
    const response = await request('files/read', { path });
    return response.result;
  },

  async getFileLocations() {
    const response = await request('files/locations');
    return response.result;
  },

  async listFiles(path = '') {
    const response = await request('files/list', { path });
    return response.result;
  },

  async readTextFile(path: string) {
    const response = await request('files/read-content', { path });
    return response.result;
  },

  async writeTextFile(path: string, content: string) {
    const response = await request('files/write', { path, content });
    return response.result;
  },

  async createDirectory(path: string) {
    const response = await request('files/create-directory', { path });
    return response.result;
  },

  async createFile(path: string, content = '') {
    const response = await request('files/create', { path, content });
    return response.result;
  },

  async deleteFile(path: string) {
    const response = await request('files/delete', { path });
    return response.result;
  },

  async openRecycleBin() {
    const response = await request('files/recycle-bin/open');
    return response.result as { opened: boolean; target: string; note: string };
  },

  async readPermissions(path: string) {
    const response = await request<{ path: string; acl: string; source: string; note: string }>('files/permissions/read', { path });
    if (!response.result) throw new Error('Windows n’a retourné aucune information de permissions.');
    return response.result;
  },

  async renameFile(path: string, newName: string) {
    if (!path.trim() || !newName.trim()) throw new Error('Le chemin et le nouveau nom sont obligatoires.');
    const response = await request('files/rename', { path, newName });
    return response.result;
  },

  async openWindowsSettings(page: WindowsSettingsPage) {
    const response = await request('system/open-settings', { page });
    return response.result;
  },

  async readPowerSchemes() {
    const response = await request('system/power/read');
    return response.result;
  },

  async setPowerScheme(guid: string) {
    if (!/^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/i.test(guid)) {
      throw new Error('Identifiant de plan d’alimentation invalide.');
    }
    const response = await request('system/power/set', { guid });
    return response.result;
  },

  async checkWindowsUpdate() {
    const response = await request('update/check');
    return response.result;
  },

  async readWifiStatus() {
    const response = await request('network/wifi/status');
    return response.result as { connected: boolean; ssid?: string | null; state?: string | null; signal?: string | null; error?: string };
  },

  async scanWifi() {
    const response = await request('network/wifi/scan');
    return response.result as { networks?: { ssid: string; signal?: string; authentication?: string; encryption?: string; bssid?: string }[]; error?: string };
  },

  async connectWifi(ssid: string, password?: string) {
    if (!ssid.trim() || ssid.length > 32) throw new Error('SSID Wi-Fi invalide.');
    const response = await request('network/wifi/connect', { ssid, password });
    return response.result;
  },

  async disconnectWifi() {
    const response = await request('network/wifi/disconnect');
    return response.result;
  },

  async readBluetoothStatus() {
    const response = await request('bluetooth/status');
    return response.result as { adapterPresent: boolean; powered: boolean; adapterName?: string | null; devices?: { id: string; name: string; status: string }[]; error?: string };
  },

  async scanBluetooth(timeoutSeconds = 6) {
    const response = await request('bluetooth/scan', { timeoutSeconds });
    return response.result as { devices?: { id: string; name: string }[]; error?: string };
  },

  async pairBluetoothDevice(deviceId: string) {
    if (!deviceId.trim()) throw new Error('Identifiant de périphérique Bluetooth manquant.');
    const response = await request('bluetooth/pair', { deviceId });
    return response.result;
  },

  async fileOperate(action: 'read' | 'write' | 'delete' | 'rename', path: string, content?: string) {
    switch (action) {
      case 'read': return this.readFile(path);
      case 'write': return this.writeTextFile(path, content ?? '');
      case 'delete': return this.deleteFile(path);
      case 'rename':
        return this.renameFile(path, content ?? '');
    }
  },
};
