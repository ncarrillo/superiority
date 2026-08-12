# Superiority landing page

The download page at
[superiority-sc2-updates.pages.dev](https://superiority-sc2-updates.pages.dev/),
built with Astro. One static page, no client-side JavaScript.

The page says nothing about a version. It links to `releases/Superiority.dmg`,
one object that every release overwrites, so shipping a release changes nothing
here and there is no version, size, or changelog to keep in step.
`SUPERIORITY_RELEASE_BASE_URL` points the download at a different bucket for a
test deploy.

## Working on it

```
npm install
npm run dev
```

It is published by `scripts/publish-update-macos.zsh`, which deploys it to the same
Cloudflare Pages project that serves `appcast.xml`. That is the only reason a
release builds it at all — a Pages deploy replaces the whole site, so the page
has to be in the same one. There is no separate deploy step.
