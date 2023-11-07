use std::{env, fs::File, io::Write, path::Path};

use binrw::BinRead;
use color_eyre::Result;
use indexmap::IndexMap;
use serde::Serialize;
use text::TextFile;

use crate::garc::GarcFile;

mod garc;
mod text;

mod learnset;
mod moves;
mod pokemon;

mod text_ids {
    pub const SPECIES_NAMES: usize = 60;

    pub const ITEM_NAMES: usize = 40;
    pub const _ITEM_DESCS: usize = 39;

    pub const ABILITY_NAMES: usize = 101;
    pub const ABILITY_DESCS: usize = 102;

    pub const MOVE_NAMES: usize = 118;
    pub const MOVE_DESCS: usize = 117;

    pub const TYPE_NAMES: usize = 112;
}

mod garc_files {
    pub const BASE_PATH: &str = "romfs/a/";

    pub const MOVE: &str = "0/1/1";
    pub const _EGG_MOVES: &str = "0/1/2";
    pub const LVL_UP_MOVES: &str = "0/1/3";

    pub const EVOLUTIONS: &str = "0/1/4";
    pub const MEGA_EVOS: &str = "0/1/5";

    pub const POKEMON_STATS: &str = "0/1/7";
}

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

fn to_id(s: String) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_uppercase() || c.is_ascii_digit())
        .collect()
}

#[allow(non_snake_case)]
#[derive(Serialize)]
struct AbilityJs {
    name: String,
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
                    num: index as _,
                    desc: ability_descs[index].clone(),
                    shortDesc: ability_descs[index].clone(),
                },
            )
        })
        .skip(1)
        .collect();

    let mut f = File::create(out_path.join("abilities.json"))?;
    write!(f, "{}", serde_json::to_string_pretty(&ability_map)?)?;

    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = Path::new(&args[1]);
    let out_path = Path::new(&args[2]);

    let mut en_text_file = File::open(path.join("romfs/a/0/3/2")).unwrap();
    let text_garc_file = GarcFile::read_le(&mut en_text_file).unwrap();
    let text_files = garc::read_files::<text::TextFile>(&text_garc_file);
    let names = pokemon::dump_pokes(path, out_path, &text_files).unwrap();
    learnset::dump_learnsets(path, out_path, &text_files, &names).unwrap();
    moves::dump_moves(path, out_path, &text_files).unwrap();
    dump_abilities(path, out_path, &text_files).unwrap();
}
