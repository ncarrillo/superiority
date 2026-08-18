import type { ThemeRegistration } from "@shikijs/types";

/**
 * Shiki theme matching the Superiority design language used by global.css:
 * comments dim, numbers/strings green, keywords orange, types blue.
 */
export const superiorityCodeTheme: ThemeRegistration = {
  name: "superiority",
  type: "dark",
  colors: {
    "editor.background": "#01060d",
    "editor.foreground": "#d6e0f0",
  },
  tokenColors: [
    { scope: ["comment", "punctuation.definition.comment"], settings: { foreground: "#7d94b0" } },
    { scope: ["string", "punctuation.definition.string"], settings: { foreground: "#47d184" } },
    { scope: ["constant.numeric", "constant.language"], settings: { foreground: "#47d184" } },
    { scope: ["keyword", "storage.type", "storage.modifier"], settings: { foreground: "#f0aa64" } },
    {
      scope: [
        "entity.name.type",
        "entity.name.struct",
        "entity.name.enum",
        "support.type",
        "support.class",
      ],
      settings: { foreground: "#6bc2f2" },
    },
    { scope: ["variable", "entity.name.field", "meta.attribute"], settings: { foreground: "#d6e0f0" } },
  ],
};
