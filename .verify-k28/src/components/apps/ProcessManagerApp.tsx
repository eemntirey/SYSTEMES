import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { NativeBridge } from '../../services/NativeBridge'
import { FluentIcon } from '../common/FluentIcon'

interface NativeProcess {
  id: number
  name: string
  memoryMB: number
  cpuUsagePercent?: number | null
  cpuSampleAvailable?: boolean
  threadCount?: number
  sessionId?: number
  startTimeUtcTicks?: string | null
  executablePath?: string | null
}
interface NativePerformanceSnapshot {
  disks?: { name: string; activePercent: number; readBytesPerSecond: number; writeBytesPerSecond: number }[]
  networks?: { name: string; receiveBytesPerSecond: number; sendBytesPerSecond: number }[]
  gpus?: { adapter: string; utilizationPercent: number | null; dedicatedMemoryBytes: number | null; sharedMemoryBytes: number | null }[]
  diskError?: string | null
  networkError?: string | null
  gpuError?: string | null
  sampledAtUtc?: string
  source?: string
}
interface ProcessSnapshot {
  processes?: NativeProcess[]
  count?: number
  logicalProcessorCount?: number
  systemCpuUsagePercent?: number | null
  performance?: NativePerformanceSnapshot
  note?: string
}
type NativeMemorySnapshot = Awaited<ReturnType<typeof NativeBridge.readMemory>>
const formatRate = (bytesPerSecond: number) => (bytesPerSecond / (1024 ** 2)).toLocaleString('fr-FR', { maximumFractionDigits: 1 }) + ' Mio/s'
const formatGiB = (bytes: number) => (bytes / (1024 ** 3)).toLocaleString('fr-FR', { maximumFractionDigits: 1 })
const formatBytes = (bytes?: number | null) => bytes == null
  ? '—'
  : bytes >= 1024 ** 3
    ? `${(bytes / (1024 ** 3)).toLocaleString('fr-FR', { maximumFractionDigits: 1 })} Gio`
    : `${(bytes / (1024 ** 2)).toLocaleString('fr-FR', { maximumFractionDigits: 0 })} Mio`
type SortMode = 'cpu' | 'memory' | 'name'

const protectedProcessNames = new Set([
  'system', 'system idle process', 'registry', 'memory compression',
  'smss', 'csrss', 'wininit', 'services', 'lsass', 'winlogon', 'secure system', 'svchost', 'fontdrvhost', 'dwm',
])
const formatMemory = (value: number) =>
  Number.isFinite(value) ? `${value.toLocaleString('fr-FR', { maximumFractionDigits: 1 })} Mo` : '—'
const formatCpu = (value?: number | null) =>
  value == null ? 'Échantillonnage…' : `${value.toLocaleString('fr-FR', { maximumFractionDigits: 1 })} %`

export const ProcessManagerApp: React.FC = () => {
  const [processes, setProcesses] = useState<NativeProcess[]>([])
  const [query, setQuery] = useState('')
  const [sortBy, setSortBy] = useState<SortMode>('cpu')
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [selectedPid, setSelectedPid] = useState<number | null>(null)
  const [busyPid, setBusyPid] = useState<number | null>(null)
  const [logicalProcessors, setLogicalProcessors] = useState<number | null>(null)
  const [systemCpuPercent, setSystemCpuPercent] = useState<number | null>(null)
  const [memory, setMemory] = useState<NativeMemorySnapshot | null>(null)
  const [performance, setPerformance] = useState<NativePerformanceSnapshot | null>(null)
  const refreshing = useRef(false)

  const refresh = useCallback(async (silent = false) => {
    if (refreshing.current) return
    refreshing.current = true
    if (!silent) setLoading(true)
    setError(null)
    try {
      const [processResult, memoryResult] = await Promise.allSettled([
        NativeBridge.readProcesses() as Promise<ProcessSnapshot>,
        NativeBridge.readMemory(),
      ])
      if (processResult.status !== 'fulfilled') throw processResult.reason
      const snapshot = processResult.value
      if (!snapshot || !Array.isArray(snapshot.processes)) {
        throw new Error('Le moteur natif a retourné une liste de processus invalide.')
      }
      setProcesses(snapshot.processes)
      setLogicalProcessors(snapshot.logicalProcessorCount ?? null)
      setSystemCpuPercent(Number.isFinite(snapshot.systemCpuUsagePercent) ? snapshot.systemCpuUsagePercent! : null)
      setPerformance(snapshot.performance ?? null)

      if (memoryResult.status === 'fulfilled') {
        const memorySnapshot = memoryResult.value
        if (!Number.isFinite(memorySnapshot.totalPhysicalBytes) ||
            !Number.isFinite(memorySnapshot.availablePhysicalBytes) ||
            memorySnapshot.totalPhysicalBytes <= 0 ||
            memorySnapshot.availablePhysicalBytes < 0 ||
            memorySnapshot.availablePhysicalBytes > memorySnapshot.totalPhysicalBytes ||
            memorySnapshot.memoryLoadPercent < 0 ||
            memorySnapshot.memoryLoadPercent > 100) {
          setMemory(null)
          setError('Les compteurs mémoire Windows retournés sont invalides.')
        } else {
          setMemory(memorySnapshot)
        }
      } else {
        setMemory(null)
        setError(`Mesure de la RAM indisponible : ${memoryResult.reason instanceof Error ? memoryResult.reason.message : String(memoryResult.reason)}`)
      }
      setNotice(snapshot.note || 'Processus Windows réels · mémoire physique mesurée via GlobalMemoryStatusEx.')
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause))
    } finally {
      refreshing.current = false
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refresh()
    const timer = window.setInterval(() => {
      if (document.visibilityState === 'visible') void refresh(true)
    }, 2000)
    return () => window.clearInterval(timer)
  }, [refresh])

  const visibleProcesses = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase('fr')
    return [...processes]
      .filter(process => !normalized ||
        process.name.toLocaleLowerCase('fr').includes(normalized) ||
        String(process.id).includes(normalized))
      .sort((a, b) => {
        if (sortBy === 'name') return a.name.localeCompare(b.name, 'fr', { sensitivity: 'base' })
        if (sortBy === 'memory') return b.memoryMB - a.memoryMB
        return (b.cpuUsagePercent ?? -1) - (a.cpuUsagePercent ?? -1)
      })
  }, [processes, query, sortBy])

  const selectedProcess = processes.find(process => process.id === selectedPid) ?? null
  const usedMemoryPercent = memory?.memoryLoadPercent ?? null
  const sampledCount = processes.filter(process => process.cpuUsagePercent != null).length

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
      if (!process.startTimeUtcTicks) throw new Error('Le démarrage du processus ne peut pas être vérifié. Actualisez la liste.')
      const result = await NativeBridge.killProcess(process.id, process.name, process.startTimeUtcTicks) as { killed?: boolean }
      if (result?.killed !== true) throw new Error('Windows n’a pas confirmé l’arrêt du processus.')
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
          <div style={{ fontSize: 12, color: 'var(--text-tertiary)', marginTop: 4 }}>Processus Windows réels · actualisation automatique toutes les 2 secondes</div>
        </div>
        <button type="button" onClick={() => void refresh()} disabled={loading} style={{ display: 'inline-flex', alignItems: 'center', gap: 8, border: '1px solid var(--window-border)', borderRadius: 8, padding: '8px 12px', background: 'var(--flyout-card)', color: 'var(--text-primary)', cursor: loading ? 'wait' : 'pointer' }}>
          <FluentIcon name="refresh-cw" size={15} /> Actualiser
        </button>
      </header>

      <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
        <div style={{ padding: '12px 16px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 145 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Processus détectés</div>
          <div style={{ fontSize: 23, fontWeight: 700 }}>{loading ? '…' : processes.length}</div>
        </div>
        <div style={{ padding: '12px 16px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 145 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>RAM physique utilisée</div>
          <div style={{ fontSize: 23, fontWeight: 700 }}>{memory ? `${formatGiB(memory.usedPhysicalBytes)} / ${formatGiB(memory.totalPhysicalBytes)} Gio` : '—'}</div>
          <div style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>{usedMemoryPercent == null ? 'Mesure indisponible' : `${usedMemoryPercent} % utilisés · ${formatGiB(memory!.availablePhysicalBytes)} Gio disponibles`}</div>
        </div>
        <div style={{ padding: '12px 16px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 145 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>CPU des processus</div>
          <div style={{ fontSize: 23, fontWeight: 700 }}>{systemCpuPercent == null ? 'Échantillonnage…' : formatCpu(systemCpuPercent)}</div>
          <div style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>{systemCpuPercent == null ? 'Première mesure en cours' : 'Charge totale du processeur'} · {logicalProcessors ?? '—'} processeurs logiques · {sampledCount}/{processes.length} processus échantillonnés</div>
        </div>
      </div>

      <div style={{ display: 'flex', gap: 12, flexWrap: 'wrap' }}>
        <div style={{ padding: '11px 14px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 170, flex: 1 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>GPU · moteur le plus actif</div>
          <div style={{ fontSize: 19, fontWeight: 700 }}>{performance?.gpus?.some(gpu => gpu.utilizationPercent != null) ? `${Math.max(...performance.gpus.map(gpu => gpu.utilizationPercent ?? 0)).toLocaleString('fr-FR', { maximumFractionDigits: 1 })} %` : 'Indisponible'}</div>
          <div style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>{performance?.gpus?.length ? `${performance.gpus.length} adaptateur(s) / moteur(s) détecté(s)` : performance?.gpuError || 'Compteur GPU Windows non exposé'}</div>
        </div>
        <div style={{ padding: '11px 14px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 170, flex: 1 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Disque · activité maximale</div>
          <div style={{ fontSize: 19, fontWeight: 700 }}>{performance?.disks?.length ? `${Math.max(...performance.disks.map(disk => disk.activePercent)).toLocaleString('fr-FR', { maximumFractionDigits: 1 })} %` : 'Indisponible'}</div>
          <div style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>{performance?.disks?.length ? formatRate(performance.disks.reduce((sum, disk) => sum + disk.readBytesPerSecond + disk.writeBytesPerSecond, 0)) + ' lecture + écriture' : performance?.diskError || 'Compteur disque indisponible'}</div>
        </div>
        <div style={{ padding: '11px 14px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 170, flex: 1 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Réseau · réception / envoi</div>
          <div style={{ fontSize: 19, fontWeight: 700 }}>{performance?.networks?.length ? formatRate(performance.networks.reduce((sum, item) => sum + item.receiveBytesPerSecond, 0)) : 'Indisponible'}</div>
          <div style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>{performance?.networks?.length ? `${formatRate(performance.networks.reduce((sum, item) => sum + item.sendBytesPerSecond, 0))} envoyés` : performance?.networkError || 'Compteur réseau indisponible'}</div>
        </div>
      </div>
      <details style={{ border: '1px solid var(--window-border)', borderRadius: 10, background: 'var(--flyout-card)', padding: '9px 12px' }}>
        <summary style={{ cursor: 'pointer', fontSize: 12, fontWeight: 650, color: 'var(--text-secondary)' }}>
          Détail des adaptateurs et compteurs · {performance?.sampledAtUtc ? new Date(performance.sampledAtUtc).toLocaleTimeString('fr-FR') : 'échantillon courant'}
        </summary>
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(220px, 1fr))', gap: 12, paddingTop: 10 }}>
          <section style={{ minWidth: 0 }}>
            <strong style={{ fontSize: 11 }}>GPU</strong>
            {performance?.gpus?.length ? performance.gpus.map(gpu => (
              <div key={gpu.adapter} style={{ fontSize: 11, paddingTop: 5, overflowWrap: 'anywhere' }}>
                <div>{gpu.adapter}</div>
                <div style={{ color: 'var(--text-tertiary)' }}>
                  {gpu.utilizationPercent == null ? 'Utilisation indisponible' : `Charge ${gpu.utilizationPercent.toLocaleString('fr-FR', { maximumFractionDigits: 1 })} %`}
                  {' · VRAM dédiée '}{formatBytes(gpu.dedicatedMemoryBytes)}
                  {' · mémoire partagée '}{formatBytes(gpu.sharedMemoryBytes)}
                </div>
              </div>
            )) : <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{performance?.gpuError || 'Aucun compteur GPU exploitable.'}</div>}
          </section>
          <section style={{ minWidth: 0 }}>
            <strong style={{ fontSize: 11 }}>Disques physiques</strong>
            {performance?.disks?.length ? performance.disks.map(disk => (
              <div key={disk.name} style={{ fontSize: 11, paddingTop: 5 }}>
                <div>{disk.name} · {disk.activePercent.toLocaleString('fr-FR', { maximumFractionDigits: 1 })} % actif</div>
                <div style={{ color: 'var(--text-tertiary)' }}>Lecture {formatRate(disk.readBytesPerSecond)} · Écriture {formatRate(disk.writeBytesPerSecond)}</div>
              </div>
            )) : <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{performance?.diskError || 'Aucun compteur disque exploitable.'}</div>}
          </section>
          <section style={{ minWidth: 0 }}>
            <strong style={{ fontSize: 11 }}>Interfaces réseau</strong>
            {performance?.networks?.length ? performance.networks.map(network => (
              <div key={network.name} style={{ fontSize: 11, paddingTop: 5, overflowWrap: 'anywhere' }}>
                <div>{network.name}</div>
                <div style={{ color: 'var(--text-tertiary)' }}>Reçu {formatRate(network.receiveBytesPerSecond)} · Envoyé {formatRate(network.sendBytesPerSecond)}</div>
              </div>
            )) : <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{performance?.networkError || 'Aucun compteur réseau exploitable.'}</div>}
          </section>
        </div>
        <div style={{ fontSize: 10, color: 'var(--text-tertiary)', paddingTop: 8 }}>
          Source : {performance?.source || 'compteurs de performances Windows'} · Les compteurs non fournis par le pilote restent indiqués comme indisponibles.
        </div>
      </details>

      <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '9px 12px', borderRadius: 8, background: 'var(--flyout-card)', flex: 1, minWidth: 220 }}>
          <FluentIcon name="search" size={16} />
          <input aria-label="Rechercher un processus" value={query} onChange={event => setQuery(event.target.value)} placeholder="Rechercher par nom ou PID…" style={{ flex: 1, minWidth: 0, border: 0, outline: 0, color: 'var(--text-primary)', background: 'transparent' }} />
        </label>
        <label style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '0 10px', borderRadius: 8, background: 'var(--flyout-card)', fontSize: 11, color: 'var(--text-secondary)' }}>
          Trier
          <select aria-label="Trier les processus" value={sortBy} onChange={event => setSortBy(event.target.value as SortMode)} style={{ padding: 8, border: 0, background: 'transparent', color: 'var(--text-primary)' }}>
            <option value="cpu">CPU</option><option value="memory">Mémoire</option><option value="name">Nom</option>
          </select>
        </label>
      </div>

      {error && <div role="alert" style={{ padding: 10, borderRadius: 8, color: '#fecaca', background: 'rgba(185,28,28,.18)', fontSize: 12 }}>{error}</div>}
      {notice && !error && <div role="status" style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{notice}</div>}

      <div style={{ flex: 1, minHeight: 0, overflow: 'auto', border: '1px solid var(--window-border)', borderRadius: 10 }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
          <thead style={{ position: 'sticky', top: 0, background: 'var(--flyout-card)', zIndex: 1 }}>
            <tr style={{ textAlign: 'left', color: 'var(--text-secondary)' }}>
              <th style={{ padding: 11 }}>Nom du processus</th>
              <th style={{ padding: 11 }}>PID</th>
              <th style={{ padding: 11, textAlign: 'right' }}>CPU</th>
              <th style={{ padding: 11, textAlign: 'right' }}>Mémoire</th>
              <th style={{ padding: 11, textAlign: 'right' }}>Threads</th>
              <th style={{ padding: 11 }}>Action</th>
            </tr>
          </thead>
          <tbody>
            {visibleProcesses.map(process => {
              const protectedProcess = process.id <= 4 || protectedProcessNames.has(process.name.toLocaleLowerCase('fr'))
              return (
                <tr key={process.id} onClick={() => setSelectedPid(process.id)} aria-selected={selectedPid === process.id} style={{ cursor: 'pointer', background: selectedPid === process.id ? 'rgba(0,120,212,.14)' : 'transparent', borderTop: '1px solid var(--window-border)' }}>
                  <td style={{ padding: 10, fontWeight: 600 }}>{process.name}</td>
                  <td style={{ padding: 10, color: 'var(--text-secondary)', fontVariantNumeric: 'tabular-nums' }}>{process.id}</td>
                  <td style={{ padding: 10, textAlign: 'right', color: process.cpuUsagePercent == null ? 'var(--text-tertiary)' : 'var(--text-primary)', fontVariantNumeric: 'tabular-nums' }}>{formatCpu(process.cpuUsagePercent)}</td>
                  <td style={{ padding: 10, textAlign: 'right', fontVariantNumeric: 'tabular-nums' }}>{formatMemory(process.memoryMB)}</td>
                  <td style={{ padding: 10, textAlign: 'right' }}>{process.threadCount ?? '—'}</td>
                  <td style={{ padding: 8 }}>
                    <button type="button" disabled={protectedProcess || busyPid === process.id || !process.startTimeUtcTicks} title={protectedProcess ? 'Processus Windows protégé' : !process.startTimeUtcTicks ? 'Heure de démarrage inaccessible : arrêt désactivé' : 'Arrêter ce processus'} onClick={event => { event.stopPropagation(); void stopProcess(process) }} style={{ padding: '6px 9px', border: 0, borderRadius: 6, background: protectedProcess || !process.startTimeUtcTicks ? 'var(--control-hover)' : 'rgba(185,28,28,.16)', color: protectedProcess || !process.startTimeUtcTicks ? 'var(--text-tertiary)' : '#fca5a5', cursor: protectedProcess || !process.startTimeUtcTicks ? 'not-allowed' : 'pointer', fontSize: 11 }}>
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

      {selectedProcess && (
        <section aria-label="Détails du processus sélectionné" style={{ padding: 12, borderRadius: 10, background: 'var(--flyout-card)', border: '1px solid var(--window-border)', display: 'grid', gap: 5, fontSize: 11, minWidth: 0 }}>
          <strong style={{ fontSize: 13 }}>{selectedProcess.name} · PID {selectedProcess.id}</strong>
          <span>Session Windows : {selectedProcess.sessionId ?? 'Non disponible'} · Threads : {selectedProcess.threadCount ?? 'Non disponible'}</span>
          <span>CPU : {formatCpu(selectedProcess.cpuUsagePercent)} · Mémoire : {formatMemory(selectedProcess.memoryMB)}</span>
          <span style={{ overflowWrap: 'anywhere', color: 'var(--text-tertiary)' }}>Exécutable : {selectedProcess.executablePath || 'Chemin non accessible avec les droits actuels'}</span>
        </section>
      )}
      <footer style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>CPU calculé à partir de la variation du temps processeur entre deux échantillons, normalisé sur le nombre de processeurs logiques. Les premières valeurs apparaissent après une seconde lecture. L’arrêt est protégé côté interface et côté moteur natif.</footer>
    </div>
  )
}
