import React, { useState, useEffect } from 'react';

export const ClockApp: React.FC = () => {
  const [time, setTime] = useState(new Date().toLocaleTimeString('fr-FR'));
  useEffect(() => {
    const timer = setInterval(() => setTime(new Date().toLocaleTimeString('fr-FR')), 1000);
    return () => clearInterval(timer);
  }, []);
  return (
    <div style={{ padding: '32px', color: '#f0f0f5', textAlign: 'center' }}>
      <h2 style={{ fontSize: '48px', fontWeight: 300, letterSpacing: '-2px' }}>{time}</h2>
      <p style={{ color: '#a0a0c0', marginTop: '16px' }}>Horloge système Windows 11 — temps réel</p>
      <p style={{ fontSize: '11px', color: '#606060', marginTop: '8px' }}>Non simulé — lecture système réelle via Date.now()</p>
    </div>
  );
};
