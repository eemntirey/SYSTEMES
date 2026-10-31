import React, { useState, useRef, useEffect, useCallback } from 'react'
import { useOS } from '../../context/OSContext'
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
  'help', 'cls', 'clear', 'dir', 'ls', 'cd', 'pwd', 'mkdir', 'md', 'cat', 'type',
  'echo', 'ver', 'whoami', 'hostname', 'date', 'time', 'systeminfo', 'ping',
  'ipconfig', 'tasklist', 'get-process', 'neofetch', 'matrix', 'history',
  'write-host', 'get-date', 'clear-host'
]

export const TerminalApp: React.FC = () => {
  const { files, createFolder, createFile, closeWindow } = useOS()

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

  const executeCommand = (cmdStr: string) => {
    const trimmed = cmdStr.trim()
    const newHistory = trimmed ? [...activeTab.history, trimmed] : activeTab.history

    if (!trimmed) {
      updateActiveTab(t => ({
        ...t,
        outputs: [...t.outputs, { id: `${Date.now()}`, command: `PS ${t.path}> ` }],
        history: newHistory,
        historyIndex: -1,
      }))
      setInputVal('')
      return
    }

    const [cmd, ...args] = trimmed.split(/\s+/)
    const lowerCmd = cmd.toLowerCase()
    let result = ''
    let isErr = false
    let isInfo = false
    let newPath = activeTab.path

    switch (lowerCmd) {
      case 'help':
        result = `Commandes PowerShell disponibles (Windows 11)\n\n  Navigation & Fichiers\n  ─────────────────────\n  help, ?              Affiche cette aide\n  dir, ls              Liste le contenu du répertoire\n  cd <chemin>          Change de répertoire\n  pwd                  Affiche le chemin actuel\n  mkdir, md <nom>      Crée un dossier\n  cat, type <fichier>  Affiche le contenu d'un fichier\n\n  Système\n  ───────\n  ver                  Version de Windows\n  whoami               Utilisateur courant\n  hostname             Nom de la machine\n  systeminfo           Infos matériel & OS\n  date, time, get-date Date et heure\n  neofetch             Affichage stylisé système\n\n  Réseau\n  ──────\n  ping <hôte>          Test de connectivité\n  ipconfig             Configuration réseau\n\n  Processus\n  ─────────\n  tasklist, get-process  Liste des processus\n\n  Utilitaires\n  ───────────\n  echo, write-host <t>  Affiche du texte\n  history              Historique des commandes\n  clear, cls, clear-host  Efface l'écran\n  matrix               Easter egg\n\n  Astuce : Tab pour l'autocomplétion des commandes.`
        break

      case 'clear':
      case 'cls':
      case 'clear-host':
        updateActiveTab(t => ({ ...t, outputs: [], history: newHistory, historyIndex: -1 }))
        setInputVal('')
        return

      case 'dir':
      case 'ls': {
        const normalizedCurrent = activeTab.path.replace(/\\/g, '/')
        const matchingFiles = files.filter(f => {
          const parts = f.path.split('/')
          parts.pop()
          return parts.join('/') === normalizedCurrent
        })

        const header = `  Répertoire : ${activeTab.path}\n\nMode                 Dernière écriture          Taille Nom\n----                 -----------------          ------ ----\n`
        const rows = matchingFiles
          .map(f => {
            const mode = f.type === 'folder' ? 'd-----' : '-a----'
            const size = f.size ? f.size.padStart(12, ' ') : '            '
            return `${mode}    ${f.modified.padEnd(25, ' ')} ${size} ${f.name}`
          })
          .join('\n')
        result = header + (rows || '  (Aucun élément)')
        break
      }

      case 'cd': {
        const target = args.join(' ')
        if (!target || target === '.') {
          // stay
        } else if (target === '..') {
          const parts = activeTab.path.split('\\')
          if (parts.length > 1) {
            parts.pop()
            newPath = parts.join('\\') || 'C:\\'
          }
        } else if (target.startsWith('C:') || target.startsWith('c:')) {
          newPath = target.replace(/\//g, '\\')
        } else {
          newPath = `${activeTab.path}\\${target}`.replace(/\\\\+/g, '\\')
        }
        break
      }

      case 'pwd':
        result = activeTab.path
        isInfo = true
        break

      case 'mkdir':
      case 'md': {
        const folderName = args.join(' ')
        if (!folderName) {
          result = 'Erreur : nom de dossier requis. Exemple : mkdir Projets'
          isErr = true
        } else {
          createFolder(activeTab.path.replace(/\\/g, '/'), folderName)
          result = `Dossier "${folderName}" créé avec succès.`
          isInfo = true
        }
        break
      }

      case 'touch': {
        const fileName = args.join(' ').trim()
        if (!fileName) {
          result = 'Erreur : nom de fichier requis. Exemple : touch notes.txt'
          isErr = true
          break
        }
        const normalizedCurrent = activeTab.path.replace(/\\/g, '/')
        const exists = files.some(file =>
          file.name.toLowerCase() === fileName.toLowerCase() &&
          file.path.slice(0, file.path.lastIndexOf('/')) === normalizedCurrent
        )
        if (exists) {
          result = `Un élément nommé "${fileName}" existe déjà dans ce dossier.`
          isErr = true
        } else {
          createFile(normalizedCurrent, fileName, '')
          result = `Fichier "${fileName}" créé dans ${activeTab.path}.`
          isInfo = true
        }
        break
      }

      case 'cat':
      case 'type': {
        const fileName = args.join(' ')
        if (!fileName) {
          result = 'Erreur : nom de fichier requis.'
          isErr = true
          break
        }
        const normalizedCurrent = activeTab.path.replace(/\\/g, '/')
        const targetFile = files.find(
          f => f.name.toLowerCase() === fileName.toLowerCase() && f.path.startsWith(normalizedCurrent)
        )
        if (targetFile) {
          result = targetFile.content || '(Fichier vide)'
        } else {
          result = `Impossible de trouver le fichier : ${fileName}`
          isErr = true
        }
        break
      }

      case 'echo':
      case 'write-host':
        result = args.join(' ')
        isInfo = true
        break

      case 'ver':
        result = 'Microsoft Windows [version 10.0.26100.1742] (Windows 11 24H2)'
        break

      case 'whoami':
        result = 'WINDOWS-11\\eemntirey'
        isInfo = true
        break

      case 'hostname':
        result = 'DESKTOP-WIN11-PRO'
        isInfo = true
        break

      case 'date':
      case 'time':
      case 'get-date':
        result = new Date().toLocaleString('fr-FR', {
          weekday: 'long',
          year: 'numeric',
          month: 'long',
          day: 'numeric',
          hour: '2-digit',
          minute: '2-digit',
          second: '2-digit',
        })
        isInfo = true
        break

      case 'systeminfo':
        result = `Nom de l'hôte:                 DESKTOP-WIN11-PRO\nNom du système d'exploitation: Microsoft Windows 11 Professionnel\nVersion du système:            10.0.26100 Version 24H2\nFabricant du système:          Antigravity High-Performance Systems\nType du système:               PC à architecture x64\nProcesseur(s):                 1 Processeur(s) installé(s).\n                               [01]: AMD64 Family 25 Model 97 Stepping 2 AuthenticAMD ~4500 Mhz\nMémoire physique totale:       32 768 Mo\nMémoire physique disponible:   18 432 Mo\nMémoire virtuelle: taille max: 37 888 Mo\nCarte graphique:               NVIDIA GeForce RTX 4090 24 Go\nStatut réseau:                 Connecté (Wi-Fi 6E / 1.2 Gbps)\nFuseau horaire:                (UTC+03:00) Nairobi`
        break

      case 'ping': {
        const host = args[0] || '1.1.1.1'
        result = `Envoi d'une requête 'Ping' sur ${host} avec 32 octets de données :\nRéponse de ${host} : octets=32 temps=12 ms TTL=56\nRéponse de ${host} : octets=32 temps=11 ms TTL=56\nRéponse de ${host} : octets=32 temps=13 ms TTL=56\nRéponse de ${host} : octets=32 temps=10 ms TTL=56\n\nStatistiques Ping pour ${host} :\n    Paquets : envoyés = 4, reçus = 4, perdus = 0 (perte 0%),\nDurée approximative des boucles en millisecondes :\n    Minimum = 10ms, Maximum = 13ms, Moyenne = 11ms`
        break
      }

      case 'ipconfig':
        result = `Configuration IP de Windows\n\nCarte réseau sans fil Wi-Fi :\n\n   Suffixe DNS propre à la connexion. . . : home.local\n   Adresse IPv6 de liaison locale. . . . . : fe80::a1b2:c3d4:e5f6:7890%12\n   Adresse IPv4. . . . . . . . . . . . . . : 192.168.1.42\n   Masque de sous-réseau. . . . . . . . . : 255.255.255.0\n   Passerelle par défaut. . . . . . . . . : 192.168.1.1\n\nCarte Ethernet Ethernet :\n\n   État du média. . . . . . . . . . . . . : Média déconnecté\n   Suffixe DNS propre à la connexion. . . :`
        break

      case 'tasklist':
      case 'get-process':
        result = `Nom de l'image                 PID Session Nom de session Utilisation mémoire\n========================= ======== ================ ================ ============\nSystem Idle Process              0 Services                   0         8 Ko\nSystem                           4 Services                   0       148 Ko\nswchost.exe                    892 Services                   0    12 456 Ko\nexplorer.exe                  4520 Console                    1    89 234 Ko\nWindowsTerminal.exe           7812 Console                    1    67 890 Ko\nmsedge.exe                    9104 Console                    1   245 678 Ko\nCode.exe                      6234 Console                    1   312 456 Ko\nRuntimeBroker.exe             3456 Console                    1    28 901 Ko\nSearchHost.exe                5678 Console                    1    45 123 Ko\nShellExperienceHost.exe       2345 Console                    1    67 890 Ko`
        break

      case 'neofetch':
        result = `                  ..,\n                .o8Oo.=.\n               .o888Oo..=\n              .o88888Oo..=\n             .o8888888Oo..=\n            .o888888888Oo..=        eemntirey@DESKTOP-WIN11-PRO\n           .o88888888888Oo..=       ─────────────────────────────\n          .o8888888888888Oo..=      OS: Windows 11 Pro 24H2\n         .o888888888888888Oo..=     Host: Antigravity Systems\n        .o88888888888888888Oo..=    Kernel: 10.0.26100.1742\n       .o8888888888888888888Oo..=   Uptime: 4h 32m\n      .o888888888888888888888Oo..=  Shell: PowerShell 7.4\n     .o88888888888888888888888Oo..= CPU: AMD Ryzen 9 7950X (16) @ 4.5GHz\n    .o8888888888888888888888888Oo..= Memory: 14.3 GiB / 32.0 GiB\n   .o888888888888888888888888888Oo..= GPU: NVIDIA GeForce RTX 4090\n  .o88888888888888888888888888888Oo..= Resolution: 1920x1080 @ 144Hz\n .o8888888888888888888888888888888Oo..= Theme: Dark Neon (custom)\n.o888888888888888888888888888888888Oo..= Terminal: Windows Terminal`
        isInfo = true
        break

      case 'matrix':
        result = `01001000 01100101 01101100 01101100 01101111 00100000 01010111 01101111 01110010 01101100 01100100\n\nBienvenue dans la matrice Windows 11.\nRéveil de l'agent terminé. Suivez le lapin blanc.`
        isInfo = true
        break

      case 'history':
        result = activeTab.history.length
          ? activeTab.history.map((h, i) => `  ${i + 1}  ${h}`).join('\n')
          : '  (historique vide)'
        break

      default:
        result = `'${cmd}' n'est pas reconnu en tant que commande interne\nou externe, un programme exécutable ou un fichier de commandes.\nVérifiez l'orthographe du nom, ou si un chemin d'accès existe,\nvérifiez que le chemin d'accès est correct et réessayez.`
        isErr = true
    }

    updateActiveTab(t => ({
      ...t,
      path: newPath,
      outputs: [
        ...t.outputs,
        {
          id: `${Date.now()}`,
          command: `PS ${t.path}> ${cmdStr}`,
          output: result,
          isError: isErr,
          isInfo,
        },
      ],
      history: newHistory,
      historyIndex: -1,
    }))
    setInputVal('')
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
            PS {activeTab.path}>
          </span>
          <input
            ref={inputRef}
            type="text"
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
        <span>PowerShell 7.4 · UTF-8</span>
        <span>{activeTab.history.length} commande(s) · Tab pour compléter</span>
      </div>
    </div>
  )
}
