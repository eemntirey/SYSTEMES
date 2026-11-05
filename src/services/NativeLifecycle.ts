import {
  NativeOperationError,
  nativeBridge,
  type NativeBridgeState,
} from '../native/NativeBridge';
import {
  NativeBridge,
  type NativeCapabilities,
  type NativeCapabilityState,
} from './NativeBridge';

export type NativeApplicationState =
  | 'checking'
  | 'ready'
  | 'unavailable'
  | 'unsupported'
  | 'permission-required'
  | 'error';

export interface NativeLifecycleSnapshot {
  state: NativeApplicationState;
  capabilities: NativeCapabilities | null;
  requiredCapability?: string;
  message?: string;
  checkedAtUtc: string;
}

export function nativeErrorState(error: unknown): NativeApplicationState {
  if (error instanceof NativeOperationError) {
    if (error.code === 'host-unavailable') return 'unavailable';
    if (error.code === 'unsupported') return 'unsupported';
    if (error.code === 'permission-required') return 'permission-required';
  }
  return 'error';
}

export function capabilityApplicationState(state: NativeCapabilityState): NativeApplicationState {
  switch (state) {
    case 'ready': return 'ready';
    case 'unavailable': return 'unavailable';
    case 'unsupported': return 'unsupported';
    case 'permission-required': return 'permission-required';
  }
}

export class NativeApplicationLifecycle {
  private snapshot: NativeLifecycleSnapshot = {
    state: 'checking',
    capabilities: null,
    checkedAtUtc: new Date().toISOString(),
  };
  private readonly listeners = new Set<(snapshot: NativeLifecycleSnapshot) => void>();

  get current(): NativeLifecycleSnapshot {
    return this.snapshot;
  }

  subscribe(listener: (snapshot: NativeLifecycleSnapshot) => void): () => void {
    this.listeners.add(listener);
    listener(this.snapshot);
    return () => this.listeners.delete(listener);
  }

  private publish(snapshot: NativeLifecycleSnapshot): NativeLifecycleSnapshot {
    this.snapshot = snapshot;
    for (const listener of this.listeners) listener(snapshot);
    return snapshot;
  }

  async check(requiredCapability?: string): Promise<NativeLifecycleSnapshot> {
    this.publish({
      state: 'checking',
      capabilities: null,
      requiredCapability,
      checkedAtUtc: new Date().toISOString(),
    });

    try {
      await nativeBridge.send('system/ping');
      const capabilities = await NativeBridge.readNativeCapabilities();
      const required = requiredCapability
        ? capabilities.capabilities.find((capability) => capability.id === requiredCapability)
        : undefined;

      if (requiredCapability && !required) {
        return this.publish({
          state: 'unsupported',
          capabilities,
          requiredCapability,
          message: `La capacité native « ${requiredCapability} » n’est pas déclarée par cet hôte.`,
          checkedAtUtc: new Date().toISOString(),
        });
      }

      const state = required
        ? capabilityApplicationState(required.state)
        : 'ready';
      return this.publish({
        state,
        capabilities,
        requiredCapability,
        message: required?.reason,
        checkedAtUtc: new Date().toISOString(),
      });
    } catch (error) {
      return this.publish({
        state: nativeErrorState(error),
        capabilities: null,
        requiredCapability,
        message: error instanceof Error ? error.message : 'Erreur native inconnue.',
        checkedAtUtc: new Date().toISOString(),
      });
    }
  }

  async run<T>(operation: () => Promise<T>): Promise<T> {
    try {
      const result = await operation();
      if (this.snapshot.state !== 'ready') {
        this.publish({ ...this.snapshot, state: 'ready', message: undefined, checkedAtUtc: new Date().toISOString() });
      }
      return result;
    } catch (error) {
      this.publish({
        ...this.snapshot,
        state: nativeErrorState(error),
        message: error instanceof Error ? error.message : 'Erreur native inconnue.',
        checkedAtUtc: new Date().toISOString(),
      });
      throw error;
    }
  }

  get transportState(): NativeBridgeState {
    return nativeBridge.connectionState;
  }
}

export const nativeApplicationLifecycle = new NativeApplicationLifecycle();
