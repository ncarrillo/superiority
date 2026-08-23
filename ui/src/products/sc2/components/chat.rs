use std::{ops::Range, rc::Rc};

use gpui::{
    AnyElement, App, Div, ElementId, HighlightStyle, IntoElement, RenderOnce, ScrollHandle,
    SharedString, Stateful, StyledText, Window, div, prelude::*, px, rgb, rgba,
};

use crate::{
    products::sc2::theme::{
        ACCENT, FONT_INTERFACE, FONT_INTERNATIONAL, FONT_NAVIGATION, MUTED, NOTICE, ONLINE, TEXT,
    },
    products::sc2::{
        DigestEvent, MembershipEvent, MembershipKind, RosterUser, RosterUserTone, Sc2Assets,
        TranscriptLine,
    },
};
use url::Url;

pub use crate::patterns::transcript::TranscriptSelection;
use crate::patterns::transcript::{self, SelectableText};

use crate::foundation::text_input::SELECTION_BACKGROUND;

/// party talk stays in the room you are reading rather than moving to a tab of
/// its own, so it is told apart by colour: a green rail down the line, a green
/// name, and a body that carries a little of the same green.
const PARTY_NAME: u32 = ONLINE;
const PARTY_BODY: u32 = 0x00c8_e8d4;
/// the rail hangs in the gutter rather than pushing the words across, so party
/// lines still read down the same column as everything else.
const PARTY_RAIL_WIDTH: f32 = 2.0;
const PARTY_RAIL_GAP: f32 = 8.0;
/// the chip a party line wears is the chip the field wears while you are typing
/// to the party, so the scope you sent in and the line you sent are the same
/// object.
const PARTY_CHIP_TEXT: u32 = 0x000a_1f12;
/// the wash fades out to the right: enough green under the name to mark the
/// line, gone by the time the sentence ends.
const PARTY_WASH: u32 = 0x47d1_8426;
const PARTY_WASH_END: u32 = 0x47d1_8400;

struct ContentStyle {
    text: String,
    color: u32,
    /// the colour the speaker's name takes. blue for the channel, green when
    /// the line was said to the party.
    emphasis_color: u32,
    emphasis: Option<Range<usize>>,
    accents: Vec<Range<usize>>,
    links: Vec<(Range<usize>, String)>,
}

fn content_style(line: &TranscriptLine) -> ContentStyle {
    match line {
        TranscriptLine::Notice { text, .. } => {
            linked_content(text.clone(), NOTICE, None, Vec::new())
        }
        TranscriptLine::SessionStart {
            channel, online, ..
        } => linked_content(
            format!("You joined {channel} · {online} online"),
            NOTICE,
            None,
            Vec::new(),
        ),
        TranscriptLine::Membership(event) => {
            linked_content(event.summary(), MUTED, None, Vec::new())
        }
        TranscriptLine::Digest(event) => linked_content(event.summary(), MUTED, None, Vec::new()),
        TranscriptLine::Message { sender, text, .. } => {
            let prefix = format!("{}:", sender.name);
            let emphasis = 0..prefix.len();
            let (body, lobby_links) = display_message_body(text);
            let body_start = prefix.len() + 1;
            let party = sender.tone == RosterUserTone::Party;
            emphasized_content(
                format!("{prefix} {body}"),
                if party { PARTY_BODY } else { TEXT },
                if party { PARTY_NAME } else { ACCENT },
                Some(emphasis),
                lobby_links
                    .into_iter()
                    .map(|link| body_start + link.range.start..body_start + link.range.end)
                    .collect(),
            )
        }
        TranscriptLine::Error { text, .. } => {
            linked_content(format!("!  {text}"), 0x00ff_6b63, Some(0..1), Vec::new())
        }
    }
}

fn linked_content(
    text: String,
    color: u32,
    emphasis: Option<Range<usize>>,
    accents: Vec<Range<usize>>,
) -> ContentStyle {
    emphasized_content(text, color, ACCENT, emphasis, accents)
}

fn emphasized_content(
    text: String,
    color: u32,
    emphasis_color: u32,
    emphasis: Option<Range<usize>>,
    mut accents: Vec<Range<usize>>,
) -> ContentStyle {
    let links = recognized_urls(&text);
    accents.extend(links.iter().map(|(range, _)| range.clone()));
    ContentStyle {
        text,
        color,
        emphasis_color,
        emphasis,
        accents,
        links,
    }
}

fn recognized_urls(text: &str) -> Vec<(Range<usize>, String)> {
    const PREFIXES: [&str; 3] = ["https://", "http://", "www."];
    let lowercase = text.to_ascii_lowercase();
    let mut links = Vec::new();
    let mut cursor = 0;

    while cursor < text.len() {
        let Some((offset, prefix)) = PREFIXES
            .iter()
            .filter_map(|prefix| {
                lowercase[cursor..]
                    .find(prefix)
                    .map(|offset| (offset, *prefix))
            })
            .min_by_key(|(offset, _)| *offset)
        else {
            break;
        };
        let start = cursor + offset;
        if text[..start]
            .chars()
            .next_back()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            cursor = start + prefix.len();
            continue;
        }

        let raw_end = text[start..]
            .char_indices()
            .skip(1)
            .find_map(|(offset, character)| {
                (character.is_whitespace() || matches!(character, '<' | '>' | '"' | '\''))
                    .then_some(start + offset)
            })
            .unwrap_or(text.len());
        let end = trim_url_end(text, start, raw_end);
        let displayed = &text[start..end];
        let destination = if prefix == "www." {
            format!("https://{displayed}")
        } else {
            displayed.to_owned()
        };
        if Url::parse(&destination)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
        {
            links.push((start..end, destination));
        }
        cursor = raw_end.max(start + prefix.len());
    }
    links
}

fn trim_url_end(text: &str, start: usize, mut end: usize) -> usize {
    loop {
        let Some(character) = text[start..end].chars().next_back() else {
            return end;
        };
        let trim = matches!(character, '.' | ',' | '!' | '?' | ';' | ':')
            || matches!(character, ')' | ']' | '}')
                && unmatched_closer(&text[start..end], character);
        if !trim {
            return end;
        }
        end -= character.len_utf8();
    }
}

fn unmatched_closer(value: &str, closer: char) -> bool {
    let opener = match closer {
        ')' => '(',
        ']' => '[',
        '}' => '{',
        _ => return false,
    };
    value
        .chars()
        .filter(|character| *character == closer)
        .count()
        > value
            .chars()
            .filter(|character| *character == opener)
            .count()
}

/// a lobby somebody linked into the channel: where its name landed in the
/// rendered text, and the advert id the link carried.
///
/// the id is a third of an `AdvertHandle` — SC2 drops the server label and
/// epoch when it writes the link — so it is only a lookup key once presence has
/// supplied a whole handle to borrow the rest from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LobbyLink {
    pub range: Range<usize>,
    pub advert_id: u32,
}

#[must_use]
pub fn display_message_body(body: &str) -> (String, Vec<LobbyLink>) {
    const PREFIX: &str = "lobbyLink(";
    let mut output = String::new();
    let mut links: Vec<LobbyLink> = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = body[cursor..].find(PREFIX) {
        let at = cursor + offset;
        output.push_str(&body[cursor..at]);
        let value = &body[at + PREFIX.len()..];
        let Some((id, rest)) = value.split_once(',') else {
            output.push_str(&body[at..]);
            return (output, links);
        };
        let Some(close) = rest.find(')') else {
            output.push_str(&body[at..]);
            return (output, links);
        };
        let name = &rest[..close];
        let Some(advert_id) = id.parse::<u32>().ok().filter(|_| !name.is_empty()) else {
            output.push_str(&body[at..at + PREFIX.len()]);
            cursor = at + PREFIX.len();
            continue;
        };
        let start = output.len();
        output.push_str(name);
        links.push(LobbyLink {
            range: start..output.len(),
            advert_id,
        });
        cursor = at + PREFIX.len() + id.len() + 1 + close + 1;
        if body[cursor..].starts_with(';') {
            cursor += 1;
        }
    }
    output.push_str(&body[cursor..]);
    (output, links)
}

fn styled_content(content: &ContentStyle, selection: Option<&Range<usize>>) -> StyledText {
    let mut boundaries = vec![0, content.text.len()];
    for range in [content.emphasis.as_ref(), selection].into_iter().flatten() {
        boundaries.extend([range.start, range.end]);
    }
    for range in &content.accents {
        boundaries.extend([range.start, range.end]);
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut highlights = Vec::new();
    for points in boundaries.windows(2) {
        let range = points[0]..points[1];
        if range.is_empty() {
            continue;
        }
        let mut style = HighlightStyle::default();
        if content
            .emphasis
            .as_ref()
            .is_some_and(|emphasis| emphasis.contains(&range.start))
        {
            style.color = Some(rgb(content.emphasis_color).into());
            style.font_weight = Some(gpui::FontWeight::BOLD);
        } else if content
            .accents
            .iter()
            .any(|accent| accent.contains(&range.start))
        {
            style.color = Some(rgb(ONLINE).into());
        }
        if selection.is_some_and(|selection| selection.contains(&range.start)) {
            style.background_color = Some(rgba(SELECTION_BACKGROUND).into());
        }
        if style != HighlightStyle::default() {
            highlights.push((range, style));
        }
    }

    let mut text = StyledText::new(content.text.clone()).with_highlights(highlights);
    if let Some(emphasis) = content.emphasis.clone() {
        text = text.with_font_family_overrides([(emphasis, FONT_INTERFACE.into())]);
    }
    text
}

#[must_use]
pub fn transcript_text(line: &TranscriptLine) -> String {
    content_style(line).text
}

/// channel events are chrome, not conversation: they sit on their own row
/// treatments below rather than going through the selectable text path.
/// events sit in the same column as messages and read at the same size — a
/// transcript with five text sizes in four lines reads as an accident.
const EVENT_TEXT_SIZE: f32 = 13.0;
/// the transcript's own timestamp gutter. events use it verbatim rather than
/// inventing their own, which is what keeps every line — and everything
/// hanging off one — on the same column.
const TIME_GUTTER_WIDTH: f32 = transcript::TIME_GUTTER_WIDTH;

/// where content hangs: past the gutter, or the left edge when the reader has
/// timestamps turned off.
fn content_offset(show_timestamps: bool) -> f32 {
    if show_timestamps {
        TIME_GUTTER_WIDTH
    } else {
        0.0
    }
}
const CHIP_AVATAR: f32 = 18.0;
/// the speaker's portrait on a message line. sized to the line box so a run of
/// messages keeps its baseline rhythm rather than opening up.
const SPEAKER_GAP: f32 = transcript::SPEAKER_GAP;
/// the portrait is square and the text sits on a 19px line, so it needs a
/// nudge to centre against the first line rather than the whole paragraph.
const CHIP_HEIGHT: f32 = 24.0;
const RULE_COLOR: u32 = 0x33a8_f080;
const RULE_FADE: u32 = 0x33a8_f000;
const CHIP_FILL: u32 = 0x1231_5e80;
const CHIP_BORDER: u32 = 0x33a8_f059;
const CHIP_BORDER_STACK: u32 = 0x33a8_f066;
const CHIP_FILL_HOVER: u32 = 0x1231_5ebf;
const CLAN_CHIP_FILL: u32 = 0x3b24_108c;
const CLAN_CHIP_BORDER: u32 = 0xd68b_4380;
const CLAN_CHIP_ACCENT: u32 = 0x00d6_8b43;
const CLAN_NAME: u32 = 0x00f0_aa64;
const DIGEST_OPACITY: f32 = 0.75;
/// the expanded digest is the digest line grown in place — no box, no header
/// band, no column rule. its counts row *is* the header, and both columns hang
/// off the same timestamp gutter as every other line.
const PANEL_ROW_HEIGHT: f32 = 22.0;
const PANEL_AVATAR: f32 = 15.0;
const PANEL_COLUMN_GAP: f32 = 24.0;
const MORE_ROW_HEIGHT: f32 = 22.0;
const PANEL_ROW_HOVER: u32 = 0x1231_5e59;

/// a column is exactly as tall as the rows it promises to show, so the number
/// behind "+ N more" is always the number actually hidden.
fn column_height() -> gpui::Pixels {
    px(PANEL_ROW_HEIGHT * f32::from(DigestEvent::VISIBLE_ROWS))
}
const LEFT_COLUMN_OPACITY: f32 = 0.7;

/// clanmates carry their own tone through the chip so the joins you actually
/// care about stay legible in a wall of arrivals.
struct ChipTone {
    fill: u32,
    border: u32,
    hover_fill: u32,
    hover_border: u32,
    name: u32,
}

fn chip_tone(tone: RosterUserTone) -> ChipTone {
    match tone {
        RosterUserTone::Clan => ChipTone {
            fill: CLAN_CHIP_FILL,
            border: CLAN_CHIP_BORDER,
            hover_fill: CLAN_CHIP_FILL,
            hover_border: CLAN_CHIP_ACCENT,
            name: CLAN_NAME,
        },
        RosterUserTone::Party | RosterUserTone::Normal => ChipTone {
            fill: CHIP_FILL,
            border: CHIP_BORDER,
            hover_fill: CHIP_FILL_HOVER,
            hover_border: ACCENT,
            name: TEXT,
        },
    }
}

#[must_use]
fn avatar_stack<'a>(members: impl IntoIterator<Item = &'a RosterUser>, assets: &Sc2Assets) -> Div {
    div()
        .flex()
        .flex_shrink_0()
        .mr(px(2.0))
        .children(members.into_iter().enumerate().map(|(index, user)| {
            div()
                .border_1()
                .border_color(rgba(CHIP_BORDER_STACK))
                .when(index > 0, |avatar| avatar.ml(px(-5.0)))
                .child(crate::products::sc2::components::roster::inline_portrait(
                    user,
                    assets,
                    CHIP_AVATAR,
                ))
        }))
}

#[must_use]
fn toggle_control(
    id: impl Into<ElementId>,
    label: &'static str,
    handler: ToggleHandler,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_shrink_0()
        .cursor_pointer()
        .font_family(FONT_INTERFACE)
        .text_size(px(EVENT_TEXT_SIZE))
        .text_color(rgb(NOTICE))
        .hover(|style| style.text_color(rgb(EVENT_TITLE)))
        .child(label)
        .on_click(move |_, window, cx| handler(window, cx))
}
const EVENT_TITLE: u32 = 0x00e6_f9ff;
const LEAVE_NAME: u32 = 0x00a9_b8cc;
/// leaves are quieter than joins — someone arriving is worth more of your eye
/// than someone going.
const LEAVE_OPACITY: f32 = 0.65;

/// the transcript's one timestamp treatment. events use the same fixed gutter
/// as messages so every row starts its content on the same column — an event
/// with its own inline clock reads as ragged next to the messages around it.
#[must_use]
fn time_gutter(time: &str) -> Div {
    transcript::time_gutter(format!("[{time}]"), FONT_INTERFACE, MUTED)
}

/// an event row: the timestamp gutter, then a gapped body. keeping the gap
/// inside the body is what puts event content on exactly the same column as
/// message content rather than one gap further right.
#[must_use]
fn event_shell(time: &str, show_timestamps: bool, body: Div) -> Div {
    div()
        .flex()
        .items_start()
        .w_full()
        .children(show_timestamps.then(|| time_gutter(time)))
        // bounded and shrinkable, so a row of chips wraps down the column
        // instead of running off the edge of the transcript.
        .child(body.flex_1().min_w_0())
}

/// the session-start rule centres its own clock instead, so it takes the size
/// without the gutter or the brackets.
#[must_use]
fn inline_time(time: &str) -> Div {
    transcript::inline_time(time.to_owned(), FONT_INTERFACE, MUTED)
}

#[must_use]
fn event_rule(fading_in: bool) -> Div {
    let (from, to) = if fading_in {
        (RULE_FADE, RULE_COLOR)
    } else {
        (RULE_COLOR, RULE_FADE)
    };
    div().flex_1().h(px(1.0)).bg(gpui::linear_gradient(
        90.0,
        gpui::linear_color_stop(rgba(from), 0.0),
        gpui::linear_color_stop(rgba(to), 1.0),
    ))
}

/// your own arrival: a full-width rule that marks where this session starts,
/// with the channel and the head count sitting in the break.
#[must_use]
pub fn session_start_row(time: &str, channel: &str, online: usize, show_timestamps: bool) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(event_rule(true))
        .child(
            div()
                .flex()
                .flex_shrink_0()
                .items_center()
                .gap(px(7.0))
                .whitespace_nowrap()
                .children(show_timestamps.then(|| inline_time(time)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.0))
                        .font_family(FONT_INTERFACE)
                        .text_size(px(EVENT_TEXT_SIZE))
                        .text_color(rgb(NOTICE))
                        .child("You joined")
                        .child(
                            div()
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(rgb(EVENT_TITLE))
                                .child(channel.to_owned()),
                        ),
                )
                .child(
                    div()
                        .font_family(FONT_INTERFACE)
                        .text_size(px(EVENT_TEXT_SIZE))
                        .text_color(rgb(MUTED))
                        .child(format!("· {online} online")),
                ),
        )
        .child(event_rule(false))
}

type MemberHandler = Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>;
type ToggleHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// a join or leave. joins carry a name chip you can click through to the
/// person; leaves stay text-only and dimmed.
#[derive(IntoElement)]
pub struct MembershipRow {
    id: ElementId,
    event: MembershipEvent,
    channel: String,
    assets: Sc2Assets,
    show_timestamps: bool,
    on_member: Option<MemberHandler>,
    on_toggle: Option<ToggleHandler>,
}

impl MembershipRow {
    #[must_use]
    pub fn new(
        id: impl Into<ElementId>,
        event: MembershipEvent,
        channel: impl Into<String>,
        assets: Sc2Assets,
        show_timestamps: bool,
    ) -> Self {
        Self {
            id: id.into(),
            event,
            channel: channel.into(),
            assets,
            show_timestamps,
            on_member: None,
            on_toggle: None,
        }
    }

    /// called with the index of the member whose chip was clicked.
    #[must_use]
    pub fn on_member(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_member = Some(Rc::new(handler));
        self
    }

    #[must_use]
    pub fn on_toggle(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    fn chip(&self, index: usize, user: &RosterUser) -> Stateful<Div> {
        let tone = chip_tone(user.tone);
        let mut chip = div()
            .id((self.id.clone(), SharedString::from(format!("chip-{index}"))))
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(6.0))
            .h(px(CHIP_HEIGHT))
            .pl(px(4.0))
            .pr(px(9.0))
            .bg(rgba(tone.fill))
            .border_1()
            .border_color(rgba(tone.border))
            .child(crate::products::sc2::components::roster::inline_portrait(
                user,
                &self.assets,
                CHIP_AVATAR,
            ))
            .child(
                div()
                    .font_family(FONT_INTERNATIONAL)
                    .text_size(px(EVENT_TEXT_SIZE))
                    .text_color(rgb(tone.name))
                    .whitespace_nowrap()
                    .child(user.name.clone()),
            );
        let tooltip_user = user.clone();
        let tooltip_channel = self.channel.clone();
        let tooltip_assets = self.assets.clone();
        chip = chip.tooltip(move |_, cx| {
            cx.new(|_| {
                crate::products::sc2::components::roster::RosterTooltip::new(
                    tooltip_user.clone(),
                    tooltip_channel.clone(),
                    tooltip_assets.clone(),
                )
            })
            .into()
        });
        if let Some(handler) = self.on_member.clone() {
            let (hover_fill, hover_border) = (tone.hover_fill, tone.hover_border);
            chip = chip
                .cursor_pointer()
                .hover(move |style| style.bg(rgba(hover_fill)).border_color(rgb(hover_border)))
                .on_click(move |_, window, cx| handler(index, window, cx));
        }
        chip
    }

    fn stack(&self) -> Div {
        avatar_stack(&self.event.members, &self.assets)
    }

    fn toggle(&self, label: &'static str) -> Option<Stateful<Div>> {
        let handler = self.on_toggle.clone()?;
        Some(toggle_control((self.id.clone(), "toggle"), label, handler))
    }
}

impl RenderOnce for MembershipRow {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let time = self.event.time.clone();
        let show_timestamps = self.show_timestamps;
        let row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .line_height(px(20.0));
        let body = match self.event.kind {
            MembershipKind::Left => row
                .opacity(LEAVE_OPACITY)
                .child(
                    div()
                        .font_family(FONT_INTERNATIONAL)
                        .text_size(px(EVENT_TEXT_SIZE))
                        .text_color(rgb(LEAVE_NAME))
                        .child(leading_name(&self.event)),
                )
                .child(
                    div()
                        .font_family(FONT_INTERFACE)
                        .text_size(px(EVENT_TEXT_SIZE))
                        .text_color(rgb(MUTED))
                        .child(trailing_verb(&self.event)),
                ),
            MembershipKind::Joined if self.event.collapsed() => row
                .child(self.stack())
                .child(
                    div()
                        .font_family(FONT_INTERNATIONAL)
                        .text_size(px(EVENT_TEXT_SIZE))
                        .text_color(rgb(TEXT))
                        .child(leading_name(&self.event)),
                )
                .child(
                    div()
                        .font_family(FONT_INTERFACE)
                        .text_size(px(EVENT_TEXT_SIZE))
                        .text_color(rgb(ONLINE))
                        .child(trailing_verb(&self.event)),
                )
                .children(self.toggle("SHOW")),
            MembershipKind::Joined => {
                let chips = self
                    .event
                    .members
                    .iter()
                    .enumerate()
                    .map(|(index, user)| self.chip(index, user))
                    .collect::<Vec<_>>();
                let expandable = self.event.members.len() > 1;
                row.flex_wrap()
                    .children(chips)
                    .child(
                        div()
                            .font_family(FONT_INTERFACE)
                            .text_size(px(EVENT_TEXT_SIZE))
                            .text_color(rgb(ONLINE))
                            .child(self.event.kind.verb()),
                    )
                    .children(expandable.then(|| self.toggle("HIDE")).flatten())
            }
        };
        event_shell(&time, show_timestamps, body)
    }
}

/// a whole minute of a busy channel on one line. quieter than a single arrival,
/// because in here no individual coming or going is the point.
#[derive(IntoElement)]
pub struct DigestRow {
    id: ElementId,
    event: DigestEvent,
    channel: String,
    assets: Sc2Assets,
    show_timestamps: bool,
    /// how far the columns are open, 0 through 1. only they animate — the
    /// counts row is on screen either way and must not flash.
    expansion: f32,
    columns: Option<(ScrollHandle, ScrollHandle)>,
    on_member: Option<MemberHandler>,
    on_toggle: Option<ToggleHandler>,
}

impl DigestRow {
    #[must_use]
    pub fn new(
        id: impl Into<ElementId>,
        event: DigestEvent,
        channel: impl Into<String>,
        assets: Sc2Assets,
        show_timestamps: bool,
    ) -> Self {
        Self {
            id: id.into(),
            event,
            channel: channel.into(),
            assets,
            show_timestamps,
            expansion: 0.0,
            columns: None,
            on_member: None,
            on_toggle: None,
        }
    }

    #[must_use]
    pub fn expansion(mut self, progress: f32) -> Self {
        self.expansion = progress.clamp(0.0, 1.0);
        self
    }

    /// the scroll handles for the expanded panel's two columns. only one digest
    /// is ever expanded, so one pair is shared across the whole transcript.
    #[must_use]
    pub fn columns(mut self, joined: &ScrollHandle, left: &ScrollHandle) -> Self {
        self.columns = Some((joined.clone(), left.clone()));
        self
    }

    /// called with the index of the joiner whose chip was clicked.
    #[must_use]
    pub fn on_member(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_member = Some(Rc::new(handler));
        self
    }

    #[must_use]
    pub fn on_toggle(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_toggle = Some(Rc::new(handler));
        self
    }

    fn counts(&self) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(4.0))
            .whitespace_nowrap()
            .font_family(FONT_INTERFACE)
            .text_size(px(EVENT_TEXT_SIZE))
            .child(
                div()
                    .text_color(rgb(ONLINE))
                    .child(format!("{} joined", self.event.joined.len())),
            )
            .child(div().text_color(rgb(LEAVE_NAME)).child("·"))
            .child(
                div()
                    .text_color(rgb(MUTED))
                    .child(format!("{} left", self.event.left.len())),
            )
    }

    fn name(text: &str, color: u32) -> Div {
        div()
            .h(px(PANEL_ROW_HEIGHT))
            .flex()
            .flex_shrink_0()
            .items_center()
            .overflow_hidden()
            .whitespace_nowrap()
            .font_family(FONT_INTERNATIONAL)
            .text_size(px(EVENT_TEXT_SIZE))
            .text_color(rgb(color))
            .child(text.to_owned())
    }

    fn joined_row(&self, index: usize, user: &RosterUser) -> Stateful<Div> {
        let mut row = div()
            .id((
                self.id.clone(),
                SharedString::from(format!("joined-{index}")),
            ))
            .h(px(PANEL_ROW_HEIGHT))
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap(px(6.0))
            .child(crate::products::sc2::components::roster::inline_portrait(
                user,
                &self.assets,
                PANEL_AVATAR,
            ))
            .child(Self::name(&user.name, TEXT).flex_1().min_w_0());
        let tooltip_user = user.clone();
        let tooltip_channel = self.channel.clone();
        let tooltip_assets = self.assets.clone();
        row = row.tooltip(move |_, cx| {
            cx.new(|_| {
                crate::products::sc2::components::roster::RosterTooltip::new(
                    tooltip_user.clone(),
                    tooltip_channel.clone(),
                    tooltip_assets.clone(),
                )
            })
            .into()
        });
        if let Some(handler) = self.on_member.clone() {
            row = row
                .cursor_pointer()
                .hover(|style| style.bg(rgba(PANEL_ROW_HOVER)))
                .on_click(move |_, window, cx| handler(index, window, cx));
        }
        row
    }

    /// "+ N more" scrolls its own column rather than growing the expansion.
    fn more_control(
        &self,
        side: &'static str,
        hidden: usize,
        scroll: &ScrollHandle,
    ) -> Option<Stateful<Div>> {
        if hidden == 0 {
            return None;
        }
        let scroll = scroll.clone();
        Some(
            div()
                .id((self.id.clone(), side))
                .flex_shrink_0()
                .pt(px(2.0))
                .cursor_pointer()
                .font_family(FONT_INTERFACE)
                .text_size(px(EVENT_TEXT_SIZE))
                .text_color(rgb(NOTICE))
                .hover(|style| style.text_color(rgb(EVENT_TITLE)))
                .child(format!("+ {hidden} more"))
                .on_click(move |_, _, _| {
                    // page by exactly one column, and wrap to the top once the
                    // end is reached so the control is never dead.
                    let offset = scroll.offset();
                    let furthest = -scroll.max_offset().y;
                    let next = if offset.y <= furthest {
                        px(0.0)
                    } else {
                        (offset.y - column_height()).max(furthest)
                    };
                    scroll.set_offset(gpui::point(offset.x, next));
                }),
        )
    }

    fn column(
        &self,
        side: &'static str,
        rows: Vec<AnyElement>,
        total: usize,
        scroll: &ScrollHandle,
    ) -> Div {
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .id((self.id.clone(), SharedString::from(format!("{side}-list"))))
                    .h(column_height())
                    .flex()
                    .flex_col()
                    .overflow_y_scroll()
                    .track_scroll(scroll)
                    .children(rows),
            )
            .children(self.more_control(side, DigestEvent::overflow(total), scroll))
    }

    /// the height the columns occupy once fully open — what the grow animates
    /// toward, and what keeps the transcript from jumping at the end of it.
    fn columns_height(&self) -> f32 {
        let overflows = DigestEvent::overflow(self.event.joined.len()) > 0
            || DigestEvent::overflow(self.event.left.len()) > 0;
        f32::from(column_height()) + if overflows { MORE_ROW_HEIGHT } else { 0.0 }
    }

    fn columns_row(&self, joined_scroll: &ScrollHandle, left_scroll: &ScrollHandle) -> Div {
        let joined = self
            .event
            .joined
            .iter()
            .enumerate()
            .map(|(index, user)| self.joined_row(index, user).into_any_element())
            .collect::<Vec<_>>();
        // leaves are name-only and dimmer, with no label: the dimming and the
        // counts above already say which column is which.
        let left = self
            .event
            .left
            .iter()
            .map(|user| Self::name(&user.name, LEAVE_NAME).into_any_element())
            .collect::<Vec<_>>();
        let left_total = self.event.left.len();
        div()
            .flex()
            .items_start()
            .gap(px(PANEL_COLUMN_GAP))
            .child(self.column("joined", joined, self.event.joined.len(), joined_scroll))
            .child(
                self.column("left", left, left_total, left_scroll)
                    .opacity(LEFT_COLUMN_OPACITY),
            )
    }
}

impl RenderOnce for DigestRow {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let time = self.event.time.clone();
        let show_timestamps = self.show_timestamps;
        let row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap(px(8.0))
            .line_height(px(20.0))
            .opacity(DIGEST_OPACITY);
        if self.expansion <= 0.0 {
            let faces = self.event.faces();
            let body =
                row.children((!faces.is_empty()).then(|| avatar_stack(faces, &self.assets)))
                    .child(self.counts())
                    .children(self.on_toggle.clone().map(|handler| {
                        toggle_control((self.id.clone(), "toggle"), "SHOW", handler)
                    }));
            return event_shell(&time, show_timestamps, body);
        }
        let Some((joined_scroll, left_scroll)) = self.columns.clone() else {
            return div();
        };
        // the expansion is the same line, grown: its counts row keeps the
        // gutter, and the columns hang off exactly where that row starts.
        let progress = self.expansion;
        let opened = self.columns_height() * progress;
        let header = row.child(self.counts()).children(
            self.on_toggle
                .clone()
                .map(|handler| toggle_control((self.id.clone(), "toggle"), "HIDE", handler)),
        );
        div()
            .flex()
            .flex_col()
            .gap(px(4.0 * progress))
            .child(event_shell(&time, show_timestamps, header))
            .child(
                div()
                    .h(px(opened))
                    .overflow_hidden()
                    .opacity(progress)
                    .pl(px(content_offset(show_timestamps)))
                    .child(self.columns_row(&joined_scroll, &left_scroll)),
            )
    }
}

/// the part of the summary that is a person's name, kept in the international
/// face so it matches how the same name renders everywhere else.
#[must_use]
fn leading_name(event: &MembershipEvent) -> String {
    event
        .members
        .first()
        .map_or_else(|| "Someone".to_owned(), |user| user.name.clone())
}

#[must_use]
fn trailing_verb(event: &MembershipEvent) -> String {
    let verb = event.kind.verb();
    match event.members.len() {
        0 | 1 => verb.to_owned(),
        count => format!("and {} others {verb}", count - 1),
    }
}

#[must_use]
fn panel() -> Div {
    div()
        .relative()
        .flex_1()
        .h_full()
        .min_w_0()
        .overflow_hidden()
}

#[derive(IntoElement)]
pub(crate) struct ChatPanel {
    content: Vec<AnyElement>,
}

impl ChatPanel {
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            content: Vec::new(),
        }
    }

    #[must_use]
    pub(crate) fn child(mut self, child: impl IntoElement) -> Self {
        self.content.push(child.into_any_element());
        self
    }
}

impl RenderOnce for ChatPanel {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        panel().children(self.content)
    }
}

#[derive(IntoElement)]
pub struct TranscriptViewport {
    inner: transcript::TranscriptViewport,
}

impl TranscriptViewport {
    #[must_use]
    pub fn new(id: impl Into<ElementId>, selection: TranscriptSelection) -> Self {
        Self {
            inner: transcript::TranscriptViewport::new(id, selection, FONT_INTERNATIONAL)
                .scrollbar_colors(crate::products::modal::ModalVariant::Sc2.scrollbar()),
        }
    }

    #[must_use]
    pub fn scroll(mut self, scroll: &ScrollHandle) -> Self {
        self.inner = self.inner.scroll(scroll);
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.inner = self.inner.child(child);
        self
    }

    #[must_use]
    pub fn children(mut self, children: impl IntoIterator<Item = impl IntoElement>) -> Self {
        self.inner = self.inner.children(children);
        self
    }

    #[must_use]
    pub fn empty(self, message: impl Into<gpui::SharedString>) -> Self {
        self.child(empty_transcript(message))
    }
}

impl RenderOnce for TranscriptViewport {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.inner
    }
}

#[must_use]
fn empty_transcript(message: impl Into<gpui::SharedString>) -> Div {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .font_family(FONT_INTERFACE)
        .text_size(px(11.0))
        .text_color(rgb(MUTED))
        .child(message.into())
}

#[must_use]
pub fn selectable_transcript_row(
    line: &TranscriptLine,
    show_timestamps: bool,
    scope: u64,
    row: usize,
    selection: &TranscriptSelection,
    assets: &Sc2Assets,
    party: PartyMark,
) -> gpui::Stateful<Div> {
    let content = content_style(line);
    let selected_range = selection.selection_for_row(row, content.text.len());
    let time = match line {
        TranscriptLine::Notice { time, .. }
        | TranscriptLine::SessionStart { time, .. }
        | TranscriptLine::Message { time, .. }
        | TranscriptLine::Error { time, .. } => time,
        TranscriptLine::Membership(event) => &event.time,
        TranscriptLine::Digest(event) => &event.time,
    };
    let text = SelectableText::new(
        format!("transcript-text-{scope}-{row}"),
        row,
        styled_content(&content, selected_range.as_ref()),
        content.links,
        selection.clone(),
    );

    transcript_row_shell(
        time,
        show_timestamps,
        content.color,
        party,
        speaker(line),
        assets,
        text,
    )
    .id(format!("transcript-row-{scope}-{row}"))
    .cursor(gpui::CursorStyle::IBeam)
}

/// what a line wears to say it was said to the party. the rail runs down every
/// one of them; the chip is spent once per run of lines from the same speaker,
/// because repeating it down a back-and-forth would say nothing new.
#[derive(Clone, Copy, Default)]
pub struct PartyMark {
    pub rail: bool,
    pub chip: bool,
}

/// the same chip the field wears while it is addressed to the party.
fn party_chip() -> Div {
    div()
        .flex_shrink_0()
        .mr(px(SPEAKER_GAP))
        .mt(px(2.0))
        .px(px(5.0))
        .bg(rgb(PARTY_NAME))
        .rounded(px(2.0))
        .font_family(FONT_NAVIGATION)
        .font_weight(gpui::FontWeight::BOLD)
        .text_size(px(9.0))
        .text_color(rgb(PARTY_CHIP_TEXT))
        .child("PARTY")
}

/// whether this line was said to the party rather than to the channel. party
/// talk has no tab of its own, so the line has to carry the difference.
#[must_use]
pub fn party_line(line: &TranscriptLine) -> bool {
    matches!(
        line,
        TranscriptLine::Message { sender, .. } if sender.tone == RosterUserTone::Party
    )
}

/// the face to put on a line. only a message has one — an event line already
/// carries portraits inside its own chips and rows.
fn speaker(line: &TranscriptLine) -> Option<&RosterUser> {
    match line {
        TranscriptLine::Message { sender, .. } => Some(sender),
        _ => None,
    }
}

fn transcript_row_shell(
    time: &str,
    show_timestamps: bool,
    color: u32,
    party: PartyMark,
    speaker: Option<&RosterUser>,
    assets: &Sc2Assets,
    content: impl IntoElement,
) -> Div {
    let leading = party
        .chip
        .then(|| party_chip().into_any_element())
        .into_iter()
        .collect();
    let portrait = speaker.map(|speaker| {
        crate::products::sc2::components::roster::inline_portrait(
            speaker,
            assets,
            transcript::SPEAKER_AVATAR_SIZE,
        )
    });
    let mut row = transcript::row_shell(
        show_timestamps,
        time_gutter(time),
        leading,
        portrait,
        FONT_INTERNATIONAL,
        color,
        content,
    );
    // the rail is pulled back out of the row's own indent, so the words still
    // line up with every other line and only the mark sits in the gutter
    if party.rail {
        row = row
            .border_l(px(PARTY_RAIL_WIDTH))
            .border_color(rgb(PARTY_NAME))
            .pl(px(PARTY_RAIL_GAP))
            .ml(px(-(PARTY_RAIL_GAP + PARTY_RAIL_WIDTH)))
            // the wash says "party" for the length of the name and then gets
            // out of the way of the sentence
            .bg(gpui::linear_gradient(
                90.0,
                gpui::linear_color_stop(rgba(PARTY_WASH), 0.0),
                gpui::linear_color_stop(rgba(PARTY_WASH_END), 1.0),
            ));
    }
    row
}

#[must_use]
pub fn split_media(body: &str) -> (String, Vec<String>) {
    let mut links = Vec::new();
    let mut text = String::with_capacity(body.len());
    for token in body.split_whitespace() {
        if is_media_link(token) {
            links.push(token.to_owned());
        } else {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(token);
        }
    }
    (text, links)
}

fn is_media_link(token: &str) -> bool {
    let Some(rest) = token
        .strip_prefix("https://")
        .or_else(|| token.strip_prefix("http://"))
    else {
        return false;
    };
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let host = host.to_ascii_lowercase();
    let path = path.to_ascii_lowercase();
    [".gif", ".png", ".jpg", ".jpeg", ".webp"]
        .iter()
        .any(|suffix| path.ends_with(suffix))
        || host.ends_with("giphy.com")
        || host.ends_with("tenor.com")
}

#[cfg(test)]
mod tests {

    #[test]
    fn party_talk_is_told_apart_by_colour_rather_than_by_a_tab_of_its_own() {
        let mut sender = crate::products::sc2::RosterUser {
            handle: 1,
            name: "NelsonTest91".to_owned(),
            presence_id: None,
            presence: crate::products::sc2::PresenceKind::Available,
            presence_label: "Available".to_owned(),
            presence_icon: crate::products::sc2::Sc2Assets::load(
                &crate::foundation::assets::NativeAssetResolver,
            )
            .status_available,
            portrait: None,
            clan_tag: None,
            clan_is_local: false,
            tone: crate::products::sc2::RosterUserTone::Normal,
            dimmed: false,
        };
        let channel = super::TranscriptLine::Message {
            time: "7:36 PM".to_owned(),
            sender: sender.clone(),
            text: "rushing 12 pool".to_owned(),
        };
        assert_eq!(super::content_style(&channel).emphasis_color, super::ACCENT);
        assert!(!super::party_line(&channel));

        sender.tone = crate::products::sc2::RosterUserTone::Party;
        let party = super::TranscriptLine::Message {
            time: "7:36 PM".to_owned(),
            sender,
            text: "rushing 12 pool".to_owned(),
        };
        // the line stays in the room you are reading, so the green name, the
        // green-tinted body, and the rail are the whole difference
        assert_eq!(
            super::content_style(&party).emphasis_color,
            super::PARTY_NAME
        );
        assert_eq!(super::content_style(&party).color, super::PARTY_BODY);
        assert!(super::party_line(&party));
    }
    use super::*;
    use crate::products::sc2::{Portrait, PresenceKind, RosterUserTone};

    fn member(name: &str) -> RosterUser {
        RosterUser {
            handle: 1,
            name: name.to_owned(),
            presence_id: None,
            presence: PresenceKind::Available,
            presence_label: "Available".to_owned(),
            presence_icon: "images/icons/status-available.png".into(),
            portrait: None::<Portrait>,
            clan_tag: None,
            clan_is_local: false,
            tone: RosterUserTone::Normal,
            dimmed: false,
        }
    }

    fn event(kind: MembershipKind, names: &[&str]) -> MembershipEvent {
        MembershipEvent {
            time: "8:02 PM".to_owned(),
            kind,
            members: names.iter().copied().map(member).collect(),
            expanded: false,
        }
    }

    #[test]
    fn a_lone_arrival_reads_as_one_name() {
        let event = event(MembershipKind::Joined, &["NelsonTest91"]);
        assert_eq!(event.summary(), "NelsonTest91 joined");
        assert!(!event.collapsed());
    }

    #[test]
    fn several_arrivals_collapse_behind_the_first_name() {
        let event = event(
            MembershipKind::Joined,
            &["QueenOfBlades", "Hierarch", "Nova"],
        );
        assert_eq!(event.summary(), "QueenOfBlades and 2 others joined");
        assert!(event.collapsed());
    }

    #[test]
    fn an_expanded_event_is_no_longer_collapsed() {
        let mut event = event(MembershipKind::Joined, &["QueenOfBlades", "Hierarch"]);
        event.expanded = true;
        assert!(!event.collapsed());
    }

    #[test]
    fn departures_name_the_channel_they_left() {
        let event = event(MembershipKind::Left, &["Hierarch"]);
        assert_eq!(event.summary(), "Hierarch left the channel");
    }

    fn digest(joined: usize, left: usize) -> DigestEvent {
        DigestEvent {
            time: "8:27 PM".to_owned(),
            joined: (0..joined)
                .map(|index| member(&format!("j{index}")))
                .collect(),
            left: (0..left)
                .map(|index| member(&format!("l{index}")))
                .collect(),
        }
    }

    #[test]
    fn a_digest_counts_both_directions() {
        assert_eq!(digest(14, 11).summary(), "14 joined · 11 left");
    }

    #[test]
    fn the_avatar_stack_caps_at_three_joiners() {
        assert_eq!(digest(14, 5).faces().len(), 3);
        assert_eq!(digest(2, 1).faces().len(), 2);
    }

    #[test]
    fn a_minute_too_busy_to_summarize_drops_the_stack() {
        assert_eq!(digest(14, 11).events(), 25);
        assert!(digest(14, 11).faces().is_empty());
        assert!(!digest(10, 5).faces().is_empty());
    }

    #[test]
    fn a_column_shows_five_names_and_hides_the_rest() {
        assert_eq!(DigestEvent::overflow(43), 38);
        assert_eq!(DigestEvent::overflow(37), 32);
    }

    #[test]
    fn a_column_that_fits_hides_nothing() {
        assert_eq!(DigestEvent::overflow(5), 0);
        assert_eq!(DigestEvent::overflow(1), 0);
        assert_eq!(DigestEvent::overflow(0), 0);
    }

    #[test]
    fn a_digest_copies_as_the_line_it_renders() {
        let line = TranscriptLine::Digest(digest(9, 12));
        assert_eq!(transcript_text(&line), "9 joined · 12 left");
    }

    #[test]
    fn a_session_start_copies_as_the_line_it_renders() {
        let line = TranscriptLine::SessionStart {
            time: "7:59 PM".to_owned(),
            channel: "Midigation".to_owned(),
            online: 1,
        };
        assert_eq!(transcript_text(&line), "You joined Midigation · 1 online");
    }

    #[test]
    fn message_text_is_one_contiguous_string() {
        let line = TranscriptLine::Message {
            time: "7:31 PM".into(),
            sender: member("Nova"),
            text: "This message wraps normally.".into(),
        };
        assert_eq!(transcript_text(&line), "Nova: This message wraps normally.");
    }

    #[test]
    fn lobby_links_show_only_their_names() {
        let (text, links) = display_message_body(
            "join lobbyLink(12345,Big Game Hunters); or lobbyLink(9,Metalopolis)",
        );
        assert_eq!(text, "join Big Game Hunters or Metalopolis");
        assert_eq!(
            links
                .iter()
                .map(|link| (&text[link.range.clone()], link.advert_id))
                .collect::<Vec<_>>(),
            vec![("Big Game Hunters", 12345), ("Metalopolis", 9)]
        );
    }

    #[test]
    fn malformed_lobby_links_remain_visible() {
        for text in [
            "lobbyLink(abc,Name);",
            "lobbyLink(12,);",
            "lobbyLink(12,unclosed",
        ] {
            assert_eq!(display_message_body(text), (text.to_owned(), Vec::new()));
        }
    }

    #[test]
    fn transcript_urls_exclude_sentence_punctuation() {
        let text = "see (https://example.com/a?q=1), then www.example.org/docs.";
        let links = recognized_urls(text);
        assert_eq!(
            links
                .iter()
                .map(|(range, destination)| (&text[range.clone()], destination.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("https://example.com/a?q=1", "https://example.com/a?q=1"),
                ("www.example.org/docs", "https://www.example.org/docs"),
            ]
        );
    }

    #[test]
    fn transcript_url_ranges_include_the_sender_prefix_offset() {
        let line = TranscriptLine::Message {
            time: "7:31 PM".into(),
            sender: member("Nova"),
            text: "open https://example.com".into(),
        };
        let content = content_style(&line);
        assert_eq!(content.links.len(), 1);
        assert_eq!(
            &content.text[content.links[0].0.clone()],
            "https://example.com"
        );
    }

    #[test]
    fn media_links_are_separated_from_conversation_text() {
        let (text, links) = split_media("look https://example.com/cat.gif now");
        assert_eq!(text, "look now");
        assert_eq!(links, vec!["https://example.com/cat.gif"]);
    }
}
