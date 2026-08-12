# SC2 protocol reference site

This Astro project is the source for `../PROTOCOL.html`. It uses build-time
components only: no component is hydrated and the generated reference contains
no client-side JavaScript.

```console
npm install
npm run build
```

The build first creates `dist/PROTOCOL.html`, then publishes the same standalone
artifact to `../PROTOCOL.html` so existing links and bookmarks continue to work.
