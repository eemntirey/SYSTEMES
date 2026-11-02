import { useCallback } from 'react'
import { NativeBridge } from '../services/NativeBridge'
import {
  validateEntryName,
  confirmFileAction,
  existingNameConflict,
} from './fileSafety'

type Item = { id: string; name: string; path: string }

/**
 * Handlers sécurisés pour l'Explorateur de fichiers (P0.5).
 * Validation des noms, détection de conflits, confirmations avec chemin.
 */
export function useSafeFileOps(opts: {
  currentPath: string
  currentItems: Item[]
  selectedItem: Item | null
  renameValue: string
  setNativeError: (msg: string | null) => void
  setNewFolderName: (v: string) => void
  setIsCreatingFolder: (v: boolean) => void
  setNewFileName: (v: string) => void
  setIsCreatingFile: (v: boolean) => void
  setIsRenaming: (v: boolean) => void
  setRenameValue: (v: string) => void
  setSelectedItemId: (id: string | null) => void
  setPermissionText: (t: string) => void
  refreshDirectory: () => Promise<void>
}) {
  const {
    currentPath,
    currentItems,
    selectedItem,
    renameValue,
    setNativeError,
    setNewFolderName,
    setIsCreatingFolder,
    setNewFileName,
    setIsCreatingFile,
    setIsRenaming,
    setRenameValue,
    setSelectedItemId,
    setPermissionText,
    refreshDirectory,
  } = opts

  const createFolder = useCallback(
    async (nameRaw: string) => {
      const name = nameRaw.trim()
      const nameError = validateEntryName(name)
      if (nameError) {
        setNativeError(nameError)
        return false
      }
      const conflict = existingNameConflict(name, currentItems)
      if (conflict) {
        setNativeError(conflict)
        return false
      }
      if (!confirmFileAction('create-folder', { name, path: currentPath })) return false
      try {
        await NativeBridge.createDirectory(currentPath + '/' + name)
        setNewFolderName('')
        setIsCreatingFolder(false)
        setNativeError(null)
        await refreshDirectory()
        return true
      } catch (cause) {
        setNativeError(cause instanceof Error ? cause.message : String(cause))
        return false
      }
    },
    [currentPath, currentItems, setNativeError, setNewFolderName, setIsCreatingFolder, refreshDirectory],
  )

  const createFile = useCallback(
    async (nameRaw: string) => {
      const name = nameRaw.trim()
      const nameError = validateEntryName(name)
      if (nameError) {
        setNativeError(nameError)
        return false
      }
      const conflict = existingNameConflict(name, currentItems)
      if (conflict) {
        setNativeError(conflict)
        return false
      }
      if (!confirmFileAction('create-file', { name, path: currentPath })) return false
      try {
        await NativeBridge.createFile(currentPath + '/' + name, '')
        setNewFileName('')
        setIsCreatingFile(false)
        setNativeError(null)
        await refreshDirectory()
        return true
      } catch (cause) {
        setNativeError(cause instanceof Error ? cause.message : String(cause))
        return false
      }
    },
    [currentPath, currentItems, setNativeError, setNewFileName, setIsCreatingFile, refreshDirectory],
  )

  const rename = useCallback(async () => {
    if (!selectedItem || !renameValue.trim()) return false
    const nameError = validateEntryName(renameValue)
    if (nameError) {
      setNativeError(nameError)
      return false
    }
    if (renameValue.trim() !== selectedItem.name) {
      const conflict = existingNameConflict(
        renameValue,
        currentItems.filter(i => i.id !== selectedItem.id),
      )
      if (conflict) {
        setNativeError(conflict)
        return false
      }
    }
    if (
      !confirmFileAction('rename', {
        name: selectedItem.name,
        path: selectedItem.path,
        newName: renameValue.trim(),
      })
    ) {
      return false
    }
    try {
      await NativeBridge.renameFile(selectedItem.path, renameValue.trim())
      setIsRenaming(false)
      setRenameValue('')
      setSelectedItemId(null)
      setNativeError(null)
      await refreshDirectory()
      return true
    } catch (cause) {
      setNativeError(cause instanceof Error ? cause.message : String(cause))
      return false
    }
  }, [
    selectedItem,
    renameValue,
    currentItems,
    setNativeError,
    setIsRenaming,
    setRenameValue,
    setSelectedItemId,
    refreshDirectory,
  ])

  const deleteToRecycleBin = useCallback(async () => {
    if (!selectedItem) return false
    if (!confirmFileAction('delete', { name: selectedItem.name, path: selectedItem.path })) {
      return false
    }
    try {
      const result: any = await NativeBridge.deleteFile(selectedItem.path)
      setPermissionText(result?.note || 'Élément envoyé vers la Corbeille Windows.')
      setSelectedItemId(null)
      setNativeError(null)
      await refreshDirectory()
      return true
    } catch (cause) {
      setNativeError(cause instanceof Error ? cause.message : String(cause))
      return false
    }
  }, [selectedItem, setPermissionText, setSelectedItemId, setNativeError, refreshDirectory])

  return { createFolder, createFile, rename, deleteToRecycleBin }
}
