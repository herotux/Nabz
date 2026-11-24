import React from 'react';
import ReactDOM from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import App from './App';
import './style.css';

const root = ReactDOM.createRoot(document.getElementById('root')!);
root.render(<React.StrictMode><App /></React.StrictMode>);

// Wait for the first committed React frame and locally bundled fonts before
// revealing the native window. This avoids exposing WebKitGTK's unpainted
// startup surface on slower Raspberry Pi hardware.
void (async () => {
  try {
    if (document.fonts?.ready) await document.fonts.ready;
    await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
    await new Promise<void>(resolve => requestAnimationFrame(() => resolve()));
    await invoke('show_main_window');
  } catch (error) {
    console.error('Could not show the Nabz main window:', error);
  }
})();
