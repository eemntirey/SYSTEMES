import React, { useState, useEffect } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'

interface NotepadAppProps {
  fileId?: string
  filePath?: string
  fileContent?: string
}

export const NotepadApp: React.FC<NotepadAppProps> = ({
  fileId: initialFileId,
  filePath: initialFilePath,
  fileContent: initialContent = '',
}) => {
  const { files, updateFileContent, createFile, addNotification } = useOS()

  const [content, setContent] = useState(initialContent)
  const [fileId, setFileId] = useState<string | undefined>(initialFileId)
  const [filePath, setFilePath] = useState<string>(initialFilePath || 'Sans titre.txt')
  const [isSaved, setIsSaved] = useState(true)
  const [showSavePrompt, setShowSavePrompt] = useState(false)
  const [newFileName, setNewFileName] = useState('Nouveau document.txt')

  // Cursor position stats
  const [cursorPos, setCursorPos] = useState({ line: 1, col: 1 })

  useEffect(() => {
    if (initialFileId && content === '') {
      const target = files.find(f => f.id === initialFileId)
      if (target) {
        setContent(target.content || '')
        setFilePath(target.path)
        setFileId(target.id)
        setIsSaved(true)
      }
    }
  }, [initialFileId])

  const handleTextChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    setContent(e.target.value)
    setIsSaved(false)
    updateCursorPos(e.target)
  }

  const updateCursorPos = (target: HTMLTextAreaElement) => {
    const textLines = target.value.substr(0, target.selectionStart).split('\n')
    const currentLineNumber = textLines.length
    const currentColumnIndex = textLines[textLines.length - 1].length + 1
    setCursorPos({ line: currentLineNumber, col: currentColumnIndex })
  }

  const handleSave = () => {
    if (fileId) {
      updateFileContent(fileId, content)
      setIsSaved(true)
      addNotification('Bloc-notes', `Le fichier "${filePath.split('/').pop()}" a été enregistré.`, 'Bloc-notes')
    } else {
      setShowSavePrompt(true)
    }
  }

  const handleSaveAsSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (newFileName.trim()) {
      const newFileObj = createFile('C:/Users/eemntirey/Documents', newFileName.trim(), content)
      if (newFileObj) {
        setFileId(newFileObj.id)
        setFilePath(newFileObj.path)
      }
      setIsSaved(true)
      setShowSavePrompt(false)
      addNotification('Bloc-notes', `Fichier "${newFileName.trim()}" créé avec succès !`, 'Bloc-notes')
    }
  }

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', backgroundColor: 'var(--window-bg)' }}>
      {/* Tabs Bar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          padding: '4px 8px 0 8px',
          borderBottom: '1px solid var(--window-border)',
          backgroundColor: 'rgba(0,0,0,0.12)',
        }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '8px',
            padding: '6px 14px',
            backgroundColor: 'var(--window-bg)',
            borderTopLeftRadius: '6px',
            borderTopRightRadius: '6px',
            border: '1px solid var(--window-border)',
            borderBottom: 'none',
            fontSize: '12px',
          }}
        >
          <FluentIcon name="file-text" size={14} />
          <span>{filePath.split('/').pop()}</span>
          {!isSaved && (
            <span
              style={{
                width: '6px',
                height: '6px',
                borderRadius: '50%',
                backgroundColor: 'var(--win-accent)',
              }}
            />
          )}
        </div>
      </div>

      {/* Menu Bar */}
      <div
        style={{
          display: 'flex',
          gap: '8px',
          padding: '4px 12px',
          borderBottom: '1px solid var(--window-border)',
          fontSize: '12px',
          color: 'var(--text-secondary)',
        }}
      >
        <button
          onClick={handleSave}
          style={{
            background: 'none',
            border: 'none',
            color: 'var(--text-primary)',
            cursor: 'pointer',
            padding: '2px 6px',
            borderRadius: '4px',
            fontSize: '12px',
          }}
          onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--control-hover)')}
          onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'transparent')}
        >
          Enregistrer (Ctrl+S)
        </button>

        <button
          onClick={() => {
            setContent('')
            setFileId(undefined)
            setFilePath('Nouveau document.txt')
            setIsSaved(true)
          }}
          style={{
            background: 'none',
            border: 'none',
            color: 'var(--text-primary)',
            cursor: 'pointer',
            padding: '2px 6px',
            borderRadius: '4px',
            fontSize: '12px',
          }}
          onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--control-hover)')}
          onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'transparent')}
        >
          Nouveau
        </button>
      </div>

      {/* Editor Content Area */}
      <div style={{ flex: 1, position: 'relative' }}>
        <textarea
          value={content}
          onChange={handleTextChange}
          onKeyDown={e => {
            if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
              e.preventDefault()
              handleSave()
            }
          }}
          onKeyUp={e => updateCursorPos(e.currentTarget)}
          onClick={e => updateCursorPos(e.currentTarget)}
          placeholder="Commencez à taper votre texte ici..."
          style={{
            width: '100%',
            height: '100%',
            padding: '14px',
            backgroundColor: 'transparent',
            border: 'none',
            outline: 'none',
            color: 'var(--text-primary)',
            fontFamily: 'var(--font-mono)',
            fontSize: '13px',
            lineHeight: '1.5',
            resize: 'none',
          }}
        />

        {/* Modal Save As */}
        {showSavePrompt && (
          <div
            style={{
              position: 'absolute',
              inset: 0,
              backgroundColor: 'rgba(0,0,0,0.6)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            <form
              onSubmit={handleSaveAsSubmit}
              style={{
                width: '320px',
                padding: '20px',
                borderRadius: '8px',
                backgroundColor: 'var(--flyout-bg)',
                border: '1px solid var(--flyout-border)',
                boxShadow: '0 12px 32px rgba(0,0,0,0.5)',
              }}
            >
              <h4 style={{ marginBottom: '12px', fontSize: '14px' }}>Enregistrer le document</h4>
              <input
                autoFocus
                type="text"
                value={newFileName}
                onChange={e => setNewFileName(e.target.value)}
                style={{
                  width: '100%',
                  padding: '8px 10px',
                  borderRadius: '4px',
                  backgroundColor: 'var(--flyout-input-bg)',
                  border: '1px solid var(--window-border)',
                  color: 'var(--text-primary)',
                  outline: 'none',
                  marginBottom: '16px',
                  fontSize: '12.5px',
                }}
              />
              <div style={{ display: 'flex', justifyContent: 'flex-end', gap: '8px' }}>
                <button
                  type="button"
                  onClick={() => setShowSavePrompt(false)}
                  style={{
                    padding: '6px 14px',
                    borderRadius: '4px',
                    border: '1px solid var(--window-border)',
                    background: 'transparent',
                    color: 'var(--text-primary)',
                    cursor: 'pointer',
                    fontSize: '12px',
                  }}
                >
                  Annuler
                </button>
                <button
                  type="submit"
                  style={{
                    padding: '6px 14px',
                    borderRadius: '4px',
                    border: 'none',
                    backgroundColor: 'var(--win-accent)',
                    color: '#ffffff',
                    cursor: 'pointer',
                    fontSize: '12px',
                    fontWeight: 500,
                  }}
                >
                  Enregistrer
                </button>
              </div>
            </form>
          </div>
        )}
      </div>

      {/* Status Bar */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'flex-end',
          gap: '20px',
          padding: '4px 14px',
          borderTop: '1px solid var(--window-border)',
          fontSize: '11px',
          color: 'var(--text-secondary)',
          backgroundColor: 'rgba(255, 255, 255, 0.02)',
        }}
      >
        <span>
          Ln {cursorPos.line}, Col {cursorPos.col}
        </span>
        <span>{content.length} caractères</span>
        <span>Windows (CRLF)</span>
        <span>UTF-8</span>
      </div>
    </div>
  )
}
