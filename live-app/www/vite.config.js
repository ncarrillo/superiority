import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

const backend = process.env.SUPERIORITY_LIVE_BACKEND ?? "https://live.superioritybot.com";
const headers = {
  "Cross-Origin-Embedder-Policy": "require-corp",
  "Cross-Origin-Opener-Policy": "same-origin"
};
const proxy = {
  "/v1": { target: backend, changeOrigin: true }
};

export default defineConfig({
  plugins: [wasm()],
  build: { target: "esnext" },
  server: {
    port: 3010,
    headers,
    proxy,
  },
  preview: {
    port: 4173,
    headers,
    proxy,
  },
  optimizeDeps: { exclude: ["./src/wasm"] }
});
