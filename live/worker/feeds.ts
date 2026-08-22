// Registration: enabling Live in the app calls this once and gets a feed —
// a public slug (the live link) and a secret token (the write credential).
// Losing the token is fine by design: re-registering mints a fresh feed and
// new events flow to the new sink; the orphan ages out via retention.

import { sha256Hex } from "./auth";
import { registrationLimiter } from "./ratelimit";

const SLUG_ALPHABET = "abcdefghijklmnopqrstuvwxyz234567";
const SLUG_BYTES = 8; // 64 bits -> 13 base32 chars: unguessable capability URL

export async function handleRegister(request: Request, env: Env): Promise<Response> {
  if (env.REGISTRATION_OPEN !== "true") {
    return json(403, { error: "registration closed" });
  }
  const ip = request.headers.get("CF-Connecting-IP") ?? "unknown";
  if (!registrationLimiter.allow(ip)) {
    return json(429, { error: "too many registrations" });
  }

  let clientVersion = "";
  let product = "sc2";
  try {
    const body: unknown = await request.json();
    if (typeof body === "object" && body !== null && "client_version" in body) {
      const value = (body as Record<string, unknown>).client_version;
      if (typeof value === "string") clientVersion = value.slice(0, 32);
    }
    if (typeof body === "object" && body !== null && "product" in body) {
      const value = (body as Record<string, unknown>).product;
      if (value === "sc2" || value === "scr" || value === "wc3") product = value;
    }
  } catch {
    // An empty or non-JSON body is fine; client_version is a courtesy.
  }

  const token = randomHex(32);
  const tokenHash = await sha256Hex(token);
  const now = Date.now();

  // Slug collisions are astronomically unlikely (2^64); the loop is a
  // formality so a collision surfaces as a retry instead of a 500.
  for (let attempt = 0; attempt < 3; attempt += 1) {
    const id = randomSlug();
    try {
      await env.DB.prepare(
        `INSERT INTO feeds (id, token_hash, product, client_version, created_at, last_seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)`,
      )
        .bind(id, tokenHash, product, clientVersion, now)
        .run();
      const url = new URL(request.url);
      return json(201, { feed: { id, token, url: `${url.origin}/${id}` } });
    } catch (error) {
      if (attempt === 2) {
        console.error("feed registration failed", { error: String(error) });
      }
    }
  }
  return json(500, { error: "registration failed" });
}

function randomSlug(): string {
  const bytes = new Uint8Array(SLUG_BYTES);
  crypto.getRandomValues(bytes);
  let bits = 0;
  let value = 0;
  let slug = "";
  for (const byte of bytes) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      bits -= 5;
      slug += SLUG_ALPHABET[(value >> bits) & 31];
    }
  }
  if (bits > 0) slug += SLUG_ALPHABET[(value << (5 - bits)) & 31];
  return slug;
}

function randomHex(byteCount: number): string {
  const bytes = new Uint8Array(byteCount);
  crypto.getRandomValues(bytes);
  return [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "Content-Type": "application/json; charset=utf-8", "Cache-Control": "no-store" },
  });
}
