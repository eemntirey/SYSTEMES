import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { NativeBridge } from '../../services/NativeBridge';
import { FluentIcon } from '../common/FluentIcon';

interface WindowsService {
  name: string;
  displayName: string;
  state: string;
  startMode: string;
  account: string;
  processId: number;
  executablePath: string;
  isProtected: boolean;
}

interface ServiceSnapshot {
  services: WindowsService[];
  count: number;
  source: string;
  note: string;
}

type ServiceAction = 'start' | 'stop';

const stateLabel = (state: string) => {
  switch (state.toLowerCase()) {
    case 'running': return 'En cours d’exécution';
    case 'stopped': return 'Arrêté';
    case 'paused': return 'En pause';
    case 'start pending': return 'Démarrage…';
    case 'stop pending': return 'Arrêt…';
    default: return state || 'Inconnu';
  }
};

const modeLabel = (mode: string) => {
  switch (mode.toLowerCase()) {
    case 'auto': return 'Automatique';
    case 'manual': return 'Manuel';
    case 'disabled': return 'Désactivé';
    case 'boot': return 'Démarrage';
    case 'system': return 'Système';
    default: return mode || 'Inconnu';
  }
};

export const ControlPanelApp: React.FC = () => {
  const [snapshot, setSnapshot] = useState<ServiceSnapshot | null>(null);
  const [query, setQuery] = useState('');
  const [loading, setLoading] = useState(true);
  const [busyService, setBusyService] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const result = await NativeBridge.readServices() as ServiceSnapshot;
      if (!result || !Array.isArray(result.services)) {
        throw new Error('Le moteur natif a retourné un inventaire de services invalide.');
      }
      setSnapshot(result);
      setNotice(result.note || 'Inventaire lu directement depuis Windows.');
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { void refresh(); }, [refresh]);

  const visibleServices = useMemo(() => {
    const normalized = query.trim().toLocaleLowerCase('fr');
    return [...(snapshot?.services ?? [])]
      .filter(service => !normalized ||
        service.name.toLocaleLowerCase('fr').includes(normalized) ||
        service.displayName.toLocaleLowerCase('fr').includes(normalized))
      .sort((a, b) => a.displayName.localeCompare(b.displayName, 'fr', { sensitivity: 'base' }));
  }, [snapshot, query]);

  const control = async (service: WindowsService, action: ServiceAction) => {
    const verb = action === 'start' ? 'démarrer' : 'arrêter';
    if (service.isProtected && action === 'stop') {
      setError('Ce service est protégé contre l’arrêt depuis SYSTEMES.');
      return;
    }
    if (!window.confirm(`Confirmer : ${verb} réellement le service « ${service.displayName} » (${service.name}) ? Certaines applications peuvent être affectées.`)) return;

    setBusyService(service.name);
    setError(null);
    setNotice(null);
    try {
      await NativeBridge.controlService(service.name, action);
      setNotice(`Commande « ${verb} » envoyée. Le nouvel état sera confirmé par Windows.`);
      await refresh();
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusyService(null);
    }
  };

  const runningCount = snapshot?.services.filter(service => service.state.toLowerCase() === 'running').length ?? 0;

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 14, padding: 20, color: 'var(--text-primary)', background: 'var(--window-bg)', fontFamily: 'var(--font-segoe)', overflow: 'hidden' }}>
      <header style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
        <div>
          <div style={{ fontSize: 20, fontWeight: 700 }}>Services Windows</div>
          <div style={{ fontSize: 12, color: 'var(--text-tertiary)', marginTop: 4 }}>Gestion native via WMI et Service Control Manager · aucune modification automatique</div>
        </div>
        <button type="button" onClick={() => void refresh()} disabled={loading || busyService !== null} style={{ display: 'inline-flex', alignItems: 'center', gap: 8, border: '1px solid var(--window-border)', borderRadius: 8, padding: '8px 12px', background: 'var(--flyout-card)', color: 'var(--text-primary)', cursor: loading ? 'wait' : 'pointer' }}>
          <FluentIcon name="refresh-cw" size={15} /> Actualiser
        </button>
      </header>

      <div style={{ display: 'flex', gap: 10, flexWrap: 'wrap' }}>
        <div style={{ padding: '11px 15px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 130 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Services détectés</div>
          <div style={{ fontSize: 22, fontWeight: 700 }}>{snapshot?.count ?? '—'}</div>
        </div>
        <div style={{ padding: '11px 15px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 130 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>En cours d’exécution</div>
          <div style={{ fontSize: 22, fontWeight: 700 }}>{snapshot ? runningCount : '—'}</div>
        </div>
        <div style={{ padding: '11px 15px', borderRadius: 10, background: 'var(--flyout-card)', minWidth: 130 }}>
          <div style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Protégés</div>
          <div style={{ fontSize: 22, fontWeight: 700 }}>{snapshot ? snapshot.services.filter(service => service.isProtected).length : '—'}</div>
        </div>
      </div>

      <label style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '9px 12px', borderRadius: 8, background: 'var(--flyout-card)' }}>
        <FluentIcon name="search" size={16} />
        <input aria-label="Rechercher un service" value={query} onChange={event => setQuery(event.target.value)} placeholder="Rechercher par nom ou nom système…" style={{ flex: 1, minWidth: 0, border: 0, outline: 0, color: 'var(--text-primary)', background: 'transparent' }} />
      </label>

      {error && <div role="alert" style={{ padding: 10, borderRadius: 8, color: '#fecaca', background: 'rgba(185,28,28,.18)', fontSize: 12 }}>{error}</div>}
      {notice && !error && <div role="status" style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>{notice}</div>}

      <div style={{ flex: 1, minHeight: 0, overflow: 'auto', border: '1px solid var(--window-border)', borderRadius: 10 }}>
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 12 }}>
          <thead style={{ position: 'sticky', top: 0, background: 'var(--flyout-card)', zIndex: 1 }}>
            <tr style={{ textAlign: 'left', color: 'var(--text-secondary)' }}>
              <th style={{ padding: 10 }}>Service</th>
              <th style={{ padding: 10 }}>État</th>
              <th style={{ padding: 10 }}>Démarrage</th>
              <th style={{ padding: 10 }}>PID</th>
              <th style={{ padding: 10 }}>Compte</th>
              <th style={{ padding: 10 }}>Actions</th>
            </tr>
          </thead>
          <tbody>
            {visibleServices.map(service => {
              const running = service.state.toLowerCase() === 'running';
              const stopped = service.state.toLowerCase() === 'stopped';
              const busy = busyService === service.name;
              return (
                <tr key={service.name} style={{ borderTop: '1px solid var(--window-border)' }}>
                  <td style={{ padding: 10, minWidth: 180 }}>
                    <div style={{ fontWeight: 650 }}>{service.displayName || service.name}</div>
                    <div style={{ color: 'var(--text-tertiary)', fontSize: 10 }}>{service.name}{service.isProtected ? ' · protégé' : ''}</div>
                  </td>
                  <td style={{ padding: 10, whiteSpace: 'nowrap' }}>
                    <span style={{ color: running ? '#4ade80' : 'var(--text-secondary)' }}>{stateLabel(service.state)}</span>
                  </td>
                  <td style={{ padding: 10, whiteSpace: 'nowrap' }}>{modeLabel(service.startMode)}</td>
                  <td style={{ padding: 10, fontVariantNumeric: 'tabular-nums' }}>{service.processId || '—'}</td>
                  <td style={{ padding: 10, maxWidth: 180, overflow: 'hidden', textOverflow: 'ellipsis' }} title={service.account}>{service.account || '—'}</td>
                  <td style={{ padding: 8, whiteSpace: 'nowrap' }}>
                    {stopped && <button type="button" disabled={busy} onClick={() => void control(service, 'start')} style={{ marginRight: 5, padding: '6px 9px', border: 0, borderRadius: 6, background: 'rgba(22,163,74,.16)', color: '#86efac', cursor: busy ? 'wait' : 'pointer', fontSize: 11 }}>{busy ? 'Action…' : 'Démarrer'}</button>}
                    {running && <button type="button" disabled={busy || service.isProtected} title={service.isProtected ? 'Service système protégé' : 'Arrêter le service Windows'} onClick={() => void control(service, 'stop')} style={{ padding: '6px 9px', border: 0, borderRadius: 6, background: service.isProtected ? 'var(--control-hover)' : 'rgba(185,28,28,.16)', color: service.isProtected ? 'var(--text-tertiary)' : '#fca5a5', cursor: busy || service.isProtected ? 'not-allowed' : 'pointer', fontSize: 11 }}>{busy ? 'Action…' : 'Arrêter'}</button>}
                    {!running && !stopped && <span style={{ color: 'var(--text-tertiary)', fontSize: 10 }}>État transitoire</span>}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {!loading && visibleServices.length === 0 && <div style={{ padding: 28, textAlign: 'center', color: 'var(--text-tertiary)' }}>Aucun service correspondant.</div>}
        {loading && <div style={{ padding: 28, textAlign: 'center', color: 'var(--text-tertiary)' }}>Lecture de l’inventaire Windows…</div>}
      </div>

      <footer style={{ fontSize: 10, color: 'var(--text-tertiary)' }}>
        Source : {snapshot?.source || 'WMI Win32_Service'}. L’arrêt de services critiques est bloqué dans le moteur natif. Les opérations peuvent être refusées par Windows si le compte courant ne dispose pas des privilèges requis. Le mode de démarrage est consultatif dans cette version.
      </footer>
    </div>
  );
};
