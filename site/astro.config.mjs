import { defineConfig } from "astro/config";

// One page with no client-side JavaScript, so the stylesheet rides inline and
// the download link is in the HTML the browser first receives.
export default defineConfig({
  output: "static",
  build: {
    format: "file",
    inlineStylesheets: "always",
  },
});
