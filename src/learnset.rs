use crate::{
    garc::{self, GarcFile},
    garc_files, pokemon,
    text::TextFile,
    text_ids, to_id, PokemonStats,
};
use binrw::{until_eof, BinRead};
use color_eyre::Result;
use indexmap::IndexMap;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[derive(BinRead, Debug)]
struct LevelUpMove {
    move_id: i16,
    level: i16,
}

#[derive(BinRead, Debug)]
struct LevelUpMoves {
    #[br(parse_with = until_eof)]
    lvl_moves: Vec<LevelUpMove>,
}

#[allow(non_camel_case_types)]
#[derive(Serialize)]
enum Method {
    lvl,
    tm,
    tutor,
    _egg,
}

#[serde_with::skip_serializing_none]
#[derive(Serialize)]
struct LearnsetEntry {
    #[serde[rename = "move"]]
    move_: String,
    how: Method,
    level: Option<i16>,
}

#[derive(Serialize)]
struct Learnset(Vec<LearnsetEntry>);

impl Learnset {
    fn merge(mut self, mut other: Learnset) -> Self {
        self.0.append(&mut other.0);
        self
    }
}

pub fn dump_learnsets(
    rom_path: &Path,
    out_path: &Path,
    text_files: &[TextFile],
    poke_names: &BTreeMap<usize, String>,
) -> Result<()> {
    let move_names = &text_files[text_ids::MOVE_NAMES].lines;
    let lvl_path = rom_path
        .join(garc_files::BASE_PATH)
        .join(garc_files::LVL_UP_MOVES);
    let lvl_ups = garc::read_files::<LevelUpMoves>(&GarcFile::read_le(&mut File::open(lvl_path)?)?);

    let pokemon_path = rom_path
        .join(garc_files::BASE_PATH)
        .join(garc_files::POKEMON_STATS);
    let pokemons =
        garc::read_files::<PokemonStats>(&GarcFile::read_le(&mut File::open(pokemon_path)?)?);
    let mut learnset_map: IndexMap<String, Learnset> = lvl_ups
        .iter()
        .enumerate()
        .filter(|(index, _)| poke_names.contains_key(index))
        .map(|(index, lvl_ups)| {
            (
                to_id(poke_names[&index].to_owned()),
                make_lvl_up_learnset(lvl_ups, move_names)
                    .merge(make_tm_learnset(&pokemons[index], move_names))
                    .merge(make_beach_learnset(&pokemons[index], move_names))
                    .merge(make_tutor_learnset(&pokemons[index], move_names)),
            )
        })
        .collect();
    manual_patches(&mut learnset_map);
    let mut f = File::create(out_path.join("learnsets.json"))?;
    write!(f, "{}", serde_json::to_string_pretty(&learnset_map)?)?;
    Ok(())
}

fn make_lvl_up_learnset(lvl_ups: &LevelUpMoves, move_names: &[String]) -> Learnset {
    Learnset(
        lvl_ups
            .lvl_moves
            .iter()
            .take_while(|lvl_up| lvl_up.move_id > 0)
            .map(|lvl_up| LearnsetEntry {
                move_: to_id(move_names[lvl_up.move_id as usize].to_owned()),
                how: Method::lvl,
                level: Some(lvl_up.level),
            })
            .collect(),
    )
}

const TMS: &[&str] = &[
    "Work Up",
    "Dragon Dance",
    "Take Down",
    "Psychic Fangs",
    "Weather Ball",
    "Earthquake",
    "Ice Fang",
    "Power-Up Punch",
    "Venoshock",
    "Hidden Power",
    "Fire Fang",
    "Nasty Plot",
    "Ice Beam",
    "Blizzard",
    "Rest",
    "Light Screen",
    "Sleep Talk",
    "Rain Dance",
    "Electric Terrain",
    "Sunny Day",
    "Solar Beam",
    "Energy Ball",
    "Rock Tomb",
    "Megaton Kick",
    "Thunder",
    "Thunderbolt",
    "Solar Blade",
    "Rock Slide",
    "Retaliate",
    "Swords Dance",
    "Grassy Terrain",
    "Scorching Sands",
    "Reflect",
    "Sludge Bomb",
    "Close Combat",
    "Sludge Wave",
    "Charge Beam",
    "Fire Blast",
    "Burning Malice",
    "Substitute",
    "Taunt",
    "Will-O-Wisp",
    "Synchronoise",
    "Agility",
    "Sucker Punch",
    "Grass Knot",
    "Mystical Fire",
    "Ominous Wind",
    "Endure",
    "Flamethrower",
    "Smart Strike",
    "Aura Sphere",
    "Power Whip",
    "Brick Break",
    "Hydro Pump",
    "Hone Claws",
    "Belch",
    "Steel Wing",
    "Dark Pulse",
    "Parting Shot",
    "Megahorn",
    "Play Rough",
    "Flash Cannon",
    "Bulk Up",
    "Shadow Punch",
    "Blaze Kick",
    "Seismic Fist",
    "Giga Impact",
    "Sandstorm",
    "Hail",
    "Volt Switch",
    "Acrobatics",
    "Natural Gift",
    "Rock Polish",
    "Toxic Spikes",
    "Surf",
    "Poison Fang",
    "Thunder Fang",
    "Aurora Veil",
    "Rock Climb",
    "Wild Charge",
    "Lunge",
    "Bulldoze",
    "Poison Jab",
    "Calm Mind",
    "Nature Power",
    "Hex",
    "Rage",
    "U-turn",
    "Hyper Beam",
    "Strength",
    "Psychic",
    "Stone Edge",
    "Roost",
    "First Impression",
    "Dazzling Gleam",
    "Shadow Ball",
    "Hurricane",
    "Focus Blast",
    "Protect",
];

fn check_bit(bits: &[u8], index: usize) -> bool {
    let byte = index / 8;
    let bit = 1 << (index % 8);

    bits[byte] & bit != 0
}

fn make_tm_learnset(pokemon: &PokemonStats, _move_names: &[String]) -> Learnset {
    Learnset(
        TMS.iter()
            .enumerate()
            .filter_map(|(index, name)| match check_bit(&pokemon.tm_bits, index) {
                true => Some(LearnsetEntry {
                    move_: to_id(name.to_string()),
                    how: Method::tm,
                    level: None,
                }),
                false => None,
            })
            .collect(),
    )
}

#[allow(clippy::zero_prefixed_literal)]
const BEACH_TUTORS: &[u16] = &[
    450, 343, 162, 530, 324, 442, 402, 529, 340, 067, 441, 253, 009, 007, 008, 277, 335, 414, 492,
    356, 393, 334, 387, 276, 527, 196, 401, 428, 406, 304, 231, 020, 173, 282, 235, 257, 272, 215,
    366, 143, 220, 202, 409, 264, 351, 352, 380, 388, 180, 495, 270, 271, 478, 472, 283, 200, 278,
    289, 446, 285, 477, 502, 432, 710, 707, 675, 673,
];

fn make_beach_learnset(pokemon: &PokemonStats, move_names: &[String]) -> Learnset {
    Learnset(
        BEACH_TUTORS
            .iter()
            .enumerate()
            .filter_map(
                |(index, move_id)| match check_bit(&pokemon.beach_bits, index) {
                    true => Some(LearnsetEntry {
                        move_: to_id(move_names[*move_id as usize].to_owned()),
                        how: Method::tutor,
                        level: None,
                    }),
                    false => None,
                },
            )
            .collect(),
    )
}

const MOVE_TUTORS: &[u16] = &[520, 519, 518, 338, 307, 308, 434, 620];
fn make_tutor_learnset(pokemon: &PokemonStats, move_names: &[String]) -> Learnset {
    Learnset(
        MOVE_TUTORS
            .iter()
            .enumerate()
            .filter_map(
                |(index, move_id)| match check_bit(&pokemon.tutor_bits, index) {
                    true => Some(LearnsetEntry {
                        move_: to_id(move_names[*move_id as usize].to_owned()),
                        how: Method::tutor,
                        level: None,
                    }),
                    false => None,
                },
            )
            .collect(),
    )
}

fn manual_patches(learnset_map: &mut IndexMap<String, Learnset>) {
    const COMBAT_FORMES: &[&str] = &[
        "minior",
        "venusaurmega",
        "charizardmegax",
        "charizardmegay",
        "blastoisemega",
        "beedrillmega",
        "pidgeotmega",
        "alakazammega",
        "slowbromega",
        "gengarmega",
        "kangaskhanmega",
        "pinsirmega",
        "gyaradosmega",
        "aerodactylmega",
        "ampharosmega",
        "steelixmega",
        "scizormega",
        "heracrossmega",
        "houndoommega",
        "tyranitarmega",
        "sceptilemega",
        "blazikenmega",
        "swampertmega",
        "gardevoirmega",
        "sableyemega",
        "mawilemega",
        "aggronmega",
        "medichammega",
        "manectricmega",
        "sharpedomega",
        "cameruptmega",
        "altariamega",
        "banettemega",
        "absolmega",
        "glaliemega",
        "salamencemega",
        "metagrossmega",
        "latiasmega",
        "latiosmega",
        "lopunnymega",
        "garchompmega",
        "lucariomega",
        "abomasnowmega",
        "gallademega",
        "audinomega",
        "dianciemega",
        "mimikyubusted",
        "mimikyubustedtotem",
        "wishiwashischool",
        "shayminsky",
        "darmanitanzen",
        "greninjaash",
        "kyuremwhite",
        "kyuremblack",
        "aegislashblade",
        "meloettapirouette",
        "cherrimsunshine",
        "castformsunny",
        "castformsnowy",
        "castformrainy",
    ];

    for combat_forme in COMBAT_FORMES {
        learnset_map.shift_remove(*combat_forme);
    }

    for remove in pokemon::UNUSABLES {
        learnset_map.shift_remove(*remove);
    }
}
