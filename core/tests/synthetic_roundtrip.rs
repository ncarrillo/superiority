#[test]
fn synthetic_profile_read_cache_round_trips() {
    let protocol = sc2_core::native::protocol::Protocol::current().unwrap();
    let bytes = protocol.profile_read_cache(0xDEAD_BEEF).unwrap();
    let (slot, cmd, payload) = protocol.decode_server_record(&bytes).unwrap();
    assert_eq!((slot, cmd), (Some(14), 0));
    match payload {
        sc2_core::native::model::Payload::ProfileRead(r) => {
            assert_eq!(r.request_id, 0xDEAD_BEEF);
            eprintln!("OK: ProfileRead request_id={:x} result={:?}", r.request_id, r.result);
        }
        other => panic!("expected ProfileRead, got {other:?}"),
    }
}
