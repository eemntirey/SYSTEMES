import React, { useState, useRef, useEffect } from 'react'
import { useOS } from '../../context/OSContext'

interface CommandOutput {
  id: string
  command?: string
  output?: string
  isError?: boolean
}

export const TerminalApp: React.FC = () => {
  const { files, createFolder } = useOS()

  const [currentPath, setCurrentPath] = useState('C:\\Users\\eemntirey')
  const [inputVal, setInputVal] = useState('')
  const [history, setHistory] = useState<string[]>([])
  const [historyIndex, setHistoryIndex] = useState(-1)
  const [outputs, setOutputs] = useState<CommandOutput[]>([
    {
      id: 'init-1',
      output: `Windows PowerShell\nCopyright (C) Microsoft Corporation. Tous droits réservés.\n\nInstallez la dernière version de PowerShell pour découvrir de nouvelles fonctionnalités et améliorations !\nTapez "help" pour voir la liste des commandes disponibles.\n`,
    },
  ])

  const terminalEndRef = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    terminalEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [outputs])

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter') {
      executeCommand(inputVal)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      if (history.length > 0) {
        const nextIdx = historyIndex + 1 < history.length ? historyIndex + 1 : historyIndex
        setHistoryIndex(nextIdx)
        setInputVal(history[history.length - 1 - nextIdx] || '')
      }
    } else if (e.key === 'ArrowDown') {
      e.preventDefault()
      if (historyIndex > 0) {
        const nextIdx = historyIndex - 1
        setHistoryIndex(nextIdx)
        setInputVal(history[history.length - 1 - nextIdx] || '')
      } else if (historyIndex === 0) {
        setHistoryIndex(-1)
        setInputVal('')
      }
    }
  }

  const executeCommand = (cmdStr: string) => {
    const trimmed = cmdStr.trim()
    const newHistory = trimmed ? [...history, trimmed] : history
    setHistory(newHistory)
    setHistoryIndex(-1)
    setInputVal('')

    if (!trimmed) {
      setOutputs(prev => [...prev, { id: `${Date.now()}`, command: `${currentPath}> ` }])
      return
    }

    const [cmd, ...args] = trimmed.split(' ')
    const lowerCmd = cmd.toLowerCase()
    let result = ''
    let isErr = false

    switch (lowerCmd) {
      case 'help':
        result = `Commandes disponibles :
  help           - Affiche ce menu d'aide
  dir, ls        - Liste les fichiers et sous-dossiers
  cd <dossier>   - Change de répertoire de travail
  mkdir <nom>    - Crée un nouveau dossier
  cat, type <f>  - Affiche le contenu d'un fichier texte
  echo <texte>   - Affiche le texte saisi
  ver            - Version de Windows 11
  systeminfo     - Informations détaillées sur le matériel et l'OS
  whoami         - Nom d'utilisateur et domaine actuels
  ping <hôte>    - Teste la connectivité réseau
  date, time     - Affiche la date et l'heure système
  matrix         - Démarre la cascade binaire Matrix
  clear, cls     - Efface l'écran du terminal`
        break

      case 'clear':
      case 'cls':
        setOutputs([])
        return

      case 'dir':
      case 'ls': {
        const normalizedCurrent = currentPath.replace(/\\/g, '/')
        const matchingFiles = files.filter(f => {
          const parts = f.path.split('/')
          parts.pop()
          return parts.join('/') === normalizedCurrent
        })

        const header = `  Répertoire : ${currentPath}\n\nMode                 Dernière écriture          Taille Nom\n----                 -----------------          ------ ----\n`
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
          const parts = currentPath.split('\\')
          if (parts.length > 1) {
            parts.pop()
            setCurrentPath(parts.join('\\') || 'C:\\')
          }
        } else {
          setCurrentPath(prev => `${prev}\\${target}`.replace(/\\\\+/g, '\\'))
        }
        break
      }

      case 'mkdir': {
        const folderName = args.join(' ')
        if (!folderName) {
          result = 'Erreur: nom de dossier requis. Exemple: mkdir Projets'
          isErr = true
        } else {
          createFolder(currentPath.replace(/\\/g, '/'), folderName)
          result = `Dossier "${folderName}" créé avec succès.`
        }
        break
      }

      case 'cat':
      case 'type': {
        const fileName = args.join(' ')
        const normalizedCurrent = currentPath.replace(/\\/g, '/')
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
        result = args.join(' ')
        break

      case 'ver':
        result = 'Microsoft Windows [version 10.0.26100.1742] (Windows 11 24H2)'
        break

      case 'whoami':
        result = 'WINDOWS-11\\eemntirey'
        break

      case 'date':
      case 'time':
        result = new Date().toLocaleString('fr-FR')
        break

      case 'systeminfo':
        result = `Nom de l'hôte:                DESKTOP-WIN11-PRO
Nom du système d'exploitation: Microsoft Windows 11 Professionnel
Version du système:           10.0.26100 Version 24H2
Fabricant du système:         Antigravity High-Performance Systems
Type du système:              PC à architecture x64
Processeur(s):                AMD Ryzen 9 7950X 16-Core @ 4.50 GHz
Mémoire physique totale:      32 768 Mo (32 Go DDR5)
Carte graphique:              NVIDIA GeForce RTX 4090 24 Go
Statut réseau:                Connecté (Wi-Fi 6E / 1.2 Gbps)`
        break

      case 'ping': {
        const host = args[0] || 'google.com'
        result = `Envoi d'une requête 'Ping' sur ${host} [142.250.190.46] avec 32 octets de données :
Réponse de 142.250.190.46 : octets=32 temps=8 ms TTL=118
Réponse de 142.250.190.46 : octets=32 temps=7 ms TTL=118
Réponse de 142.250.190.46 : octets=32 temps=9 ms TTL=118
Réponse de 142.250.190.46 : octets=32 temps=8 ms TTL=118

Statistiques Ping pour 142.250.190.46:
    Paquets : envoyés = 4, reçus = 4, perdus = 0 (perte 0%)`
        break
      }

      case 'matrix':
        result = `01001000 01100101 01101100 01101100 01101111 00100000 01010111 01101111 01110010 01101100 01100100
Bienvenue dans la matrice Windows 11. Réveil de l'agent terminé.`
        break

      default:
        result = `'${cmd}' n'est pas reconnu en tant que commande interne ou externe, un programme exécutable ou un fichier de commandes.`
        isErr = true
    }

    setOutputs(prev => [
      ...prev,
      {
        id: `${Date.now()}`,
        command: `${currentPath}> ${cmdStr}`,
        output: result,
        isError: isErr,
      },
    ])
  }

  return (
    <div
      onClick={() => inputRef.current?.focus()}
      style={{
        height: '100%',
        backgroundColor: '#0c0c0c',
        color: '#cccccc',
        fontFamily: 'var(--font-mono)',
        fontSize: '13px',
        padding: '12px',
        overflow: 'auto',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      {/* Terminal Outputs */}
      {outputs.map(item => (
        <div key={item.id} style={{ marginBottom: '10px' }}>
          {item.command && (
            <div style={{ color: '#ffffff', fontWeight: 500, marginBottom: '2px' }}>
              {item.command}
            </div>
          )}
          {item.output && (
            <pre
              style={{
                margin: 0,
                color: item.isError ? '#f87171' : '#38bdf8',
                whiteSpace: 'pre-wrap',
                fontFamily: 'inherit',
                fontSize: '12.5px',
                lineHeight: '1.4',
              }}
            >
              {item.output}
            </pre>
          )}
        </div>
      ))}

      {/* Active Command Line Input */}
      <div style={{ display: 'flex', alignItems: 'center' }}>
        <span style={{ color: '#ffffff', marginRight: '8px', whiteSpace: 'nowrap' }}>
          {currentPath}&gt;
        </span>
        <input
          ref={inputRef}
          type="text"
          value={inputVal}
          onChange={e => setInputVal(e.target.value)}
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
            caretColor: '#ffffff',
          }}
        />
      </div>

      <div ref={terminalEndRef} />
    </div>
  )
}
