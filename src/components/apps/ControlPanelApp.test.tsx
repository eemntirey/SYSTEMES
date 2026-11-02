import React from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { ControlPanelApp } from './ControlPanelApp';
import { NativeBridge } from '../../services/NativeBridge';

vi.mock('../../services/NativeBridge', () => ({
  NativeBridge: {
    readServices: vi.fn(),
    controlService: vi.fn(),
  },
}));

const services = [
  {
    name: 'Spooler',
    displayName: 'Spouleur d’impression',
    state: 'Running',
    startMode: 'Auto',
    account: 'LocalSystem',
    processId: 1200,
    executablePath: 'C:/Windows/System32/spoolsv.exe',
    isProtected: false,
  },
  {
    name: 'RpcSs',
    displayName: 'Appel de procédure distante (RPC)',
    state: 'Running',
    startMode: 'Auto',
    account: 'NT AUTHORITY\\NetworkService',
    processId: 900,
    executablePath: 'C:/Windows/System32/svchost.exe',
    isProtected: true,
  },
  {
    name: 'WSearch',
    displayName: 'Windows Search',
    state: 'Stopped',
    startMode: 'Manual',
    account: 'LocalSystem',
    processId: 0,
    executablePath: 'C:/Windows/System32/SearchIndexer.exe',
    isProtected: false,
  },
];

describe('ControlPanelApp native Windows services', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(NativeBridge.readServices).mockResolvedValue({
      services,
      count: services.length,
      source: 'WMI Win32_Service',
      note: 'Inventaire réel',
    });
    vi.mocked(NativeBridge.controlService).mockResolvedValue({ changed: true });
  });

  it('loads the real service inventory and displays its source', async () => {
    render(<ControlPanelApp />);
    expect(await screen.findByText('Spouleur d’impression')).toBeTruthy();
    expect(screen.getByText(/WMI Win32_Service/)).toBeTruthy();
    expect(screen.getByText('3')).toBeTruthy();
  });

  it('filters services by system name or display name', async () => {
    render(<ControlPanelApp />);
    await screen.findByText('Spouleur d’impression');
    fireEvent.change(screen.getByRole('textbox', { name: 'Rechercher un service' }), {
      target: { value: 'WSearch' },
    });
    expect(screen.getByText('Windows Search')).toBeTruthy();
    expect(screen.queryByText('Spouleur d’impression')).toBeNull();
  });

  it('does not offer stop for protected services and allows starting stopped services', async () => {
    render(<ControlPanelApp />);
    await screen.findByText('Spouleur d’impression');
    expect(screen.getByTitle('Service système protégé')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Démarrer' })).toBeTruthy();
  });

  it('shows a native inventory error without fabricating service data', async () => {
    vi.mocked(NativeBridge.readServices).mockRejectedValueOnce(new Error('WMI indisponible'));
    render(<ControlPanelApp />);
    expect((await screen.findByRole('alert')).textContent).toContain('WMI indisponible');
    expect(screen.queryByText('Spouleur d’impression')).toBeNull();
  });
});
