import React from 'react';
import ReactDOM from 'react-dom/client';
import './styles.css';

function App() {
  return (
    <main className="app-shell">
      <section className="hero-panel">
        <p className="eyebrow">Tone Desktop</p>
        <h1>Photo editing workspace scaffold</h1>
        <p className="lede">
          React, TypeScript, Vite, and Tauri are ready for the first color pipeline tasks.
        </p>
      </section>
    </main>
  );
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
