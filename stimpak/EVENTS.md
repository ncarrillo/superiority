# Stimpak event schema 2

Every native event is one UTF-8 JSON object with a snake-case `type`
discriminator. Fields described as optional are either a value or `null`.
Unknown native event types become `UnknownEvent` in the managed binding;
malformed known events become `EventProtocolError` instead of disappearing.

## Connection

- `stage`: `stage` is `disconnected`, `web_authentication`, `game_utilities`,
  `native_authentication`, `chat_bootstrap`, or `connected`.
- `authentication_required`: `auth_id`, `url`, and `fresh_account`. Reply with
  the same id through submit or cancel. An optional provider uses
  `fresh_account` to clear its browser identity before showing the page.
- `account`: `account` contains optional `account_id`, `battle_tag`, `region`,
  and `games` FourCCs.
- `command_error`: `message`; the connection remains usable.
- `error`: `message`; a disconnected stage normally follows.
- `session_ended`: the native worker has finished and no more events will arrive.

## Channels

A channel is tagged by `kind`:

- `public`: `id`, resolved `name`
- `private`: `name`
- `group`: `club_id`, resolved `name`
- `party`: `name`

Channel events are:

- `public_channels`: `channels`
- `joined`: `channel_index`, `channel`, `local_handle`
- `join_rejected`: optional `channel`, optional numeric `reason`
- `left`: `channel_index`, optional numeric `reason`

`channel_index` belongs to one connection. Never persist it across a disconnect.

## People and chat

A user contains `handle`, optional `presence_id`, display `name`, optional
`clan_tag`, and `presence`. A handle identifies membership within one channel;
only `presence_id` can connect the same account across channels.

- `roster`: `channel_index`, `complete`, `users`. It replaces the preceding
  roster for that channel.
- `member_joined`, `member_left`: `channel_index`, `user`
- `message`: `channel_index`, `sender`, `body`
- `whisper`: `peer`, `body`, `outgoing`
- `whisper_failed`: `peer`, `reason`
- `friends`: `friends`, each with `name` and `presence`

Presence is `available`, `away`, `busy`, `in_game`, `offline`, or `unknown`.

## Groups and parties

- `group_invitation`: `club_id`
- `party_invitation`: optional `inviter`, `channel_index`
- `group_summary`: `club_id`, optional `name`, numeric `kind` and `category`,
  `private`, `member`, optional `member_count`, optional `online`
- `group_search`: `club_ids`; matching summaries arrive separately

## Deliberately opaque protocol activity

`other` contains only `kind`. Low-level activity, conference pages, block lists,
and future internal protocol details remain outside the public contract until
Stimpak gives them a stable, privacy-reviewed meaning.
