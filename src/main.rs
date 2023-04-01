use std::{env, fs::File, path::Path};

use binrw::BinRead;

use crate::garc::GarcFile;

mod garc;
mod text;


#[allow(dead_code)]
#[derive(BinRead, Debug)]
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
    priority: u8,
    hit_min_max: u8,
    inflict: u16,
    inflict_percent: u8,
    inflict_duration: u8,
    turn_min: u8,
    turn_max: u8,
    crit_stage: u8,
    flinch: u8,
    effect_sequence: u16,
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


fn main() {
    let args: Vec<String> = env::args().collect();
    let path = Path::new(&args[1]);
    let pokemon_stats_file = path.join("romfs/a/0/1/7");
    let mut file = File::open(pokemon_stats_file).unwrap();
    let garc_file = GarcFile::read_le(&mut file).unwrap();
    let stats = garc::read_file::<PokemonStats>(1, 0, &garc_file);
    println!("{:?}", stats);

    let mut en_text_file = File::open(path.join("romfs/a/0/3/2")).unwrap();
    let text_garc_file = GarcFile::read_le(&mut en_text_file).unwrap();
    let text_file = garc::read_file::<text::TextFile>(102, 0, &text_garc_file).unwrap();
    println!("{:?}", text_file);
}
