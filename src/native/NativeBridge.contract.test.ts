import { describe, expect, it } from 'vitest';
import { ALLOWED_NATIVE_METHODS } from './NativeBridge';
import nativeServerSource from '../../native/Windver/Core/NativeServer.cs?raw';
import typedFacadeSource from '../services/NativeBridge.ts?raw';

const extractQuotedMethods = (source: string, pattern: RegExp): string[] =>
  [...source.matchAll(pattern)].map((match) => match[1]);

describe('Native IPC contract parity', () => {
  const clientMethods = [...ALLOWED_NATIVE_METHODS];
  const serverMethods = extractQuotedMethods(nativeServerSource, /case\s+"([^"]+)"\s*:/g);
  const facadeMethods = extractQuotedMethods(
    typedFacadeSource,
    /request(?:<[\s\S]*?>)?\(\s*'([^']+)'/gs,
  );

  it('keeps every client-allowed operation implemented by the native server, with no extras', () => {
    expect(new Set(serverMethods).size).toBe(serverMethods.length);
    expect([...serverMethods].sort()).toEqual([...clientMethods].sort());
  });

  it('keeps every typed application operation inside the client allowlist', () => {
    expect(facadeMethods.length).toBeGreaterThan(0);
    expect(facadeMethods.every((method) => clientMethods.includes(method))).toBe(true);
  });
});
