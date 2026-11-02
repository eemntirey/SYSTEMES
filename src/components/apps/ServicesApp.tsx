import React, { useCallback, useEffect, useMemo, useState } from 'react'
import { FluentIcon } from '../common/FluentIcon'
import {
  readServices,
  startService,
  stopService,
  type ServiceInfo,
} from '../../native/services'

type SortKey = 'name' | 'status' | 'startMode'
type FilterKey = 'all' | 'running' | 'stopped'

const stateColor = (state: string) => {
  const s = state.toLowerCase()
  if (s === 'running') return '#3fb950'
  if (s === 'stopped') return '#8b949e'
  if (s.includes('pending')) return '#d29922'
  return 'var(--text-secondary)'
}

const stateLabel = (state: string) => {
  const s = state.toLowerCase()
  if (s === 'running') return 'En cours d’exécution'
  if (s === 'stopped') return 'Arrêté'
  if (s === 'start pending') return 'Démarrage…'
  if (s === 'stop pending') return 'Arrêt…'
  return state
}

export const ServicesApp: React.FC = () => {
  const [services, setServices] = useState<ServiceInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [query, setQuery] = useState('')
  const [filter, setFilter] = useState<FilterKey>('all')
  const [sortBy, setSortBy] = useState<SortKey>('name')
  const [selected, setSelected] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [source, setSource] = useState<string>('')

  const refresh = useCallback(async (silent = false) => {
    if (!silent) setLoading(true)
    setError(null)
    try {
      const snap = await readServices()
      setServices(snap.services || [])
      setSource(snap.source || 'WMI')
      if (selected && !(snap.services || []).some(s => s.name === selected)) {
        setSelected(null)
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      if (!silent) setLoading(false)
    }
  }, [selected])

  useEffect(() => {
    void refresh()
  }, [refresh])

  const filtered = useMemo(() => {
    const q = query.trim().toLocaleLowerCase('fr')
    let list = services
    if (filter === 'running') list = list.filter(s => s.state.toLowerCase() === 'running')
    if (filter === 'stopped') list = list.filter(s => s.state.toLowerCase() === 'stopped')
    if (q) {
      list = list.filter(
        s =>
          s.name.toLocaleLowerCase('fr').includes(q) ||
          s.displayName.toLocaleLowerCase('fr').includes(q),
      )
    }
    const sorted = [...list]
    sorted.sort((a, b) => {
      if (sortBy === 'status') return a.state.localeCompare(b.state, 'fr') || a.displayName.localeCompare(b.displayName, 'fr')
      if (sortBy === 'startMode') return a.startMode.localeCompare(b.startMode, 'fr') || a.displayName.localeCompare(b.displayName, 'fr')
      return a.displayName.localeCompare(b.displayName, 'fr')
    })
    return sorted
  }, [services, query, filter, sortBy])

  const selectedService = selected ? services.find(s => s.name === selected) : null

  const runAction = async (name: string, action: 'start' | 'stop') => {
    setBusy(name)
    setNotice(null)
    setError(null)
    try {
      const result = action === 'start' ? await startService(name) : await stopService(name)
      setNotice(
        result.changed
          ? `« ${result.displayName || name} » → ${stateLabel(result.state)}`
          : result.note || `Aucun changement pour ${name}`,
      )
      await refresh(true)
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(null)
    }
  }

  const counts = useMemo(() => {
    const running = services.filter(s => s.state.toLowerCase() === 'running').length
    const stopped = services.filter(s => s.state.toLowerCase() === 'stopped').length
    return { running, stopped, total: services.length }
  }, [services])

  const shell: React.CSSProperties = {
    display: 'flex',
    flexDirection: 'column',
    height: '100%',
    minHeight: 0,
    background: 'var(--window-bg)',
    color: 'var(--text-primary)',
    fontFamily: 'var(--font-segoe, Segoe UI, sans-serif)',
  }
  const toolbar: React.CSSProperties = {
    display: 'flex',
    flexWrap: 'wrap',
    gap: 10,
    alignItems: 'center',
    padding: '12px 14px',
    borderBottom: '1px solid var(--window-border)',
    background: 'var(--flyout-card)',
  }
  const btn: React.CSSProperties = {
    border: '1px solid var(--window-border)',
    borderRadius: 6,
    padding: '6px 12px',
    background: 'var(--window-bg)',
    color: 'var(--text-primary)',
    cursor: 'pointer',
    fontSize: 12,
  }
  const input: React.CSSProperties = {
    ...btn,
    minWidth: 200,
    flex: 1,
    maxWidth: 320,
    cursor: 'text',
  }

  return (
    <div style={shell}>
      <div style={toolbar}>
        <FluentIcon name="activity" size={18} color="var(--win-accent)" />
        <div style={{ fontWeight: 650, fontSize: 14, marginRight: 8 }}>Services</div>
        <input
          aria-label="Rechercher un service"
          placeholder="Rechercher nom ou affichage…"
          value={query}
          onChange={e => setQuery(e.target.value)}
          style={input}
        />
        <select
          aria-label="Filtrer"
          value={filter}
          onChange={e => setFilter(e.target.value as FilterKey)}
          style={btn}
        >
          <option value="all">Tous ({counts.total})</option>
          <option value="running">En cours ({counts.running})</option>
          <option value="stopped">Arrêtés ({counts.stopped})</option>
        </select>
        <select
          aria-label="Trier"
          value={sortBy}
          onChange={e => setSortBy(e.target.value as SortKey)}
          style={btn}
        >
          <option value="name">Nom</option>
          <option value="status">État</option>
          <option value="startMode">Type de démarrage</option>
        </select>
        <button type="button" style={btn} onClick={() => void refresh()} disabled={loading}>
          Actualiser
        </button>
        <div style={{ flex: 1 }} />
        <span style={{ fontSize: 11, color: 'var(--text-secondary)' }}>{source || '—'}</span>
      </div>

      {(error || notice) && (
        <div
          style={{
            padding: '8px 14px',
            fontSize: 12,
            background: error ? 'rgba(248,81,73,0.12)' : 'rgba(63,185,80,0.12)',
            color: error ? '#f85149' : '#3fb950',
            borderBottom: '1px solid var(--window-border)',
          }}
        >
          {error || notice}
        </div>
      )}

      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        <div style={{ flex: 1, overflow: 'auto', minWidth: 0 }}>
          {loading && services.length === 0 ? (
            <div style={{ padding: 24, color: 'var(--text-secondary)' }}>Lecture WMI des services…</div>
          ) : (
            <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
              <thead>
                <tr style={{ position: 'sticky', top: 0, background: 'var(--flyout-card)', zIndex: 1 }}>
                  {['Nom', 'État', 'Type de démarrage', 'Compte'].map(h => (
                    <th
                      key={h}
                      style={{
                        textAlign: 'left',
                        padding: '8px 12px',
                        borderBottom: '1px solid var(--window-border)',
                        fontWeight: 600,
                        color: 'var(--text-secondary)',
                      }}
                    >
                      {h}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {filtered.map(svc => {
                  const isSel = selected === svc.name
                  return (
                    <tr
                      key={svc.name}
                      onClick={() => setSelected(svc.name)}
                      style={{
                        cursor: 'pointer',
                        background: isSel ? 'rgba(139,92,246,0.16)' : 'transparent',
                        borderBottom: '1px solid rgba(255,255,255,0.04)',
                      }}
                    >
                      <td style={{ padding: '8px 12px' }}>
                        <div style={{ fontWeight: 600 }}>{svc.displayName || svc.name}</div>
                        <div style={{ color: 'var(--text-secondary)', fontSize: 11 }}>{svc.name}</div>
                      </td>
                      <td style={{ padding: '8px 12px' }}>
                        <span style={{ color: stateColor(svc.state), fontWeight: 600 }}>
                          {stateLabel(svc.state)}
                        </span>
                        {svc.protectedService && (
                          <span style={{ marginLeft: 8, fontSize: 10, color: 'var(--text-secondary)' }}>
                            protégé
                          </span>
                        )}
                      </td>
                      <td style={{ padding: '8px 12px', color: 'var(--text-secondary)' }}>{svc.startMode}</td>
                      <td style={{ padding: '8px 12px', color: 'var(--text-secondary)', maxWidth: 160, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                        {svc.account || '—'}
                      </td>
                    </tr>
                  )
                })}
                {filtered.length === 0 && !loading && (
                  <tr>
                    <td colSpan={4} style={{ padding: 24, color: 'var(--text-secondary)' }}>
                      Aucun service ne correspond au filtre.
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
          )}
        </div>

        <aside
          style={{
            width: 280,
            flexShrink: 0,
            borderLeft: '1px solid var(--window-border)',
            padding: 16,
            background: 'var(--flyout-card)',
            overflow: 'auto',
          }}
        >
          {!selectedService ? (
            <div style={{ color: 'var(--text-secondary)', fontSize: 12, lineHeight: 1.5 }}>
              Sélectionnez un service pour voir les détails et démarrer ou arrêter (droits administrateur souvent requis).
            </div>
          ) : (
            <>
              <div style={{ fontSize: 15, fontWeight: 650, marginBottom: 4 }}>
                {selectedService.displayName}
              </div>
              <div style={{ fontSize: 12, color: 'var(--text-secondary)', marginBottom: 14 }}>
                {selectedService.name}
              </div>

              <div style={{ fontSize: 12, display: 'grid', gap: 8, marginBottom: 16 }}>
                <div>
                  <span style={{ color: 'var(--text-secondary)' }}>État · </span>
                  <span style={{ color: stateColor(selectedService.state), fontWeight: 600 }}>
                    {stateLabel(selectedService.state)}
                  </span>
                </div>
                <div>
                  <span style={{ color: 'var(--text-secondary)' }}>Démarrage · </span>
                  {selectedService.startMode}
                </div>
                <div>
                  <span style={{ color: 'var(--text-secondary)' }}>PID · </span>
                  {selectedService.processId || '—'}
                </div>
                <div style={{ wordBreak: 'break-all' }}>
                  <span style={{ color: 'var(--text-secondary)' }}>Compte · </span>
                  {selectedService.account || '—'}
                </div>
                {selectedService.executablePath && (
                  <div style={{ wordBreak: 'break-all', fontSize: 11, color: 'var(--text-secondary)' }}>
                    {selectedService.executablePath}
                  </div>
                )}
              </div>

              {selectedService.protectedService ? (
                <div style={{ fontSize: 12, color: '#d29922', lineHeight: 1.45 }}>
                  Service protégé : le démarrage / arrêt est refusé pour éviter d’instabiliser Windows.
                </div>
              ) : (
                <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                  <button
                    type="button"
                    style={{
                      ...btn,
                      background: 'var(--win-accent)',
                      borderColor: 'var(--win-accent)',
                      color: '#fff',
                      opacity:
                        busy === selectedService.name || selectedService.state.toLowerCase() === 'running'
                          ? 0.5
                          : 1,
                    }}
                    disabled={
                      busy === selectedService.name || selectedService.state.toLowerCase() === 'running'
                    }
                    onClick={() => void runAction(selectedService.name, 'start')}
                  >
                    {busy === selectedService.name ? 'Patientez…' : 'Démarrer'}
                  </button>
                  <button
                    type="button"
                    style={{
                      ...btn,
                      opacity:
                        busy === selectedService.name || selectedService.state.toLowerCase() === 'stopped'
                          ? 0.5
                          : 1,
                    }}
                    disabled={
                      busy === selectedService.name || selectedService.state.toLowerCase() === 'stopped'
                    }
                    onClick={() => void runAction(selectedService.name, 'stop')}
                  >
                    {busy === selectedService.name ? 'Patientez…' : 'Arrêter'}
                  </button>
                </div>
              )}
            </>
          )}
        </aside>
      </div>
    </div>
  )
}
