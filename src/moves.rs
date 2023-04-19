use std::{collections::BTreeMap, default::Default, fs::File, io::Write, path::Path};

use binrw::{BinRead, FilePtr};
use color_eyre::Result;
use indexmap::IndexMap;
use serde::Serialize;

use crate::{garc, garc_files, text::TextFile, text_ids, to_id};

pub fn default<T: Default>() -> T {
    std::default::Default::default()
}

#[allow(dead_code)]
#[derive(BinRead, Debug)]
struct MoveStats {
    move_type: u8,
    quality: u8,
    category: u8,
    power: u8,

    accuracy: u8,
    pp: u8,
    priority: i8,
    hit_min_max: u8,

    inflict: u16,
    inflict_percent: u8,
    inflict_duration: u8,

    turn_min: u8,
    turn_max: u8,
    crit_stage: u8,
    flinch: u8,

    effect_sequence: u16,
    recoil_absorption: i8,
    heal: u8,

    target: u8,
    stat: [u8; 3],
    stat_stage: [i8; 3],
    stat_percent: [u8; 3],

    z_move: u16,
    z_power: u8,
    z_effect: u8,

    refresh_type: u8,
    refresh_percent: u8,

    flags: u32,
}

#[allow(dead_code)]
#[derive(BinRead)]
#[br(magic = b"WD")]
struct BinLinkedMoves {
    ccount: u16,
    #[br(count = ccount)]
    files: Vec<FilePtr<u32, MoveStats>>,
}

#[derive(Serialize)]
#[serde(untagged)]
enum MoveJsAccuracy {
    Number(i32),
    Boolean(bool),
}

#[derive(Serialize)]
#[serde(untagged)]
enum MoveJsMultihit {
    Number(i32),
    Range(i32, i32),
}

#[serde_with::skip_serializing_none]
#[derive(Serialize, Default)]
struct BoostTable {
    atk: Option<i8>,
    def: Option<i8>,
    spa: Option<i8>,
    spd: Option<i8>,
    spe: Option<i8>,
    accuracy: Option<i8>,
    evasion: Option<i8>,
}

#[derive(Serialize)]
struct SelfEffect {
    boosts: BoostTable,
}

#[allow(non_snake_case)]
#[serde_with::skip_serializing_none]
#[derive(Serialize, Default)]
struct MoveSecondaryJs {
    chance: i32,
    boosts: Option<BoostTable>,
    status: Option<String>,
    volatileStatus: Option<String>,
    #[serde(rename = "self")]
    selfEffects: Option<SelfEffect>,
}

#[allow(non_snake_case)]
#[serde_with::skip_serializing_none]
#[derive(Serialize)]
struct MoveJs {
    num: u32,
    accuracy: MoveJsAccuracy,
    basePower: u32,
    category: String,
    name: String,
    pp: u32,
    priority: i32,
    critRatio: i32,
    drain: Option<(i32, i32)>,
    recoil: Option<(i32, i32)>,
    multihit: Option<MoveJsMultihit>,

    secondaries: Option<Vec<MoveSecondaryJs>>,
    flags: BTreeMap<String, u8>,
    r#type: String,
    target: String,
    desc: String,
    shortDesc: String,
}

fn move_flags(mmove: &MoveStats) -> BTreeMap<String, u8> {
    const FLAGS: &[(u32, &str)] = &[
        (1 << 0, "contact"),
        (1 << 1, "charge"),
        (1 << 2, "recharge"),
        (1 << 3, "protect"),
        (1 << 4, "reflectable"),
        (1 << 5, "snatch"),
        (1 << 6, "mirror"),
        (1 << 7, "punch"),
        (1 << 8, "sound"),
        (1 << 9, "gravity"),
        (1 << 10, "defrost"),
        (1 << 12, "heal"),
        (1 << 13, "bypasssub"),
        (1 << 16, "dance"),
    ];

    FLAGS
        .iter()
        .filter_map(|(bit, text)| {
            if bit & mmove.flags != 0 {
                Some(((*text).to_owned(), 1))
            } else {
                None
            }
        })
        .collect()
}

fn tuple_ratio(percent: i32) -> (i32, i32) {
    match percent {
        0 => (0, 1),
        1 => (1, 1),
        25 => (1, 4),
        50 => (1, 2),
        75 => (3, 4),
        n => (n, 100),
    }
}

fn get_recoil(stats: &MoveStats) -> Option<(i32, i32)> {
    if stats.recoil_absorption < 0 {
        Some(tuple_ratio(-stats.recoil_absorption as _))
    } else {
        None
    }
}

fn get_drain(stats: &MoveStats) -> Option<(i32, i32)> {
    if stats.recoil_absorption > 0 {
        Some(tuple_ratio(stats.recoil_absorption as _))
    } else {
        None
    }
}

fn stat_table(stat: u8, change: i8) -> BoostTable {
    match stat {
        1 => BoostTable {
            atk: Some(change),
            ..default()
        },
        2 => BoostTable {
            def: Some(change),
            ..default()
        },
        3 => BoostTable {
            spa: Some(change),
            ..default()
        },
        4 => BoostTable {
            spd: Some(change),
            ..default()
        },
        5 => BoostTable {
            spe: Some(change),
            ..default()
        },
        6 => BoostTable {
            accuracy: Some(change),
            ..default()
        },
        7 => BoostTable {
            evasion: Some(change),
            ..default()
        },
        8 => BoostTable {
            atk: Some(change),
            def: Some(change),
            spa: Some(change),
            spd: Some(change),
            spe: Some(change),
            ..default()
        },
        _ => default(),
    }
}
const INFLICT: &[&str] = &["none", "par", "slp", "frz", "brn", "psn", "tox"];
fn get_secondaries(stats: &MoveStats) -> Option<Vec<MoveSecondaryJs>> {
    let mut vec = Vec::new();
    if stats.inflict > 0 && stats.inflict_percent > 0 {
        if (stats.inflict as usize) < INFLICT.len() {
            vec.push(MoveSecondaryJs {
                chance: stats.inflict_percent as _,
                status: Some(INFLICT[stats.inflict as usize].to_owned()),
                ..default()
            })
        } else {
            vec.push(MoveSecondaryJs {
                chance: stats.inflict_percent as _,
                volatileStatus: Some(stats.inflict.to_string()),
                ..default()
            })
        }
    }

    if stats.flinch > 0 {
        vec.push(MoveSecondaryJs {
            chance: stats.flinch as _,
            volatileStatus: Some("flinch".to_owned()),
            ..default()
        })
    }

    for i in 0..3 {
        if stats.stat[i] > 0 {
            if stats.quality == 7 {
                vec.push(MoveSecondaryJs {
                    chance: stats.stat_percent[i] as _,
                    selfEffects: Some(SelfEffect {
                        boosts: stat_table(stats.stat[i], stats.stat_stage[i]),
                    }),
                    ..default()
                })
            } else {
                vec.push(MoveSecondaryJs {
                    chance: stats.stat_percent[i] as _,
                    boosts: Some(stat_table(stats.stat[i], stats.stat_stage[i])),
                    ..default()
                })
            }
        }
    }

    if vec.is_empty() {
        None
    } else {
        Some(vec)
    }
}

fn get_multihit(move_stats: &MoveStats) -> Option<MoveJsMultihit> {
    use MoveJsMultihit::*;
    let min = move_stats.hit_min_max & 0xf;
    let max = move_stats.hit_min_max >> 4;
    match (min, max) {
        (0, 0) => None,
        (a, b) if a == b => Some(Number(a as i32)),
        (a, b) => Some(Range(a as i32, b as i32)),
    }
}

pub fn dump_moves(rom_path: &Path, out_path: &Path, text_files: &[TextFile]) -> Result<()> {
    let move_names = &text_files[text_ids::MOVE_NAMES].lines;
    let move_descs = &text_files[text_ids::MOVE_DESCS].lines;
    let type_names = &text_files[text_ids::TYPE_NAMES].lines;

    let move_path = rom_path.join(garc_files::BASE_PATH).join(garc_files::MOVE);
    let moves =
        &garc::read_files::<BinLinkedMoves>(&garc::GarcFile::read_le(&mut File::open(move_path)?)?)
            [0]
        .files;
    let move_map: IndexMap<String, MoveJs> = moves
        .iter()
        .enumerate()
        .map(|(index, cmove)| {
            let name = &move_names[index];
            (
                to_id(name.clone()),
                MoveJs {
                    num: index as _,
                    name: name.clone(),
                    accuracy: match cmove.accuracy {
                        101 => MoveJsAccuracy::Boolean(true),
                        a => MoveJsAccuracy::Number(a as _),
                    },
                    basePower: cmove.power as _,
                    pp: cmove.pp as _,
                    category: match cmove.category {
                        1 => "Physical",
                        2 => "Special",
                        _ => "Status",
                    }
                    .to_owned(),
                    priority: cmove.priority as _,
                    critRatio: (cmove.crit_stage as i32) + 1,
                    flags: move_flags(cmove),
                    drain: get_drain(cmove),
                    recoil: get_recoil(cmove),
                    secondaries: get_secondaries(cmove),
                    r#type: type_names[cmove.move_type as usize].clone(),
                    target: match cmove.target {
                        1 | 2 => "adjacentAlly",
                        4 => "allAdjacent",
                        5 => "allAdjacentFoes",
                        7 => "self",
                        9 => "randomNormal",
                        _ => "normal",
                    }
                    .to_owned(),
                    multihit: get_multihit(cmove),
                    desc: move_descs[index].clone(),
                    shortDesc: move_descs[index].clone(),
                },
            )
        })
        .skip(1)
        .collect();

    let mut f = File::create(out_path.join("moves.json"))?;
    write!(f, "{}", serde_json::to_string_pretty(&move_map)?)?;
    Ok(())
}
