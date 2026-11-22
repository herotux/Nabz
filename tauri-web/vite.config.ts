import { defineConfig } from "vite";

export default defineConfig({
  // Relative asset URLs work both in Tauri's local asset protocol and when
  // the web build is hosted below a repository path (for example /Nabz/).
  base: "./",
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
  build: {
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari13",
  },
});
