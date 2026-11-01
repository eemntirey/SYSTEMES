import React from 'react';

export const SystemCompare = () => {
  return (
    <div style={{ padding: 20, background: '#0a0a12', color: '#fff', fontFamily: 'Segoe UI' }}>
      <h2>Comparaison avec Windows 11</h2>
      <table style={{ borderCollapse: 'collapse', width: '100%' }}>
        <thead><tr style={{ background: '#222' }}><th>Élément</th><th>SYSTEMES (avant)</th><th>Windows 11 (réel)</th></tr></thead>
        <tbody>
          <tr><td>MenuBar</td><td>Transport (manque)</td><td>Présent (header, glass, menu contextuel)</td></tr>
          <tr><td>Taskbar</td><td>Dock avec + (ok)</td><td>Dock centré, icônes ouvertes, heure, notifications</td></tr>
          <tr><td>StartMenu</td><td>Panel simple</td><td>App épinglées + recherche + profil</td></tr>
          <tr><td>Desktop</td><td>Fond violet, icônes fixes</td><td>Fond réel, icônes dynamiques, sélection rectangle</td></tr>
          <tr><td>LockScreen</td><td>PIN 1234 (simulé)</td><td>Verrouillage réel, photo de fond</td></tr>
          <tr><td>Settings</td><td>Paramètres visuels</td><td>Connecté au vrai Windows (WMI)</td></tr>
        </tbody>
      </table>
      <p>Objectif : atteindre le niveau du système réel sans simulation.</p>
    </div>
  );
};
