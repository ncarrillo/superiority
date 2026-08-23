use super::*;

/// the popup is the fast path, not a browser: it offers the best few matches
/// and leaves the rest to the panels.
pub(in crate::app::client) const COMMAND_RESULTS: usize = 6;

const JOIN_COMMAND: &str = "/join";
const WHISPER_COMMAND: &str = "/w";
const PARTY_COMMAND: &str = "/p";
const MENTION_SIGIL: char = '@';

/// a command word is orange, a whisper purple, a mention a solid blue token —
/// so the field says which of the three you are in before the popup does.
const JOIN_ACCENT: u32 = 0x00f0_aa64;
const WHISPER_ACCENT: u32 = 0x00c0_84e8;
const MENTION_ACCENT: u32 = 0x006b_c2f2;
const MENTION_FILL: u32 = 0x33a8_f026;
/// the party colour. it is the theme's `ONLINE` green, and section P reserves
/// it: nothing else in the app may use green, so one glance separates the three
/// scopes.
const PARTY_ACCENT: u32 = 0x0047_d184;

/// what the composer is being asked for. one popup answers all three; they
/// differ in what they list and in where taking a row leaves you.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub(in crate::app::client) enum CommandKind {
    /// `/join` — channels.
    Join,
    /// `/w` — a person to whisper, from anywhere.
    Whisper,
    /// `@` — a person to name mid-message, from this channel only.
    Mention,
    /// `/p` — talk to your party. this one lists nothing: it is a scope the
    /// field wears rather than a search, because a party has no window of its
    /// own to send you to.
    Party,
}

impl CommandKind {
    pub(in crate::app::client) const fn trigger(self) -> &'static str {
        match self {
            Self::Join => JOIN_COMMAND,
            Self::Whisper => WHISPER_COMMAND,
            Self::Party => PARTY_COMMAND,
            Self::Mention => "@",
        }
    }

    const fn accent(self) -> u32 {
        match self {
            Self::Join => JOIN_ACCENT,
            Self::Whisper => WHISPER_ACCENT,
            Self::Party => PARTY_ACCENT,
            Self::Mention => MENTION_ACCENT,
        }
    }

    /// a mention is a block in the middle of a sentence; a command word is
    /// coloured text at the head of one.
    const fn fill(self) -> Option<u32> {
        match self {
            Self::Mention => Some(MENTION_FILL),
            Self::Join | Self::Whisper | Self::Party => None,
        }
    }

    /// the colours the popup wears while it answers this trigger. `/w` is
    /// purple everywhere it appears — the command word, the list of people, and
    /// the conversation it opens are one thread of colour.
    pub(in crate::app::client) const fn tint(self) -> CommandTint {
        match self {
            Self::Whisper => WHISPER_TINT,
            Self::Party => PARTY_TINT,
            Self::Join | Self::Mention => CHANNEL_TINT,
        }
    }

    /// what the footer says the return key does.
    pub(in crate::app::client) const fn confirm_label(self) -> &'static str {
        match self {
            Self::Join => "join",
            Self::Whisper => "whisper",
            Self::Party => "party",
            Self::Mention => "mention",
        }
    }

    /// the line that completing a row leaves in the field, with the caret after
    /// it. the trailing space is what lets `/w` carry on into a message — the
    /// name is finished, and the next thing typed is the next word, the same
    /// way a completed mention leaves one behind it. a mention is not a line —
    /// it is spliced into one — so it has no answer here.
    fn completed_line(self, name: &str) -> Option<String> {
        match self {
            Self::Join | Self::Whisper => Some(format!("{} {name} ", self.trigger())),
            Self::Mention | Self::Party => None,
        }
    }
}

/// how the popup paints itself for one trigger: the rail down the selected row,
/// the fills behind it, and the colour a selected row's text lifts to.
#[derive(Clone, Copy)]
pub(in crate::app::client) struct CommandTint {
    pub(in crate::app::client) accent: u32,
    pub(in crate::app::client) selected_fill: u32,
    pub(in crate::app::client) hover_fill: u32,
    pub(in crate::app::client) selected_text: u32,
}

const CHANNEL_TINT: CommandTint = CommandTint {
    accent: MENTION_ACCENT,
    selected_fill: 0x1231_5e80,
    hover_fill: 0x1231_5e59,
    selected_text: 0x00e6_f9ff,
};

/// green is the party colour everywhere and nothing else uses it, so one
/// glance separates the three scopes: channel blue, whisper purple, party
/// green.
const PARTY_TINT: CommandTint = CommandTint {
    accent: PARTY_ACCENT,
    selected_fill: 0x1a53_3399,
    hover_fill: 0x1a53_3359,
    selected_text: 0x00e6_fff0,
};

const WHISPER_TINT: CommandTint = CommandTint {
    accent: WHISPER_ACCENT,
    selected_fill: 0x5028_7899,
    hover_fill: 0x5028_7859,
    selected_text: 0x00f3_e6ff,
};

/// a trigger the composer is sitting in, and what has been typed after it.
#[derive(Clone, Eq, PartialEq, Debug)]
pub(in crate::app::client) struct ChatCommand {
    pub(in crate::app::client) kind: CommandKind,
    /// the bytes the trigger itself occupies, so the field can paint it.
    pub(in crate::app::client) trigger: Range<usize>,
    /// the bytes the whole token occupies. a mention replaces only its own
    /// token; a command owns the line.
    pub(in crate::app::client) span: Range<usize>,
    pub(in crate::app::client) query: String,
}

/// reads what the caret is sitting in. a slash command has to open the line and
/// be a whole word — `/joining a clan` is a message about joining. a mention is
/// wherever the caret is, since it is typed mid-sentence.
pub(in crate::app::client) fn parse_command(line: &str, cursor: usize) -> Option<ChatCommand> {
    parse_slash(line).or_else(|| parse_mention(line, cursor))
}

fn parse_slash(line: &str) -> Option<ChatCommand> {
    [
        (JOIN_COMMAND, CommandKind::Join),
        (WHISPER_COMMAND, CommandKind::Whisper),
        (PARTY_COMMAND, CommandKind::Party),
    ]
    .into_iter()
    .find_map(|(word, kind)| {
        let head = line.get(..word.len())?;
        if !head.eq_ignore_ascii_case(word) {
            return None;
        }
        let rest = &line[word.len()..];
        if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
            return None;
        }
        Some(ChatCommand {
            kind,
            trigger: 0..word.len(),
            span: 0..line.len(),
            query: rest.trim().to_owned(),
        })
    })
}

fn parse_mention(line: &str, cursor: usize) -> Option<ChatCommand> {
    let span = token_at(line, cursor)?;
    let token = &line[span.clone()];
    token.starts_with(MENTION_SIGIL).then(|| ChatCommand {
        kind: CommandKind::Mention,
        trigger: span.start..span.start + MENTION_SIGIL.len_utf8(),
        query: token[MENTION_SIGIL.len_utf8()..].to_owned(),
        span,
    })
}

/// the whitespace-delimited token the caret is inside of.
fn token_at(line: &str, cursor: usize) -> Option<Range<usize>> {
    let cursor = valid_boundary(line, cursor);
    let start = line[..cursor]
        .char_indices()
        .rev()
        .find(|(_, letter)| letter.is_whitespace())
        .map_or(0, |(offset, letter)| offset + letter.len_utf8());
    let end = line[cursor..]
        .char_indices()
        .find(|(_, letter)| letter.is_whitespace())
        .map_or(line.len(), |(offset, _)| cursor + offset);
    (start < end).then_some(start..end)
}

fn valid_boundary(line: &str, offset: usize) -> usize {
    let mut offset = offset.min(line.len());
    while !line.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}

/// how the field paints what is typed in it: the command word in its own
/// colour, or every mention as a solid token. a command line has no mentions in
/// it — it is an instruction, not a message.
pub(in crate::app::client) fn accents(line: &str) -> Vec<ui_text_input::AccentSpan> {
    if let Some(command) = parse_slash(line) {
        return vec![ui_text_input::AccentSpan {
            range: command.trigger,
            color: command.kind.accent(),
            background: command.kind.fill(),
        }];
    }
    mention_spans(line)
        .into_iter()
        .map(|range| ui_text_input::AccentSpan {
            range,
            color: CommandKind::Mention.accent(),
            background: CommandKind::Mention.fill(),
        })
        .collect()
}

/// every `@name` token on the line, finished or still being typed. a sigil only
/// opens a token at the start of a word, so an address is not a mention.
fn mention_spans(line: &str) -> Vec<Range<usize>> {
    line.char_indices()
        .filter(|(offset, letter)| {
            *letter == MENTION_SIGIL
                && line[..*offset]
                    .chars()
                    .next_back()
                    .is_none_or(char::is_whitespace)
        })
        .filter_map(|(start, _)| {
            let end = line[start..]
                .char_indices()
                .find(|(offset, letter)| *offset > 0 && letter.is_whitespace())
                .map_or(line.len(), |(offset, _)| start + offset);
            (end > start + MENTION_SIGIL.len_utf8()).then_some(start..end)
        })
        .collect()
}

pub(in crate::app::client) fn command_prefix() -> String {
    format!("{JOIN_COMMAND} ")
}

/// the command word a line opens with, whether or not it is one we answer to.
/// a bare slash is not a command, and neither is a slash with a space after it.
pub(in crate::app::client) fn command_word(content: &str) -> Option<&str> {
    let word = content
        .strip_prefix('/')?
        .split(char::is_whitespace)
        .next()?;
    (!word.is_empty()).then_some(word)
}

/// what the transcript says about a command we do not have. there are two, so
/// the line names them rather than pointing at a help page.
pub(in crate::app::client) fn unknown_command_notice(word: &str) -> String {
    format!("/{word} is not a command. The commands are /join, /w, and /p.")
}

/// what the transcript says when you address a party you are not in. The field
/// only wears the party scope while there is a party behind it.
pub(in crate::app::client) fn no_party_notice() -> String {
    "You are not in a party.".to_owned()
}

#[derive(Clone)]
pub(in crate::app::client) enum CommandRow {
    Channel(JoinRow),
    Person(PersonRow),
}

impl CommandRow {
    pub(in crate::app::client) fn name(&self) -> &str {
        match self {
            Self::Channel(row) => &row.name,
            Self::Person(row) => &row.name,
        }
    }
}

/// what taking a row does. the three triggers list different things and end in
/// different places, which is the only part of the popup that is not shared.
pub(in crate::app::client) enum CommandAction {
    Join(ChatChannel),
    Whisper(WhisperPeer),
    /// splice a name into the message being written, over the given bytes.
    Mention {
        name: String,
        span: Range<usize>,
    },
}

/// what the popup offers, in the order it draws them: the matches, then the
/// create action when nothing in the catalogue answers to a typed channel name.
#[derive(Clone)]
pub(in crate::app::client) struct CommandResults {
    pub(in crate::app::client) kind: CommandKind,
    pub(in crate::app::client) rows: Vec<CommandRow>,
    pub(in crate::app::client) create: Option<ChatChannel>,
    pub(in crate::app::client) query: String,
    /// the bytes a mention would replace.
    pub(in crate::app::client) span: Range<usize>,
}

impl CommandResults {
    /// the create action is the last selectable line, so it is addressed by the
    /// index one past the results.
    pub(in crate::app::client) fn len(&self) -> usize {
        self.rows.len() + usize::from(self.create.is_some())
    }

    /// nothing to show is nothing to open — an empty popup would be a hole in
    /// the chat rather than an answer.
    pub(in crate::app::client) fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// the first result is pre-selected, so `/join pro↵` is two keystrokes from
    /// done; a selection past the end falls back to it rather than being
    /// corrected at every keystroke.
    pub(in crate::app::client) fn selection(&self, current: usize) -> usize {
        if current < self.len() { current } else { 0 }
    }

    /// what the return key would do. a selection past the results is the create
    /// action, which is only ever there when there is a room to create.
    pub(in crate::app::client) fn action(&self, current: usize) -> Option<CommandAction> {
        let Some(row) = self.rows.get(self.selection(current)) else {
            return self.create.clone().map(CommandAction::Join);
        };
        match row {
            CommandRow::Channel(row) => Some(CommandAction::Join(row.target.clone())),
            CommandRow::Person(row) => Some(match self.kind {
                CommandKind::Mention => CommandAction::Mention {
                    name: row.name.clone(),
                    span: self.span.clone(),
                },
                // the party scope lists nobody, so it has no rows to take
                CommandKind::Party => return None,
                CommandKind::Join | CommandKind::Whisper => CommandAction::Whisper(row.peer()),
            }),
        }
    }

    /// the line tab writes into the field. the create action has nothing to
    /// complete — what you typed is already the name — and a mention completes
    /// by being taken, not by rewriting the line.
    pub(in crate::app::client) fn completion(&self, current: usize) -> Option<String> {
        self.rows
            .get(self.selection(current))
            .and_then(|row| self.kind.completed_line(row.name()))
    }

    /// steps the highlight, wrapping at both ends so the create action at the
    /// bottom is one press up from the first result.
    pub(in crate::app::client) fn step(&self, current: usize, delta: isize) -> usize {
        let len = isize::try_from(self.len()).unwrap_or(isize::MAX);
        if len == 0 {
            return 0;
        }
        let position = isize::try_from(self.selection(current)).unwrap_or(0);
        usize::try_from((((position + delta) % len) + len) % len).unwrap_or(0)
    }
}

/// where the typed text sits inside a result name, so the popup can underline
/// the part you actually typed. the search walks the name itself rather than a
/// lowercased copy of it, because case folding moves byte offsets and the span
/// has to index back into the original.
pub(in crate::app::client) fn match_span(name: &str, query: &str) -> Option<Range<usize>> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    for (start, _) in name.char_indices() {
        let mut found = name[start..].char_indices();
        let mut end = start;
        let matched = query.chars().all(|wanted| match found.next() {
            Some((offset, letter)) if same_letter(letter, wanted) => {
                end = start + offset + letter.len_utf8();
                true
            }
            _ => false,
        });
        if matched {
            return Some(start..end);
        }
    }
    None
}

fn same_letter(left: char, right: char) -> bool {
    left == right || left.to_lowercase().eq(right.to_lowercase())
}

impl ComposerComponent {
    /// the trigger the composer currently holds, if it holds one and it has not
    /// been dismissed.
    pub(in crate::app::client) fn active_command(&self) -> Option<ChatCommand> {
        if self.command_dismissed {
            return None;
        }
        parse_command(&self.command_line, self.command_cursor)
    }
}
