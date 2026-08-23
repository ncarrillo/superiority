import { defineConfig } from "astro/config";

export default defineConfig({
  output: "static",
  build: {
    format: "file",
    inlineStylesheets: "always",
  },
  vite: {
    build: {
      // Fonts and the hex backdrop are inlined into the standalone page.
      assetsInlineLimit: 256 * 1024,
    },
  },
});
