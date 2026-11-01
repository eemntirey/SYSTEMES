import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { ProcessManagerApp } from './ProcessManagerApp'

const mocks = vi.hoisted(() => ({
  readProcesses: vi.fn(),
  readMemory: vi.fn(),
  killProcess: vi.fn(),
}))

vi.mock('../../services/NativeBridge', () => ({
  NativeBridge: mocks,
}))

beforeEach(() => {
  mocks.readMemory.mockResolvedValue({
    totalPhysicalBytes: 16 * 1024 ** 3,
    availablePhysicalBytes: 4 * 1024 ** 3,
    usedPhysicalBytes: 12 * 1024 ** 3,
    memoryLoadPercent: 75,
    totalPageFileBytes: 20 * 1024 ** 3,
    availablePageFileBytes: 8 * 1024 ** 3,
    totalVirtualBytes: 100 * 1024 ** 3,
    availableVirtualBytes: 80 * 1024 ** 3,
    sampleSource: 'Windows GlobalMemoryStatusEx',
    sampledAtUtc: '2026-11-01T12:00:00Z',
  })
})

afterEach(() => {
  vi.clearAllMocks()
  vi.restoreAllMocks()
})

describe('ProcessManagerApp', () => {
  it('renders the real process list returned by the native bridge', async () => {
    mocks.readProcesses.mockResolvedValue({
      count: 2,
      note: 'Liste réelle des processus Windows',
      processes: [
        { id: 100, name: 'editor', memoryMB: 125.5, cpuUsagePercent: 12.4, threadCount: 8, sessionId: 1, startTimeUtcTicks: '638975808000000000', executablePath: 'C:/Apps/editor.exe' },
        { id: 4, name: 'System', memoryMB: 20, cpuUsagePercent: null },
      ],
    })

    render(<ProcessManagerApp />)

    expect(await screen.findByText('editor')).toBeDefined()
    expect(screen.getByText('125,5 Mo')).toBeDefined()
    expect(screen.getByText('12 / 16 Gio')).toBeDefined()
    expect(screen.getByText('75 % utilisés · 4 Gio disponibles')).toBeDefined()
    expect(screen.getAllByText('12,4 %').length).toBeGreaterThan(0)
    expect(screen.getByText('8')).toBeDefined()
    expect(mocks.readProcesses).toHaveBeenCalledTimes(1)
  })

  it('requires confirmation before stopping a process and refreshes after success', async () => {
    mocks.readProcesses.mockResolvedValue({
      processes: [{ id: 100, name: 'editor', memoryMB: 125, startTimeUtcTicks: '638975808000000000' }],
    })
    mocks.killProcess.mockResolvedValue({ killed: true, pid: 100 })
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(true)

    render(<ProcessManagerApp />)
    fireEvent.click(await screen.findByRole('button', { name: 'Fin de tâche' }))

    await waitFor(() => expect(mocks.killProcess).toHaveBeenCalledWith(100, 'editor', '638975808000000000'))
    expect(confirm).toHaveBeenCalledTimes(1)
    expect(mocks.readProcesses).toHaveBeenCalledTimes(2)
  })

  it('sorts the list by CPU and shows selected process details', async () => {
    mocks.readProcesses.mockResolvedValue({
      logicalProcessorCount: 8,
      processes: [
        { id: 100, name: 'editor', memoryMB: 125, cpuUsagePercent: 7.5, threadCount: 8, sessionId: 1, startTimeUtcTicks: 638975808000000000, executablePath: 'C:/Apps/editor.exe' },
        { id: 101, name: 'worker', memoryMB: 90, cpuUsagePercent: 42, threadCount: 4, sessionId: 1 },
      ],
    })
    render(<ProcessManagerApp />)
    expect(await screen.findByText('worker')).toBeDefined()
    const processRows = screen.getAllByRole('row')
    expect(processRows[1].textContent).toContain('worker')
    fireEvent.click(screen.getByText('editor'))
    expect(screen.getByLabelText('Détails du processus sélectionné')).toBeDefined()
    expect(screen.getByText(/C:\/Apps\/editor.exe/)).toBeDefined()
    expect(screen.getByLabelText('Trier les processus')).toHaveProperty('value', 'cpu')
  })

  it('does not allow stopping protected Windows processes', async () => {
    mocks.readProcesses.mockResolvedValue({
      processes: [{ id: 4, name: 'System', memoryMB: 20 }],
    })

    render(<ProcessManagerApp />)
    const button = await screen.findByRole('button', { name: 'Fin de tâche' })
    expect(button).toHaveProperty('disabled', true)
    expect(mocks.killProcess).not.toHaveBeenCalled()
  })
})
