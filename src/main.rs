use std::{
    collections::{BTreeMap, HashMap},
    env,
    fs::File,
    io::Write,
    path::Path,
    vec,
};

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
#[serde(untagged)]
enum PokemonJsGenderRatio {
    Ratio { M: f32, F: f32 },
    All(String),
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

    baseSpecies: Option<String>,
    forme: Option<String>,
    otherFormes: Option<Vec<String>>,
    formeOrder: Option<Vec<String>>,
}

fn to_id(s: String) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_uppercase() || c.is_ascii_digit())
        .collect()
}

fn dump_pokes(
    rom_path: &Path,
    out_path: &Path,
    text_files: &[TextFile],
) -> Result<BTreeMap<usize, String>> {
    const NORMAL_FORME_COUNT: usize = 808;
    let mut dex_map: BTreeMap<usize, PokemonJs> = BTreeMap::new();

    let pokemon_path = rom_path
        .join(garc_files::BASE_PATH)
        .join(garc_files::POKEMON_STATS);
    let pokemons =
        garc::read_files::<PokemonStats>(&GarcFile::read_le(&mut File::open(pokemon_path)?)?);

    let species_names = &text_files[text_ids::SPECIES_NAMES].lines;
    let ability_names = &text_files[text_ids::ABILITY_NAMES].lines;
    let type_names = &text_files[text_ids::TYPE_NAMES].lines;
    let item_names = &text_files[text_ids::ITEM_NAMES].lines;

    for (index, pokemon) in pokemons.iter().take(NORMAL_FORME_COUNT).enumerate() {
        let name = &species_names[index];
        let poke = make_poke(pokemon, type_names, ability_names, index, name);
        dex_map.insert(index, poke);
    }

    for (base_index, pokemon) in pokemons.iter().take(NORMAL_FORME_COUNT).enumerate() {
        if pokemon.form_count <= 1 || (pokemon.form_stats_id as usize) < NORMAL_FORME_COUNT {
            continue;
        }
        let base_name = &species_names[base_index];
        let mut other_formes: Vec<String> = vec![];
        let mut forme_order: Vec<String> = vec![base_name.to_owned()];
        for form_id in 1..pokemon.form_count {
            let index = pokemon.form_stats_id as usize + form_id as usize - 1;
            let forme_name =
                get_forme_name(base_name, form_id as _).unwrap_or_else(|| form_id.to_string());
            let name = format!("{}-{}", base_name, forme_name);
            other_formes.push(name.clone());
            forme_order.push(name.clone());
            let pokemon_forme = &pokemons[index];
            let mut poke = make_poke(pokemon_forme, type_names, ability_names, index, &name);
            poke.num = base_index as _;
            poke.forme = Some(forme_name.to_owned());
            poke.baseSpecies = Some(base_name.clone());
            dex_map.insert(index, poke);
        }

        if let Some(dex) = dex_map.get_mut(&base_index) {
            dex.otherFormes = Some(other_formes);
            dex.formeOrder = Some(forme_order)
        }
    }

    let evo_path = rom_path
        .join(garc_files::BASE_PATH)
        .join(garc_files::EVOLUTIONS);
    let evolutions =
        garc::read_files::<[PokemonEvolution; 8]>(&GarcFile::read_le(&mut File::open(evo_path)?)?);
    handle_evos(evolutions, item_names, &mut dex_map);

    let name_map = dex_map.iter().map(|(i, s)| (*i, s.name.clone())).collect();
    let dex_map: IndexMap<String, PokemonJs> = dex_map
        .into_values()
        .skip(1) // Skip Egg
        .map(|dex| (to_id(dex.name.clone()), dex))
        .collect();

    let mut f = File::create(out_path.join("pokedex.js"))?;
    write!(
        f,
        "exports.BattlePokedex = {}",
        serde_json::to_string_pretty(&dex_map)?
    )?;
    Ok(name_map)
}

const EGG_GROUPS: &[&str] = &[
    "---",
    "Monster",
    "Water 1",
    "Bug",
    "Flying",
    "Field",
    "Fairy",
    "Grass",
    "Human-Like",
    "Water 3",
    "Mineral",
    "Amorphous",
    "Water 2",
    "Ditto",
    "Dragon",
    "Undiscovered",
];

fn make_poke(
    pokemon: &PokemonStats,
    type_names: &[String],
    ability_names: &[String],
    index: usize,
    name: &str,
) -> PokemonJs {
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
    if pokemon.abilities[2] != pokemon.abilities[0] && pokemon.abilities[2] != pokemon.abilities[1]
    {
        abilities.insert(
            "H".to_owned(),
            ability_names[pokemon.abilities[2] as usize].clone(),
        );
    }

    #[allow(non_snake_case)]
    let mut eggGroups: Vec<String> = pokemon
        .egg_groups
        .iter()
        .map(|id| EGG_GROUPS[*id as usize].to_owned())
        .collect();
    eggGroups.dedup();

    let gender = match pokemon.gender {
        0 => PokemonJsGenderRatio::All("M".to_owned()),
        254 => PokemonJsGenderRatio::All("F".to_owned()),
        255 => PokemonJsGenderRatio::All("N".to_owned()),
        g => PokemonJsGenderRatio::Ratio {
            M: (256. - (g + 1) as f32) / 256.,
            F: ((g + 1) as f32 / 256.),
        },
    };
    PokemonJs {
        num: index as _,
        name: name.to_owned(),
        types,
        genderRatio: gender,
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
        eggGroups,
        baseSpecies: None,
        forme: None,
        otherFormes: None,
        formeOrder: None,
    }
}

const FORME_NAMES: &[((&str, usize), &str)] = &[
    (("Venusaur", 1), "Mega"),
    (("Charizard", 1), "Mega-X"),
    (("Charizard", 2), "Mega-Y"),
    (("Blastoise", 1), "Mega"),
    (("Beedrill", 1), "Mega"),
    (("Pidgeot", 1), "Mega"),
    (("Rattata", 1), "Alola"),
    (("Raticate", 1), "Alola"),
    (("Raticate", 2), "Alola-Totem"),
    (("Pikachu", 1), "Cosplay"),
    (("Pikachu", 2), "Rock-Star"),
    (("Pikachu", 3), "Belle"),
    (("Pikachu", 4), "Pop-Star"),
    (("Pikachu", 5), "PhD"),
    (("Pikachu", 6), "Libre"),
    (("Pikachu", 7), "Original"),
    (("Pikachu", 8), "Hoenn"),
    (("Pikachu", 9), "Sinnoh"),
    (("Pikachu", 10), "Unova"),
    (("Pikachu", 11), "Kalos"),
    (("Pikachu", 12), "Alola"),
    (("Pikachu", 13), "Partner"),
    (("Pikachu", 14), "Starter"),
    (("Pikachu", 15), "World"),
    (("Raichu", 1), "Alola"),
    (("Sandshrew", 1), "Alola"),
    (("Sandslash", 1), "Alola"),
    (("Vulpix", 1), "Alola"),
    (("Ninetales", 1), "Alola"),
    (("Diglett", 1), "Alola"),
    (("Dugtrio", 1), "Alola"),
    (("Meowth", 1), "Alola"),
    (("Meowth", 2), "Galar"),
    (("Persian", 1), "Alola"),
    (("Growlithe", 1), "Hisui"),
    (("Arcanine", 1), "Hisui"),
    (("Alakazam", 1), "Mega"),
    (("Geodude", 1), "Alola"),
    (("Graveler", 1), "Alola"),
    (("Golem", 1), "Alola"),
    (("Ponyta", 1), "Galar"),
    (("Rapidash", 1), "Galar"),
    (("Slowpoke", 1), "Galar"),
    (("Slowbro", 1), "Mega"),
    (("Slowbro", 2), "Galar"),
    (("Farfetch’d", 1), "Galar"),
    (("Grimer", 1), "Alola"),
    (("Muk", 1), "Alola"),
    (("Gengar", 1), "Mega"),
    (("Voltorb", 1), "Hisui"),
    (("Electrode", 1), "Hisui"),
    (("Exeggutor", 1), "Alola"),
    (("Marowak", 1), "Alola"),
    (("Marowak", 2), "Alola-Totem"),
    (("Weezing", 1), "Galar"),
    (("Kangaskhan", 1), "Mega"),
    (("Mr. Mime", 1), "Galar"),
    (("Pinsir", 1), "Mega"),
    (("Tauros", 1), "Paldea-Combat"),
    (("Tauros", 2), "Paldea-Blaze"),
    (("Tauros", 3), "Paldea-Aqua"),
    (("Gyarados", 1), "Mega"),
    (("Eevee", 1), "Starter"),
    (("Aerodactyl", 1), "Mega"),
    (("Articuno", 1), "Galar"),
    (("Zapdos", 1), "Galar"),
    (("Moltres", 1), "Galar"),
    (("Mewtwo", 1), "Mega-X"),
    (("Mewtwo", 2), "Mega-Y"),
    (("Typhlosion", 1), "Hisui"),
    (("Pichu", 1), "Spiky-eared"),
    (("Ampharos", 1), "Mega"),
    (("Wooper", 1), "Paldea"),
    (("Slowking", 1), "Galar"),
    (("Steelix", 1), "Mega"),
    (("Qwilfish", 1), "Hisui"),
    (("Scizor", 1), "Mega"),
    (("Heracross", 1), "Mega"),
    (("Sneasel", 1), "Hisui"),
    (("Corsola", 1), "Galar"),
    (("Houndoom", 1), "Mega"),
    (("Tyranitar", 1), "Mega"),
    (("Sceptile", 1), "Mega"),
    (("Blaziken", 1), "Mega"),
    (("Swampert", 1), "Mega"),
    (("Zigzagoon", 1), "Galar"),
    (("Linoone", 1), "Galar"),
    (("Gardevoir", 1), "Mega"),
    (("Sableye", 1), "Mega"),
    (("Mawile", 1), "Mega"),
    (("Aggron", 1), "Mega"),
    (("Medicham", 1), "Mega"),
    (("Manectric", 1), "Mega"),
    (("Sharpedo", 1), "Mega"),
    (("Camerupt", 1), "Mega"),
    (("Altaria", 1), "Mega"),
    (("Castform", 1), "Sunny"),
    (("Castform", 2), "Rainy"),
    (("Castform", 3), "Snowy"),
    (("Banette", 1), "Mega"),
    (("Absol", 1), "Mega"),
    (("Glalie", 1), "Mega"),
    (("Salamence", 1), "Mega"),
    (("Metagross", 1), "Mega"),
    (("Latias", 1), "Mega"),
    (("Latios", 1), "Mega"),
    (("Kyogre", 1), "Primal"),
    (("Groudon", 1), "Primal"),
    (("Rayquaza", 1), "Mega"),
    (("Deoxys", 1), "Attack"),
    (("Deoxys", 2), "Defense"),
    (("Deoxys", 3), "Speed"),
    (("Wormadam", 1), "Sandy"),
    (("Wormadam", 2), "Trash"),
    (("Cherrim", 1), "Sunshine"),
    (("Lopunny", 1), "Mega"),
    (("Garchomp", 1), "Mega"),
    (("Lucario", 1), "Mega"),
    (("Abomasnow", 1), "Mega"),
    (("Gallade", 1), "Mega"),
    (("Rotom", 1), "Heat"),
    (("Rotom", 2), "Wash"),
    (("Rotom", 3), "Frost"),
    (("Rotom", 4), "Fan"),
    (("Rotom", 5), "Mow"),
    (("Dialga", 1), "Origin"),
    (("Palkia", 1), "Origin"),
    (("Giratina", 1), "Origin"),
    (("Shaymin", 1), "Sky"),
    (("Arceus", 1), "Bug"),
    (("Arceus", 2), "Dark"),
    (("Arceus", 3), "Dragon"),
    (("Arceus", 4), "Electric"),
    (("Arceus", 5), "Fairy"),
    (("Arceus", 6), "Fighting"),
    (("Arceus", 7), "Fire"),
    (("Arceus", 8), "Flying"),
    (("Arceus", 9), "Ghost"),
    (("Arceus", 10), "Grass"),
    (("Arceus", 11), "Ground"),
    (("Arceus", 12), "Ice"),
    (("Arceus", 13), "Poison"),
    (("Arceus", 14), "Psychic"),
    (("Arceus", 15), "Rock"),
    (("Arceus", 16), "Steel"),
    (("Arceus", 17), "Water"),
    (("Samurott", 1), "Hisui"),
    (("Audino", 1), "Mega"),
    (("Lilligant", 1), "Hisui"),
    (("Basculin", 1), "Blue-Striped"),
    (("Basculin", 2), "White-Striped"),
    (("Darumaka", 1), "Galar"),
    (("Darmanitan", 1), "Zen"),
    (("Darmanitan", 2), "Galar"),
    (("Darmanitan", 3), "Galar-Zen"),
    (("Yamask", 1), "Galar"),
    (("Zorua", 1), "Hisui"),
    (("Zoroark", 1), "Hisui"),
    (("Stunfisk", 1), "Galar"),
    (("Braviary", 1), "Hisui"),
    (("Tornadus", 1), "Therian"),
    (("Thundurus", 1), "Therian"),
    (("Landorus", 1), "Therian"),
    (("Kyurem", 1), "Black"),
    (("Kyurem", 2), "White"),
    (("Keldeo", 1), "Resolute"),
    (("Meloetta", 1), "Pirouette"),
    (("Genesect", 1), "Douse"),
    (("Genesect", 2), "Shock"),
    (("Genesect", 3), "Burn"),
    (("Genesect", 4), "Chill"),
    (("Greninja", 1), "Ash"),
    (("Vivillon", 1), "Fancy"),
    (("Vivillon", 2), "Pokeball"),
    (("Floette", 1), "Eternal"),
    (("Meowstic", 1), "F"),
    (("Aegislash", 1), "Blade"),
    (("Sliggoo", 1), "Hisui"),
    (("Goodra", 1), "Hisui"),
    (("Pumpkaboo", 1), "Small"),
    (("Pumpkaboo", 2), "Large"),
    (("Pumpkaboo", 3), "Super"),
    (("Gourgeist", 1), "Small"),
    (("Gourgeist", 2), "Large"),
    (("Gourgeist", 3), "Super"),
    (("Avalugg", 1), "Hisui"),
    (("Xerneas", 1), "Neutral"),
    (("Zygarde", 1), "10%"),
    (("Zygarde", 2), "Unused-1"),
    (("Zygarde", 3), "Unused-2"),
    (("Zygarde", 4), "Complete"),
    (("Diancie", 1), "Mega"),
    (("Hoopa", 1), "Unbound"),
    (("Decidueye", 1), "Hisui"),
    (("Gumshoos", 1), "Totem"),
    (("Vikavolt", 1), "Totem"),
    (("Oricorio", 1), "Pom-Pom"),
    (("Oricorio", 2), "Pa'u"),
    (("Oricorio", 3), "Sensu"),
    (("Ribombee", 1), "Totem"),
    (("Lycanroc", 1), "Midnight"),
    (("Lycanroc", 2), "Dusk"),
    (("Wishiwashi", 1), "School"),
    (("Araquanid", 1), "Totem"),
    (("Lurantis", 1), "Totem"),
    (("Salazzle", 1), "Totem"),
    (("Silvally", 1), "Bug"),
    (("Silvally", 2), "Dark"),
    (("Silvally", 3), "Dragon"),
    (("Silvally", 4), "Electric"),
    (("Silvally", 5), "Fairy"),
    (("Silvally", 6), "Fighting"),
    (("Silvally", 7), "Fire"),
    (("Silvally", 8), "Flying"),
    (("Silvally", 9), "Ghost"),
    (("Silvally", 10), "Grass"),
    (("Silvally", 11), "Ground"),
    (("Silvally", 12), "Ice"),
    (("Silvally", 13), "Poison"),
    (("Silvally", 14), "Psychic"),
    (("Silvally", 15), "Rock"),
    (("Silvally", 16), "Steel"),
    (("Silvally", 17), "Water"),
    (("Minior", 1), "Meteor"),
    (("Togedemaru", 1), "Totem"),
    (("Mimikyu", 1), "Busted"),
    (("Mimikyu", 2), "Totem"),
    (("Mimikyu", 3), "Busted-Totem"),
    (("Kommo-o", 1), "Totem"),
    (("Necrozma", 1), "Dusk-Mane"),
    (("Necrozma", 2), "Dawn-Wings"),
    (("Necrozma", 3), "Ultra"),
];

fn get_forme_name(species: &str, forme_id: usize) -> Option<String> {
    FORME_NAMES
        .iter()
        .find(|((name, id), _)| **name == *species && *id == forme_id)
        .map(|(_, forme_name)| (*forme_name).to_owned())
}

fn handle_evos(
    evolutions: Vec<[PokemonEvolution; 8]>,
    item_names: &[String],
    dex_map: &mut BTreeMap<usize, PokemonJs>,
) {
    for (index, evo_list) in evolutions.iter().enumerate() {
        let mut evo_set: IndexSet<String> = IndexSet::new();
        let Some(current_name) = dex_map.get(&index).map(|d|d.name.to_owned()) else {
            continue;
        };

        for evo in evo_list {
            if evo.method == 0 {
                continue;
            }
            let Some(poke_entry) = dex_map.get_mut(&(evo.species as usize)) else {
                continue;
            };

            let evo_name = &poke_entry.name;
            evo_set.insert(evo_name.clone());
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
            dex_map.get_mut(&index).unwrap().evos = Some(evo_set.into_iter().collect());
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
        .skip(1)
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
        .skip(1)
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
    let names = dump_pokes(path, out_path, &text_files).unwrap();
    learnset::dump_learnsets(path, out_path, &text_files, &names).unwrap();
    dump_abilities(path, out_path, &text_files).unwrap();
    dump_moves(path, out_path, &text_files).unwrap();
}
