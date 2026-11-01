import { afterEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { OSProvider } from '../../context/OSContext';
import { SettingsApp } from './SettingsApp';

const nativeMocks = vi.hoisted(() => ({
  readHardware: vi.fn(),
  readDevices: vi.fn(),
  checkWindowsUpdate: vi.fn(),
}));

vi.mock('../../services/NativeBridge', () => ({
  NativeBridge: nativeMocks,
}));

const renderSettings = () => render(<OSProvider><SettingsApp /></OSProvider>);

afterEach(() => vi.clearAllMocks());

describe('Settings native integrations', () => {
  it('shows real hardware values returned by the native bridge', async () => {
    nativeMocks.readHardware.mockResolvedValue({
      cpuName: 'Test CPU 8-Core',
      cpuCores: 8,
      cpuThreads: 16,
      ramTotalGB: 32,
      gpuName: 'Test GPU',
      gpuMemoryBytes: 8 * 1024 * 1024 * 1024,
      disks: [{ name: 'C:', totalBytes: 512 * 1024 ** 3, availableBytes: 128 * 1024 ** 3, format: 'NTFS' }],
      windowsVersion: 'Microsoft Windows 11',
      note: 'Données réelles du test',
    });

    renderSettings();
    fireEvent.click(screen.getAllByText('À propos')[0]);

    expect(await screen.findByText('Test CPU 8-Core')).toBeDefined();
    expect(screen.getByText('Test GPU')).toBeDefined();
    expect(screen.getByText('32 Go')).toBeDefined();
    expect(nativeMocks.readHardware).toHaveBeenCalledTimes(1);
  });

  it('shows USB devices and active network adapters returned by Windows', async () => {
    nativeMocks.readDevices.mockResolvedValue({
      usb: ['Test USB device'],
      network: ['Test Ethernet adapter'],
      note: 'Détection WMI',
    });

    renderSettings();
    fireEvent.click(screen.getAllByText('Bluetooth et appareils')[0]);

    expect(await screen.findByText('Test USB device')).toBeDefined();
    expect(screen.getByText('Test Ethernet adapter')).toBeDefined();
    expect(nativeMocks.readDevices).toHaveBeenCalledTimes(1);
  });

  it('reports the Windows Update service state without claiming updates are installed', async () => {
    nativeMocks.checkWindowsUpdate.mockResolvedValue({
      serviceRunning: true,
      updatesAvailable: null,
      note: 'Service interrogé sur Windows',
    });

    renderSettings();
    fireEvent.click(screen.getAllByText('Windows Update')[0]);
    fireEvent.click(screen.getByText('Vérifier le service'));

    expect(await screen.findByText('Service Windows Update actif')).toBeDefined();
    expect(screen.getByText(/La recherche du catalogue de mises à jour/)).toBeDefined();
    expect(nativeMocks.checkWindowsUpdate).toHaveBeenCalledTimes(1);
  });
});
