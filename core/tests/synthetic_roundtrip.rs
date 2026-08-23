#[test]
fn synthetic_profile_read_cache_round_trips() {
    let protocol = superiority_core::native::protocol::Protocol::current().unwrap();
    let bytes = protocol.profile_read_cache(0xDEAD_BEEF).unwrap();
    let (slot, cmd, payload) = protocol.decode_server_record(&bytes).unwrap();
    assert_eq!(
        (slot, cmd),
        (
            Some(superiority_core::native::protocol::PROFILE_SLOT),
            superiority_core::native::protocol::PROFILE_READ_COMMAND,
        )
    );
    match payload {
        superiority_core::native::model::Payload::ProfileRead(r) => {
            assert_eq!(r.request_id, 0xDEAD_BEEF);
        }
        other => panic!("expected ProfileRead, got {other:?}"),
    }
}

#[test]
fn synthetic_profile_read_record_round_trips() {
    use superiority_core::native::model::{Payload, ProfileReadResult};

    let protocol = superiority_core::native::protocol::Protocol::current().unwrap();
    let [start, block] = protocol
        .profile_read_record(0xDEAD_BEEF, 6145, &[0])
        .unwrap();

    let (_, _, Payload::ProfileRead(start)) = protocol.decode_server_record(&start).unwrap() else {
        panic!("expected profile start");
    };
    assert_eq!(start.request_id, 0xDEAD_BEEF);
    assert_eq!(
        start.result,
        ProfileReadResult::Start {
            packet_count: 1,
            record_type: 6145,
        }
    );

    let (_, _, Payload::ProfileRead(block)) = protocol.decode_server_record(&block).unwrap() else {
        panic!("expected profile block");
    };
    assert_eq!(block.request_id, 0xDEAD_BEEF);
    assert_eq!(block.result, ProfileReadResult::Block(vec![0]));

    let empty = protocol.profile_read_empty(7, 1046).unwrap();
    let (_, _, Payload::ProfileRead(empty)) = protocol.decode_server_record(&empty).unwrap() else {
        panic!("expected empty profile start");
    };
    assert_eq!(empty.request_id, 7);
    assert_eq!(
        empty.result,
        ProfileReadResult::Start {
            packet_count: 0,
            record_type: 1046,
        }
    );
}
