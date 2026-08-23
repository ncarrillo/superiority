import { readFileSync } from "node:fs";

export type BsnTypeKind =
  | "Alias"
  | "Array"
  | "BitArray"
  | "Blob"
  | "Bool"
  | "ByteString"
  | "Choice"
  | "Enum"
  | "Float"
  | "FourCc"
  | "Integer"
  | "Optional"
  | "String"
  | "Struct"
  | "Void";

export interface BsnIntegerRange {
  encoding: number;
  bitWidth?: number;
  controlFlag: boolean;
  minimum: string;
  maximum: string;
}

export interface BsnType {
  id: number;
  name?: string;
  kind: BsnTypeKind;
  implicitIndices: boolean;
  obfuscated: boolean;
  range?: BsnIntegerRange;
  elementType?: number;
  indexValues: number[];
  memberTypes: number[];
  memberNames: Array<string | undefined>;
}

const source = readFileSync(
  new URL("../../../core/src/games/sc2/native/schema/wire.rs", import.meta.url),
  "utf8",
);

const shapePattern = /StaticTypeShape \{\s*type_id: (\d+),\s*name: (None|Some\("((?:\\.|[^"\\])*)"\)),\s*kind: TypeKind::(\w+),\s*implicit_indices: (true|false),\s*obfuscated: (true|false),\s*value_range: (None|Some\(IntegerRange \{ encoding: (\d+), bit_width: (None|Some\((\d+)\)), control_flag: (true|false), minimum: (-?\d+), maximum: (-?\d+) \}\)),\s*element_type: (None|Some\((\d+)\)),\s*index_values: &\[([^\]]*)\],\s*member_types: &\[([^\]]*)\],\s*member_names: &\[([^\]]*)\],\s*\},/g;

function decodeRustString(value: string): string {
  return JSON.parse(`"${value}"`) as string;
}

function parseNumbers(value: string): number[] {
  if (!value.trim()) return [];
  return value.split(",").map((part) => Number.parseInt(part.trim(), 10));
}

function parseNames(value: string): Array<string | undefined> {
  if (!value.trim()) return [];
  return [...value.matchAll(/None|Some\("((?:\\.|[^"\\])*)"\)/g)].map(
    (match) => match[0] === "None" ? undefined : decodeRustString(match[1]),
  );
}

export const bsnTypes: BsnType[] = [];

for (const match of source.matchAll(shapePattern)) {
  bsnTypes.push({
    id: Number.parseInt(match[1], 10),
    name: match[2] === "None" ? undefined : decodeRustString(match[3]),
    kind: match[4] as BsnTypeKind,
    implicitIndices: match[5] === "true",
    obfuscated: match[6] === "true",
    range: match[7] === "None"
      ? undefined
      : {
          encoding: Number.parseInt(match[8], 10),
          bitWidth: match[9] === "None" ? undefined : Number.parseInt(match[10], 10),
          controlFlag: match[11] === "true",
          minimum: match[12],
          maximum: match[13],
        },
    elementType: match[14] === "None" ? undefined : Number.parseInt(match[15], 10),
    indexValues: parseNumbers(match[16]),
    memberTypes: parseNumbers(match[17]),
    memberNames: parseNames(match[18]),
  });
}

const sourceShapeCount = source.match(/StaticTypeShape \{/g)?.length ?? 0;
if (bsnTypes.length !== sourceShapeCount) {
  throw new Error(
    `parsed ${bsnTypes.length} of ${sourceShapeCount} BSN metadata records`,
  );
}

export const bsnTypeById = new Map(bsnTypes.map((type) => [type.id, type]));
export const bsnNamedTypes = bsnTypes
  .filter((type): type is BsnType & { name: string } => Boolean(type.name))
  .sort((left, right) => left.name.localeCompare(right.name));

export function typeAnchor(type: BsnType & { name: string }): string {
  return `type-${type.name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")}`;
}

export function shortTypeName(name: string): string {
  return name.split("::").at(-1) ?? name;
}

interface TypeAlias {
  alias: string;
  priority: number;
}

function typeAliases(name: string): TypeAlias[] {
  const parts = name.split("::");
  const aliases: TypeAlias[] = [
    { alias: name, priority: 3 },
    { alias: parts.at(-1) ?? name, priority: 1 },
  ];
  for (let length = 2; length <= parts.length; length += 1) {
    const suffix = parts.slice(-length);
    aliases.push({ alias: suffix.join("::"), priority: 2 });
    aliases.push({ alias: suffix.join(""), priority: 2 });
  }
  if (parts.at(-1) === "Enum" && parts.length >= 2) {
    aliases.push({ alias: parts.at(-2)!, priority: 2 });
  }
  return aliases;
}

const schemaAliases: Record<string, string> = {
  AccountBlock: "Battlenet::Friends::AccountBlockContainer",
  AccountFriend: "Battlenet::Friends::FriendContainer5::Account",
  AchievementHandle: "Battlenet::Achievement::ProgramHandleAggregation",
  AddressQueryResult: "Battlenet::Client::Profile::AddressQueryResponse::Result",
  AuthProofRequest: "Battlenet::Client::Authentication::ProofRequest",
  ChannelConfig: "Battlenet::Conference::ConferenceConfiguration",
  ChannelListType: "Battlenet::Client::Chat::ChannelListType::Enum",
  CharacterFriend: "Battlenet::Friends::FriendContainer5::Character",
  ClientHeader: "Battlenet::Frame::Header::Data::Client",
  ClubSearch: "Battlenet::Client::Club::SearchClubsRequest::Search",
  ConferenceDescription: "Battlenet::Conference::FullConferenceDescription",
  ConferenceLocator: "Battlenet::Conference::LocatorKey",
  ContentHeader: "Battlenet::Frame::Header::Data::Content",
  CorrelationHeader: "Battlenet::Frame::Header::Data::Correlation",
  CurrentSeasonRequest: "Battlenet::Client::S2Master::CurrentSeason",
  CurrentSeasonResult: "Battlenet::Client::S2Master::CurrentSeasonResponse::Result",
  Data: "Battlenet::Client::Achievement::Data",
  ErrorHeader: "Battlenet::Frame::Header::Data::Error",
  FieldSpecAnnounce: "Battlenet::Client::Presence::FieldSpecAnnounce",
  FieldSize: "Battlenet::Presence::FieldSpec::Size",
  FavoritesPage: "Battlenet::Client::S2Map::S2ListMapFavoritesResponse::Result::Success",
  FriendToon: "Battlenet::Friends::ToonOfFriend",
  FriendOperation: "Battlenet::Friends::FriendshipUpdate5::Update",
  FriendSubject: "Battlenet::Friends::FriendContainer5",
  FriendUpdate: "Battlenet::Friends::FriendshipUpdate5",
  FriendsListNotify: "Battlenet::Client::Friends::FriendsListNotify5",
  FrameType: "Battlenet::Frame::Type::Enum",
  GameGroup: "Battlenet::Client::S2Map::GameGroupUpdate::Results::GameGroup",
  GameGroupResults: "Battlenet::Client::S2Map::GameGroupUpdate::Results",
  GetStreamItemsRequest: "Battlenet::Client::Cache::GetStreamItems",
  GetToonClubsRequest: "Battlenet::Client::Club::GetToonClubs",
  InviteAction: "Battlenet::Client::Club::InviteAction",
  InviteActionCode: "Battlenet::Club::InviteCode::Enum",
  InviteMember: "Battlenet::Toon::Handle",
  Ipv4AddressPort: "Battlenet::IP4::AddressPort",
  JoinResult: "Battlenet::Chat::JoinNotifyResult2",
  JoinNotify: "Battlenet::Client::Chat::JoinNotify2",
  JoinRequest: "Battlenet::Client::Chat::JoinRequest2",
  LevelZero: "Battlenet::Presence::SharedPackets::Level0Info",
  ListMapFavoritesRequest: "Battlenet::Client::S2Map::S2ListMapFavoritesRequest",
  ListMapFavoritesResponse: "Battlenet::Client::S2Map::S2ListMapFavoritesResponse",
  LogonFailure: "Battlenet::Client::Authentication::ResponseFailure",
  LogonResult: "Battlenet::Client::Authentication::LogonResponse::Result",
  LogonSuccess: "Battlenet::Client::Authentication::LogonResponseSuccess",
  LocalPresenceId: "Battlenet::Presence::Id",
  MapGroup: "Battlenet::Client::S2Map::GameGroupUpdate::Results::MapGroup",
  Matches: "Battlenet::Client::Club::SearchClubsResponse::Result::Success",
  MembershipChangeJoining: "Battlenet::Chat::MembershipChange::JoinChannel",
  MMQGetListRequest: "Battlenet::Client::S2Master::MMQGetList",
  PlayerTarget: "Battlenet::Client::Defines::PlayerTarget",
  PresenceFriend: "Battlenet::Friends::FriendContainer5::PersistentPresenceUpdate",
  RafState: "Battlenet::GameAccount::RafDataBlob",
  ReadRequest: "Battlenet::Client::Profile::Read",
  ReadBlock: "Battlenet::Profile::ProfileDataResponse::Block",
  ReadCache: "Battlenet::Profile::ProfileDataResponse::Cache",
  ReadFailure: "Battlenet::Profile::ProfileDataResponse::Failure",
  ReadResult: "Battlenet::Profile::ProfileDataResponse",
  ReadStart: "Battlenet::Profile::ProfileDataResponse::Start",
  ReplicateHeader: "Battlenet::Frame::Header::Data::Replicate",
  ResolveToonNameRequest: "Battlenet::Client::Profile::ResolveToonNameToHandleRequest",
  ResolveToonNameResponse: "Battlenet::Client::Profile::ResolveToonNameToHandleResponse",
  RegulatorRules: "Battlenet::Regulator::Info",
  ResumeFailure: "Battlenet::Client::Authentication::ResponseFailure::Result",
  ResumeResult: "Battlenet::Client::Authentication::ResumeResponse::Result",
  RouteHeader: "Battlenet::Frame::Header::Data::Route",
  SearchResult: "Battlenet::Client::Club::SearchClubsResponse::Result",
  ServiceHeader: "Battlenet::Frame::Header::Data::Service",
  SettingType: "Battlenet::Client::Profile::SettingsType::Enum",
  StatusChange: "Battlenet::Chat::MemberStatusSingle",
  StreamHeader: "Battlenet::Frame::Header::Data::Stream",
  TargetHeader: "Battlenet::Frame::Header::Data::Target",
  TimestampHeader: "Battlenet::Frame::Header::Data::Timestamp",
  ToonCreationFailure: "Battlenet::Client::Toon::Failure",
  TraceRouteHeader: "Battlenet::Frame::Header::Data::TraceRoute",
  UnlockableDefinition: "Battlenet::Toon::UnlockDefinitionFile",
  UpdateNotify: "Battlenet::Client::Presence::UpdateNotify",
  Version: "Battlenet::Version::Record",
  WhisperSend: "Battlenet::Client::Chat::Whisper",
};

interface AliasCandidate {
  type: BsnType & { name: string };
  priority: number;
}

const aliasCandidates = new Map<string, AliasCandidate[]>();
for (const type of bsnNamedTypes) {
  for (const candidate of typeAliases(type.name)) {
    const candidates = aliasCandidates.get(candidate.alias) ?? [];
    const existing = candidates.find((entry) => entry.type.id === type.id);
    if (existing) {
      existing.priority = Math.max(existing.priority, candidate.priority);
    } else {
      candidates.push({ type, priority: candidate.priority });
    }
    aliasCandidates.set(candidate.alias, candidates);
  }
}

for (const [alias, name] of Object.entries(schemaAliases)) {
  const type = bsnNamedTypes.find((candidate) => candidate.name === name);
  if (!type) throw new Error(`schema alias ${alias} refers to missing BSN type ${name}`);
  const candidates = aliasCandidates.get(alias) ?? [];
  candidates.push({ type, priority: 4 });
  aliasCandidates.set(alias, candidates);
}

export const typeLinks = new Map<string, { anchor: string; name: string; tooltip: string }>();
for (const [alias, candidates] of aliasCandidates) {
  const maximumPriority = Math.max(...candidates.map((candidate) => candidate.priority));
  const preferred = candidates.filter((candidate) => candidate.priority === maximumPriority);
  if (preferred.length !== 1) continue;
  const type = preferred[0].type;
  typeLinks.set(alias, {
    anchor: typeAnchor(type),
    name: type.name,
    tooltip: typeTooltip(type),
  });
}

export function namedTypeForReference(typeId: number): (BsnType & { name: string }) | undefined {
  const visited = new Set<number>();
  let current = bsnTypeById.get(typeId);

  while (current && !current.name && current.kind === "Alias" && current.elementType !== undefined) {
    if (visited.has(current.id)) return undefined;
    visited.add(current.id);
    current = bsnTypeById.get(current.elementType);
  }

  return current?.name ? current as BsnType & { name: string } : undefined;
}

export function displayTypeReference(typeId: number): string {
  const type = bsnTypeById.get(typeId);
  if (!type) return `metadata record ${typeId}`;
  if (type.name) return shortTypeName(type.name);

  if (type.kind === "Alias" && type.elementType !== undefined) {
    return displayTypeReference(type.elementType);
  }
  if (type.kind === "Optional" && type.elementType !== undefined) {
    return `optional ${displayTypeReference(type.elementType)}`;
  }
  if (type.kind === "Array" && type.elementType !== undefined) {
    return `array of ${displayTypeReference(type.elementType)}`;
  }
  return `${type.kind.toLowerCase()} record ${type.id}`;
}

function rangeSummary(type: BsnType, subject: string): string | undefined {
  if (!type.range) return undefined;
  const width = type.range.bitWidth === undefined ? "variable width" : `u${type.range.bitWidth}`;
  return `${subject}: ${width} · accepted ${type.range.minimum}..=${type.range.maximum}`;
}

function memberName(type: BsnType, index: number): string {
  return type.memberNames[index] ?? `field_${type.indexValues[index] ?? index}`;
}

function limitedMembers(type: BsnType, format: (memberType: number, index: number) => string): string[] {
  const shown = type.memberTypes.slice(0, 6).map(format);
  const remaining = type.memberTypes.length - shown.length;
  if (remaining > 0) shown.push(`… ${remaining} more`);
  return shown;
}

export function typeTooltip(type: BsnType & { name: string }): string {
  const lines = [type.name, `${type.kind.toLowerCase()} · metadata record ${type.id}`];

  switch (type.kind) {
    case "Alias":
      if (type.elementType !== undefined) lines.push(`target: ${displayTypeReference(type.elementType)}`);
      break;
    case "Array":
      if (type.range) lines.push(rangeSummary(type, "count")!);
      if (type.elementType !== undefined) lines.push(`element: ${displayTypeReference(type.elementType)}`);
      break;
    case "BitArray":
      if (type.range) lines.push(rangeSummary(type, "bit count")!);
      break;
    case "Blob":
    case "ByteString":
    case "String":
      if (type.range) lines.push(rangeSummary(type, "byte count")!);
      lines.push("alignment: byte");
      break;
    case "Optional":
      lines.push("presence: u1");
      if (type.elementType !== undefined) lines.push(`value: ${displayTypeReference(type.elementType)}`);
      break;
    case "Choice":
      if (type.range) lines.push(rangeSummary(type, "selector")!);
      lines.push(...limitedMembers(type, (memberType, index) =>
        `${type.indexValues[index] ?? index} => ${memberName(type, index)}: ${displayTypeReference(memberType)}`));
      break;
    case "Enum":
      if (type.range) lines.push(rangeSummary(type, "value")!);
      type.memberNames.slice(0, 6).forEach((name, index) => {
        lines.push(`${name ?? `value_${type.indexValues[index] ?? index}`} = ${type.indexValues[index] ?? index}`);
      });
      if (type.memberNames.length > 6) lines.push(`… ${type.memberNames.length - 6} more`);
      break;
    case "Integer":
    case "Float":
      if (type.range) lines.push(rangeSummary(type, "value")!);
      break;
    case "Bool":
      lines.push("value: u1");
      break;
    case "FourCc":
      lines.push("value: u32");
      break;
    case "Struct":
      lines.push(type.obfuscated ? "field order: generated wire layout" : "field order: metadata");
      lines.push(...limitedMembers(type, (memberType, index) =>
        `${memberName(type, index)}: ${displayTypeReference(memberType)}`));
      break;
  }

  return lines.join("\n");
}
