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
