import { afterEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { OSProvider } from '../../context/OSContext'
import { FileExplorerApp } from './FileExplorerApp'

const mocks = vi.hoisted(() => ({
  listFiles: vi.fn(),
  getFileLocations: vi.fn(),
  readHardware: vi.fn(),
  readTextFile: vi.fn(),
  createDirectory: vi.fn(),
  createFile: vi.fn(),
  deleteFile: vi.fn(),
  openRecycleBin: vi.fn(),
  renameFile: vi.fn(),
}))

vi.mock('../../services/NativeBridge', () => ({
  NativeBridge: mocks,
}))

const renderExplorer = () => render(<OSProvider><FileExplorerApp /></OSProvider>)

afterEach(() => vi.clearAllMocks())

describe('FileExplorer native integration', () => {
  it('opens the native Windows Recycle Bin', async () => {
    mocks.listFiles.mockResolvedValue({ path: 'C:/Users/test', count: 0, items: [] })
    mocks.getFileLocations.mockResolvedValue({ locations: [] })
    mocks.readHardware.mockResolvedValue({ disks: [] })
    mocks.openRecycleBin.mockResolvedValue({ opened: true, target: 'shell:RecycleBinFolder' })

    renderExplorer()
    fireEvent.click(await screen.findByRole('button', { name: 'Corbeille' }))

    expect(mocks.openRecycleBin).toHaveBeenCalledTimes(1)
  })

  it('lists actual directory entries returned by the native host', async () => {
    mocks.listFiles.mockResolvedValue({
      path: 'C:/Users/test',
      count: 1,
      items: [{
        id: 'C:/Users/test/readme.txt',
        name: 'readme.txt',
        type: 'file',
        path: 'C:/Users/test/readme.txt',
        sizeBytes: 1024,
        modified: '2026-11-01 08:00:00',
        extension: 'txt',
      }],
    })
    mocks.getFileLocations.mockResolvedValue({ locations: [{ name: 'Accueil', path: 'C:/Users/test', icon: 'home' }] })
    mocks.readHardware.mockResolvedValue({ disks: [{ name: 'C:/', totalBytes: 100 * 1024 ** 3, availableBytes: 25 * 1024 ** 3, format: 'NTFS' }] })

    renderExplorer()

    expect(await screen.findByText('readme.txt')).toBeDefined()
    expect(screen.getByText('1.0 Ko')).toBeDefined()
    expect(await screen.findByText('C:/')).toBeDefined()
    expect(screen.getByText(/25 Gio libres \/ 100 Gio/)).toBeDefined()
    expect(mocks.listFiles).toHaveBeenCalledWith('')
    expect(mocks.getFileLocations).toHaveBeenCalled()
  })

  it('creates a real directory and refreshes the current listing', async () => {
    mocks.listFiles.mockResolvedValue({
      path: 'C:/Users/test',
      count: 0,
      items: [],
    })
    mocks.getFileLocations.mockResolvedValue({ locations: [] })
    mocks.readHardware.mockResolvedValue({ disks: [] })
    mocks.createDirectory.mockResolvedValue({ created: true })

    renderExplorer()
    fireEvent.click(await screen.findByRole('button', { name: 'Nouveau dossier' }))
    const input = await screen.findByPlaceholderText('Nom du nouveau dossier...')
    fireEvent.change(input, { target: { value: 'NativeFolder' } })
    fireEvent.submit(input.closest('form') as HTMLFormElement)

    await waitFor(() => expect(mocks.createDirectory).toHaveBeenCalledWith('C:/Users/test/NativeFolder'))
    await waitFor(() => expect(mocks.listFiles).toHaveBeenCalledTimes(3))
  })
  it('renames a selected native file and refreshes the directory', async () => {
    mocks.listFiles.mockResolvedValue({
      path: 'C:/Users/test',
      count: 1,
      items: [{ id: 'C:/Users/test/old.txt', name: 'old.txt', type: 'file', path: 'C:/Users/test/old.txt', sizeBytes: 5, modified: '2026-11-01 08:00:00', extension: 'txt' }],
    })
    mocks.getFileLocations.mockResolvedValue({ locations: [] })
    mocks.readTextFile.mockResolvedValue({ content: 'Preview text' })
    mocks.renameFile.mockResolvedValue({ renamed: true, path: 'C:/Users/test/new.txt' })

    renderExplorer()
    fireEvent.click(await screen.findByText('old.txt'))
    fireEvent.click(screen.getByRole('button', { name: 'Inspecteur & Aperçu' }))
    fireEvent.click(await screen.findByRole('button', { name: 'Renommer' }))
    fireEvent.change(screen.getByRole('textbox', { name: 'Nouveau nom' }), { target: { value: 'new.txt' } })
    fireEvent.click(screen.getByRole('button', { name: 'Valider' }))

    await waitFor(() => expect(mocks.renameFile).toHaveBeenCalledWith('C:/Users/test/old.txt', 'new.txt'))
    await waitFor(() => expect(mocks.listFiles).toHaveBeenCalledTimes(3))
  })

})
