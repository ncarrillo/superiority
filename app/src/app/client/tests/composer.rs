use crate::{
    app::client::{
        composer::{
            CommandKind, CommandResults, CommandRow, PersonRow, accents, command_word, match_span,
            no_party_notice, parse_command, unknown_command_notice,
        },
        join::{JoinRow, JoinSource},
    },
    chat::ChatChannel,
};
use superiority_core::native::WhisperTarget;
use superiority_ui::products::sc2::PresenceKind;

/// the caret sits at the end of the line, which is where it is while you type.
fn parse(line: &str) -> Option<(CommandKind, String)> {
    parse_command(line, line.len()).map(|command| (command.kind, command.query))
}

#[test]
fn only_a_whole_command_word_opens_the_popup() {
    // the word has to start the line and end there, or every sentence about
    // joining something would turn into a command
    assert_eq!(
        parse("/join pro"),
        Some((CommandKind::Join, "pro".to_owned()))
    );
    assert_eq!(
        parse("/JOIN Pro"),
        Some((CommandKind::Join, "Pro".to_owned()))
    );
    assert_eq!(parse("/join"), Some((CommandKind::Join, String::new())));
    assert_eq!(
        parse("/join   deep space  "),
        Some((CommandKind::Join, "deep space".to_owned()))
    );

    assert_eq!(
        parse("/w nel"),
        Some((CommandKind::Whisper, "nel".to_owned()))
    );
    assert_eq!(parse("/w"), Some((CommandKind::Whisper, String::new())));

    assert_eq!(parse("/joining a clan"), None);
    assert_eq!(parse("/were you there"), None);
    assert_eq!(parse("look, /join general"), None);
    assert_eq!(parse("hello"), None);
}

#[test]
fn the_party_scope_is_a_command_that_lists_nothing() {
    // `/p` is a scope the field wears rather than a search: there is nobody to
    // pick, so it parses like the others and opens no list
    assert_eq!(parse("/p"), Some((CommandKind::Party, String::new())));
    assert_eq!(
        parse("/p ready in 30s"),
        Some((CommandKind::Party, "ready in 30s".to_owned()))
    );
    // it is a whole word, like the rest — `/pro` is not a party
    assert_eq!(parse("/pro"), None);

    // green is the party colour and nothing else uses it, so the field says
    // which scope it is in before anything is sent
    let party = accents("/p ready");
    assert_eq!(party.len(), 1);
    assert_eq!(party[0].range, 0..2);
    assert_eq!(party[0].color, 0x0047_d184);
    assert_ne!(party[0].color, accents("/w nel")[0].color);
    assert_ne!(party[0].color, accents("/join pro")[0].color);

    // and it is a command we have, so it is never typed at the channel
    assert_eq!(command_word("/p"), Some("p"));
    assert!(unknown_command_notice("me").contains("/p"));
    assert_eq!(no_party_notice(), "You are not in a party.");
}

#[test]
fn a_mention_is_the_token_under_the_caret() {
    // mentions are typed mid-sentence, so the trigger is wherever the caret is
    // rather than at the head of the line
    assert_eq!(
        parse("good game @wer"),
        Some((CommandKind::Mention, "wer".to_owned()))
    );
    assert_eq!(parse("@"), Some((CommandKind::Mention, String::new())));

    // the caret is what picks the token, not the end of the line
    let line = "gg @wer and @nel";
    assert_eq!(
        parse_command(line, 6).map(|command| command.query),
        Some("wer".to_owned())
    );
    assert_eq!(
        parse_command(line, line.len()).map(|command| command.query),
        Some("nel".to_owned())
    );
    // the span covers the token the completion replaces, not the whole line
    assert_eq!(
        parse_command(line, 6).map(|command| command.span),
        Some(3..7)
    );

    // a sigil inside a word is an address, not a mention
    assert_eq!(parse("mail me at a@b"), None);
    // and a command line is an instruction, with nobody in it to name
    assert_eq!(
        parse("/join @general"),
        Some((CommandKind::Join, "@general".to_owned()))
    );
}

#[test]
fn the_field_paints_the_trigger_apart_from_the_message() {
    let join = accents("/join pro");
    assert_eq!(join.len(), 1);
    assert_eq!(join[0].range, 0..5);
    assert!(join[0].background.is_none());

    // a whisper wears its own colour, so the two commands are not confused
    let whisper = accents("/w nel");
    assert_eq!(whisper[0].range, 0..2);
    assert_ne!(whisper[0].color, join[0].color);

    // every mention on the line is a solid token, finished or not
    let mentions = accents("gg @wer and @nel");
    assert_eq!(
        mentions
            .iter()
            .map(|accent| accent.range.clone())
            .collect::<Vec<_>>(),
        vec![3..7, 12..16]
    );
    assert!(mentions[0].background.is_some());

    // a command line has no mentions in it
    assert!(accents("/join @general").len() == 1);
    assert!(accents("plain message").is_empty());
}

#[test]
fn a_command_we_do_not_have_is_answered_rather_than_sent() {
    // a leading slash means an instruction, so it is never typed at the
    // channel — the transcript says so instead
    assert_eq!(command_word("/me waves"), Some("me"));
    assert_eq!(
        unknown_command_notice("me"),
        "/me is not a command. The commands are /join, /w, and /p."
    );

    // a bare slash, or one that opens an ordinary sentence, is just text
    assert_eq!(command_word("/"), None);
    assert_eq!(command_word("/ join"), None);
    assert_eq!(command_word("and/or"), None);
}

#[test]
fn the_underline_covers_what_was_typed_wherever_it_sits() {
    let name = "Protoss Strategy";
    assert_eq!(match_span(name, "pro"), Some(0..3));
    assert_eq!(&name[match_span(name, "STRAT").unwrap()], "Strat");
    assert_eq!(match_span(name, "zerg"), None);
    assert_eq!(match_span(name, "  "), None);

    // case folding moves byte offsets, so the span has to be found in the
    // name itself rather than in a lowercased copy of it
    let cyrillic = "Серраль";
    assert_eq!(&cyrillic[match_span(cyrillic, "РР").unwrap()], "рр");
}

#[test]
fn the_first_result_is_armed_and_the_list_wraps_through_the_create_action() {
    let results = channels(
        vec![channel("Protoss Strategy"), channel("Probes & Pylons")],
        Some(ChatChannel::Private("pro".to_owned())),
    );

    // "/join pro↵" is two keystrokes from done, so nothing has to be picked
    assert_eq!(results.selection(0), 0);
    // an index left over from a longer list falls back rather than joining
    // whatever happens to sit at the end
    assert_eq!(results.selection(9), 0);

    assert_eq!(results.step(0, 1), 1);
    // the create action is the last line, and one press up from the first
    assert_eq!(results.step(1, 1), 2);
    assert_eq!(results.step(2, 1), 0);
    assert_eq!(results.step(0, -1), 2);
}

#[test]
fn taking_a_row_does_what_its_trigger_means() {
    let joining = channels(vec![channel("Protoss Strategy")], None);
    assert!(matches!(
        joining.action(0),
        Some(crate::app::client::composer::CommandAction::Join(_))
    ));
    // completing leaves a space behind the name, so `/w somebody ` carries
    // straight on into the message
    assert_eq!(
        joining.completion(0).as_deref(),
        Some("/join Protoss Strategy ")
    );

    let whispering = people(CommandKind::Whisper, 0..0);
    assert!(matches!(
        whispering.action(0),
        Some(crate::app::client::composer::CommandAction::Whisper(_))
    ));
    assert_eq!(
        whispering.completion(0).as_deref(),
        Some("/w NelsonTest91 ")
    );

    // a mention is spliced into the sentence over its own token, so there is
    // no line to complete
    let mentioning = people(CommandKind::Mention, 3..7);
    match mentioning.action(0) {
        Some(crate::app::client::composer::CommandAction::Mention { name, span }) => {
            assert_eq!(name, "NelsonTest91");
            assert_eq!(span, 3..7);
        }
        _ => panic!("a mention row has to mention somebody"),
    }
    assert_eq!(mentioning.completion(0), None);
}

#[test]
fn a_command_with_nothing_to_offer_does_not_open() {
    let empty = channels(Vec::new(), None);
    assert!(empty.is_empty());
    assert!(empty.action(0).is_none());
    assert_eq!(empty.step(0, 1), 0);

    // a name nobody is using is still an offer, so the popup stays open on it
    let create_only = channels(Vec::new(), Some(ChatChannel::Private("pro".to_owned())));
    assert!(!create_only.is_empty());
}

fn channels(rows: Vec<JoinRow>, create: Option<ChatChannel>) -> CommandResults {
    CommandResults {
        kind: CommandKind::Join,
        rows: rows.into_iter().map(CommandRow::Channel).collect(),
        create,
        query: "pro".to_owned(),
        span: 0..0,
    }
}

fn people(kind: CommandKind, span: std::ops::Range<usize>) -> CommandResults {
    CommandResults {
        kind,
        rows: vec![CommandRow::Person(PersonRow {
            name: "NelsonTest91".to_owned(),
            clan_tag: None,
            own_clan: false,
            portrait: None,
            presence: PresenceKind::Available,
            context: "Friend · General".to_owned(),
            offline: false,
            target: WhisperTarget::Name("NelsonTest91".to_owned()),
        })],
        create: None,
        query: "nel".to_owned(),
        span,
    }
}

fn channel(name: &str) -> JoinRow {
    JoinRow {
        name: name.to_owned(),
        note: None,
        source: JoinSource::Public,
        target: ChatChannel::Private(name.to_owned()),
        icon: "images/icons/channel.png",
        count: Some(96),
    }
}

#[test]
fn a_run_of_party_lines_from_one_speaker_wears_the_chip_once() {
    use crate::app::client::chat::{ChatLine, follows_party_line};
    use superiority_ui::products::sc2::RosterUserTone;

    let mut speaker = super::user(7, "NelsonTest91");
    speaker.tone = RosterUserTone::Party;
    let mut other = super::user(8, "ncarrillo");
    other.tone = RosterUserTone::Party;
    let channel = super::user(9, "TerranItUp");

    let say = |sender: &crate::app::client::roster::UiUser| ChatLine::Message {
        time: "7:36 PM".to_owned(),
        sender: sender.clone(),
        text: "rushing 12 pool".to_owned(),
    };
    let transcript = vec![
        say(&channel),
        say(&speaker),
        say(&speaker),
        say(&other),
        say(&speaker),
    ];

    // the first party line after channel talk introduces itself
    assert!(!follows_party_line(&transcript, 1, &transcript[1], true));
    // the second from the same speaker does not
    assert!(follows_party_line(&transcript, 2, &transcript[2], true));
    // a new speaker starts a new run, and so does coming back after them
    assert!(!follows_party_line(&transcript, 3, &transcript[3], true));
    assert!(!follows_party_line(&transcript, 4, &transcript[4], true));
    // channel talk never wears the chip at all
    assert!(!follows_party_line(&transcript, 0, &transcript[0], true));
}
