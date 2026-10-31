import { spawn, ChildProcess } from 'child_process';

export interface NativeResponse {
  result?: any;
  error?: string;
  id?: string;
}

export class NativeBridge {
  private process: ChildProcess | null = null;

  async connect(): Promise<void> {
    return new Promise((resolve, reject) => {
      // Lance le moteur natif .NET (compilé ou via dotnet run)
      // Pour le développement, on utilise le chemin du projet natif
      const nativePath = 'native/Windver/bin/Debug/net8.0-windows/SystemesNative.exe';
      // En développement sans build, on peut utiliser dotnet run, mais pour l'instant on essaie l'exe
      this.process = spawn('dotnet', ['run', '--project', 'native/Windver/SystemesNative.csproj'], {
        cwd: process.cwd(),
        stdio: ['pipe', 'pipe', 'pipe'],
      });

      if (!this.process || !this.process.stdin) {
        reject(new Error('Impossible de lancer le moteur natif'));
        return;
      }

      this.process.stdout?.on('data', (data) => {
        const line = data.toString();
        console.log('[NATIVE]', line);
      });

      this.process.stderr?.on('data', (data) => {
        console.error('[NATIVE ERROR]', data.toString());
      });

      this.process.on('error', (err) => {
        console.error('[NATIVE SPAWN ERROR]', err);
      });

      // Attend un court délai pour s'assurer que le processus démarre
      setTimeout(() => resolve(), 500);
    });
  }

  async send(method: string, params?: any): Promise<NativeResponse> {
    if (!this.process || !this.process.stdin) {
      throw new Error('Moteur natif non connecté');
    }
    return new Promise((resolve, reject) => {
      const request = { method, params: params || {}, id: Math.random().toString(36) };
      const line = JSON.stringify(request) + '\n';

      let buffer = '';
      const handler = (data: Buffer) => {
        buffer += data.toString();
        const lines = buffer.split('\n');
        for (let i = 0; i < lines.length - 1; i++) {
          try {
            const response: NativeResponse = JSON.parse(lines[i]);
            this.process?.stdout?.off('data', handler as any);
            resolve(response);
            return;
          } catch (e) {
            // Ignorer les lignes non JSON
          }
        }
        buffer = lines[lines.length - 1];
      };

      this.process.stdout?.on('data', handler as any);
      this.process.stdin.write(line, (err) => {
        if (err) reject(err);
      });
    });
  }

  disconnect(): void {
    if (this.process) {
      this.process.kill();
      this.process = null;
    }
  }
}

export const nativeBridge = new NativeBridge();
