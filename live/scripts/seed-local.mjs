// Seeds a local Superiority Live worker with one feed carrying three product
// sessions — StarCraft II, StarCraft: Remastered, and Warcraft III — so the
// multi-product read API and viewer can be exercised without three real
// Battle.net accounts.
//
//   cd live && npm run dev            # in one terminal: worker + viewer + local D1
//   node scripts/seed-local.mjs       # in another: registers a feed and posts to it
//
// Prints the feed URL. Open it to see the viewer; or curl the product routes:
//   curl -s http://localhost:8787/v1/feeds/<id>/overview | jq '.products[].product'
//   curl -s http://localhost:8787/v1/feeds/<id>/products/scr/channels/public:9/roster | jq

const base = (process.argv[2] ?? "http://localhost:8787").replace(/\/$/, "");
const now = Date.now();
const sessionId = () => [...crypto.getRandomValues(new Uint8Array(16))].map((b) => b.toString(16).padStart(2, "0")).join("");

async function post(path, body, token) {
  const headers = { "content-type": "application/json" };
  if (token) headers.authorization = `Bearer ${token}`;
  const response = await fetch(`${base}${path}`, { method: "POST", headers, body: JSON.stringify(body) });
  const text = await response.text();
  if (!response.ok) throw new Error(`${response.status} ${path}: ${text}`);
  return text ? JSON.parse(text) : {};
}

// One envelope = one product session. `events` must have strictly increasing seq.
function envelope(product, events) {
  return { v: 1, session: { id: sessionId(), product, client_version: "seed", started_at: now }, events };
}

const member = (handle, name, extra = {}) => ({ handle, name, presence: "available", ...extra });

// A synced snapshot: sync_started, a complete roster on one channel, some
// transcript, session_synced. This is what makes the session the feed's visible
// snapshot for that product in `overview.products[]`.
const sc2 = envelope("sc2", [
  { seq: 1, ts: now, kind: "sync_started" },
  { seq: 2, ts: now + 1, kind: "roster", channel: { key: "public:1033", name: "Public 1033" }, complete: true,
    users: [member(1, "Overmind", { is_local: true }), member(2, "Kerrigan", { clan_tag: "SWRM" }), member(3, "Raynor", { presence: "away" })] },
  { seq: 3, ts: now + 2, kind: "message", channel: { key: "public:1033" }, sender: { handle: 2, name: "Kerrigan", clan_tag: "SWRM" }, body: "gl hf" },
  { seq: 4, ts: now + 3, kind: "session_synced" },
]);

const scr = envelope("scr", [
  { seq: 1, ts: now, kind: "sync_started" },
  { seq: 2, ts: now + 1, kind: "roster", channel: { key: "public:9", name: "Public Chat 1" }, complete: true,
    users: [
      member(101, "Commander", { is_local: true, is_operator: true, avatar: "avatar_terran_marine" }),
      member(102, "Darko", { avatar: "avatar_protoss_zealot" }),
      member(103, "Kerrigan", { presence: "away", avatar: "avatar_zerg_queen" }),
    ] },
  { seq: 3, ts: now + 2, kind: "message", channel: { key: "public:9" }, subkind: "talk", sender: { handle: 102, name: "Darko" }, body: "anyone up for a game?" },
  { seq: 4, ts: now + 3, kind: "message", channel: { key: "public:9" }, subkind: "emote", sender: { handle: 101, name: "Commander" }, body: "waves" },
  { seq: 5, ts: now + 4, kind: "notice", channel: { key: "public:9" }, subkind: "broadcast", body: "Server maintenance in 10 minutes." },
  { seq: 6, ts: now + 5, kind: "notice", channel: { key: "public:9" }, subkind: "information", body: "Welcome to Public Chat 1." },
  { seq: 7, ts: now + 6, kind: "session_synced" },
]);

const wc3 = envelope("wc3", [
  { seq: 1, ts: now, kind: "sync_started" },
  { seq: 2, ts: now + 1, kind: "roster", channel: { key: "private:W3 General", name: "W3 General" }, complete: true,
    users: [
      member(201, "Grubby", { avatar: "p126", clan_tag: "4K" }),
      member(202, "Moon", { avatar: "p003" }),
      member(203, "Sky", { presence: "offline" }),
    ] },
  { seq: 3, ts: now + 2, kind: "message", channel: { key: "private:W3 General" }, body: "The hall stirs." },
  { seq: 4, ts: now + 3, kind: "notice", channel: { key: "private:W3 General" }, subkind: "information", body: "Grubby entered the channel." },
  { seq: 5, ts: now + 4, kind: "session_synced" },
]);

const feed = (await post("/v1/feeds", { product: "sc2" })).feed;
for (const env of [sc2, scr, wc3]) {
  const accepted = await post("/v1/events", env, feed.token);
  console.log(`  ${env.session.product}: ${accepted.accepted} events`);
}
console.log(`\nfeed ready: ${feed.url}`);
console.log(`overview:   ${base}/v1/feeds/${feed.id}/overview`);
