use std::{collections::HashMap, env, fs::File, io::Write, path::Path};

use binrw::{BinRead, FilePtr};
use color_eyre::Result;
use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use text::TextFile;

use crate::garc::GarcFile;

mod garc;
mod text;

mod learnset;

#[allow(dead_code)]
#[derive(BinRead, Serialize, Debug, Clone)]
struct Stats {
    hp: u8,
    atk: u8,
    def: u8,
    spe: u8,
    spa: u8,
    spd: u8,
}

#[allow(dead_code)]
#[derive(BinRead, Debug)]
struct PokemonStats {
    stats: Stats,
    types: (u8, u8),
    catch_rate: u8,
    evo_stage: u8,
    ev_yield: u16,
    items: [u16; 3],
    gender: u8,
    hatch_cycles: u8,
    base_friendship: u8,
    exp_growth: u8,
    egg_groups: [u8; 2],
    abilities: [u8; 3],
    escape_rate: u8,
    form_stats_id: u16,
    form_sprite: u16,
    form_count: u8,
    sprite_bits: u8,
    base_exp: u16,
    height: u16,
    weight: u16,
    tm_bits: [u8; 0x10],
    tutor_bits: [u8; 0x4],
    beach_bits: [u8; 0xa],
}

#[allow(dead_code)]
#[derive(BinRead, Debug)]
struct PokemonEvolution {
    method: u16,
    argument: u16,
    species: u16,
    form: i8,
    level: u8,
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
    recoil: u8,
    heal: u8,

    target: u8,
    stat: [u8; 3],
    stat_stage: [u8; 3],
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
    #[br(dbg)]
    ccount: u16,
    #[br(count = ccount)]
    files: Vec<FilePtr<u32, MoveStats>>,
}

mod text_ids {
    pub const SPECIES_NAMES: usize = 60;

    pub const ITEM_NAMES: usize = 40;
    pub const ITEM_DESCS: usize = 39;

    pub const ABILITY_NAMES: usize = 101;
    pub const ABILITY_DESCS: usize = 102;

    pub const MOVE_NAMES: usize = 118;
    pub const MOVE_DESCS: usize = 117;

    pub const TYPE_NAMES: usize = 112;
}

mod garc_files {
    pub const BASE_PATH: &str = "romfs/a/";

    pub const MOVE: &str = "0/1/1";
    pub const EGG_MOVES: &str = "0/1/2";
    pub const LVL_UP_MOVES: &str = "0/1/3";

    pub const EVOLUTIONS: &str = "0/1/4";
    pub const MEGA_EVOS: &str = "0/1/5";

    pub const POKEMON_STATS: &str = "0/1/7";
}

#[allow(non_snake_case)]
#[derive(Serialize)]
struct PokemonJsGenderRatio {
    M: f32,
    F: f32,
}

#[allow(non_snake_case)]
#[serde_with::skip_serializing_none]
#[derive(Serialize)]
struct PokemonJs {
    num: u32,
    name: String,
    types: Vec<String>,
    genderRatio: PokemonJsGenderRatio,
    baseStats: Stats,
    abilities: HashMap<String, String>,
    heightm: f32,
    weightkg: f32,
    prevo: Option<String>,
    evoLevel: Option<u16>,
    evoType: Option<String>,
    evoItem: Option<String>,
    evoCondition: Option<String>,
    evos: Option<Vec<String>>,
    eggGroups: Vec<String>,
}

fn to_id(s: String) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_uppercase() || c.is_ascii_digit())
        .collect()
}

fn dump_pokes(rom_path: &Path, out_path: &Path, text_files: &[TextFile]) -> Result<()> {
    let mut dex_map: IndexMap<String, PokemonJs> = IndexMap::new();

    let pokemon_path = rom_path
        .join(garc_files::BASE_PATH)
        .join(garc_files::POKEMON_STATS);
    let pokemons =
        garc::read_files::<PokemonStats>(&GarcFile::read_le(&mut File::open(pokemon_path)?)?);

    let species_names = &text_files[text_ids::SPECIES_NAMES].lines;
    let ability_names = &text_files[text_ids::ABILITY_NAMES].lines;
    let type_names = &text_files[text_ids::TYPE_NAMES].lines;
    let item_names = &text_files[text_ids::ITEM_NAMES].lines;

    for (index, pokemon) in pokemons.iter().enumerate() {
        if index >= 808 {
            break;
        }
        let name = species_names[index].clone();
        let mut types: Vec<String> = [pokemon.types.0, pokemon.types.1]
            .iter()
            .map(|t| type_names[*t as usize].clone())
            .collect();
        types.dedup();

        let mut abilities = HashMap::new();
        abilities.insert(
            "0".to_owned(),
            ability_names[pokemon.abilities[0] as usize].clone(),
        );
        if pokemon.abilities[1] != pokemon.abilities[0] {
            abilities.insert(
                "1".to_owned(),
                ability_names[pokemon.abilities[1] as usize].clone(),
            );
        }
        if pokemon.abilities[2] != pokemon.abilities[0]
            && pokemon.abilities[2] != pokemon.abilities[1]
        {
            abilities.insert(
                "H".to_owned(),
                ability_names[pokemon.abilities[2] as usize].clone(),
            );
        }

        dex_map.insert(
            to_id(name.clone()),
            PokemonJs {
                num: index as _,
                name,
                types,
                genderRatio: PokemonJsGenderRatio { M: 0., F: 0. },
                baseStats: pokemon.stats.clone(),
                abilities,
                heightm: pokemon.height as f32 / 100.,
                weightkg: pokemon.weight as f32 / 10.,
                prevo: None,
                evoType: None,
                evoLevel: None,
                evoItem: None,
                evoCondition: None,
                evos: None,
                eggGroups: Vec::new(),
            },
        );
    }

    let evo_path = rom_path
        .join(garc_files::BASE_PATH)
        .join(garc_files::EVOLUTIONS);
    let evolutions =
        garc::read_files::<[PokemonEvolution; 8]>(&GarcFile::read_le(&mut File::open(evo_path)?)?);
    handle_evos(evolutions, species_names, item_names, &mut dex_map);

    let mut f = File::create(out_path.join("pokedex.js"))?;
    write!(
        f,
        "exports.BattlePokedex = {}",
        serde_json::to_string_pretty(&dex_map)?
    )?;
    Ok(())
}

fn handle_evos(
    evolutions: Vec<[PokemonEvolution; 8]>,
    species_names: &[String],
    item_names: &[String],
    dex_map: &mut IndexMap<String, PokemonJs>,
) {
    for (index, evo_list) in evolutions.iter().enumerate() {
        if index >= 808 {
            continue;
        }
        let mut evo_set: IndexSet<String> = IndexSet::new();
        let current_name = &species_names[index];

        for evo in evo_list {
            if evo.method == 0 {
                continue;
            }
            let evo_name = &species_names[evo.species as usize];
            let evo_id = to_id(evo_name.clone());
            evo_set.insert(evo_name.clone());
            let Some(poke_entry) = dex_map.get_mut(&evo_id) else {
                continue;
            };

            if poke_entry.prevo.is_some() {
                continue;
            }
            poke_entry.prevo = Some(current_name.clone());

            if evo.level > 0 {
                poke_entry.evoLevel = Some(evo.level as _);
            }

            match evo.method {
                1 => poke_entry.evoType = Some("levelFriendship".to_owned()),
                2 => {
                    poke_entry.evoType = Some("levelFriendship".to_owned());
                    poke_entry.evoCondition = Some("during the day".to_owned());
                }
                3 => {
                    poke_entry.evoType = Some("levelFriendship".to_owned());
                    poke_entry.evoCondition = Some("during the night".to_owned());
                }
                5 => {
                    poke_entry.evoType = Some("trade".to_owned());
                }
                6 => {
                    poke_entry.evoType = Some("trade".to_owned());
                    poke_entry.evoItem = Some(item_names[evo.argument as usize].clone());
                }
                8 | 17 | 18 | 19 | 20 => {
                    poke_entry.evoType = Some("useItem".to_owned());
                    poke_entry.evoItem = Some(item_names[evo.argument as usize].clone());
                }
                _ => {}
            }
        }
        if !evo_set.is_empty() {
            dex_map.get_index_mut(index).unwrap().1.evos = Some(evo_set.into_iter().collect());
        }
    }
}

#[allow(non_snake_case)]
#[derive(Serialize)]
struct AbilityJs {
    name: String,
    rating: f32,
    num: u32,
    desc: String,
    shortDesc: String,
}

fn dump_abilities(_rom_path: &Path, out_path: &Path, text_files: &[TextFile]) -> Result<()> {
    let ability_names = &text_files[text_ids::ABILITY_NAMES].lines;
    let ability_descs = &text_files[text_ids::ABILITY_DESCS].lines;

    let ability_map: IndexMap<String, AbilityJs> = ability_names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            (
                to_id(name.clone()),
                AbilityJs {
                    name: name.clone(),
                    rating: 1.,
                    num: index as _,
                    desc: ability_descs[index].clone(),
                    shortDesc: ability_descs[index].clone(),
                },
            )
        })
        .collect();

    let mut f = File::create(out_path.join("abilities.js"))?;
    write!(
        f,
        "exports.BattleAbilities = {}",
        serde_json::to_string_pretty(&ability_map)?
    )?;

    Ok(())
}

#[derive(Serialize)]
#[serde(untagged)]
enum MoveJsAccuracy {
    Number(i32),
    Boolean(bool),
}

#[allow(non_snake_case)]
#[derive(Serialize)]
struct MoveJs {
    num: u32,
    accuracy: MoveJsAccuracy,
    basePower: u32,
    category: String,
    name: String,
    pp: u32,
    priority: i32,
    flags: HashMap<String, u8>,
    r#type: String,
    target: String,
    desc: String,
    shortDesc: String,
}

fn move_flags(mmove: &MoveStats) -> HashMap<String, u8> {
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

fn dump_moves(rom_path: &Path, out_path: &Path, text_files: &[TextFile]) -> Result<()> {
    let move_names = &text_files[text_ids::MOVE_NAMES].lines;
    let move_descs = &text_files[text_ids::MOVE_DESCS].lines;
    let type_names = &text_files[text_ids::TYPE_NAMES].lines;

    let move_path = rom_path.join(garc_files::BASE_PATH).join(garc_files::MOVE);
    let moves =
        &garc::read_files::<BinLinkedMoves>(&garc::GarcFile::read_le(&mut File::open(move_path)?)?)
            [0]
        .files;
    println!("{:?}", moves[1]);
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
                    flags: move_flags(cmove),
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
                    desc: move_descs[index].clone(),
                    shortDesc: move_descs[index].clone(),
                },
            )
        })
        .collect();

    let mut f = File::create(out_path.join("moves.js"))?;
    write!(
        f,
        "exports.BattleMovedex = {}",
        serde_json::to_string_pretty(&move_map)?
    )?;
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = Path::new(&args[1]);
    let out_path = Path::new(&args[2]);

    let mut en_text_file = File::open(path.join("romfs/a/0/3/2")).unwrap();
    let text_garc_file = GarcFile::read_le(&mut en_text_file).unwrap();
    let text_files = garc::read_files::<text::TextFile>(&text_garc_file);
    dump_pokes(path, out_path, &text_files).unwrap();
    dump_abilities(path, out_path, &text_files).unwrap();
    dump_moves(path, out_path, &text_files).unwrap();
    learnset::dump_learnsets(path, out_path, &text_files).unwrap();
}
