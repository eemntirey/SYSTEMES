import React, { useCallback, useEffect, useMemo, useState } from 'react'
import { NativeBridge } from '../../services/NativeBridge'
import { FluentIcon } from '../common/FluentIcon'

interface NativeProcess {
  id: number
  name: string
  memoryMB: number
  cpuUsagePercent?: number | null
}

interface ProcessSnapshot {
  processes?: NativeProcess[]
  count?: number
  note?: string
}

const protectedProcessNames = new Set([
  'system', 'system idle process', 'registry', 'memory compression',
  'smss', 'csrss', 'wininit', 'services', 'lsass', 'winlogon',
])

const formatMemory = (value: number) =>
  Number.isFinite(value) ? `${value.toLocaleString('fr-FR', { maximumFractionDigits: 1 })} Mo` : '—'

export const ProcessManagerApp: React.FC = () => {
  const [processes, setProcesses] = useState<NativeProcess[]>([])
  const [query, setQuery] = useState('')
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [selectedPid, setSelectedPid] = useState<number | null>(null)
  const [busyPid, setBusyPid] = useState<number | null>(null)

  const refresh = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const snapshot = await NativeBridge.readProcesses() as ProcessSnapshot
      if (!snapshot || !Array.isArray(snapshot.processes)) {
        throw new Error('Le moteur natif a retourné une liste de processus invalide.')
      }
      setProcesses(snapshot.processes)
      setNotice(snapshot.note || null)
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { void refresh() }, [refresh])

  const visibleProcesses = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase('fr')
    return [...processes]
      .filter(process => !normalized || process.name.toLocaleLowerCase('fr').includes(normalized) || String(process.id).includes(normalized))
      .sort((a, b) => b.memoryMB - a.memoryMB)
  }, [processes, query])

  const stopProcess = async (process: NativeProcess) => {
    const normalizedName = process.name.toLocaleLowerCase('fr')
    if (process.id <= 4 || protectedProcessNames.has(normalizedName)) {
      setError('Ce processus Windows essentiel est protégé contre l’arrêt depuis SYSTEMES.')
      return
    }
    const confirmed = window.confirm(`Arrêter réellement « ${process.name} » (PID ${process.id}) ? Les données non enregistrées de cette application peuvent être perdues.`)
    if (!confirmed) return

    setBusyPid(process.id)
    setError(null)
    try {
      await NativeBridge.killProcess(process.id)
      setSelectedPid(null)
      setNotice(`Le processus ${process.name} (PID ${process.id}) a été arrêté.`)
      await refresh()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      setBusyPid(null)
    }
  }

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 14, padding: 20, color: 'var(--text-primary)', background: 'var(--window-bg)', fontFamily: 'var(--font-segoe)', overflow: 'hidden' }}>
      <header style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
        <div>
          <div style={{ fontSize: 20, fontWeight: 700 }}>Gestionnaire des tâches</div>
          <div style={{ fontSize: 12, color: 'var(--text-tertiary)', marginTop: 4 }}>Processus Windows réels · triés par mémoire utilisée</div>
        </div>
        <button type="button" onClick={() => void refresh()} disabled={loading} style={{ display: 'inline-flex', alignItems: 'center', gap: 8, border: '1px solid var(--window-border)', borderRadius: 8, padding: '8px 12px', background: 'var(--flyout-card)', color: 'var(--text-primary)', cursor: loading ? 'wait' : 'pointer' }}>
          <FluentIcon name="refresh-cw" size={15} /> Actualiser
        </button>
      </header>

      <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
        <div style={{ padding: '12px 16px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 150 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Processus détectés</div>
          <div style={{ fontSize: 23, fontWeight: 700 }}>{loading ? '…' : processes.length}</div>
        </div>
        <div style={{ padding: '12px 16px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 150 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Mémoire cumulée listée</div>
          <div style={{ fontSize: 23, fontWeight: 700 }}>{formatMemory(processes.reduce((sum, process) => sum + (process.memoryMB || 0), 0))}</div>
        </div>
      </div>

      <label style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '9px 12px', borderRadius: 8, background: 'var(--flyout-card)' }}>
        <FluentIcon name="search" size={16} />
        <input aria-label="Rechercher un processus" value={query} onChange={event => setQuery(event.target.value)} placeholder="Rechercher par nom ou PID…" style={{ flex: 1, minWidth: 0, border: 0, outline: 0, color: 'var(--text-primary)', background: 'transparent' }} />
      </label>

      {error && <div role="alert" style={{ padding: 10, borderRadius: 8, color: '#fecaca', background: 'rgba(185,28,28,.18)', fontSize: 12 }}>{error}</div>}
      {notice && !error && <div role="status" style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{notice}</div>}

      <div style={{ flex: 1, minHeight: 0, overflow: 'auto', border: '1px solid var(--window-border)', borderRadius: 10 }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
          <thead style={{ position: 'sticky', top: 0, background: 'var(--flyout-card)', zIndex: 1 }}>
            <tr style={{ textAlign: 'left', color: 'var(--text-secondary)' }}>
              <th style={{ padding: 11 }}>Nom du processus</th>
              <th style={{ padding: 11 }}>PID</th>
              <th style={{ padding: 11, textAlign: 'right' }}>Mémoire</th>
              <th style={{ padding: 11, textAlign: 'right' }}>CPU</th>
              <th style={{ padding: 11 }}>Action</th>
            </tr>
          </thead>
          <tbody>
            {visibleProcesses.map(process => {
              const protectedProcess = process.id <= 4 || protectedProcessNames.has(process.name.toLocaleLowerCase('fr'))
              return (
                <tr key={process.id} onClick={() => setSelectedPid(process.id)} style={{ background: selectedPid === process.id ? 'rgba(0,120,212,.14)' : 'transparent', borderTop: '1px solid var(--window-border)' }}>
                  <td style={{ padding: 10, fontWeight: 600 }}>{process.name}</td>
                  <td style={{ padding: 10, color: 'var(--text-secondary)', fontVariantNumeric: 'tabular-nums' }}>{process.id}</td>
                  <td style={{ padding: 10, textAlign: 'right', fontVariantNumeric: 'tabular-nums' }}>{formatMemory(process.memoryMB)}</td>
                  <td style={{ padding: 10, textAlign: 'right', color: 'var(--text-tertiary)' }}>{process.cpuUsagePercent == null ? 'Non mesuré' : `${process.cpuUsagePercent.toFixed(1)} %`}</td>
                  <td style={{ padding: 8 }}>
                    <button type="button" disabled={protectedProcess || busyPid === process.id} title={protectedProcess ? 'Processus Windows protégé' : 'Arrêter ce processus'} onClick={event => { event.stopPropagation(); void stopProcess(process) }} style={{ padding: '6px 9px', border: 0, borderRadius: 6, background: protectedProcess ? 'var(--control-hover)' : 'rgba(185,28,28,.16)', color: protectedProcess ? 'var(--text-tertiary)' : '#fca5a5', cursor: protectedProcess ? 'not-allowed' : 'pointer', fontSize: 11 }}>
                      {busyPid === process.id ? 'Arrêt…' : 'Fin de tâche'}
                    </button>
                  </td>
                </tr>
              )
            })}
          </tbody>
        </table>
        {!loading && visibleProcesses.length === 0 && <div style={{ padding: 28, textAlign: 'center', color: 'var(--text-tertiary)' }}>Aucun processus correspondant.</div>}
        {loading && <div style={{ padding: 28, textAlign: 'center', color: 'var(--text-tertiary)' }}>Lecture des processus Windows…</div>}
      </div>
      <footer style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>La mesure CPU n’est pas encore fournie par le moteur natif. Les processus Windows essentiels sont protégés. Toute fin de tâche nécessite une confirmation.</footer>
    </div>
  )
}
