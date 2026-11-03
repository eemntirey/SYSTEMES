import React, { useEffect, useMemo, useState } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'
import { FileItem } from '../../types'
import { NativeBridge } from '../../services/NativeBridge'
import { useSafeFileOps } from '../../utils/useSafeFileOps'

interface FileExplorerAppProps {
  initialPath?: string
}

const NavButton: React.FC<{
  icon: string
  label: string
  onClick: () => void
  disabled?: boolean
}> = ({ icon, label, onClick, disabled = false }) => (
  <button
    type="button"
    title={label}
    aria-label={label}
    onClick={onClick}
    disabled={disabled}
    style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      width: 34,
      height: 34,
      borderRadius: 'var(--radius-sm)',
      border: 'none',
      background: disabled ? 'var(--control-hover)' : 'transparent',
      color: disabled ? 'var(--text-tertiary)' : 'var(--text-primary)',
      cursor: disabled ? 'default' : 'pointer',
      transition: 'background 0.12s, color 0.12s',
    }}
  >
    <FluentIcon name={icon} size={17} />
  </button>
)

const ActionButton: React.FC<{
  icon: string
  label: string
  onClick: () => void
}> = ({ icon, label, onClick }) => (
  <button
    type="button"
    title={label}
    onClick={onClick}
    style={{
      display: 'flex',
      alignItems: 'center',
      gap: '6px',
      padding: '7px 12px',
      borderRadius: 'var(--radius-sm)',
      border: 'none',
      background: 'var(--flyout-card)',
      color: 'var(--text-primary)',
      fontWeight: 600,
      fontSize: '12px',
      cursor: 'pointer',
      transition: 'background 0.12s, border-color 0.12s',
    }}
  >
    <FluentIcon name={icon} size={17} />
    <span>{label}</span>
  </button>
)

const ToggleButton: React.FC<{
  icon: string
  label: string
  active: boolean
  onClick: () => void
}> = ({ icon, label, active, onClick }) => (
  <button
    type="button"
    title={label}
    onClick={onClick}
    style={{
      display: 'flex',
      alignItems: 'center',
      gap: '6px',
      padding: '7px 12px',
      borderRadius: 'var(--radius-sm)',
      border: 'none',
      background: active ? 'rgba(139, 92, 246, 0.18)' : 'var(--flyout-card)',
      color: active ? 'var(--win-accent)' : 'var(--text-secondary)',
      fontWeight: 600,
      fontSize: '11.5px',
      cursor: 'pointer',
      transition: 'background 0.12s, border-color 0.12s',
    }}
  >
    <FluentIcon name={icon} size={16} />
    <span>{label}</span>
  </button>
)

const FileCard: React.FC<{
  item: FileItem
  isSelected: boolean
  isFav: boolean
  onSelect: () => void
  onToggleFav: (e: React.MouseEvent) => void
  onDoubleClick: () => void
}> = ({ item, isSelected, isFav, onSelect, onToggleFav, onDoubleClick }) => (
  <div
    className={`obsidian-explorer-file${isSelected ? ' is-selected' : ''}`}
    onClick={onSelect}
    onDoubleClick={onDoubleClick}
    style={{
      position: 'relative',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      padding: '18px 14px',
      borderRadius: 'var(--radius-sm)',
      cursor: 'pointer',
      background: isSelected ? 'rgba(139, 92, 246, 0.28)' : 'var(--flyout-card)',
      border: 'none',
      boxShadow: isSelected ? '0 0 14px rgba(0, 212, 255, 0.25)' : 'none',
      transition: 'background 0.14s, border-color 0.14s, box-shadow 0.14s',
    }}
    onMouseEnter={(e) => {
      if (!isSelected) {
        e.currentTarget.style.background = 'var(--flyout-card-hover)'
      }
    }}
    onMouseLeave={(e) => {
      if (!isSelected) {
        e.currentTarget.style.background = 'var(--flyout-card)'
      }
    }}
  >
    <button
      type="button"
      onClick={(e) => onToggleFav(e)}
      style={{
        position: 'absolute',
        top: 8,
        right: 8,
        border: 'none',
        background: 'transparent',
        color: isFav ? '#fbbf24' : 'rgba(255,255,255,0.2)',
        cursor: 'pointer',
        fontSize: '15px',
      }}
    >
      <FluentIcon name="star" size={15} />
    </button>
    <FluentIcon name={item.type === 'folder' ? 'folder' : 'file-text'} size={44} />
    <span
      style={{
        fontSize: '12px',
        fontWeight: 500,
        textAlign: 'center',
        marginTop: '10px',
        wordBreak: 'break-word',
        color: isSelected ? '#ffffff' : 'var(--text-primary)',
        maxHeight: '40px',
        overflow: 'hidden',
        lineHeight: 1.2,
      }}
    >
      {item.name}
    </span>
    <span style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginTop: '6px' }}>
      {item.type === 'folder' ? 'Dossier' : (item.size || '—')}
    </span>
  </div>
)

export const FileExplorerApp: React.FC<FileExplorerAppProps> = ({
  initialPath = '',
}) => {
  const { openWindow } = useOS()

  const [currentPath, setCurrentPath] = useState<string>(initialPath)
  const [nativeFiles, setNativeFiles] = useState<FileItem[]>([])
  const [nativeLocations, setNativeLocations] = useState<{ name: string; path: string; icon?: string }[]>([])
  const [nativeDisks, setNativeDisks] = useState<{ name: string; totalBytes: number; availableBytes: number; format: string }[]>([])
  const [nativeError, setNativeError] = useState<string | null>(null)
  const [isNativeLoading, setIsNativeLoading] = useState(true)
  const [isRenaming, setIsRenaming] = useState(false)
  const [renameValue, setRenameValue] = useState('')
  const [permissionText, setPermissionText] = useState<string | null>(null)
  const [history, setHistory] = useState<string[]>([initialPath])
  const [historyIndex, setHistoryIndex] = useState(0)
  const [searchQuery, setSearchQuery] = useState('')
  const [selectedItemId, setSelectedItemId] = useState<string | null>(null)
  const [isCreatingFolder, setIsCreatingFolder] = useState(false)
  const [newFolderName, setNewFolderName] = useState('')
  const [isCreatingFile, setIsCreatingFile] = useState(false)
  const [newFileName, setNewFileName] = useState('')
  const [showInspector, setShowInspector] = useState(false)
  const [favorites, setFavorites] = useState<string[]>(['doc-welcome'])
  const [clipboard, setClipboard] = useState<{ path: string; name: string; mode: 'copy' | 'move' } | null>(null)
  const [sortBy, setSortBy] = useState<'name' | 'type' | 'modified'>('name')

  const refreshDirectory = async (path = currentPath) => {
    setIsNativeLoading(true)
    setNativeError(null)
    try {
      const result = await NativeBridge.listFiles(path) as {
        path?: string
        items?: { id: string; name: string; type: 'file' | 'folder'; path: string; sizeBytes?: number | null; modified?: string; extension?: string | null }[]
      }
      if (!result || !Array.isArray(result.items) || !result.path) throw new Error('Le moteur Windows a retourné un contenu de dossier invalide.')
      setNativeFiles(result.items.map(item => ({
        id: item.id,
        name: item.name,
        type: item.type,
        path: item.path,
        size: item.type === 'folder' ? 'Dossier' : (item.sizeBytes == null ? '—' : item.sizeBytes < 1024 ? item.sizeBytes + ' o' : (item.sizeBytes / 1024).toFixed(1) + ' Ko'),
        modified: item.modified || '—',
        extension: item.extension || undefined,
      })))
      if (!path) {
        setCurrentPath(initialPath || result.path)
        setHistory([initialPath || result.path])
        setHistoryIndex(0)
      }
    } catch (cause) {
      setNativeError(cause instanceof Error ? cause.message : String(cause))
      setNativeFiles([])
    } finally {
      setIsNativeLoading(false)
    }
  }

  useEffect(() => {
    void refreshDirectory(currentPath)
  }, [currentPath])

  useEffect(() => {
    void NativeBridge.getFileLocations().then((result: any) => {
      if (Array.isArray(result?.locations)) setNativeLocations(result.locations)
    }).catch((cause: unknown) => {
      setNativeError(cause instanceof Error ? cause.message : String(cause))
    })
    void NativeBridge.readHardware().then((result: any) => {
      if (Array.isArray(result?.disks)) setNativeDisks(result.disks.filter((disk: any) => disk && typeof disk.name === 'string' && Number.isFinite(disk.totalBytes) && Number.isFinite(disk.availableBytes)))
    }).catch((cause: unknown) => {
      setNativeError(cause instanceof Error ? cause.message : String(cause))
    })
  }, [])

  const navigateTo = (path: string) => {
    const formatted = path.replace(/\/+/g, '/')
    const newHist = history.slice(0, historyIndex + 1)
    newHist.push(formatted)
    setHistory(newHist)
    setHistoryIndex(newHist.length - 1)
    setCurrentPath(formatted)
    setSelectedItemId(null)
  }

  const goBack = () => {
    if (historyIndex > 0) {
      const newIdx = historyIndex - 1
      setHistoryIndex(newIdx)
      setCurrentPath(history[newIdx])
      setSelectedItemId(null)
    }
  }

  const goForward = () => {
    if (historyIndex < history.length - 1) {
      const newIdx = historyIndex + 1
      setHistoryIndex(newIdx)
      setCurrentPath(history[newIdx])
      setSelectedItemId(null)
    }
  }

  const goUp = () => {
    const parts = currentPath.split('/')
    if (parts.length > 1) {
      parts.pop()
      navigateTo(parts.join('/'))
    }
  }

  const currentItems = useMemo(() => {
    const visible = nativeFiles.filter((f) => {
      const parts = f.path.split('/')
      parts.pop()
      const parent = parts.join('/')
      const inCurrentDir =
        parent === currentPath ||
        (f.path.startsWith(currentPath + '/') &&
          f.path.split('/').length === currentPath.split('/').length + 1)
      if (!inCurrentDir) return false
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase()
        const matchName = f.name.toLowerCase().includes(q)
        const matchContent = f.content?.toLowerCase().includes(q)
        if (!matchName && !matchContent) return false
      }
      return true
    })
    return [...visible].sort((a, b) => {
      if (sortBy === 'type') return a.type.localeCompare(b.type) || a.name.localeCompare(b.name)
      if (sortBy === 'modified') return b.modified.localeCompare(a.modified) || a.name.localeCompare(b.name)
      return a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' })
    })
  }, [nativeFiles, currentPath, searchQuery, sortBy])

  const selectedItem = useMemo(
    () => nativeFiles.find((f) => f.id === selectedItemId) || null,
    [nativeFiles, selectedItemId],
  )

  const safeFileOps = useSafeFileOps({
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
  })

  const handleItemDoubleClick = (item: FileItem) => {
    if (item.type === 'folder') {
      navigateTo(item.path)
    } else {
      void NativeBridge.readTextFile(item.path).then((result: any) => openWindow('notepad', { fileId: item.id, filePath: item.path, fileContent: result?.content || '' }, `${item.name} - Bloc-notes`)).catch((cause: unknown) => setNativeError(cause instanceof Error ? cause.message : String(cause)))
    }
  }

  const toggleFavorite = (id: string, e: React.MouseEvent) => {
    e.stopPropagation()
    setFavorites((prev) => (prev.includes(id) ? prev.filter((favId) => favId !== id) : [...prev, id]))
  }

  const handleCreateFolderSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    void safeFileOps.createFolder(newFolderName)
  }

  const handleCreateFileSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    void safeFileOps.createFile(newFileName)
  }

  const handleRename = async () => {
    await safeFileOps.rename()
  }

  const pasteClipboard = async () => {
    if (!clipboard) return
    const parent = currentPath.replace(/\/+$/, '')
    const destination = `${parent}/${clipboard.name}`
    setNativeError(null)
    try {
      if (clipboard.mode === 'copy') {
        await NativeBridge.copyFile(clipboard.path, destination)
      } else {
        await NativeBridge.moveFile(clipboard.path, destination)
        setClipboard(null)
      }
      setSelectedItemId(null)
      await refreshDirectory()
    } catch (cause) {
      setNativeError(cause instanceof Error ? cause.message : String(cause))
    }
  }


  return (
    <div
      className="obsidian-file-explorer"
      style={{
        display: 'flex',
        flexDirection: 'column',
        height: '100%',
        backgroundColor: 'var(--window-bg)',
        color: 'var(--text-primary)',
        fontFamily: 'var(--font-segoe)',
      }}
    >
      <div
        className="obsidian-explorer-toolbar"
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '10px',
          padding: '8px 14px',
          background: 'transparent',
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: '2px' }}>
          <NavButton icon="arrow-left" label="Précédent" onClick={goBack} disabled={historyIndex === 0} />
          <NavButton icon="arrow-right" label="Suivant" onClick={goForward} disabled={historyIndex >= history.length - 1} />
          <NavButton icon="arrow-up" label="Monter" onClick={goUp} />
        </div>

        <div className="obsidian-explorer-location" style={{ flex: 1, display: 'flex', alignItems: 'center', gap: '10px', minWidth: 0 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '4px', minWidth: 0 }}>
            <FluentIcon name="pc" size={15} style={{ flexShrink: 0, color: 'var(--text-tertiary)' }} />
            <div style={{ flex: 1, minWidth: 0, display: 'flex', alignItems: 'center', gap: '4px' }}>
              {currentPath.split('/').map((seg, i, arr) => (
                <React.Fragment key={i}>
                  <span
                    onClick={() => navigateTo(arr.slice(0, i + 1).join('/'))}
                    style={{
                      cursor: 'pointer',
                      color: i === arr.length - 1 ? 'var(--win-accent-cyan)' : 'var(--text-secondary)',
                      fontWeight: i === arr.length - 1 ? 600 : 400,
                      overflow: 'hidden',
                      textOverflow: 'ellipsis',
                      whiteSpace: 'nowrap',
                    }}
                  >
                    {seg}
                  </span>
                  {i < arr.length - 1 && <span style={{ color: 'var(--text-tertiary)', fontSize: 11 }}>/</span>}
                </React.Fragment>
              ))}
            </div>
          </div>

          <div className="obsidian-explorer-search" style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            padding: '6px 12px',
            borderRadius: 'var(--radius-sm)',
            background: 'var(--flyout-card)',
            border: 'none',
            flex: '1 1 220px',
          }}>
            <FluentIcon name="search" size={16} style={{ color: 'var(--text-tertiary)' }} />
            <input
              type="text"
              placeholder="Rechercher un fichier ou dossier..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              style={{
                flex: 1,
                background: 'transparent',
                border: 'none',
                outline: 'none',
                color: 'var(--text-primary)',
                fontSize: '12.5px',
              }}
            />
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
          <label htmlFor="explorer-sort" style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Trier</label>
          <select id="explorer-sort" aria-label="Trier les éléments" value={sortBy} onChange={e => setSortBy(e.target.value as 'name' | 'type' | 'modified')} style={{ maxWidth: 112, padding: '6px 8px', borderRadius: 6, border: '1px solid var(--window-border)', background: 'var(--flyout-card)', color: 'var(--text-primary)', fontSize: 11 }}>
            <option value="name">Nom</option>
            <option value="type">Type</option>
            <option value="modified">Date</option>
          </select>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: '4px' }}>
          {selectedItem && <>
            <ActionButton icon="copy" label="Copier" onClick={() => setClipboard({ path: selectedItem.path, name: selectedItem.name, mode: 'copy' })} />
            <ActionButton icon="scissors" label="Couper" onClick={() => setClipboard({ path: selectedItem.path, name: selectedItem.name, mode: 'move' })} />
          </>}
          {clipboard && <ActionButton icon="clipboard" label={`Coller · ${clipboard.name}`} onClick={() => void pasteClipboard()} />}
          <ActionButton icon="trash" label="Corbeille" onClick={() => { void NativeBridge.openRecycleBin().catch((cause: unknown) => setNativeError(cause instanceof Error ? cause.message : String(cause))) }} />
          <ActionButton icon="folder" label="Nouveau dossier" onClick={() => setIsCreatingFolder(true)} />
          <ActionButton icon="file-text" label="Nouveau fichier" onClick={() => setIsCreatingFile(true)} />
          <ToggleButton icon="sliders" label="Inspecteur & Aperçu" active={showInspector} onClick={() => setShowInspector((v) => !v)} />
        </div>
      </div>

        <div className="obsidian-explorer-workspace" style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
          <div
            className="obsidian-explorer-sidebar"
            style={{
              width: 196,
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
              background: 'rgba(0, 0, 0, 0.18)',
            }}
          >
            <div style={{ padding: '10px 8px 6px 8px' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', padding: '4px 8px', fontSize: '11px', fontWeight: 700, letterSpacing: '0.5px', color: 'var(--text-tertiary)' }}>
                <FluentIcon name="star" size={14} />
                <span>Favoris</span>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '3px', padding: '2px 6px' }}>
                {nativeLocations.map((item) => (
                  <div
                    key={item.path}
                    onClick={() => navigateTo(item.path)}
                    style={{
                      display: 'flex',
                      alignItems: 'center',
                      gap: '8px',
                      padding: '6px 8px',
                      borderRadius: 'var(--radius-sm)',
                      cursor: 'pointer',
                      background: currentPath === item.path ? 'rgba(139, 92, 246, 0.22)' : 'transparent',
                      border: 'none',
                      color: currentPath === item.path ? 'var(--win-accent-cyan)' : 'var(--text-primary)',
                      fontSize: '12px',
                      fontWeight: currentPath === item.path ? 600 : 400,
                      transition: 'background 0.12s, border-color 0.12s',
                    }}
                    onMouseEnter={(e) => {
                      if (currentPath !== item.path) e.currentTarget.style.background = 'var(--control-hover)'
                    }}
                    onMouseLeave={(e) => {
                      if (currentPath !== item.path) e.currentTarget.style.background = 'transparent'
                    }}
                  >
                    <FluentIcon name={item.icon || "folder"} size={16} />
                    <span>{item.name}</span>
                  </div>
                ))}
              </div>
            </div>

            <div style={{ padding: '10px 8px 8px 8px', marginTop: 'auto' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '6px', padding: '4px 8px', fontSize: '11px', fontWeight: 700, letterSpacing: '0.5px', color: 'var(--text-tertiary)' }}>
                <FluentIcon name="pc" size={14} />
                <span>Stockage</span>
              </div>
              {nativeDisks.map(disk => {
                const usedBytes = Math.max(0, disk.totalBytes - disk.availableBytes)
                const usedPercent = disk.totalBytes > 0 ? Math.min(100, Math.round(usedBytes / disk.totalBytes * 100)) : 0
                const formatSize = (bytes: number) => (bytes / 1024 ** 3).toLocaleString('fr-FR', { maximumFractionDigits: 1 }) + ' Gio'
                return <div key={disk.name} role="button" tabIndex={0} onClick={() => navigateTo(disk.name)} onKeyDown={event => { if (event.key === 'Enter') navigateTo(disk.name) }} style={{ padding: 10, marginBottom: 6, borderRadius: 'var(--radius-sm)', background: 'var(--flyout-card)', cursor: 'pointer' }}>
                  <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                    <FluentIcon name="pc" size={20} />
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontWeight: 600, color: 'var(--text-primary)', fontSize: 12 }}>{disk.name}</div>
                      <div style={{ height: 4, borderRadius: 2, background: 'rgba(255,255,255,0.08)', overflow: 'hidden', marginTop: 5 }}>
                        <div style={{ height: '100%', width: usedPercent + '%', background: 'var(--win-accent-cyan)' }} />
                      </div>
                      <div style={{ fontSize: 10, color: 'var(--text-tertiary)', marginTop: 4 }}>{formatSize(disk.availableBytes)} libres / {formatSize(disk.totalBytes)} · {disk.format}</div>
                    </div>
                  </div>
                </div>
              })}
              {!nativeDisks.length && <div style={{ padding: 8, fontSize: 10, color: 'var(--text-tertiary)' }}>Volumes Windows indisponibles.</div>}
            </div>
          </div>

        <div className="obsidian-explorer-content" style={{ flex: 1, overflow: 'auto', padding: 24, display: 'flex', flexDirection: 'column' }}>
          {nativeError && <div role="alert" style={{ padding: 10, marginBottom: 12, borderRadius: 8, background: 'rgba(185,28,28,.18)', color: '#fecaca', fontSize: 12 }}>{nativeError}</div>}
          {isNativeLoading && <div role="status" style={{ padding: 8, color: 'var(--text-tertiary)', fontSize: 11 }}>Lecture du dossier Windows…</div>}
          {isCreatingFolder && (
            <form onSubmit={handleCreateFolderSubmit} onClick={(e) => e.stopPropagation()} style={{ display: 'flex', alignItems: 'center', gap: '10px', padding: '12px 16px', borderRadius: 'var(--radius-sm)', background: 'rgba(139, 92, 246, 0.2)', border: 'none' }}>
              <FluentIcon name="folder" size={26} />
              <input autoFocus type="text" value={newFolderName} onChange={(e) => setNewFolderName(e.target.value)} placeholder="Nom du nouveau dossier..." style={{ flex: 1, padding: '6px 10px',                 borderRadius: '4px', border: 'none', background: 'var(--flyout-input-bg)', color: 'var(--text-primary)', outline: 'none', fontSize: '13px' }} />
              <button type="submit" onMouseDown={(e) => e.preventDefault()} style={{ padding: '6px 14px', borderRadius: '4px', border: 'none', background: 'var(--win-accent-cyan)', color: '#000', fontWeight: 600, fontSize: '12px', cursor: 'pointer' }}>Créer</button>
            </form>
          )}

          {isCreatingFile && (
            <form onSubmit={handleCreateFileSubmit} onClick={(e) => e.stopPropagation()} style={{ display: 'flex', alignItems: 'center', gap: '10px', padding: '12px 16px', borderRadius: 'var(--radius-sm)', background: 'rgba(139, 92, 246, 0.2)', border: 'none' }}>
              <FluentIcon name="file-text" size={26} />
              <input autoFocus type="text" value={newFileName} onChange={(e) => setNewFileName(e.target.value)} placeholder="nouveau_fichier.txt..." style={{ flex: 1, padding: '6px 10px',                 borderRadius: '4px', border: 'none', background: 'var(--flyout-input-bg)', color: 'var(--text-primary)', outline: 'none', fontSize: '13px' }} />
              <button type="submit" onMouseDown={(e) => e.preventDefault()} style={{ padding: '6px 14px', borderRadius: '4px', border: 'none', background: 'var(--win-accent-cyan)', color: '#000', fontWeight: 600, fontSize: '12px', cursor: 'pointer' }}>Créer</button>
            </form>
          )}

          <div className="obsidian-explorer-grid" style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(140px, 1fr))', gap: '20px', alignItems: 'start' }}>
            {currentItems.map((item) => (
              <FileCard
                key={item.id}
                item={item}
                isSelected={selectedItemId === item.id}
                isFav={favorites.includes(item.id)}
                onSelect={() => setSelectedItemId(item.id)}
                onToggleFav={(e) => toggleFavorite(item.id, e)}
                onDoubleClick={() => handleItemDoubleClick(item)}
              />
            ))}
          </div>

          {currentItems.length === 0 && !isNativeLoading && !isCreatingFolder && !isCreatingFile && (
            <div style={{ padding: '60px 20px', textAlign: 'center', color: 'var(--text-tertiary)', fontSize: '12.5px' }}>
              Aucun élément ne correspond à votre recherche.
            </div>
          )}
        </div>

        {showInspector && (
          <div
            className="obsidian-explorer-inspector"
            style={{
              width: 280,
              display: 'flex',
              flexDirection: 'column',
              background: 'rgba(10, 10, 18, 0.88)',
              overflow: 'hidden',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 16px',               borderBottom: 'none', background: 'rgba(10, 10, 18, 0.6)', gap: '8px' }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', fontSize: '12.5px', fontWeight: 700, color: 'var(--win-accent-cyan)' }}>
                <FluentIcon name="sliders" size={16} />
                <span>Inspecteur & Aperçu</span>
              </div>
              <button
                onClick={() => setShowInspector(false)}
                title="Fermer"
                style={{ border: 'none', background: 'transparent', color: 'var(--text-tertiary)', cursor: 'pointer', padding: '4px' }}
              >
                <FluentIcon name="x" size={16} />
              </button>
            </div>
            <div style={{ flex: 1, overflowY: 'auto', padding: '16px 18px', display: 'flex', flexDirection: 'column', gap: '14px' }}>
              {selectedItem ? (
                <>
                  <div style={{ display: 'flex', alignItems: 'center', padding: '14px', borderRadius: 'var(--radius-sm)', background: 'var(--flyout-card)', border: 'none' }}>
                    <FluentIcon name={selectedItem.type === 'folder' ? 'folder' : 'file-text'} size={44} />
                    <div style={{ marginLeft: '12px', flex: 1, minWidth: 0 }}>
                      <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--text-primary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{selectedItem.name}</div>
                      <div style={{ fontSize: '11px', color: 'var(--text-tertiary)' }}>{selectedItem.type === 'folder' ? 'Dossier système' : `Fichier ${selectedItem.extension ? `.${selectedItem.extension}` : ''}`}</div>
                    </div>
                  </div>
                  <div style={{ display: 'grid', gridTemplateColumns: '100px 1fr', rowGap: '10px', fontSize: '12px' }}>
                    <span style={{ color: 'var(--text-tertiary)' }}>Emplacement</span>
                    <span style={{ color: 'var(--text-secondary)', wordBreak: 'break-all' }}>{selectedItem.path}</span>
                    <span style={{ color: 'var(--text-tertiary)' }}>Taille</span>
                    <span style={{ color: 'var(--text-secondary)' }}>{selectedItem.size || '—'}</span>
                    <span style={{ color: 'var(--text-tertiary)' }}>Modifié</span>
                    <span style={{ color: 'var(--text-secondary)' }}>{selectedItem.modified}</span>
                  </div>
                  {selectedItem.type === 'file' && (
                    <div>
                      <div style={{ fontSize: '11px', fontWeight: 600, color: 'var(--text-secondary)', marginBottom: '6px' }}>Aperçu du contenu</div>
                      <div style={{ padding: '12px', borderRadius: 'var(--radius-sm)', background: '#05050a', border: 'none', maxHeight: '150px', overflowY: 'auto', fontSize: '11px', fontFamily: 'var(--font-mono)', lineHeight: '1.4', color: '#a78bfa', whiteSpace: 'pre-wrap' }}>
                        {selectedItem.content || '(Fichier vide)'}
                      </div>
                    </div>
                  )}
                  <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
                    {selectedItem.type === 'file' && (
                      <button onClick={() => handleItemDoubleClick(selectedItem)} style={{ padding: '10px', borderRadius: 'var(--radius-sm)', border: 'none', background: 'var(--win-accent)', color: '#fff', fontWeight: 600, fontSize: '12.5px', cursor: 'pointer', textAlign: 'left' }}>Ouvrir dans le Bloc-notes</button>
                    )}
                    <button onClick={() => { setRenameValue(selectedItem.name); setIsRenaming(true) }} style={{ padding: '10px', borderRadius: 'var(--radius-sm)', border: 'none', background: 'var(--flyout-card)', color: 'var(--text-primary)', fontWeight: 600, fontSize: '12.5px', cursor: 'pointer', textAlign: 'left' }}>Renommer</button>
                    <button type="button" onClick={() => { void NativeBridge.readPermissions(selectedItem.path).then(result => setPermissionText(result.acl)).catch((cause: unknown) => setNativeError(cause instanceof Error ? cause.message : String(cause))) }} style={{ padding: '10px', borderRadius: 'var(--radius-sm)', border: 'none', background: 'var(--flyout-card)', color: 'var(--text-primary)', fontWeight: 600, fontSize: '12.5px', cursor: 'pointer', textAlign: 'left' }}>Permissions Windows</button>
                    {permissionText && <pre aria-label="Permissions Windows" style={{ whiteSpace: 'pre-wrap', overflowWrap: 'anywhere', fontSize: 10, color: 'var(--text-secondary)' }}>{permissionText}</pre>}
                    {isRenaming && <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}><input aria-label="Nouveau nom" value={renameValue} onChange={event => setRenameValue(event.target.value)} /><button type="button" onClick={() => void handleRename()}>Valider</button></div>}
                    <button onClick={() => { void safeFileOps.deleteToRecycleBin() }} style={{ padding: '10px', borderRadius: 'var(--radius-sm)', border: 'none', background: 'transparent', color: '#f87171', fontWeight: 600, fontSize: '12.5px', cursor: 'pointer', textAlign: 'left' }}>Envoyer vers la Corbeille</button>
                  </div>
                </>
              ) : (
                <div style={{ textAlign: 'center', padding: '30px 10px', color: 'var(--text-tertiary)', fontSize: '12px' }}>
                  Sélectionnez un fichier ou dossier pour afficher son aperçu et ses propriétés.
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      <div
        className="obsidian-explorer-statusbar"
        style={{
          padding: '6px 14px',
          borderTop: 'none',
          fontSize: '11.5px',
          color: 'var(--text-secondary)',
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          background: 'rgba(10, 10, 18, 0.85)',
        }}
      >
        <span>{currentItems.length} élément(s)</span>
        <div style={{ display: 'flex', alignItems: 'center', gap: '18px' }}>
          {selectedItem && <span>{selectedItem.name} · {selectedItem.size || 'Dossier'}</span>}
          <span>Encodage: UTF-8</span>
          <span>Protocole: NVMe v2.0</span>
        </div>
      </div>
    </div>
  )
}
