use std::{env, fs::File, io::Cursor, path::Path};

use binrw::BinRead;

#[derive(BinRead, Debug)]
#[br(magic = b"CRAG")]
struct GarcHeader {
    header_size: u32,
    endian: u16,
    version: u16,
    chunk_count: u32,
    data_offset: u32,
    file_size: u32,

    #[br(pad_before = header_size - 0x18)]
    _end: (),
}

#[derive(BinRead, Debug)]
#[br(magic = b"OTAF")]
struct FileAllocationTableOffsets {
    header_size: u32,
    entry_count: u16,
    #[br(pad_before = 2, count = entry_count)]
    entries: Vec<u32>,
}

#[derive(BinRead, Debug, Clone, Copy)]
struct FileSubEntry {
    start: u32,
    end: u32,
    length: u32,
}

#[derive(Debug)]
struct FileEntry {
    entry_bits: u32,
    entries: [Option<FileSubEntry>; 32],
}

impl BinRead for FileEntry {
    type Args<'a> = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let entry_bits = u32::read_options(reader, endian, ())?;
        let mut entries: [Option<FileSubEntry>; 32] = Default::default();
        for (index, entry) in entries.iter_mut().enumerate() {
            if (entry_bits & (1 << index)) != 0 {
                *entry = Some(FileSubEntry::read_options(reader, endian, ())?);
            }
        }
        Ok(Self {
            entry_bits,
            entries,
        })
    }
}

#[derive(BinRead, Debug)]
#[br(magic = b"BTAF")]
struct FileAllocationTableBits {
    header_size: u32,
    file_count: u32,

    #[br(count = file_count)]
    file_entries: Vec<FileEntry>,
}

#[derive(BinRead, Debug)]
#[br(magic = b"BMIF")]
struct FileImageBytes {
    header_size: u32,
    data_size: u32,
    #[br(count = data_size)]
    data: Vec<u8>,
}

#[derive(BinRead, Debug)]
struct GarcFile {
    header: GarcHeader,
    fato: FileAllocationTableOffsets,
    fatb: FileAllocationTableBits,
    fimb: FileImageBytes,
}

#[derive(BinRead, Debug)]
struct Stats {
    hp: u8,
    atk: u8,
    def: u8,
    spe: u8,
    spa: u8,
    spd: u8,
}

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

mod text {
    use binrw::BinRead;

    #[derive(BinRead, Debug)]
    struct TextFileHeader {
        text_sections: u16,
        line_count: u16,
        total_length: u32,
        intial_key: u32,
        section_data_offset: u32,
        section_length: u32,
    }
    #[derive(BinRead, Debug)]
    struct LineInfo {
        offset: u32,
        length: u32,
    }

    #[derive(BinRead)]
    #[br(import(len: u32))]
    struct EncryptedLine {
        #[br(count = len)]
        data: Vec<u16>,
    }

    impl EncryptedLine {
        fn into_string(self, mut key: u16) -> String {
            return self
                .data
                .iter()
                .map_while(|u| {
                    let c = std::char::from_u32((*u ^ key) as u32).unwrap_or(' ');
                    key = key << 3 | key >> 13;
                    if c == '\0' {
                        None
                    } else {
                        Some(c)
                    }
                })
                .collect();
        }
    }

    const KEY_BASE: u16 = 0x7c89;
    const KEY_ADVANCE: u16 = 0x2983;

    #[derive(Debug)]
    pub struct TextFile {
        header: TextFileHeader,
        pub lines: Vec<String>,
    }

    impl BinRead for TextFile {
        type Args<'a> = ();

        fn read_options<R: std::io::Read + std::io::Seek>(
            reader: &mut R,
            endian: binrw::Endian,
            _: Self::Args<'_>,
        ) -> binrw::BinResult<Self> {
            let header = TextFileHeader::read_options(reader, endian, ())?;
            let mut lines: Vec<String> = Vec::new();
            let mut key = KEY_BASE;
            reader.seek(std::io::SeekFrom::Start(
                header.section_data_offset as u64 + 4,
            ))?;
            for _ in 0..header.line_count {
                let line_info = LineInfo::read_options(reader, endian, ())?;
                println!("{:x?}", line_info);
                let pos = reader.stream_position()?;
                reader.seek(std::io::SeekFrom::Start(
                    line_info.offset as u64 + header.section_data_offset as u64,
                ))?;
                lines.push(
                    EncryptedLine::read_options(reader, endian, (line_info.length,))?
                        .into_string(key),
                );
                reader.seek(std::io::SeekFrom::Start(pos))?;
                key = key.wrapping_add(KEY_ADVANCE);
            }

            Ok(TextFile { header, lines })
        }
    }
}

fn read_file<T: BinRead>(file: usize, subfile: usize, garc: &GarcFile) -> Option<T>
where
    for<'a> <T as binrw::BinRead>::Args<'a>: std::default::Default,
{
    let file_entry = garc.fatb.file_entries[file].entries[subfile]?;
    let file_bytes = &garc.fimb.data[file_entry.start as usize..file_entry.end as usize];
    T::read_le(&mut Cursor::new(file_bytes)).ok()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = Path::new(&args[1]);
    let pokemon_stats_file = path.join("romfs/a/0/1/7");
    let mut file = File::open(pokemon_stats_file).unwrap();
    let garc_file = GarcFile::read_le(&mut file).unwrap();
    let stats = read_file::<PokemonStats>(1, 0, &garc_file);
    println!("{:?}", stats);

    let mut text_file = File::open(path.join("romfs/a/0/3/2")).unwrap();
    let text_garc_file = GarcFile::read_le(&mut text_file).unwrap();
    let text_file = read_file::<text::TextFile>(102, 0, &text_garc_file).unwrap();
    println!("{:?}", text_file);
}
