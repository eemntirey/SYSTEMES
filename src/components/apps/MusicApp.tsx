import React, { useState } from 'react';

export const MusicApp: React.FC = () => {
  const [playing, setPlaying] = useState(false);
  return (
    <div style={{ padding: '24px', color: '#f0f0f5' }}>
      <h3>Musique</h3>
      <p style={{ color: '#a0a0c0' }}>
        Lecteur non connecté : aucun fichier audio n’est lu.
      </p>
      <button onClick={() => setPlaying(!playing)} style={{ padding: '8px 16px', marginTop: '12px' }}>
        {playing ? 'Pause' : 'Lecture'}
      </button>
      <p style={{ fontSize: '11px', color: '#606060', marginTop: '12px' }}>
        Le moteur audio Windows n’est pas encore relié à ce lecteur.
      </p>
    </div>
  );
};
