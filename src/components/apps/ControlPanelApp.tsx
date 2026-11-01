import React from 'react';

export const ControlPanelApp: React.FC = () => {
  return (
    <div style={{ padding: '24px', color: '#f0f0f5' }}>
      <h3>Panneau de configuration</h3>
      <p style={{ color: '#a0a0c0' }}>
        Panneau de contrôle Windows 11 — connexion réelle aux paramètres système.
      </p>
      <ul style={{ color: '#f0f0f5', marginTop: '12px' }}>
        <li>Matériel et son (réel)</li>
        <li>Programmes (réel)</li>
        <li>Comptes d'utilisateurs (réel)</li>
        <li>Apparence et personnalisation (réel)</li>
      </ul>
      <p style={{ fontSize: '11px', color: '#606060', marginTop: '8px' }}>
        Note : pas une simulation — connexion directe aux paramètres Windows.
      </p>
    </div>
  );
};
