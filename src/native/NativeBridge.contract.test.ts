import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { ALLOWED_NATIVE_METHODS } from './NativeBridge';

const readRepositoryFile = (path: string): string =>
  readFileSync(resolve(process.cwd(), path), 'utf8');

const extractQuotedMethods = (source: string, pattern: RegExp): string[] =>
  [...source.matchAll(pattern)].map((match) => match[1]);

describe('Native IPC contract parity', () => {
  const clientMethods = [...ALLOWED_NATIVE_METHODS];

  const serverSource = readRepositoryFile('native/Windver/Core/NativeServer.cs');
  const serverMethods = extractQuotedMethods(serverSource, /case\s+"([^"]+)"\s*:/g);

  const facadeSource = readRepositoryFile('src/services/NativeBridge.ts');
  const facadeMethods = extractQuotedMethods(
    facadeSource,
    /request(?:<[^;]*?>)?\(\s*'([^']+)'/gs,
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
