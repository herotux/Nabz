import { defineConfig } from "vite";

export default defineConfig({
  // Tauri serves the bundled frontend from the application origin root.
  // GitHub Pages is the only build that should use the repository subpath.
  base: process.env.TAURI_ENV_PLATFORM ? "/" : process.env.GITHUB_ACTIONS ? "/Nabz/" : "/",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
  },
});
