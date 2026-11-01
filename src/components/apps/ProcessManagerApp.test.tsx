import React from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { ProcessManagerApp } from './ProcessManagerApp'

const mocks = vi.hoisted(() => ({
  readProcesses: vi.fn(),
  killProcess: vi.fn(),
}))

vi.mock('../../services/NativeBridge', () => ({
  NativeBridge: mocks,
}))

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
        { id: 100, name: 'editor', memoryMB: 125.5, cpuUsagePercent: null },
        { id: 4, name: 'System', memoryMB: 20, cpuUsagePercent: null },
      ],
    })

    render(<ProcessManagerApp />)

    expect(await screen.findByText('editor')).toBeDefined()
    expect(screen.getByText('125,5 Mo')).toBeDefined()
    expect(mocks.readProcesses).toHaveBeenCalledTimes(1)
  })

  it('requires confirmation before stopping a process and refreshes after success', async () => {
    mocks.readProcesses.mockResolvedValue({
      processes: [{ id: 100, name: 'editor', memoryMB: 125 }],
    })
    mocks.killProcess.mockResolvedValue({ killed: true, pid: 100 })
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(true)

    render(<ProcessManagerApp />)
    fireEvent.click(await screen.findByRole('button', { name: 'Fin de tâche' }))

    await waitFor(() => expect(mocks.killProcess).toHaveBeenCalledWith(100))
    expect(confirm).toHaveBeenCalledTimes(1)
    expect(mocks.readProcesses).toHaveBeenCalledTimes(2)
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
