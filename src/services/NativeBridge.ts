// Typed native operations built on the single WebView2 transport.
import { nativeBridge as webViewBridge } from '../native/NativeBridge';

type NativeReply<T = unknown> = {
  id?: string;
  result?: T;
  error?: string;
};

const request = <T>(method: string, params: Record<string, unknown> = {}): Promise<NativeReply<T>> =>
  webViewBridge.send(method, params) as Promise<NativeReply<T>>;

export const NativeBridge = {
  async readHardware() {
    const response = await request('hardware/read');
    return response.result;
  },

  async readDevices() {
    const response = await request('devices/read');
    return response.result;
  },

  async readProcesses() {
    const response = await request('processes/read');
    return response.result;
  },

  async killProcess(pid: number) {
    if (!Number.isInteger(pid) || pid <= 0) throw new Error('PID invalide.');
    const response = await request('processes/kill', { pid });
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

  async checkWindowsUpdate() {
    const response = await request('update/check');
    return response.result;
  },

  async fileOperate(action: 'read' | 'write' | 'delete' | 'rename', path: string, content?: string) {
    switch (action) {
      case 'read': return this.readFile(path);
      case 'write': return this.writeTextFile(path, content ?? '');
      case 'delete': return this.deleteFile(path);
      case 'rename':
        throw new Error('Le renommage natif n’est pas encore exposé par NativeServer.');
    }
  },
};
