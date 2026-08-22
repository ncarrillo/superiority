// wire dtos shared by ingest and persistence.

export interface WireSession {
  id: string;
  product: ProductSlug;
  client_version: string;
  started_at: number;
}

export type ProductSlug = "sc2" | "scr" | "wc3";

const PRODUCTS = new Set<ProductSlug>(["sc2", "scr", "wc3"]);

export interface WireChannel {
  key: string;
  name?: string | null;
}

export interface WirePortrait {
  t: number;
  o: number;
}

export interface WireUser {
  handle: number;
  name?: string | null;
  clan_tag?: string | null;
  presence?: string | null;
  portrait?: WirePortrait | null;
  avatar?: string | null;
  is_operator?: boolean | null;
  is_local?: boolean | null;
  joined_order?: number | null;
}

export type WireEvent = {
  seq: number;
  ts: number;
  kind: string;
  subkind?: string;
  channel?: WireChannel;
  sender?: WireUser;
  user?: WireUser;
  users?: WireUser[];
  body?: string;
  complete?: boolean;
  count?: number;
} & Record<string, unknown>;

export interface Envelope {
  v: 1;
  session: WireSession;
  events: WireEvent[];
}

export const MAX_EVENTS_PER_POST = 64;
export const MAX_BODY_BYTES = 1_048_576;
export const MAX_EVENT_JSON_CHARS = 900_000;
export const MAX_MESSAGE_CHARS = 4000;
export const MAX_ROSTER_USERS = 4096;

const EVENT_KINDS = new Set([
  "session_started",
  "sync_started",
  "session_synced",
  "heartbeat",
  "session_ended",
  "joined",
  "left",
  "message",
  "notice",
  "member_joined",
  "member_left",
  "roster",
  "roster_delta",
  "dropped",
]);

// What a stored transcript row is (messages.kind). Chat refines itself with
// an optional subkind (talk unless said otherwise); notices must say which;
// membership rows are written from the member events.
export type MessageKind = "talk" | "emote" | "broadcast" | "information" | "member_joined" | "member_left";

const MESSAGE_SUBKINDS = new Set(["talk", "emote"]);
const NOTICE_SUBKINDS = new Set(["broadcast", "information"]);

export const MESSAGE_KINDS: ReadonlySet<MessageKind> = new Set<MessageKind>([
  "talk",
  "emote",
  "broadcast",
  "information",
  "member_joined",
  "member_left",
]);

const CHANNEL_KEY_NUMERIC = /^(?:public|club):\d{1,10}$/;

type Valid = { ok: true; envelope: Envelope };
type Invalid = { ok: false; error: string };

export function validateEnvelope(data: unknown): Valid | Invalid {
  if (!isRecord(data)) return fail("body: not an object");
  if (data.v !== 1) return fail("v: unsupported version");

  const session = data.session;
  if (!isRecord(session)) return fail("session: not an object");
  if (!isNonEmptyString(session.id, 64)) return fail("session.id: not a string of 1..64 chars");
  // Envelopes produced before product-aware Live sessions were introduced
  // can only have come from SC2. Keep accepting them during rollout.
  const product = session.product === undefined ? "sc2" : session.product;
  if (!isProductSlug(product)) return fail("session.product: unsupported product");
  if (!isString(session.client_version, 32)) return fail("session.client_version: not a string of <=32 chars");
  if (!isPositiveInt(session.started_at)) return fail("session.started_at: not a positive integer");

  const events = data.events;
  if (!Array.isArray(events)) return fail("events: not an array");
  if (events.length < 1 || events.length > MAX_EVENTS_PER_POST) {
    return fail(`events: expected 1..${MAX_EVENTS_PER_POST} entries`);
  }

  let previousSeq = -1;
  for (const [index, event] of events.entries()) {
    const error = validateEvent(event, index);
    if (error !== null) return fail(error);
    const seq = (event as Record<string, unknown>).seq as number;
    if (seq <= previousSeq) return fail(`events[${index}].seq: not strictly increasing`);
    previousSeq = seq;
  }

  return {
    ok: true,
    envelope: {
      v: 1,
      session: {
        id: session.id as string,
        product,
        client_version: session.client_version as string,
        started_at: session.started_at as number,
      },
      events: events as WireEvent[],
    },
  };
}

function isProductSlug(value: unknown): value is ProductSlug {
  return typeof value === "string" && PRODUCTS.has(value as ProductSlug);
}

function validateEvent(event: unknown, index: number): string | null {
  const at = `events[${index}]`;
  if (!isRecord(event)) return `${at}: not an object`;
  if (!isNonNegativeInt(event.seq)) return `${at}.seq: not a non-negative integer`;
  if (!isPositiveInt(event.ts)) return `${at}.ts: not a positive integer`;
  if (!isNonEmptyString(event.kind, 32)) return `${at}.kind: not a string of 1..32 chars`;
  if (!EVENT_KINDS.has(event.kind as string)) return `${at}.kind: unsupported event`;

  if (event.channel !== undefined) {
    const channel = event.channel;
    if (!isRecord(channel)) return `${at}.channel: not an object`;
    if (!isValidChannelKey(channel.key)) return `${at}.channel.key: not a channel identity`;
    if (channel.name !== undefined && channel.name !== null && !isString(channel.name, 128)) {
      return `${at}.channel.name: not a string of <=128 chars`;
    }
  }
  if (event.body !== undefined && !isString(event.body, MAX_MESSAGE_CHARS)) {
    return `${at}.body: not a string of <=${MAX_MESSAGE_CHARS} chars`;
  }
  if (event.kind === "message") {
    if (typeof event.body !== "string") return `${at}.body: required`;
    if (event.subkind !== undefined && !MESSAGE_SUBKINDS.has(event.subkind as string)) {
      return `${at}.subkind: not 'talk' | 'emote'`;
    }
  }
  if (event.kind === "notice") {
    if (event.channel === undefined) return `${at}.channel: required`;
    if (typeof event.body !== "string") return `${at}.body: required`;
    if (!NOTICE_SUBKINDS.has(event.subkind as string)) {
      return `${at}.subkind: not 'broadcast' | 'information'`;
    }
  }
  if (event.users !== undefined) {
    if (!Array.isArray(event.users) || event.users.length > MAX_ROSTER_USERS) {
      return `${at}.users: not an array of <=${MAX_ROSTER_USERS} entries`;
    }
  }
  if (JSON.stringify(event).length > MAX_EVENT_JSON_CHARS) return `${at}: too large`;
  return null;
}

export function isValidChannelKey(value: unknown): value is string {
  if (typeof value !== "string") return false;
  if (CHANNEL_KEY_NUMERIC.test(value)) return true;
  return value.startsWith("private:") && value.length > "private:".length && value.length <= 72;
}

/** 'public:1033' -> 'public'; anything before the first colon. */
export function channelKind(key: string): string {
  const colon = key.indexOf(":");
  return colon > 0 ? key.slice(0, colon) : "unknown";
}

export function userField(value: unknown): WireUser | null {
  if (!isRecord(value) || !isNonNegativeInt(value.handle)) return null;
  return {
    handle: value.handle as number,
    name: isString(value.name, 128) ? (value.name as string) : null,
    clan_tag: isString(value.clan_tag, 32) ? (value.clan_tag as string) : null,
    presence: isString(value.presence, 16) ? (value.presence as string) : null,
    portrait: portraitField(value.portrait),
    avatar: isString(value.avatar, 128) ? (value.avatar as string) : null,
    is_operator: typeof value.is_operator === "boolean" ? value.is_operator : null,
    is_local: typeof value.is_local === "boolean" ? value.is_local : null,
    joined_order: isNonNegativeInt(value.joined_order) ? (value.joined_order as number) : null,
  };
}

function portraitField(value: unknown): WirePortrait | null {
  if (!isRecord(value)) return null;
  const t = value.t;
  const o = value.o;
  if (!isNonNegativeInt(t) || !isNonNegativeInt(o)) return null;
  if ((t as number) > 64 || (o as number) > 255) return null;
  return { t: t as number, o: o as number };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isString(value: unknown, maxLength: number): boolean {
  return typeof value === "string" && value.length <= maxLength;
}

function isNonEmptyString(value: unknown, maxLength: number): boolean {
  return typeof value === "string" && value.length >= 1 && value.length <= maxLength;
}

function isPositiveInt(value: unknown): boolean {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

function isNonNegativeInt(value: unknown): boolean {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function fail(error: string): Invalid {
  return { ok: false, error };
}
