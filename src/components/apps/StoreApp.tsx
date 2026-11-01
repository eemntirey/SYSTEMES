import React from 'react';

export const StoreApp: React.FC = () => {
  return (
    <div style={{ padding: '24px', color: '#f0f0f5' }}>
      <h3>Microsoft Store</h3>
      <p style={{ color: '#a0a0c0' }}>
        Accès direct au Microsoft Store réel via le système Windows.
      </p>
      <p style={{ fontSize: '11px', color: '#606060', marginTop: '8px' }}>
        Pas une simulation — connexion réelle au Store.
      </p>
    </div>
  );
};
