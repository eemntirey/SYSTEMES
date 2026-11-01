import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';
import { OSProvider } from '../../context/OSContext';
import { SettingsApp } from './SettingsApp';

const nativeMocks = vi.hoisted(() => ({
  readHardware: vi.fn(),
  readDevices: vi.fn(),
  checkWindowsUpdate: vi.fn(),
  openWindowsSettings: vi.fn(),
  readPowerSchemes: vi.fn(),
  setPowerScheme: vi.fn(),
  readBluetoothStatus: vi.fn(),
  scanBluetooth: vi.fn(),
  pairBluetoothDevice: vi.fn(),
  readWifiStatus: vi.fn(),
  scanWifi: vi.fn(),
  connectWifi: vi.fn(),
  disconnectWifi: vi.fn(),
  readServices: vi.fn(),
  controlService: vi.fn(),
}));

vi.mock('../../services/NativeBridge', () => ({
  NativeBridge: nativeMocks,
}));

const renderSettings = () => render(<OSProvider><SettingsApp /></OSProvider>);

beforeEach(() => {
  nativeMocks.readBluetoothStatus.mockResolvedValue({ adapterPresent: false, powered: false, devices: [] });
  nativeMocks.readWifiStatus.mockResolvedValue({ connected: false });
  nativeMocks.scanWifi.mockResolvedValue({ networks: [] });
  nativeMocks.readServices.mockResolvedValue({ services: [], count: 0 });
});

afterEach(() => { vi.restoreAllMocks(); vi.clearAllMocks(); });

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

    nativeMocks.openWindowsSettings.mockResolvedValue({ opened: true, page: 'windows-update' });

    renderSettings();
    fireEvent.click(screen.getAllByText('Windows Update')[0]);
    fireEvent.click(screen.getByText('Vérifier le service'));
    fireEvent.click(screen.getByText('Ouvrir Windows Update'));

    expect(await screen.findByText('Service Windows Update actif')).toBeDefined();
    expect(nativeMocks.openWindowsSettings).toHaveBeenCalledWith('windows-update');
    expect(screen.getByText(/La recherche du catalogue de mises à jour/)).toBeDefined();
    expect(nativeMocks.checkWindowsUpdate).toHaveBeenCalledTimes(1);
  });
  it('opens real Windows privacy controls instead of changing a local mock toggle', async () => {
    nativeMocks.openWindowsSettings.mockResolvedValue({ opened: true, page: 'privacy-location' })

    renderSettings()
    fireEvent.click(screen.getAllByText('Confidentialité et sécurité')[0])
    fireEvent.click(await screen.findByText('Position'))

    expect(nativeMocks.openWindowsSettings).toHaveBeenCalledWith('privacy-location')
  })

  it('shows real active network adapters in the network settings section', async () => {
    nativeMocks.readDevices.mockResolvedValue({
      usb: [],
      network: ['Real Wi-Fi adapter'],
      note: 'WMI network adapter enumeration',
    })

    renderSettings()
    fireEvent.click(screen.getAllByText('Réseau et Internet')[0])

    expect(await screen.findByText('Real Wi-Fi adapter')).toBeDefined()
    expect(nativeMocks.readDevices).toHaveBeenCalledTimes(1)
  })

  it('lists Windows services and confirms an explicit stop operation', async () => {
    nativeMocks.readServices
      .mockResolvedValueOnce({ services: [{ name: 'W32Time', displayName: 'Windows Time', state: 'Running', startMode: 'Auto', processId: 100 }] })
      .mockResolvedValueOnce({ services: [{ name: 'W32Time', displayName: 'Windows Time', state: 'Stopped', startMode: 'Auto', processId: 0 }] })
    nativeMocks.controlService.mockResolvedValue({ changed: true, name: 'W32Time', state: 'Stopped' })
    vi.spyOn(window, 'confirm').mockReturnValue(true)

    renderSettings()
    fireEvent.click(screen.getAllByText('Système')[0])
    expect(await screen.findByText(/Windows Time/)).toBeDefined()
    fireEvent.click(await screen.findByRole('button', { name: 'Arrêter' }))
    expect(nativeMocks.controlService).toHaveBeenCalledWith('W32Time', 'stop')
    expect(await screen.findByText(/État de « Windows Time » confirmé/)).toBeDefined()
  })

  it('reads and applies a real Windows power scheme through the native engine', async () => {
    const balanced = '381b4222-f694-41f0-9685-ff5bb260df2e'
    const performance = 'e9a42b02-d5df-448d-aa00-03f14749eb61'
    nativeMocks.readPowerSchemes
      .mockResolvedValueOnce({
        schemes: [{ guid: balanced, name: 'Équilibré' }, { guid: performance, name: 'Performances élevées' }],
        activeGuid: balanced,
        note: 'Lecture powercfg réelle',
      })
      .mockResolvedValueOnce({
        schemes: [{ guid: balanced, name: 'Équilibré' }, { guid: performance, name: 'Performances élevées' }],
        activeGuid: performance,
        note: 'Plan relu après application',
      })
    nativeMocks.setPowerScheme.mockResolvedValue({ changed: true, activeGuid: performance, activeName: 'Performances élevées' })
    vi.spyOn(window, 'confirm').mockReturnValue(true)

    renderSettings()
    fireEvent.click(screen.getAllByText('Système')[0])
    const select = await screen.findByLabelText('Plan d’alimentation Windows')
    fireEvent.change(select, { target: { value: performance } })
    fireEvent.click(screen.getByRole('button', { name: 'Appliquer le plan à Windows' }))

    expect(nativeMocks.setPowerScheme).toHaveBeenCalledWith(performance)
    expect(await screen.findByText(/Plan actif : Performances élevées/)).toBeDefined()
  })

});
