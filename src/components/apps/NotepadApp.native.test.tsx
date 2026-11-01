import { afterEach, describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { OSProvider } from '../../context/OSContext'
import { NotepadApp } from './NotepadApp'

const mocks = vi.hoisted(() => ({
  writeTextFile: vi.fn(),
  createFile: vi.fn(),
}))

vi.mock('../../services/NativeBridge', () => ({
  NativeBridge: mocks,
}))

afterEach(() => vi.clearAllMocks())

describe('Notepad native file saving', () => {
  it('writes edited content to the real file path for native files', async () => {
    mocks.writeTextFile.mockResolvedValue({ written: true })

    render(
      <OSProvider>
        <NotepadApp nativeFile filePath="C:/Users/test/notes.txt" fileContent="Initial" />
      </OSProvider>,
    )

    fireEvent.change(screen.getByPlaceholderText('Commencez à taper votre texte ici...'), {
      target: { value: 'Saved to Windows' },
    })
    fireEvent.click(screen.getByRole('button', { name: /Enregistrer/ }))

    await waitFor(() => expect(mocks.writeTextFile).toHaveBeenCalledWith(
      'C:/Users/test/notes.txt',
      'Saved to Windows',
    ))
  })
})
