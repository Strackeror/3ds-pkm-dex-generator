use std::io::Cursor;

use binrw::BinRead;

#[derive(BinRead, Debug)]
#[br(magic = b"CRAG")]
struct GarcHeader {
    _header_size: u32,
    _endian: u16,
    _version: u16,
    _chunk_count: u32,
    _data_offset: u32,
    _file_size: u32,

    #[br(pad_before = _header_size - 0x18)]
    _end: (),
}

#[derive(BinRead, Debug)]
#[br(magic = b"OTAF")]
struct FileAllocationTableOffsets {
    _header_size: u32,
    _entry_count: u16,
    #[br(pad_before = 2, count = _entry_count)]
    _entries: Vec<u32>,
}

#[derive(BinRead, Debug, Clone, Copy)]
struct FileSubEntry {
    start: u32,
    end: u32,
    _length: u32,
}

#[derive(Debug)]
struct FileEntry {
    _entry_bits: u32,
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
            _entry_bits: entry_bits,
            entries,
        })
    }
}

#[derive(BinRead, Debug)]
#[br(magic = b"BTAF")]
struct FileAllocationTableBits {
    _header_size: u32,
    _file_count: u32,

    #[br(count = _file_count)]
    file_entries: Vec<FileEntry>,
}

#[derive(BinRead, Debug)]
#[br(magic = b"BMIF")]
struct FileImageBytes {
    _header_size: u32,
    _data_size: u32,
    #[br(count = _data_size)]
    data: Vec<u8>,
}

#[derive(BinRead, Debug)]
pub struct GarcFile {
    _header: GarcHeader,
    _fato: FileAllocationTableOffsets,
    fatb: FileAllocationTableBits,
    fimb: FileImageBytes,
}

pub fn _read_file<T: BinRead>(file: usize, subfile: usize, garc: &GarcFile) -> Option<T>
where
    for<'a> <T as binrw::BinRead>::Args<'a>: std::default::Default,
{
    let file_entry = garc.fatb.file_entries[file].entries[subfile]?;
    let file_bytes = &garc.fimb.data[file_entry.start as usize..file_entry.end as usize];
    T::read_le(&mut Cursor::new(file_bytes)).ok()
}

pub fn read_files<T: BinRead>(garc: &GarcFile) -> Vec<T>
where
    for<'a> <T as binrw::BinRead>::Args<'a>: std::default::Default,
{
    garc.fatb
        .file_entries
        .iter()
        .map(|e| e.entries[0].unwrap())
        .map(|sub_entry| {
            let file_bytes = &garc.fimb.data[sub_entry.start as usize..sub_entry.end as usize];
            T::read_le(&mut Cursor::new(file_bytes)).unwrap()
        })
        .collect()
}
