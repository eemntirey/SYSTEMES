import { describe, expect, it } from 'vitest';
import { capabilityApplicationState, nativeErrorState } from './NativeLifecycle';
import { NativeOperationError } from '../native/NativeBridge';

describe('native application lifecycle state mapping', () => {
  it('keeps host unavailability distinct from operation errors', () => {
    expect(nativeErrorState(new NativeOperationError('Host absent', 'host-unavailable'))).toBe('unavailable');
    expect(nativeErrorState(new Error('Unexpected'))).toBe('error');
  });

  it('exposes permission and unsupported states without collapsing them into generic errors', () => {
    expect(nativeErrorState(new NativeOperationError('Access denied', 'permission-required'))).toBe('permission-required');
    expect(nativeErrorState(new NativeOperationError('Not supported', 'unsupported'))).toBe('unsupported');
  });

  it('maps detected capability states to application states', () => {
    expect(capabilityApplicationState('ready')).toBe('ready');
    expect(capabilityApplicationState('unavailable')).toBe('unavailable');
    expect(capabilityApplicationState('unsupported')).toBe('unsupported');
    expect(capabilityApplicationState('permission-required')).toBe('permission-required');
  });
});
