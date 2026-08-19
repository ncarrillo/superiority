use super::*;
use crate::app::client::channel::retitle_notices;

#[test]
fn resolved_group_names_retitle_only_generated_notices() {
    let mut transcript = vec![
        ChatLine::Notice {
            time: "4:25 PM".to_owned(),
            text: "Joined Group 535220.".to_owned(),
        },
        ChatLine::Message {
            time: "4:26 PM".to_owned(),
            sender: user(20, "Nova"),
            text: "Group 535220 is the good one".to_owned(),
        },
    ];

    retitle_notices(&mut transcript, "Group 535220", "CTest2");

    let ChatLine::Notice { text, .. } = &transcript[0] else {
        panic!("first line must remain a notice");
    };
    assert_eq!(text, "Joined CTest2.");
    let ChatLine::Message { text, .. } = &transcript[1] else {
        panic!("second line must remain a message");
    };
    assert_eq!(text, "Group 535220 is the good one");
}

#[test]
fn resolved_group_names_retitle_the_session_start_line() {
    let mut transcript = vec![ChatLine::SessionStart {
        time: "4:25 PM".to_owned(),
        channel: "Group 535220".to_owned(),
    }];

    retitle_notices(&mut transcript, "Group 535220", "CTest2");

    let ChatLine::SessionStart { channel, .. } = &transcript[0] else {
        panic!("first line must remain a session start");
    };
    assert_eq!(channel, "CTest2");
}
