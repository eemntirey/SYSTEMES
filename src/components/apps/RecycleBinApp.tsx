import React from 'react'
import { useOS } from '../../context/OSContext'
import { FluentIcon } from '../common/FluentIcon'

export const RecycleBinApp: React.FC = () => {
  const { recycleItems, restoreItem, permanentlyDeleteItem, emptyRecycleBin } = useOS()

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0, background: 'var(--window-bg)', color: 'var(--text-primary)' }}>
      <header style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '12px 16px', borderBottom: '1px solid var(--window-border)' }}>
        <div style={{ flex: 1 }}>
          <strong style={{ fontSize: 14 }}>Corbeille</strong>
          <div style={{ color: 'var(--text-tertiary)', fontSize: 11, marginTop: 3 }}>{recycleItems.length} élément(s)</div>
        </div>
        <button onClick={emptyRecycleBin} disabled={!recycleItems.length} style={actionStyle} title="Vider la corbeille">Vider la corbeille</button>
      </header>
      {recycleItems.length ? (
        <div style={{ flex: 1, overflow: 'auto', padding: 12 }}>
          <div style={{ display: 'grid', gridTemplateColumns: 'minmax(140px, 1fr) minmax(180px, 2fr) 130px 150px', gap: 8, padding: '8px 10px', color: 'var(--text-tertiary)', fontSize: 11, borderBottom: '1px solid var(--window-border)' }}>
            <span>Nom</span><span>Emplacement d’origine</span><span>Type</span><span>Actions</span>
          </div>
          {recycleItems.map(item => <div key={item.id} style={{ display: 'grid', gridTemplateColumns: 'minmax(140px, 1fr) minmax(180px, 2fr) 130px 150px', alignItems: 'center', gap: 8, padding: '10px', borderBottom: '1px solid var(--window-border)', fontSize: 12 }}>
            <span style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}><FluentIcon name={item.type === 'folder' ? 'folder' : 'file-text'} size={17} /><span style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{item.name}</span></span>
            <span title={item.originalPath} style={{ color: 'var(--text-secondary)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{item.originalPath}</span>
            <span style={{ color: 'var(--text-secondary)' }}>{item.type === 'folder' ? 'Dossier' : 'Fichier'}</span>
            <span style={{ display: 'flex', gap: 6 }}>
              <button onClick={() => restoreItem(item.id)} style={actionStyle} title="Restaurer"><FluentIcon name="refresh" size={14} /></button>
              <button onClick={() => permanentlyDeleteItem(item.id)} style={{ ...actionStyle, color: '#f87171' }} title="Supprimer définitivement"><FluentIcon name="trash" size={14} /></button>
            </span>
          </div>)}
        </div>
      ) : <div style={{ flex: 1, display: 'grid', placeItems: 'center', color: 'var(--text-tertiary)' }}><div style={{ textAlign: 'center' }}><FluentIcon name="recycle" size={42} /><p>La Corbeille est vide</p></div></div>}
    </div>
  )
}

const actionStyle: React.CSSProperties = { display: 'inline-flex', alignItems: 'center', justifyContent: 'center', gap: 6, padding: '6px 9px', borderRadius: 6, border: '1px solid var(--window-border)', background: 'var(--flyout-card)', color: 'var(--text-primary)', cursor: 'pointer', fontSize: 11 }
