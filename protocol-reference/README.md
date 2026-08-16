# SC2Docs

This Astro project is the source for `../PROTOCOL.html`. It uses static Astro
components, and the generated reference contains zero client-side JavaScript.

```console
npm install
npm run build
```

The build first creates `dist/PROTOCOL.html`, then publishes the same standalone
artifact to `../PROTOCOL.html` so existing links and bookmarks continue to work.

## Writing standard

The document follows controlled-language principles:

- Use one fact or instruction in each sentence.
- Use active voice when the actor is known.
- Use the same term for the same protocol object.
- Use direct requirements. Prefer positive instructions for constraints and
  prohibited states.
- Use `can` only for a capability.
- Keep protocol names and route names unchanged.
- Use snake case for explanatory payload field names.
- Prefer tables, diagrams, and ordered steps to compound prose.
- Render decoded Sunken payloads as concise structured definitions.
- Label wire widths and metadata bounds separately in short comments.
- State the actual wire sequence alongside each structured definition.

## Client-derived notes

- [BSN obfuscation and generated wire layouts](BSN_OBFUSCATION.md) explains
  metadata bit `0x40`, field permutation, filler runs, and the layout model.
