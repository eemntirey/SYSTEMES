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
  const [nativeReady, setNativeReady] = useState<boolean | null>(null)
  const [engineLabel, setEngineLabel] = useState('Windows PowerShell')

  const createInitialTab = (path = ''): TerminalTab => ({
    id: `tab-${Date.now()}-${Math.random().toString(16).slice(2, 6)}`,
    title: 'Windows PowerShell',
    path,
    outputs: [{
      id: 'init-1',
      output:
        'Terminal SYSTEMES — exécution native PowerShell\n' +
        'Les commandes partent vers le moteur Windows (compte utilisateur courant, sans élévation).\n' +
        'Aucune donnée fictive : dir, tasklist, systeminfo, ipconfig, etc. sont de vraies commandes.\n' +
        'Tapez Get-Help pour l\'aide PowerShell.\n',
      isInfo: true,
    }],
    history: [],
    historyIndex: -1,
  })

  const [tabs, setTabs] = useState<TerminalTab[]>(() => [createInitialTab()])
  const [activeTabId, setActiveTabId] = useState('')
  const [inputVal, setInputVal] = useState('')
  const [suggestions, setSuggestions] = useState<string[]>([])
  const [showSuggestions, setShowSuggestions] = useState(false)

  const terminalEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (!activeTabId && tabs[0]) setActiveTabId(tabs[0].id)
  }, [activeTabId, tabs])

  useEffect(() => {
    let cancelled = false
    ;(async () => {
      try {
        const locations = await NativeBridge.getFileLocations() as { locations?: { name: string; path: string }[] }
        const home = locations?.locations?.find(l => /documents|bureau|desktop|profil|user/i.test(l.name))?.path
          || locations?.locations?.[0]?.path
          || ''
        if (cancelled) return
        setNativeReady(true)
        if (home) {
          setTabs(prev => prev.map((tab, index) => (index === 0 && !tab.path ? { ...tab, path: home } : tab)))
        }
      } catch {
        if (cancelled) return
        setNativeReady(false)
        setTabs(prev => prev.map((tab, index) => {
          if (index !== 0) return tab
          return {
            ...tab,
            outputs: [
              ...tab.outputs,
              {
                id: 'native-missing',
                output:
                  'Moteur natif indisponible.\n' +
                  'Démarrez SYSTEMES depuis l\'application Windows (hôte WebView2) pour exécuter de vraies commandes PowerShell.\n' +
                  'Aucune commande ne sera simulée localement.',
                isError: true,
              },
            ],
          }
        }))
      }
    })()
    return () => { cancelled = true }
  }, [])

  const activeTab = tabs.find(t => t.id === activeTabId) || tabs[0]

  useEffect(() => {
    if (typeof terminalEndRef.current?.scrollIntoView === 'function') {
      terminalEndRef.current.scrollIntoView({ behavior: 'smooth' })
    }
  }, [activeTab?.outputs, activeTabId])

  const updateActiveTab = useCallback((updater: (tab: TerminalTab) => TerminalTab) => {
    setTabs(prev => prev.map(t => t.id === activeTabId ? updater(t) : t))
  }, [activeTabId])

  const handleNewTab = () => {
    const newTab = createInitialTab(activeTab?.path || '')
    setTabs(prev => [...prev, newTab])
    setActiveTabId(newTab.id)
    setInputVal('')
  }

  const handleCloseTab = (e: React.MouseEvent, id: string) => {
    e.stopPropagation()
    if (tabs.length <= 1) return
    const idx = tabs.findIndex(t => t.id === id)
    const next = tabs.filter(t => t.id !== id)
    setTabs(next)
    if (activeTabId === id) setActiveTabId(next[Math.max(0, idx - 1)].id)
  }

  const handleInputChange = (val: string) => {
    setInputVal(val)
    if (!val.trim()) {
      setSuggestions([])
      setShowSuggestions(false)
      return
    }
    const lower = val.toLowerCase()
    const matches = KNOWN_COMMANDS.filter(c => c.startsWith(lower) || c.includes(lower)).slice(0, 8)
    setSuggestions(matches)
    setShowSuggestions(matches.length > 0)
  }

  const applySuggestion = (s: string) => {
    setInputVal(s)
    setShowSuggestions(false)
    inputRef.current?.focus()
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      void executeCommand(inputVal)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (!activeTab?.history.length) return
      const next = activeTab.historyIndex < 0 ? activeTab.history.length - 1 : Math.max(0, activeTab.historyIndex - 1)
      updateActiveTab(tab => ({ ...tab, historyIndex: next }))
      setInputVal(activeTab.history[next] || '')
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (!activeTab || activeTab.historyIndex < 0) return
      const next = activeTab.historyIndex + 1
      if (next >= activeTab.history.length) {
        updateActiveTab(tab => ({ ...tab, historyIndex: -1 }))
        setInputVal('')
      } else {
        updateActiveTab(tab => ({ ...tab, historyIndex: next }))
        setInputVal(activeTab.history[next] || '')
      }
    } else if (e.key === 'Tab' && suggestions.length > 0) {
      e.preventDefault()
      applySuggestion(suggestions[0])
    }
  }

  const executeCommand = async (cmdStr: string) => {
    const trimmed = cmdStr.trim()
    if (!trimmed || isExecuting || !activeTab) return

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

    if (nativeReady === false) {
      setTabs(prev => prev.map(tab => tab.id === targetTabId
        ? {
            ...tab,
            history: newHistory,
            historyIndex: -1,
            outputs: [
              ...tab.outputs,
              { id: `command-${Date.now()}`, command: `PS ${tab.path || '~'}> ${trimmed}` },
              {
                id: `error-${Date.now()}`,
                output: 'Exécution refusée : moteur natif Windows indisponible. Aucune simulation locale.',
                isError: true,
              },
            ],
          }
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
      setNativeReady(true)
      if (result.source) setEngineLabel(String(result.source))
      const output = [
        result.output,
        result.commandError,
        result.truncated ? '[Sortie tronquée : limite de 64 Kio atteinte.]' : '',
      ].filter(Boolean).join('\n')
      setTabs(prev => prev.map(tab => tab.id === targetTabId
        ? {
            ...tab,
            path: result.workingDirectory || tab.path,
            outputs: [...tab.outputs, {
              id: `result-${Date.now()}`,
              output: output || `PowerShell terminé (code ${result.exitCode}).`,
              isError: result.exitCode !== 0 || !!result.commandError,
              isInfo: result.exitCode === 0 && !result.commandError,
            }],
          }
        : tab))
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (/Native Host indisponible|canal natif|WebView2/i.test(message)) setNativeReady(false)
      setTabs(prev => prev.map(tab => tab.id === targetTabId
        ? {
            ...tab,
            outputs: [...tab.outputs, {
              id: `error-${Date.now()}`,
              output: message,
              isError: true,
            }],
          }
        : tab))
    } finally {
      setIsExecuting(false)
    }
  }

  if (!activeTab) return null

  return (
    <div
      onClick={() => inputRef.current?.focus()}
      style={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        background: '#0c0c14',
        color: '#e0e0e0',
        fontFamily: 'Consolas, Cascadia Code, monospace',
        fontSize: 13,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 4, padding: '6px 8px', background: 'rgba(0,0,0,0.35)', borderBottom: '1px solid rgba(139,92,246,0.2)' }}>
        {tabs.map(tab => (
          <button
            key={tab.id}
            type="button"
            onClick={() => setActiveTabId(tab.id)}
            style={{
              display: 'flex', alignItems: 'center', gap: 6,
              padding: '4px 10px', borderRadius: 6, border: 'none',
              background: tab.id === activeTabId ? 'rgba(139,92,246,0.25)' : 'transparent',
              color: tab.id === activeTabId ? '#fff' : 'var(--text-secondary)',
              cursor: 'pointer', fontSize: 12,
            }}
          >
            <FluentIcon name="terminal" size={14} />
            <span>{tab.title}</span>
            {tabs.length > 1 && (
              <span onClick={e => handleCloseTab(e, tab.id)} style={{ opacity: 0.6, marginLeft: 4 }}>×</span>
            )}
          </button>
        ))}
        <button type="button" onClick={handleNewTab} title="Nouvel onglet" style={{ border: 'none', background: 'transparent', color: 'var(--text-secondary)', cursor: 'pointer', padding: 4 }}>+</button>
      </div>

      <div style={{ flex: 1, overflow: 'auto', padding: '12px 14px', position: 'relative' }}>
        {activeTab.outputs.map(item => (
          <div key={item.id} style={{ marginBottom: 8 }}>
            {item.command && (
              <div style={{ color: 'var(--win-accent-cyan)', marginBottom: 2, whiteSpace: 'pre-wrap' }}>{item.command}</div>
            )}
            {item.output && (
              <div style={{
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
                color: item.isError ? '#f87171' : item.isInfo ? '#a5b4fc' : '#d4d4d8',
              }}>{item.output}</div>
            )}
          </div>
        ))}

        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ color: 'var(--win-accent-cyan)', flexShrink: 0, whiteSpace: 'nowrap', fontWeight: 600 }}>
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
              flex: 1, background: 'transparent', border: 'none', outline: 'none',
              color: '#ffffff', fontFamily: 'inherit', fontSize: '13px',
              caretColor: 'var(--win-accent-cyan)',
            }}
          />
        </div>

        {showSuggestions && suggestions.length > 0 && (
          <div style={{
            position: 'absolute', left: 14, bottom: 40,
            background: 'rgba(10,10,18,0.95)', border: '1px solid rgba(139,92,246,0.35)',
            borderRadius: 8, padding: '6px 0', minWidth: 180,
            boxShadow: '0 8px 24px rgba(0,0,0,0.5)', zIndex: 10,
          }}>
            {suggestions.map(s => (
              <div
                key={s}
                onClick={() => applySuggestion(s)}
                style={{ padding: '6px 14px', cursor: 'pointer', fontSize: 12.5, color: 'var(--text-primary)' }}
                onMouseEnter={e => (e.currentTarget.style.background = 'rgba(139,92,246,0.2)')}
                onMouseLeave={e => (e.currentTarget.style.background = 'transparent')}
              >{s}</div>
            ))}
          </div>
        )}

        <div ref={terminalEndRef} />
      </div>

      <div style={{
        display: 'flex', justifyContent: 'space-between', alignItems: 'center',
        padding: '4px 12px', borderTop: '1px solid rgba(139,92,246,0.15)',
        fontSize: 11, color: 'var(--text-tertiary)', background: 'rgba(0,0,0,0.25)', flexShrink: 0,
      }}>
        <span>{engineLabel} · {nativeReady === false ? 'hors ligne' : nativeReady ? 'exécution native' : 'connexion…'}</span>
        <span>{isExecuting ? 'Exécution…' : `${activeTab.history.length} commande(s) · Tab pour compléter`}</span>
      </div>
    </div>
  )
}
