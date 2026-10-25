import { defineConfig } from 'vite';
import config from './tauri.conf.json' with { type: 'json' };

export default defineConfig({
  // Exercise the same policy as the packaged webview, including local data-URI icons.
  preview: { headers: { 'Content-Security-Policy': config.app.security.csp } },
});
