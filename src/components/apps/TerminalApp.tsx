import React, { useState, useRef, useEffect, useCallback } from 'react'
import { NativeBridge } from '../../services/NativeBridge'
import { FluentIcon } from '../common/FluentIcon'

interface CommandOutput {
  id: string
  command?: string
  output?: string
  isError?: boolean
  isInfo?: boolean
}

interface TerminalTab {
  id: string
  title: string
  path: string
  outputs: CommandOutput[]
  history: string[]
  historyIndex: number
}

const KNOWN_COMMANDS = [
  'get-childitem', 'dir', 'ls', 'get-location', 'pwd', 'set-location', 'cd',
  'new-item', 'mkdir', 'get-content', 'cat', 'type', 'set-content', 'add-content',
  'write-output', 'write-host', 'get-date', 'whoami', 'hostname', 'systeminfo',
  'get-process', 'get-service', 'get-netadapter', 'ipconfig', 'ping', 'tasklist',
  'get-command', 'get-help', 'clear-host', 'cls', 'history'
]

export const TerminalApp: React.FC = () => {
  const [isExecuting, setIsExecuting] = useState(false)

  const createInitialTab = (): TerminalTab => ({
    id: `tab-${Date.now()}`,
    title: 'Windows PowerShell',
    path: 'C:\\Users\\eemntirey',
    outputs: [{
      id: 'init-1',
      output: `Windows PowerShell\nCopyright (C) Microsoft Corporation. Tous droits réservés.\n\nEssaie les nouvelles commandes : neofetch, ipconfig, tasklist, get-process, hostname\nTape "help" pour la liste complète.\n`,
    }],
    history: [],
    historyIndex: -1,
  })

  const [tabs, setTabs] = useState<TerminalTab[]>([createInitialTab()])
  const [activeTabId, setActiveTabId] = useState(tabs[0].id)
  const [inputVal, setInputVal] = useState('')
  const [suggestions, setSuggestions] = useState<string[]>([])
  const [showSuggestions, setShowSuggestions] = useState(false)

  const terminalEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  const activeTab = tabs.find(t => t.id === activeTabId) || tabs[0]

  useEffect(() => {
    if (typeof terminalEndRef.current?.scrollIntoView === 'function') {
      terminalEndRef.current.scrollIntoView({ behavior: 'smooth' })
    }
  }, [activeTab.outputs, activeTabId])

  const updateActiveTab = useCallback((updater: (tab: TerminalTab) => TerminalTab) => {
    setTabs(prev => prev.map(t => t.id === activeTabId ? updater(t) : t))
  }, [activeTabId])

  const handleNewTab = () => {
    const newTab = createInitialTab()
    setTabs(prev => [...prev, newTab])
    setActiveTabId(newTab.id)
    setInputVal('')
  }

  const handleCloseTab = (e: React.MouseEvent, id: string) => {
    e.stopPropagation()
    if (tabs.length === 1) return
    const remaining = tabs.filter(t => t.id !== id)
    setTabs(remaining)
    if (activeTabId === id) {
      setActiveTabId(remaining[remaining.length - 1].id)
    }
  }

  const handleInputChange = (val: string) => {
    setInputVal(val)
    if (val.trim()) {
      const partial = val.trim().toLowerCase().split(' ')[0]
      const matches = KNOWN_COMMANDS.filter(c => c.startsWith(partial) && c !== partial)
      setSuggestions(matches.slice(0, 6))
      setShowSuggestions(matches.length > 0)
    } else {
      setShowSuggestions(false)
      setSuggestions([])
    }
  }

  const applySuggestion = (cmd: string) => {
    setInputVal(cmd + ' ')
    setShowSuggestions(false)
    inputRef.current?.focus()
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      executeCommand(inputVal)
      setShowSuggestions(false)
    } else if (e.key === 'Tab' && suggestions.length > 0) {
      e.preventDefault()
      applySuggestion(suggestions[0])
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (activeTab.history.length > 0) {
        const nextIdx = activeTab.historyIndex + 1 < activeTab.history.length
          ? activeTab.historyIndex + 1
          : activeTab.historyIndex
        updateActiveTab(t => ({ ...t, historyIndex: nextIdx }))
        setInputVal(activeTab.history[activeTab.history.length - 1 - nextIdx] || '')
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (activeTab.historyIndex > 0) {
        const nextIdx = activeTab.historyIndex - 1
        updateActiveTab(t => ({ ...t, historyIndex: nextIdx }))
        setInputVal(activeTab.history[activeTab.history.length - 1 - nextIdx] || '')
      } else if (activeTab.historyIndex === 0) {
        updateActiveTab(t => ({ ...t, historyIndex: -1 }))
        setInputVal('')
      }
    } else if (e.key === 'Escape') {
      setShowSuggestions(false)
    }
  }

  const executeCommand = async (cmdStr: string) => {
    const trimmed = cmdStr.trim()
    if (!trimmed || isExecuting) return

    const targetTabId = activeTab.id
    const workingDirectory = activeTab.path
    const newHistory = [...activeTab.history, trimmed]
    setInputVal('')
    setShowSuggestions(false)
    setIsExecuting(true)

    if (['clear', 'cls', 'clear-host'].includes(trimmed.toLowerCase())) {
      setTabs(prev => prev.map(tab => tab.id === targetTabId
        ? { ...tab, outputs: [], history: newHistory, historyIndex: -1 }
        : tab))
      setIsExecuting(false)
      return
    }

    setTabs(prev => prev.map(tab => tab.id === targetTabId
      ? {
          ...tab,
          outputs: [...tab.outputs, { id: `command-${Date.now()}`, command: `PS ${tab.path || '~'}> ${trimmed}` }],
          history: newHistory,
          historyIndex: -1,
        }
      : tab))

    try {
      const result = await NativeBridge.executePowerShell(trimmed, workingDirectory)
      const output = [
        result.output,
        result.error,
        result.truncated ? '[Sortie tronquée : limite de 64 Kio atteinte.]' : '',
      ].filter(Boolean).join('\n')
      setTabs(prev => prev.map(tab => tab.id === targetTabId
        ? {
            ...tab,
            path: result.workingDirectory || tab.path,
            outputs: [...tab.outputs, {
              id: `result-${Date.now()}`,
              output: output || `PowerShell terminé (code ${result.exitCode}).`,
              isError: result.exitCode !== 0 || !!result.error,
              isInfo: result.exitCode === 0 && !result.error,
            }],
          }
        : tab))
    } catch (error) {
      setTabs(prev => prev.map(tab => tab.id === targetTabId
        ? {
            ...tab,
            outputs: [...tab.outputs, {
              id: `error-${Date.now()}`,
              output: error instanceof Error ? error.message : String(error),
              isError: true,
            }],
          }
        : tab))
    } finally {
      setIsExecuting(false)
    }
  }

  return (
    <div
      onClick={() => inputRef.current?.focus()}
      style={{
        height: '100%',
        backgroundColor: '#0a0a12',
        color: '#cccccc',
        fontFamily: 'var(--font-mono)',
        fontSize: '13px',
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      {/* Tabs bar */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          background: 'rgba(0,0,0,0.35)',
          borderBottom: '1px solid rgba(139,92,246,0.2)',
          padding: '4px 8px 0',
          gap: '4px',
          flexShrink: 0,
        }}
      >
        {tabs.map(tab => {
          const isActive = tab.id === activeTabId
          return (
            <div
              key={tab.id}
              onClick={() => setActiveTabId(tab.id)}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
                padding: '6px 12px',
                minWidth: 120,
                maxWidth: 180,
                borderRadius: '6px 6px 0 0',
                background: isActive ? '#0a0a12' : 'transparent',
                border: isActive ? '1px solid rgba(139,92,246,0.25)' : '1px solid transparent',
                borderBottom: 'none',
                cursor: 'pointer',
                fontSize: '12px',
                color: isActive ? 'var(--win-accent-cyan)' : 'var(--text-secondary)',
              }}
            >
              <FluentIcon name="terminal" size={13} color={isActive ? 'var(--win-accent)' : undefined} />
              <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                {tab.title}
              </span>
              {tabs.length > 1 && (
                <button
                  onClick={e => handleCloseTab(e, tab.id)}
                  style={{
                    border: 'none',
                    background: 'transparent',
                    color: 'var(--text-tertiary)',
                    borderRadius: '50%',
                    padding: 2,
                    display: 'flex',
                    cursor: 'pointer',
                  }}
                >
                  <FluentIcon name="x" size={11} />
                </button>
              )}
            </div>
          )
        })}
        <button
          onClick={handleNewTab}
          title="Nouvel onglet"
          style={{
            border: 'none',
            background: 'transparent',
            color: 'var(--text-secondary)',
            padding: 6,
            borderRadius: 4,
            cursor: 'pointer',
            display: 'flex',
          }}
        >
          <FluentIcon name="plus" size={14} />
        </button>
      </div>

      {/* Content */}
      <div style={{ flex: 1, overflow: 'auto', padding: '12px 14px', position: 'relative' }}>
        {activeTab.outputs.map(item => (
          <div key={item.id} style={{ marginBottom: 10 }}>
            {item.command && (
              <div style={{ color: '#e0e0ff', fontWeight: 500, marginBottom: 2 }}>
                {item.command}
              </div>
            )}
            {item.output && (
              <pre
                style={{
                  margin: 0,
                  color: item.isError ? '#f87171' : item.isInfo ? '#a78bfa' : '#38bdf8',
                  whiteSpace: 'pre-wrap',
                  fontFamily: 'inherit',
                  fontSize: '12.5px',
                  lineHeight: 1.45,
                }}
              >
                {item.output}
              </pre>
            )}
          </div>
        ))}

        {/* Input line */}
        <div style={{ display: 'flex', alignItems: 'center', position: 'relative' }}>
          <span style={{ color: 'var(--win-accent-cyan)', marginRight: 8, whiteSpace: 'nowrap', fontWeight: 600 }}>
            PS {activeTab.path || '~'}{'>'}
          </span>
          <input
            ref={inputRef}
            type="text"
            disabled={isExecuting}
            value={inputVal}
            onChange={e => handleInputChange(e.target.value)}
            onKeyDown={handleKeyDown}
            autoFocus
            style={{
              flex: 1,
              background: 'transparent',
              border: 'none',
              outline: 'none',
              color: '#ffffff',
              fontFamily: 'inherit',
              fontSize: '13px',
              caretColor: 'var(--win-accent-cyan)',
            }}
          />
        </div>

        {/* Suggestions dropdown */}
        {showSuggestions && suggestions.length > 0 && (
          <div
            style={{
              position: 'absolute',
              left: 14,
              bottom: 40,
              background: 'rgba(10,10,18,0.95)',
              border: '1px solid rgba(139,92,246,0.35)',
              borderRadius: 8,
              padding: '6px 0',
              minWidth: 180,
              boxShadow: '0 8px 24px rgba(0,0,0,0.5)',
              zIndex: 10,
            }}
          >
            {suggestions.map(s => (
              <div
                key={s}
                onClick={() => applySuggestion(s)}
                style={{
                  padding: '6px 14px',
                  cursor: 'pointer',
                  fontSize: 12.5,
                  color: 'var(--text-primary)',
                }}
                onMouseEnter={e => (e.currentTarget.style.background = 'rgba(139,92,246,0.2)')}
                onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
              >
                {s}
              </div>
            ))}
          </div>
        )}

        <div ref={terminalEndRef} />
      </div>

      {/* Status bar */}
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'center',
          padding: '4px 12px',
          borderTop: '1px solid rgba(139,92,246,0.15)',
          fontSize: 11,
          color: 'var(--text-tertiary)',
          background: 'rgba(0,0,0,0.25)',
          flexShrink: 0,
        }}
      >
        <span>Windows PowerShell 5.1 · exécution native</span>
        <span>{isExecuting ? 'Exécution…' : `${activeTab.history.length} commande(s) · Tab pour compléter`}</span>
      </div>
    </div>
  )
}
