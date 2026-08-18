import { defineConfig } from "astro/config";

export default defineConfig({
  output: "static",
  build: {
    format: "file",
    inlineStylesheets: "always",
  },
  vite: {
    build: {
      // Fonts and the hex backdrop are inlined as data URIs so the built
      // PROTOCOL.html stays a single self-contained file.
      assetsInlineLimit: 256 * 1024,
    },
  },
});
