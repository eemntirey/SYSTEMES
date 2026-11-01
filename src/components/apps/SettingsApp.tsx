import React, { useEffect, useState } from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'
import { WALLPAPERS } from '../../data/constants'
import { NativeBridge } from '../../services/NativeBridge'

interface HardwareSnapshot {
  cpuName?: string | null
  cpuCores?: number | null
  cpuThreads?: number | null
  ramTotalGB?: number | null
  gpuName?: string | null
  gpuMemoryBytes?: number | null
  disks?: Array<{ name: string; totalBytes: number; availableBytes: number; format: string }>
  windowsVersion?: string
  note?: string
  error?: string
}

interface UpdateSnapshot {
  serviceRunning?: boolean
  updatesAvailable?: boolean | null
  note?: string
  error?: string
}

interface DevicesSnapshot {
  usb?: string[]
  network?: string[]
  note?: string
  error?: string
}

type SettingsSection =
  | 'home'
  | 'system'
  | 'bluetooth'
  | 'network'
  | 'personalization'
  | 'apps'
  | 'accounts'
  | 'time'
  | 'gaming'
  | 'accessibility'
  | 'privacy'
  | 'update'
  | 'about'

const ACCENT_COLORS = [
  { color: '#0078d4', name: 'Bleu Windows' },
  { color: '#00b4d8', name: 'Cyan' },
  { color: '#008272', name: 'Sarcelle' },
  { color: '#107c41', name: 'Vert forêt' },
  { color: '#8764b8', name: 'Iris' },
  { color: '#744da9', name: 'Violet' },
  { color: '#8b5cf6', name: 'Neon Violet' },
  { color: '#d13438', name: 'Cramoisi' },
  { color: '#ca5010', name: 'Orange' },
  { color: '#e3008c', name: 'Rose vif' },
  { color: '#ff4b82', name: 'Rose' },
  { color: '#ffb900', name: 'Or' },
]

// Re-usable toggle switch component
const Toggle: React.FC<{ value: boolean; onChange: () => void }> = ({ value, onChange }) => (
  <button
    onClick={onChange}
    style={{
      width: '48px',
      height: '26px',
      borderRadius: '14px',
      border: 'none',
      backgroundColor: value ? 'linear-gradient(135deg, var(--win-accent), var(--win-accent-cyan))' : 'rgba(255,255,255,0.08)',
      cursor: 'pointer',
      position: 'relative',
      transition: 'background 0.25s ease, box-shadow 0.25s ease',
      flexShrink: 0,
      boxShadow: value ? '0 0 12px rgba(139,92,246,0.35), inset 0 0 6px rgba(255,255,255,0.15)' : 'none',
    }}
  >
    <span
      style={{
        position: 'absolute',
        top: '3px',
        left: value ? '25px' : '3px',
        width: '20px',
        height: '20px',
        borderRadius: '50%',
        background: 'linear-gradient(145deg, #fff, #e0e0e0)',
        transition: 'left 0.25s cubic-bezier(0.4, 0, 0.2, 1)',
        boxShadow: '0 2px 6px rgba(0,0,0,0.35), 0 0 4px rgba(0,0,0,0.2)',
      }}
    />
  </button>
)

// Row with a toggle
const SettingRow: React.FC<{
  icon?: string
  title: string
  desc?: string
  toggle?: boolean
  onToggle?: () => void
  onClick?: () => void
  badge?: string
  children?: React.ReactNode
}> = ({ icon, title, desc, toggle, onToggle, onClick, badge, children }) => (
  <div
    onClick={onClick}
    style={{
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      padding: '16px 20px',
      borderRadius: '14px',
      backgroundColor: 'rgba(139,92,246,0.06)',
      border: '1px solid rgba(139,92,246,0.2)',
      marginBottom: '10px',
      cursor: onClick ? 'pointer' : 'default',
      transition: 'all 0.18s ease',
      gap: '14px',
      boxShadow: '0 2px 12px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.05)',
    }}
    onMouseEnter={e => { if (onClick) { e.currentTarget.style.backgroundColor = 'rgba(139,92,246,0.14)'; e.currentTarget.style.borderColor = 'rgba(139,92,246,0.45)'; e.currentTarget.style.boxShadow = '0 4px 20px rgba(139,92,246,0.15), inset 0 1px 0 rgba(255,255,255,0.08)'; } }}
    onMouseLeave={e => { if (onClick) { e.currentTarget.style.backgroundColor = 'rgba(139,92,246,0.06)'; e.currentTarget.style.borderColor = 'rgba(139,92,246,0.2)'; e.currentTarget.style.boxShadow = '0 2px 12px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.05)'; } }}
  >
    <div style={{ display: 'flex', alignItems: 'center', gap: '16px', flex: 1, minWidth: 0 }}>
      {icon && (
        <div style={{
          width: '38px', height: '38px', borderRadius: '10px',
          background: 'linear-gradient(135deg, rgba(139,92,246,0.2), rgba(0,212,255,0.1))',
          border: '1px solid rgba(139,92,246,0.25)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          flexShrink: 0,
        }}>
          <FluentIcon name={icon} size={18} color="var(--win-accent)" />
        </div>
      )}
      <div style={{ minWidth: 0 }}>
        <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--text-primary)', letterSpacing: '0.01em' }}>{title}</div>
        {desc && <div style={{ fontSize: '12.5px', color: 'var(--text-secondary)', marginTop: '3px', lineHeight: 1.4 }}>{desc}</div>}
        {children}
      </div>
    </div>
    <div style={{ display: 'flex', alignItems: 'center', gap: '12px', flexShrink: 0 }}>
      {badge && (
        <span style={{
          fontSize: '10.5px', padding: '3px 10px', borderRadius: '12px',
          backgroundColor: 'rgba(139,92,246,0.15)', color: 'var(--win-accent)', fontWeight: 700,
          border: '1px solid rgba(139,92,246,0.2)', letterSpacing: '0.02em',
        }}>{badge}</span>
      )}
      {toggle !== undefined && onToggle && <Toggle value={toggle} onChange={onToggle} />}
      {onClick && !toggle && <FluentIcon name="chevron-right" size={16} color="var(--text-tertiary)" />}
    </div>
  </div>
)

const SectionTitle: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <h3 style={{
    fontSize: '11px',
    fontWeight: 700,
    color: 'var(--win-accent)',
    textTransform: 'uppercase',
    letterSpacing: '0.12em',
    marginBottom: '10px',
    marginTop: '24px',
    textShadow: '0 0 10px rgba(139,92,246,0.35)',
  }}>
    {children}
  </h3>
)

export const SettingsApp: React.FC = () => {
  const {
    theme, setTheme,
    wallpaper, setWallpaper,
    accentColor, setAccentColor,
    addNotification,
  } = useOS()

  const [section, setSection] = useState<SettingsSection>('home')
  const [searchQuery, setSearchQuery] = useState('')
  const [hardwareSnapshot, setHardwareSnapshot] = useState<HardwareSnapshot | null>(null)
  const [hardwareLoading, setHardwareLoading] = useState(false)
  const [hardwareError, setHardwareError] = useState<string | null>(null)
  const [updateSnapshot, setUpdateSnapshot] = useState<UpdateSnapshot | null>(null)
  const [updateLoading, setUpdateLoading] = useState(false)
  const [updateError, setUpdateError] = useState<string | null>(null)
  const [devicesSnapshot, setDevicesSnapshot] = useState<DevicesSnapshot | null>(null)
  const [devicesLoading, setDevicesLoading] = useState(false)
  const [devicesError, setDevicesError] = useState<string | null>(null)

  useEffect(() => {
    if (section !== 'bluetooth' && section !== 'network') return
    let cancelled = false
    setDevicesLoading(true)
    setDevicesError(null)
    NativeBridge.readDevices()
      .then((result) => {
        if (!cancelled) setDevicesSnapshot(result as DevicesSnapshot)
      })
      .catch((error: unknown) => {
        if (!cancelled) setDevicesError(error instanceof Error ? error.message : String(error))
      })
      .finally(() => {
        if (!cancelled) setDevicesLoading(false)
      })
    return () => { cancelled = true }
  }, [section])

  useEffect(() => {
    if (section !== 'about') return
    let cancelled = false
    setHardwareLoading(true)
    setHardwareError(null)
    NativeBridge.readHardware()
      .then((result) => {
        if (!cancelled) setHardwareSnapshot(result as HardwareSnapshot)
      })
      .catch((error: unknown) => {
        if (!cancelled) setHardwareError(error instanceof Error ? error.message : String(error))
      })
      .finally(() => {
        if (!cancelled) setHardwareLoading(false)
      })
    return () => { cancelled = true }
  }, [section])

  const handleCheckWindowsUpdate = async () => {
    setUpdateLoading(true)
    setUpdateError(null)
    try {
      const result = await NativeBridge.checkWindowsUpdate() as UpdateSnapshot
      setUpdateSnapshot(result)
    } catch (error) {
      setUpdateSnapshot(null)
      setUpdateError(error instanceof Error ? error.message : String(error))
    } finally {
      setUpdateLoading(false)
    }
  }

  const openWindowsSettings = async (page: Parameters<typeof NativeBridge.openWindowsSettings>[0], label: string) => {
    try {
      const result = await NativeBridge.openWindowsSettings(page) as { opened?: boolean }
      if (!result?.opened) throw new Error('Windows n’a pas confirmé l’ouverture de la page Paramètres.')
      addNotification('Paramètres Windows', `Demande d’ouverture transmise à Windows : ${label}.`, 'Paramètres', 'settings')
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      addNotification('Paramètres Windows', `Impossible d’ouvrir ${label} : ${message}`, 'Paramètres', 'info')
    }
  }

  const navItems: { id: SettingsSection; label: string; icon: string; desc?: string }[] = [
    { id: 'home', label: 'Accueil', icon: 'pc' },
    { id: 'system', label: 'Système', icon: 'sliders', desc: 'Écran, son, notifications, alimentation' },
    { id: 'bluetooth', label: 'Bluetooth et appareils', icon: 'battery', desc: 'Souris, clavier, casque, autres appareils' },
    { id: 'network', label: 'Réseau et Internet', icon: 'wifi', desc: 'Wi-Fi, réseau câblé, VPN' },
    { id: 'personalization', label: 'Personnalisation', icon: 'palette', desc: 'Arrière-plan, couleurs, écran de verrouillage' },
    { id: 'apps', label: 'Applications', icon: 'square', desc: 'Applications installées, fonctionnalités optionnelles' },
    { id: 'accounts', label: 'Comptes', icon: 'lock', desc: 'Compte Microsoft, famille, autres utilisateurs' },
    { id: 'time', label: 'Heure et langue', icon: 'clock', desc: 'Fuseau horaire, région, langue d\'affichage' },
    { id: 'gaming', label: 'Jeux', icon: 'terminal', desc: 'Xbox Game Bar, Game Mode, captures' },
    { id: 'accessibility', label: 'Accessibilité', icon: 'minimize', desc: 'Loupe, narrateur, contraste élevé' },
    { id: 'privacy', label: 'Confidentialité et sécurité', icon: 'lock', desc: 'Autorisations Windows, données de diagnostic' },
    { id: 'update', label: 'Windows Update', icon: 'refresh', desc: 'Mises à jour, Windows Insider' },
    { id: 'about', label: 'À propos', icon: 'file-text', desc: 'Spécifications de l\'appareil et Windows' },
  ]

  const filteredNav = searchQuery.trim()
    ? navItems.filter(n => n.label.toLowerCase().includes(searchQuery.toLowerCase()) || n.desc?.toLowerCase().includes(searchQuery.toLowerCase()))
    : navItems

  return (
    <div style={{ display: 'flex', height: '100%', backgroundColor: 'var(--window-bg)', fontFamily: 'var(--font-segoe)', overflow: 'hidden' }}>

      {/* ──── Left Sidebar ──── */}
      <div style={{
        width: '270px',
        borderRight: '1px solid rgba(139,92,246,0.2)',
        display: 'flex',
        flexDirection: 'column',
        background: 'linear-gradient(180deg, rgba(10,10,18,0.95), rgba(10,10,18,0.98))',
        flexShrink: 0,
        backdropFilter: 'blur(20px)',
        WebkitBackdropFilter: 'blur(20px)',
      }}>
        {/* User profile */}
        <div style={{ padding: '24px 16px 14px' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: '14px', padding: '14px', borderRadius: '14px', background: 'linear-gradient(135deg, rgba(139,92,246,0.1), rgba(0,212,255,0.06))', border: '1px solid rgba(139,92,246,0.2)', marginBottom: '14px', boxShadow: '0 4px 20px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.04)' }}>
            <div style={{ width: '46px', height: '46px', borderRadius: '50%', background: 'linear-gradient(135deg, var(--win-accent), var(--win-accent-cyan))', display: 'flex', alignItems: 'center', justifyContent: 'center', color: '#fff', fontWeight: 700, fontSize: '18px', flexShrink: 0, boxShadow: '0 0 16px rgba(139,92,246,0.5)' }}>W</div>
            <div>
              <div style={{ fontSize: '14px', fontWeight: 700, color: 'var(--text-primary)' }}>Session Windows</div>
              <div style={{ fontSize: '11.5px', color: 'var(--win-accent-cyan)', marginTop: '3px', fontWeight: 500 }}>Identité non lue par SYSTEMES</div>
            </div>
          </div>

          {/* Search */}
          <div style={{ display: 'flex', alignItems: 'center', gap: '10px', padding: '10px 14px', backgroundColor: 'rgba(255,255,255,0.04)', borderRadius: '10px', border: '1px solid rgba(139,92,246,0.15)', boxShadow: 'inset 0 2px 6px rgba(0,0,0,0.2)' }}>
            <FluentIcon name="search" size={15} color="var(--win-accent)" />
            <input
              type="text"
              placeholder="Rechercher dans les paramètres"
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              style={{ border: 'none', background: 'transparent', outline: 'none', fontSize: '12.5px', color: 'var(--text-primary)', width: '100%', fontFamily: 'inherit' }}
            />
          </div>
        </div>

        {/* Nav */}
        <div style={{ flex: 1, overflowY: 'auto', padding: '0 8px 8px' }}>
          {filteredNav.map(item => {
            const active = section === item.id
            return (
              <button
                key={item.id}
                onClick={() => setSection(item.id)}
                style={{
                  width: '100%', display: 'flex', alignItems: 'center', gap: '12px',
                  padding: '10px 14px', borderRadius: '10px', border: 'none',
                  background: active ? 'linear-gradient(135deg, rgba(139,92,246,0.2), rgba(0,212,255,0.1))' : 'transparent',
                  color: active ? 'var(--win-accent)' : 'var(--text-primary)',
                  fontSize: '13px', fontWeight: active ? 600 : 400, cursor: 'pointer',
                  textAlign: 'left', transition: 'all 0.15s ease', marginBottom: '2px',
                  borderLeft: active ? '3px solid var(--win-accent)' : '3px solid transparent',
                  boxShadow: active ? 'inset 0 1px 0 rgba(255,255,255,0.08), 0 2px 8px rgba(139,92,246,0.15)' : 'none',
                }}
                onMouseEnter={e => { if (!active) { e.currentTarget.style.backgroundColor = 'rgba(139,92,246,0.08)'; } }}
                onMouseLeave={e => { if (!active) { e.currentTarget.style.backgroundColor = 'transparent'; } }}
              >
                <FluentIcon name={item.icon} size={18} color={active ? 'var(--win-accent-cyan)' : 'var(--text-secondary)'} />
                <span>{item.label}</span>
              </button>
            )
          })}
        </div>
      </div>

      {/* ──── Main Content ──── */}
      <div style={{ flex: 1, overflowY: 'auto', padding: '28px 36px', background: 'linear-gradient(180deg, rgba(139,92,246,0.03) 0%, rgba(10,10,18,0) 60%)' }}>

        {/* HOME */}
        {section === 'home' && (
          <div>
            <h1 style={{ fontSize: '36px', fontWeight: 800, marginBottom: '6px', letterSpacing: '-0.03em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Paramètres</h1>
            <p style={{ fontSize: '13.5px', color: 'var(--win-accent-cyan)', marginBottom: '30px', marginTop: '6px', fontWeight: 500 }}>Configuration de cet appareil et des paramètres Windows</p>

            {/* Recommended settings cards */}
            <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '14px', marginBottom: '28px' }}>
              {[
                { icon: 'pc', title: 'Affichage Windows', desc: 'Ouvrir les réglages réels de l’écran', action: () => void openWindowsSettings('display', 'Affichage') },
                { icon: 'wifi', title: 'Réseau et Internet', desc: 'Consulter les connexions Windows', action: () => setSection('network') },
                { icon: 'refresh', title: 'Windows Update', desc: 'Ouvrir la gestion des mises à jour', action: () => setSection('update') },
                { icon: 'lock', title: 'Confidentialité', desc: 'Gérer les autorisations Windows', action: () => setSection('privacy') },
              ].map(card => (
                <div key={card.title} onClick={card.action} style={{
                  padding: '20px 22px', borderRadius: '18px',
                  background: 'linear-gradient(135deg, rgba(139,92,246,0.08), rgba(0,212,255,0.06))',
                  border: '1px solid rgba(139,92,246,0.2)',
                  cursor: 'pointer', transition: 'all 0.2s ease',
                  boxShadow: '0 8px 28px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.05)',
                }}
                  onMouseEnter={e => {
                    e.currentTarget.style.background = 'linear-gradient(135deg, rgba(139,92,246,0.14), rgba(0,212,255,0.1))';
                    e.currentTarget.style.borderColor = 'rgba(139,92,246,0.45)';
                    e.currentTarget.style.transform = 'translateY(-3px)';
                    e.currentTarget.style.boxShadow = '0 14px 35px rgba(139,92,246,0.2), inset 0 1px 0 rgba(255,255,255,0.08)';
                  }}
                  onMouseLeave={e => {
                    e.currentTarget.style.background = 'linear-gradient(135deg, rgba(139,92,246,0.08), rgba(0,212,255,0.06))';
                    e.currentTarget.style.borderColor = 'rgba(139,92,246,0.2)';
                    e.currentTarget.style.transform = 'none';
                    e.currentTarget.style.boxShadow = '0 8px 28px rgba(0,0,0,0.3), inset 0 1px 0 rgba(255,255,255,0.05)';
                  }}
                >
                  <div style={{
                    width: '44px', height: '44px', borderRadius: '14px',
                    background: 'linear-gradient(135deg, rgba(139,92,246,0.25), rgba(0,212,255,0.2))',
                    border: '1px solid rgba(139,92,246,0.3)',
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    boxShadow: '0 0 14px rgba(139,92,246,0.25)',
                  }}>
                    <FluentIcon name={card.icon} size={22} color="#fff" />
                  </div>
                  <div style={{ fontSize: '14px', fontWeight: 700, marginTop: '14px', color: 'var(--text-primary)' }}>{card.title}</div>
                  <div style={{ fontSize: '11.5px', color: 'var(--text-secondary)', marginTop: '5px', lineHeight: 1.4 }}>{card.desc}</div>
                </div>
              ))}
            </div>

            <h2 style={{ fontSize: '18px', fontWeight: 700, marginBottom: '14px', color: 'var(--text-primary)', letterSpacing: '0.01em' }}>Toutes les catégories</h2>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: '12px' }}>
              {navItems.filter(n => n.id !== 'home').map(item => (
                <div key={item.id} onClick={() => setSection(item.id)} style={{ padding: '18px 16px', borderRadius: '14px', background: 'linear-gradient(135deg, rgba(139,92,246,0.06), rgba(0,212,255,0.04))', border: '1px solid rgba(139,92,246,0.15)', cursor: 'pointer', transition: 'all 0.2s ease', boxShadow: '0 4px 16px rgba(0,0,0,0.2)' }}
                  onMouseEnter={e => { e.currentTarget.style.background = 'linear-gradient(135deg, rgba(139,92,246,0.12), rgba(0,212,255,0.08))'; e.currentTarget.style.borderColor = 'rgba(139,92,246,0.35)'; e.currentTarget.style.transform = 'translateY(-2px)'; e.currentTarget.style.boxShadow = '0 10px 28px rgba(139,92,246,0.15)'; }}
                  onMouseLeave={e => { e.currentTarget.style.background = 'linear-gradient(135deg, rgba(139,92,246,0.06), rgba(0,212,255,0.04))'; e.currentTarget.style.borderColor = 'rgba(139,92,246,0.15)'; e.currentTarget.style.transform = 'none'; e.currentTarget.style.boxShadow = '0 4px 16px rgba(0,0,0,0.2)'; }}
                >
                  <div style={{
                    width: '40px', height: '40px', borderRadius: '12px',
                    background: 'linear-gradient(135deg, var(--win-accent), var(--win-accent-cyan))',
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    boxShadow: '0 0 14px rgba(139,92,246,0.3)', marginBottom: '10px',
                  }}>
                    <FluentIcon name={item.icon} size={20} color="#fff" />
                  </div>
                  <div style={{ fontSize: '14px', fontWeight: 600, color: 'var(--text-primary)' }}>{item.label}</div>
                  {item.desc && <div style={{ fontSize: '11.5px', color: 'var(--text-secondary)', marginTop: '4px', lineHeight: 1.35 }}>{item.desc}</div>}
                </div>
              ))}
            </div>
          </div>
        )}

        {/* SYSTEM */}
        {section === 'system' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Système</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Les réglages matériels sont désormais délégués aux paramètres Windows réels.</p>

            <SectionTitle>Affichage</SectionTitle>
            <SettingRow icon="sun" title="Luminosité et affichage" desc="Ouvrir la résolution, la luminosité et les écrans dans Windows." onClick={() => void openWindowsSettings('display', 'Affichage')} />
            <SettingRow icon="moon" title="Éclairage nocturne" desc="Gérer la température de couleur directement dans Windows." onClick={() => void openWindowsSettings('night-light', 'Éclairage nocturne')} />
            <SettingRow icon="maximize" title="Taille du texte et mise à l’échelle" desc="Modifier les paramètres d’accessibilité de Windows." onClick={() => void openWindowsSettings('accessibility-display', 'Taille du texte')} />

            <SectionTitle>Son</SectionTitle>
            <SettingRow icon="volume" title="Volume et périphériques audio" desc="Ouvrir le mélangeur et les périphériques audio Windows." onClick={() => void openWindowsSettings('sound', 'Son')} />

            <SectionTitle>Notifications</SectionTitle>
            <SettingRow icon="bell" title="Notifications des applications" desc="Configurer les notifications dans Windows." onClick={() => void openWindowsSettings('notifications', 'Notifications')} />
            <SettingRow icon="moon" title="Ne pas déranger et assistance Focus" desc="Gérer les périodes silencieuses dans Windows." onClick={() => void openWindowsSettings('quiet-hours', 'Ne pas déranger')} />

            <SectionTitle>Alimentation</SectionTitle>
            <SettingRow icon="battery-charging" title="Batterie et économiseur d’énergie" desc="Consulter et modifier les paramètres d’alimentation Windows." onClick={() => void openWindowsSettings('battery', 'Batterie et énergie')} />
            <SettingRow icon="power" title="Veille et alimentation" desc="Modifier les délais de veille et les boutons d’alimentation." onClick={() => void openWindowsSettings('power', 'Alimentation et veille')} />
          </div>
        )}

                {/* BLUETOOTH */}
        {section === 'bluetooth' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Bluetooth et appareils</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Gérez vos appareils connectés</p>

            <SettingRow icon="sliders" title="Bluetooth" desc="Ouvrir les contrôles Bluetooth réels de Windows." onClick={() => void openWindowsSettings('bluetooth', 'Bluetooth')} />
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '14px', marginTop: '4px' }}></div>

            <SectionTitle>Périphériques détectés par Windows</SectionTitle>
            <div style={{ padding: '14px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '12px' }}>
              {devicesLoading ? (
                <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Lecture des périphériques…</div>
              ) : devicesError ? (
                <div role="alert" style={{ fontSize: '13px', color: '#f87171' }}>Détection impossible : {devicesError}</div>
              ) : devicesSnapshot ? (
                <>
                  <div style={{ fontSize: '13px', fontWeight: 600, marginBottom: '8px' }}>USB ({devicesSnapshot.usb?.length ?? 0})</div>
                  {(devicesSnapshot.usb ?? []).length > 0
                    ? devicesSnapshot.usb?.map((device, index) => <div key={`usb-${index}-${device}`} style={{ fontSize: '12.5px', color: 'var(--text-secondary)', padding: '4px 0' }}>{device}</div>)
                    : <div style={{ fontSize: '12.5px', color: 'var(--text-secondary)' }}>Aucun périphérique USB détecté.</div>}
                  <div style={{ fontSize: '13px', fontWeight: 600, marginTop: '14px', marginBottom: '8px' }}>Adaptateurs réseau actifs ({devicesSnapshot.network?.length ?? 0})</div>
                  {(devicesSnapshot.network ?? []).length > 0
                    ? devicesSnapshot.network?.map((device, index) => <div key={`network-${index}-${device}`} style={{ fontSize: '12.5px', color: 'var(--text-secondary)', padding: '4px 0' }}>{device}</div>)
                    : <div style={{ fontSize: '12.5px', color: 'var(--text-secondary)' }}>Aucun adaptateur réseau actif détecté.</div>}
                  <div style={{ fontSize: '11.5px', color: 'var(--text-tertiary)', marginTop: '10px' }}>{devicesSnapshot.note}</div>
                </>
              ) : (
                <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Aucune lecture effectuée. Ouvrez cette section depuis l'hôte Windows SYSTEMES.</div>
              )}
            </div>

            <SectionTitle>Autres appareils</SectionTitle>
            <SettingRow icon="square" title="Imprimantes et scanners" desc="Consulter et gérer les imprimantes installées dans Windows." onClick={() => void openWindowsSettings('printers', 'Imprimantes et scanners')} />
            
            <SettingRow icon="pc" title="Stylo et saisie tactile" desc="Ouvrir les réglages de saisie Windows." onClick={() => void openWindowsSettings('pen-touch', 'Stylo et saisie tactile')} />
            <SettingRow icon="pc" title="Appareils mobiles" desc="Gérer les appareils mobiles associés à Windows." onClick={() => void openWindowsSettings('mobile-devices', 'Appareils mobiles')} />
          </div>
        )}

        {/* NETWORK */}
        {section === 'network' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Réseau et Internet</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Les réseaux et connexions sont gérés par Windows, sans état fictif dans SYSTEMES.</p>
            <div style={{ padding: '16px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '16px', fontSize: '12.5px', color: 'var(--text-secondary)', lineHeight: 1.6 }}>
              SYSTEMES ne prétend plus connaître le nom du Wi-Fi, son débit ou l’état d’une connexion sans mesure native vérifiée. Utilisez les pages Windows ci-dessous pour consulter ou modifier la configuration réelle.
            </div>
            <SectionTitle>Adaptateurs réseau actifs détectés par Windows</SectionTitle>
            <div style={{ padding: '14px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '14px' }}>
              {devicesLoading ? (
                <div style={{ fontSize: '12.5px', color: 'var(--text-secondary)' }}>Lecture des adaptateurs réseau…</div>
              ) : devicesError ? (
                <div role="alert" style={{ fontSize: '12.5px', color: '#f87171' }}>Lecture impossible : {devicesError}</div>
              ) : devicesSnapshot ? (
                <>
                  {(devicesSnapshot.network ?? []).length > 0
                    ? devicesSnapshot.network?.map((adapter, index) => <div key={`adapter-${index}-${adapter}`} style={{ fontSize: '12.5px', color: 'var(--text-secondary)', padding: '4px 0' }}>{adapter}</div>)
                    : <div style={{ fontSize: '12.5px', color: 'var(--text-secondary)' }}>Aucun adaptateur avec NetEnabled actif n’a été retourné par WMI.</div>}
                  <div style={{ fontSize: '11px', color: 'var(--text-tertiary)', marginTop: 8 }}>{devicesSnapshot.note}</div>
                </>
              ) : (
                <div style={{ fontSize: '12.5px', color: 'var(--text-secondary)' }}>Données réseau non lues. Lancez SYSTEMES depuis l’hôte Windows.</div>
              )}
            </div>
            <SettingRow icon="wifi" title="Wi-Fi et état de connexion" desc="Voir le réseau actif, les réseaux disponibles et les connexions." onClick={() => void openWindowsSettings('network-status', 'Réseau et Internet')} />
            <SettingRow icon="globe" title="Ethernet" desc="Consulter les adaptateurs et paramètres Ethernet." onClick={() => void openWindowsSettings('network-ethernet', 'Ethernet')} />
            <SettingRow icon="lock" title="VPN" desc="Gérer les connexions VPN Windows." onClick={() => void openWindowsSettings('network-vpn', 'VPN')} />
            <SettingRow icon="refresh" title="Mode avion" desc="Consulter et modifier l’état réel du mode avion." onClick={() => void openWindowsSettings('airplane-mode', 'Mode avion')} />
            <SettingRow icon="wifi" title="Point d’accès mobile" desc="Configurer le partage de connexion Windows." onClick={() => void openWindowsSettings('mobile-hotspot', 'Point d’accès mobile')} />
            <SettingRow icon="info" title="Utilisation des données" desc="Consulter les données réellement mesurées par Windows." onClick={() => void openWindowsSettings('data-usage', 'Utilisation des données')} />
            <SettingRow icon="sliders" title="Paramètres proxy" desc="Voir et modifier le proxy système." onClick={() => void openWindowsSettings('proxy', 'Proxy')} />
          </div>
        )}

                {/* PERSONALIZATION */}
        {section === 'personalization' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Personnalisation de SYSTEMES</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '22px', marginTop: '4px' }}>Ces choix modifient l’apparence de l’interface SYSTEMES, pas les paramètres du bureau Windows.</p>

            {/* Wallpaper preview */}
            <div style={{ height: '160px', borderRadius: '14px', backgroundImage: `url(${wallpaper.url})`, backgroundSize: 'cover', backgroundPosition: 'center', border: '1px solid var(--window-border)', marginBottom: '20px', position: 'relative', display: 'flex', alignItems: 'flex-end', padding: '14px' }}>
              <div style={{ backgroundColor: 'rgba(0,0,0,0.6)', backdropFilter: 'blur(8px)', padding: '5px 12px', borderRadius: '12px', color: '#fff', fontSize: '12px' }}>
                {wallpaper.name}
              </div>
            </div>

            <SectionTitle>Arrière-plan</SectionTitle>
            <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(120px, 1fr))', gap: '10px', marginBottom: '20px' }}>
              {WALLPAPERS.map(wp => (
                <div key={wp.id} onClick={() => setWallpaper(wp)} style={{ height: '75px', borderRadius: '8px', backgroundImage: `url(${wp.url})`, backgroundSize: 'cover', backgroundPosition: 'center', cursor: 'pointer', border: wallpaper.id === wp.id ? '3px solid var(--win-accent)' : '2px solid transparent', boxShadow: wallpaper.id === wp.id ? '0 0 14px rgba(139,92,246,0.6)' : 'none', transition: 'all 0.15s', position: 'relative' }}
                  onMouseEnter={e => { (e.currentTarget as HTMLElement).style.transform = 'scale(1.04)' }}
                  onMouseLeave={e => { (e.currentTarget as HTMLElement).style.transform = 'scale(1)' }}
                />
              ))}
            </div>

            <SectionTitle>Mode de couleur</SectionTitle>
            <div style={{ display: 'flex', gap: '12px', marginBottom: '16px' }}>
              {[
                { id: 'dark', label: 'Sombre', icon: 'moon', desc: 'Recommandé' },
                { id: 'light', label: 'Clair', icon: 'sun', desc: 'Haute visibilité' },
              ].map(t => (
                <button key={t.id} onClick={() => setTheme(t.id as 'dark' | 'light')} style={{ flex: 1, padding: '14px', borderRadius: '10px', backgroundColor: theme === t.id ? 'var(--win-accent-light)' : 'var(--flyout-card)', border: theme === t.id ? '2px solid var(--win-accent)' : '1px solid var(--window-border)', color: 'var(--text-primary)', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: '12px' }}>
                  <FluentIcon name={t.icon} size={20} color={theme === t.id ? 'var(--win-accent)' : 'var(--text-secondary)'} />
                  <div style={{ textAlign: 'left' }}>
                    <div style={{ fontSize: '13px', fontWeight: 600 }}>{t.label}</div>
                    <div style={{ fontSize: '11px', color: 'var(--text-secondary)' }}>{t.desc}</div>
                  </div>
                </button>
              ))}
            </div>

            <SectionTitle>Couleur d'accentuation</SectionTitle>
            <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap', marginBottom: '16px' }}>
              {ACCENT_COLORS.map(({ color, name }) => {
                const selected = accentColor.toLowerCase() === color.toLowerCase()
                return (
                  <button key={color} title={name} onClick={() => { setAccentColor(color); document.documentElement.style.setProperty('--win-accent', color) }} style={{ width: '34px', height: '34px', borderRadius: '50%', backgroundColor: color, border: selected ? '3px solid #fff' : '2px solid transparent', boxShadow: selected ? `0 0 10px ${color}aa` : 'none', cursor: 'pointer', transition: 'all 0.15s' }} />
                )
              })}
            </div>

            <SectionTitle>Paramètres Windows</SectionTitle>
            <SettingRow icon="square" title="Barre des tâches" desc="Ouvrir les paramètres Windows réels de la barre des tâches." onClick={() => void openWindowsSettings('taskbar', 'Barre des tâches')} />
            <SettingRow icon="lock" title="Écran de verrouillage" desc="Gérer l’arrière-plan et les informations de l’écran de verrouillage Windows." onClick={() => void openWindowsSettings('lock-screen', 'Écran de verrouillage')} />
          </div>
        )}

        {/* APPS */}
        {section === 'apps' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Applications</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Applications intégrées à SYSTEMES. Cette version ne gère pas de paquets externes.</p>

            <SectionTitle>Applications installées</SectionTitle>
            {[
              { name: 'Photos', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'image' },
              { name: 'Terminal', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'terminal' },
              { name: 'Bloc-notes', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'notepad' },
              { name: 'Paint', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'paint' },
              { name: 'Calculatrice', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'calculator' },
              { name: 'Paramètres', publisher: 'SYSTEMES', size: 'Intégrée', version: '1.0', icon: 'settings' },

            ].map(app => (
              <div key={app.name} style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '6px' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '14px' }}>
                  <FluentIcon name={app.icon} size={24} />
                  <div>
                    <div style={{ fontSize: '13.5px', fontWeight: 500 }}>{app.name}</div>
                    <div style={{ fontSize: '11.5px', color: 'var(--text-secondary)' }}>{app.publisher} · {app.size} · v{app.version}</div>
                  </div>
                </div>
                <div style={{ display: 'flex', gap: '8px' }}>
                  <span style={{ fontSize: 11, color: 'var(--text-tertiary)' }}>Application système</span>
                </div>
              </div>
            ))}

            <SectionTitle>Fonctionnalités optionnelles</SectionTitle>
            <SettingRow icon="plus" title="Ajouter une fonctionnalité optionnelle" desc="Ouvrir la gestion réelle des fonctionnalités facultatives Windows." onClick={() => void openWindowsSettings('optional-features', 'Fonctionnalités facultatives')} />
            <SettingRow icon="square" title="Applications par défaut" desc="Choisir les applications par défaut dans Windows." onClick={() => void openWindowsSettings('default-apps', 'Applications par défaut')} />
          </div>
        )}

        {/* ACCOUNTS */}
        {section === 'accounts' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Comptes</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Les informations du compte ne sont pas simulées : Windows reste la source de vérité.</p>
            <SettingRow icon="pc" title="Vos informations" desc="Consulter le compte Windows réellement connecté." onClick={() => void openWindowsSettings('accounts', 'Informations du compte')} />
            <SettingRow icon="lock" title="Options de connexion" desc="Windows Hello, code PIN, clé de sécurité et mot de passe." onClick={() => void openWindowsSettings('sign-in-options', 'Options de connexion')} />
            <SettingRow icon="refresh" title="Synchronisation des paramètres" desc="Consulter les options de synchronisation de votre compte Windows." onClick={() => void openWindowsSettings('accounts-sync', 'Synchronisation du compte')} />
            <SettingRow icon="pc" title="Accès professionnel ou scolaire" desc="Gérer les comptes professionnels et l’inscription de l’appareil." onClick={() => void openWindowsSettings('work-access', 'Accès professionnel ou scolaire')} />
            <SettingRow icon="pc" title="Autres utilisateurs" desc="Gérer les comptes locaux et les utilisateurs de cet appareil." onClick={() => void openWindowsSettings('other-users', 'Autres utilisateurs')} />
          </div>
        )}

                {/* TIME */}
        {section === 'time' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Heure et langue</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Horloge locale de l’appareil · configuration gérée par Windows</p>
            <div style={{ padding: '20px', borderRadius: '14px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '20px', textAlign: 'center' }}>
              <div style={{ fontSize: '48px', fontWeight: 200, letterSpacing: '0.05em', color: 'var(--win-accent-cyan)', fontVariantNumeric: 'tabular-nums' }}>
                {new Date().toLocaleTimeString('fr-FR', { hour: '2-digit', minute: '2-digit' })}
              </div>
              <div style={{ fontSize: '14px', color: 'var(--text-secondary)', marginTop: '4px' }}>
                {new Date().toLocaleDateString('fr-FR', { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' })}
              </div>
            </div>
            <SettingRow icon="clock" title="Date et heure" desc="Régler l’heure automatique, le fuseau horaire et l’heure d’été." onClick={() => void openWindowsSettings('date-time', 'Date et heure')} />
            <SettingRow icon="globe" title="Région et format" desc="Consulter et modifier le pays, les formats régionaux et le calendrier." onClick={() => void openWindowsSettings('region', 'Région')} />
            <SettingRow icon="globe" title="Langue et clavier" desc="Gérer les langues d’affichage et les dispositions du clavier." onClick={() => void openWindowsSettings('language', 'Langue et clavier')} />
          </div>
        )}

                {/* GAMING */}
        {section === 'gaming' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Jeux</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Les fonctions Xbox et les optimisations sont pilotées par Windows.</p>
            <SettingRow icon="terminal" title="Xbox Game Bar et captures" desc="Ouvrir les réglages réels de la Game Bar, des captures et de l’enregistrement." onClick={() => void openWindowsSettings('gaming', 'Xbox Game Bar et captures')} />
            <SettingRow icon="terminal" title="Mode Jeu" desc="Consulter et modifier le Mode Jeu de Windows." onClick={() => void openWindowsSettings('game-mode', 'Mode Jeu')} />
            <SettingRow icon="pc" title="Fonctionnalités graphiques" desc="Gérer les options graphiques Windows et les préférences GPU." onClick={() => void openWindowsSettings('graphics', 'Paramètres graphiques')} />
          </div>
        )}

                {/* ACCESSIBILITY */}
        {section === 'accessibility' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Accessibilité</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Ouverture des contrôles d’accessibilité réellement appliqués par Windows.</p>
            <SectionTitle>Vision</SectionTitle>
            <SettingRow icon="maximize" title="Affichage et taille du texte" desc="Régler la taille du texte, la loupe et les effets visuels." onClick={() => void openWindowsSettings('accessibility-display', 'Affichage et taille du texte')} />
            <SettingRow icon="maximize" title="Loupe" desc="Activer et configurer la loupe Windows." onClick={() => void openWindowsSettings('accessibility-magnifier', 'Loupe')} />
            <SettingRow icon="maximize" title="Filtres de couleur et contraste" desc="Configurer les filtres de couleur et le contraste élevé." onClick={() => void openWindowsSettings('accessibility-color-filters', 'Filtres de couleur et contraste')} />
            <SectionTitle>Audition</SectionTitle>
            <SettingRow icon="volume" title="Légendes et sous-titres" desc="Gérer les sous-titres et les alertes visuelles." onClick={() => void openWindowsSettings('accessibility-captions', 'Légendes')} />
            <SectionTitle>Interaction</SectionTitle>
            <SettingRow icon="square" title="Clavier" desc="Configurer le clavier visuel, les touches rémanentes et les filtres." onClick={() => void openWindowsSettings('accessibility-keyboard', 'Clavier')} />
            <SettingRow icon="square" title="Souris et pointeur" desc="Modifier la vitesse et les options d’accessibilité du pointeur." onClick={() => void openWindowsSettings('accessibility-mouse', 'Souris et pointeur')} />
            <SettingRow icon="volume" title="Commande vocale" desc="Configurer l’accès vocal et la dictée Windows." onClick={() => void openWindowsSettings('accessibility-speech', 'Commande vocale')} />
          </div>
        )}

                {/* PRIVACY */}
        {section === 'privacy' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Confidentialité et sécurité</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Les autorisations ne sont plus présentées comme activées sans lecture système vérifiée.</p>
            <SectionTitle>Sécurité Windows</SectionTitle>
            <SettingRow icon="lock" title="Sécurité Windows" desc="Ouvrir le tableau de bord de sécurité réel de Windows." onClick={() => void openWindowsSettings('windows-security', 'Sécurité Windows')} />
            <SettingRow icon="lock" title="Chiffrement de l’appareil" desc="Consulter l’état réel du chiffrement disponible sur cet appareil." onClick={() => void openWindowsSettings('device-encryption', 'Chiffrement de l’appareil')} />
            <SectionTitle>Autorisations Windows</SectionTitle>
            <SettingRow icon="sliders" title="Données de diagnostic et commentaires" desc="Consulter les paramètres de diagnostic Windows." onClick={() => void openWindowsSettings('privacy-diagnostics', 'Données de diagnostic')} />
            <SettingRow icon="sliders" title="Expériences personnalisées" desc="Gérer les recommandations et expériences personnalisées." onClick={() => void openWindowsSettings('privacy', 'Confidentialité')} />
            <SettingRow icon="clock" title="Historique des activités" desc="Consulter les paramètres d’historique des activités." onClick={() => void openWindowsSettings('privacy-activity', 'Historique des activités')} />
            <SectionTitle>Autorisations des applications</SectionTitle>
            <SettingRow icon="globe" title="Position" desc="Gérer les autorisations de localisation des applications." onClick={() => void openWindowsSettings('privacy-location', 'Position')} />
            <SettingRow icon="image" title="Caméra" desc="Gérer les autorisations d’accès à la caméra." onClick={() => void openWindowsSettings('privacy-camera', 'Caméra')} />
            <SettingRow icon="volume" title="Microphone" desc="Gérer les autorisations d’accès au microphone." onClick={() => void openWindowsSettings('privacy-microphone', 'Microphone')} />
            <SettingRow icon="bell" title="Notifications" desc="Gérer les autorisations de notifications." onClick={() => void openWindowsSettings('privacy-notifications', 'Notifications')} />
          </div>
        )}

                {/* UPDATE */}
        {section === 'update' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>Windows Update</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Maintenir Windows à jour</p>

            <div style={{ padding: '20px 24px', borderRadius: '14px', background: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '24px', display: 'flex', alignItems: 'center', gap: '16px' }}>
              <div style={{ width: '44px', height: '44px', borderRadius: '50%', background: 'var(--win-accent-light)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <FluentIcon name="refresh" size={24} color="var(--win-accent)" />
              </div>
              <div style={{ minWidth: 0, flex: 1 }}>
                <div style={{ fontSize: '16px', fontWeight: 600 }}>
                  {updateSnapshot
                    ? updateSnapshot.serviceRunning ? 'Service Windows Update actif' : 'Service Windows Update arrêté'
                    : updateError ? 'Vérification impossible' : 'État non vérifié'}
                </div>
                <div style={{ fontSize: '12.5px', color: 'var(--text-secondary)', marginTop: '3px' }}>
                  {updateError ?? updateSnapshot?.note ?? 'Le contrôle interroge le service Windows Update sur cet ordinateur.'}
                </div>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
                <button
                  disabled={updateLoading}
                  style={{ padding: '10px 20px', borderRadius: '10px', border: '1px solid var(--window-border)', background: 'var(--win-accent-light)', color: 'var(--text-primary)', fontSize: '13px', fontWeight: 700, cursor: updateLoading ? 'wait' : 'pointer', opacity: updateLoading ? 0.65 : 1 }}
                  onClick={handleCheckWindowsUpdate}
                >{updateLoading ? 'Vérification…' : 'Vérifier le service'}</button>
                <button type="button" onClick={() => void openWindowsSettings('windows-update', 'Windows Update')} style={{ padding: '10px 20px', borderRadius: '10px', border: '1px solid var(--window-border)', background: 'var(--flyout-card)', color: 'var(--text-primary)', fontSize: '13px', fontWeight: 700, cursor: 'pointer' }}>Ouvrir Windows Update</button>
              </div>
            </div>

            <SectionTitle>Portée du contrôle</SectionTitle>
            <div style={{ padding: '16px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '16px', fontSize: '12.5px', color: 'var(--text-secondary)', lineHeight: 1.6 }}>
              Cette vérification lit uniquement l'état réel du service Windows Update. La recherche du catalogue de mises à jour, le téléchargement et l'installation ne sont pas encore implémentés. Aucun statut « Windows est à jour » n'est déduit de l'état du service.
            </div>

            <SectionTitle>Options avancées</SectionTitle>
            <SettingRow icon="clock" title="Heures actives" desc="Configurer les heures de redémarrage autorisées dans Windows." onClick={() => void openWindowsSettings('windows-update-hours', 'Heures actives')} />
            <SettingRow icon="sliders" title="Mises à jour facultatives" desc="Consulter les pilotes et mises à jour optionnelles proposés par Windows." onClick={() => void openWindowsSettings('windows-update-optional', 'Mises à jour facultatives')} />
            <SettingRow icon="sliders" title="Windows Insider Program" desc="Gérer l’inscription aux versions préliminaires Windows." onClick={() => void openWindowsSettings('windows-insider', 'Windows Insider Program')} />
            
          </div>
        )}

        {/* ABOUT */}
        {section === 'about' && (
          <div>
            <h1 style={{ fontSize: '30px', fontWeight: 800, marginBottom: '4px', letterSpacing: '-0.02em', background: 'linear-gradient(135deg, #fff, var(--win-accent-cyan))', WebkitBackgroundClip: 'text', WebkitTextFillColor: 'transparent', backgroundClip: 'text' }}>À propos</h1>
            <p style={{ fontSize: '13px', color: 'var(--text-secondary)', marginBottom: '26px', marginTop: '4px' }}>Spécifications de l'appareil et de Windows 11</p>
            <div style={{ height: '2px', background: 'linear-gradient(90deg, transparent, rgba(0,212,255,0.3), transparent)', borderRadius: '2px', marginBottom: '16px' }}></div>

            <div style={{ padding: '24px', borderRadius: '14px', background: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '20px' }}>
              <div style={{ fontSize: '17px', fontWeight: 700, marginBottom: '6px' }}>Diagnostic matériel réel</div>
              <div style={{ fontSize: '12.5px', color: 'var(--text-secondary)', marginBottom: '16px' }}>
                {hardwareLoading ? 'Lecture des informations matérielles…' : hardwareError ? hardwareError : hardwareSnapshot?.note ?? 'Les caractéristiques sont lues depuis Windows via le moteur natif.'}
              </div>
              {hardwareLoading ? (
                <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Chargement en cours…</div>
              ) : hardwareError ? (
                <div role="alert" style={{ fontSize: '13px', color: '#f87171' }}>Moteur natif indisponible : {hardwareError}</div>
              ) : hardwareSnapshot ? (
                <div style={{ display: 'grid', gap: '10px' }}>
                  {[
                    ['Processeur', hardwareSnapshot.cpuName ?? 'Non disponible'],
                    ['Cœurs / threads', hardwareSnapshot.cpuCores != null ? `${hardwareSnapshot.cpuCores} cœurs / ${hardwareSnapshot.cpuThreads ?? '—'} threads` : 'Non disponible'],
                    ['Mémoire vive', hardwareSnapshot.ramTotalGB != null ? `${hardwareSnapshot.ramTotalGB} Go` : 'Non disponible'],
                    ['Carte graphique', hardwareSnapshot.gpuName ?? 'Non disponible'],
                    ['Mémoire GPU', hardwareSnapshot.gpuMemoryBytes != null ? `${Math.round(hardwareSnapshot.gpuMemoryBytes / (1024 * 1024))} Mo` : 'Non communiquée par le pilote'],
                    ['Système', hardwareSnapshot.windowsVersion ?? 'Non disponible'],
                  ].map(([label, value]) => (
                    <div key={label} style={{ display: 'grid', gridTemplateColumns: '180px 1fr', gap: '12px', fontSize: '13px' }}>
                      <span style={{ color: 'var(--text-secondary)' }}>{label}</span>
                      <span>{value}</span>
                    </div>
                  ))}
                  <SectionTitle>Volumes détectés</SectionTitle>
                  {(hardwareSnapshot.disks ?? []).length > 0 ? hardwareSnapshot.disks?.map((disk) => (
                    <div key={disk.name} style={{ display: 'grid', gridTemplateColumns: '80px 1fr', gap: '12px', fontSize: '13px' }}>
                      <span style={{ color: 'var(--text-secondary)' }}>{disk.name}</span>
                      <span>{(disk.totalBytes / (1024 ** 3)).toFixed(1)} Go · {(disk.availableBytes / (1024 ** 3)).toFixed(1)} Go libres · {disk.format}</span>
                    </div>
                  )) : <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Aucun volume prêt détecté.</div>}
                </div>
              ) : (
                <div style={{ fontSize: '13px', color: 'var(--text-secondary)' }}>Aucune lecture matérielle disponible. Lancez SYSTEMES depuis l'hôte Windows pour accéder au moteur natif.</div>
              )}
            </div>

            <SectionTitle>Informations système</SectionTitle>
            <div style={{ padding: '16px 18px', borderRadius: '10px', backgroundColor: 'var(--flyout-card)', border: '1px solid var(--window-border)', marginBottom: '16px', fontSize: '12.5px', color: 'var(--text-secondary)', lineHeight: 1.6 }}>
              Les identifiants de périphérique, l'édition exacte, la date d'installation et les fonctions tactiles ne sont pas encore exposés par le moteur natif. Ils ne sont donc pas affichés comme des valeurs vérifiées.
            </div>

            <SectionTitle>Outils Windows</SectionTitle>
            <SettingRow icon="pc" title="Renommer ce PC" desc="Ouvrir la page À propos de Windows." onClick={() => void openWindowsSettings('about', 'Renommer ce PC')} />
            <SettingRow icon="sliders" title="Paramètres système avancés" desc="Ouvrir les propriétés système avancées." onClick={() => void openWindowsSettings('advanced-system', 'Paramètres système avancés')} />
            <SettingRow icon="lock" title="Sécurité Windows" desc="Ouvrir le tableau de bord de sécurité." onClick={() => void openWindowsSettings('windows-security', 'Sécurité Windows')} />
            <SettingRow icon="pc" title="Gestionnaire de périphériques" desc="Ouvrir le gestionnaire natif des périphériques." onClick={() => void openWindowsSettings('device-manager', 'Gestionnaire de périphériques')} />
            <SettingRow icon="globe" title="Connexions réseau avancées" desc="Ouvrir les paramètres réseau Windows." onClick={() => void openWindowsSettings('network-status', 'Réseau et Internet')} />
          </div>
        )}
      </div>
    </div>
  )
}
