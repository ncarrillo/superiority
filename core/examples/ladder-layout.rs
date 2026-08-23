use std::{env, fs};

use superiority_core::{
    bsn::{
        FromBsn,
        codec::{StructWireLayout, WireField},
    },
    native::Protocol,
};

const P2: [[WireField; 2]; 2] = [
    [WireField::new(0, 0), WireField::new(1, 0)],
    [WireField::new(1, 0), WireField::new(0, 0)],
];
const P3: [[WireField; 3]; 6] = [
    [
        WireField::new(0, 0),
        WireField::new(1, 0),
        WireField::new(2, 0),
    ],
    [
        WireField::new(0, 0),
        WireField::new(2, 0),
        WireField::new(1, 0),
    ],
    [
        WireField::new(1, 0),
        WireField::new(0, 0),
        WireField::new(2, 0),
    ],
    [
        WireField::new(1, 0),
        WireField::new(2, 0),
        WireField::new(0, 0),
    ],
    [
        WireField::new(2, 0),
        WireField::new(0, 0),
        WireField::new(1, 0),
    ],
    [
        WireField::new(2, 0),
        WireField::new(1, 0),
        WireField::new(0, 0),
    ],
];

fn main() {
    let path = env::args().nth(1).expect("xport reply fixture path");
    let line = fs::read_to_string(path)
        .expect("read fixture")
        .lines()
        .find(|line| line.starts_with("1\t"))
        .expect("command 1 fixture")
        .split_once('\t')
        .expect("tab-separated fixture")
        .1
        .to_owned();
    let body = hex::decode(line).expect("hex fixture");
    let protocol = Protocol::current().expect("current protocol");
    let root_type = protocol
        .codec()
        .schema()
        .unique_type_id("Battlenet::Client::Ladder::GetRankingsResponse")
        .expect("ladder response type");
    let mut candidates = Vec::new();

    for success in 0..P3.len() {
        for membership in 0..P2.len() {
            for member_id in 0..P2.len() {
                for ranking in 0..P3.len() {
                    for game_data in 0..P2.len() {
                        for key_value in 0..P2.len() {
                            let mut codec = protocol.codec().clone();
                            codec
                                .register_struct_wire_layout(
                                    "Battlenet::Ladder::RankingResponse::Success",
                                    StructWireLayout::new("ladder success candidate", &P3[success]),
                                )
                                .unwrap();
                            codec
                                .register_struct_wire_layout(
                                    "Battlenet::Ladder::Membership",
                                    StructWireLayout::new(
                                        "ladder membership candidate",
                                        &P2[membership],
                                    ),
                                )
                                .unwrap();
                            codec
                                .register_struct_wire_layout(
                                    "Battlenet::Profile::RecordAddress",
                                    StructWireLayout::new(
                                        "ladder member id candidate",
                                        &P2[member_id],
                                    ),
                                )
                                .unwrap();
                            codec
                                .register_struct_wire_layout(
                                    "Battlenet::Ladder::Ranking",
                                    StructWireLayout::new("ladder ranking candidate", &P3[ranking]),
                                )
                                .unwrap();
                            codec
                                .register_struct_wire_layout(
                                    "Battlenet::Ladder::GameData",
                                    StructWireLayout::new(
                                        "ladder game data candidate",
                                        &P2[game_data],
                                    ),
                                )
                                .unwrap();
                            codec
                                .register_struct_wire_layout(
                                    "Battlenet::Ladder::KeyValue",
                                    StructWireLayout::new(
                                        "ladder key value candidate",
                                        &P2[key_value],
                                    ),
                                )
                                .unwrap();
                            if let Ok(decoded) = codec.decode(root_type, &body, None, 0) {
                                if [
                                    success, membership, member_id, ranking, game_data, key_value,
                                ] == [1, 0, 0, 0, 0, 0]
                                {
                                    let typed = superiority_core::native::schema::ladder::ClientLadderGetRankingsResponse::from_bsn(&decoded.value).unwrap();
                                    eprintln!(
                                        "selected bits={} typed={typed:#?}",
                                        decoded.bit_count
                                    );
                                }
                                candidates.push((
                                    decoded.bit_count,
                                    [
                                        success, membership, member_id, ranking, game_data,
                                        key_value,
                                    ],
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    candidates.sort_unstable_by_key(|candidate| candidate.0);
    for (bits, layout) in candidates.into_iter().rev().take(40) {
        println!("bits={bits}/{} layout={layout:?}", body.len() * 8);
    }
}
