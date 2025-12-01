#![allow(unused)]
#![allow(clippy::unit_arg)]

use std::io::Read;

use binrw::{BinRead, FilePtr32, NullString, file_ptr::FilePtrArgs, helpers::until_exclusive};

#[derive(BinRead, Debug)]
#[br(little, magic = b"-Furnace module-")]
pub struct FurHeader {
    #[br(assert(version >= 232))]
    pub version: u16,
    #[br(pad_before = 2, pad_after = 8)]
    pub pointer: FilePtr32<FurInfoBlock>,
}

#[derive(Debug)]
struct FurSoundChips;
impl BinRead for FurSoundChips {
    type Args<'a> = ();

    fn read_options<R: Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let bytes = <[u8; 32]>::read_options(reader, endian, args)?;
        assert_eq!(bytes[0], 0x9c);
        assert_eq!(bytes[1..], [0; 31]);
        Ok(Self)
    }
}

#[derive(BinRead, Debug)]
#[br(little)]
pub struct FurInfoBlock {
    id: u32,
    pub size: u32,
    pub time_base: u8,
    pub speed_1: u8,
    pub speed_2: u8,
    pub arpeggio_time: u8,
    pub ticks_per_second: f32,
    pub pattern_length: u16,
    pub orders_length: u16,
    pub highlight_a: u8,
    pub highlight_b: u8,
    instrument_count: u16,
    wavetable_count: u16,
    sample_count: u16,
    pattern_count: u32,
    #[br(pad_after = 192)]
    sound_chips: FurSoundChips,
    song_name: NullString,
    song_author: NullString,
    #[br(pad_after = 20)]
    tuning: f32,
    #[br(count = instrument_count)]
    pub instruments: Vec<FilePtr32<FurInstrument>>,
    #[br(count = wavetable_count)]
    pub wavetables: Vec<FilePtr32<FurWavetable>>,
    #[br(count = sample_count)]
    pub samples: Vec<u32>,
    #[br(count = pattern_count, args { inner: FilePtrArgs { offset: 0, inner: FurPatternBinReadArgs { pattern_length } } })]
    pub patterns: Vec<FilePtr32<FurPattern>>,
    #[br(count = orders_length)]
    pub orders: [Vec<u8>; 6],
    effect_columns: [u8; 6],
    channel_hide_status: [u8; 6],
    channel_collapse_status: [u8; 6],
    channel_names: [NullString; 6],
    channel_short_names: [NullString; 6],
    song_comment: NullString,
    #[br(pad_after = 28)]
    master_volume: f32,
    virtual_tempo_numerator: u16,
    virtual_tempo_denominator: u16,
    first_subsong_name: NullString,
    first_subsong_comment: NullString,
    #[br(pad_after = 3)]
    additional_subsong_count: u8,
    #[br(count = additional_subsong_count)]
    pub subsong_pointers: Vec<u32>,
}

#[derive(BinRead, Debug)]
#[br(little, magic = b"INS2")]
pub struct FurInstrument {
    size: u32,
    #[br(assert(version >= 232))]
    version: u16,
    #[br(assert(instrument_type == 16))]
    instrument_type: u16,
    #[br(parse_with = until_exclusive(|e: &FurFeatureEnvelope| e.is_end()), map = |e: Vec<FurFeatureEnvelope>| e.into_iter().map(|e| e.feature).collect())]
    pub features: Vec<FurFeature>,
}
impl FurInstrument {
    pub fn wavetable_synth_data(&self) -> Option<&FurWavetableSynthData> {
        self.features.iter().find_map(|f| match f {
            FurFeature::WavetableSynthData(data) => Some(data),
            _ => None,
        })
    }
    pub fn macros(&self) -> Option<&[FurMacro]> {
        self.features.iter().find_map(|f| match f {
            FurFeature::MacroData(data) => Some(data.as_slice()),
            _ => None,
        })
    }
}

#[derive(BinRead, Debug)]
#[br(little)]
struct FurFeatureEnvelope {
    feature_code: u16,
    length: u16,
    #[br(args(&feature_code.to_le_bytes(), length))]
    feature: FurFeature,
}
impl FurFeatureEnvelope {
    fn is_end(&self) -> bool {
        matches!(self.feature, FurFeature::End)
    }
}

#[derive(BinRead, Debug)]
#[br(little, import(code: &[u8; 2], length: u16))]
pub enum FurFeature {
    #[br(pre_assert(code == b"NA"))]
    InstrumentName(NullString),
    #[br(pre_assert(code == b"MA"))]
    MacroData(
        #[br(pad_before = 2, parse_with = until_exclusive(|m: &FurMacro| m.is_end()))]
        Vec<FurMacro>,
    ),
    #[br(pre_assert(code == b"WS"))]
    WavetableSynthData(FurWavetableSynthData),
    #[br(pre_assert(code == b"EN"))]
    End,
    Unknown {
        #[br(calc = code.to_owned())]
        code: [u8; 2],
        #[br(count = length)]
        data: Vec<u8>,
    },
}

#[derive(BinRead, Debug)]
#[br(little)]
pub enum FurMacro {
    #[br(magic = 0u8)]
    Volume(FurMacroBody<u8>),
    #[br(magic = 1u8)]
    Arpeggio(FurMacroBody<i8>),
    #[br(magic = 255u8)]
    End,
}
impl FurMacro {
    fn is_end(&self) -> bool {
        matches!(self, Self::End)
    }
}

#[derive(BinRead, Debug)]
#[br(little)]
pub struct FurMacroBody<T>
where
    T: BinRead + 'static,
    for<'a> <T as BinRead>::Args<'a>: Default + Clone,
{
    macro_length: u8,
    pub macro_loop: i8,
    macro_release: i8,
    macro_mode: u8,
    macro_flags: u8,
    pub macro_delay: u8,
    pub macro_speed: u8,
    #[br(count = macro_length)]
    pub data: Vec<T>,
}

#[derive(BinRead, Debug)]
#[br(little)]
pub struct FurWavetableSynthData {
    pub first_wave: u32,
    second_wave: u32,
    rate_divider: u8,
    effect: u8,
    enabled: u8,
    global: u8,
    speed: u8,
    params: [u8; 4],
}

#[derive(BinRead, Debug)]
#[br(little, magic = b"WAVE")]
pub struct FurWavetable {
    size: u32,
    name: NullString,
    #[br(pad_after = 4)]
    width: u32,
    height: u32,
    #[br(count = width)]
    pub data: Vec<u32>,
}

#[derive(BinRead, Debug)]
#[br(little, magic = b"PATN", import { pattern_length: u16 })]
pub struct FurPattern {
    size: u32,
    subsong: u8,
    pub channel: u8,
    pub index: u16,
    name: NullString,
    #[br(parse_with = pattern_parser, args (pattern_length))]
    pub data: Vec<FurPatternRow>,
}

#[binrw::parser(reader, endian)]
fn effect_parser(bits: u8) -> binrw::BinResult<Option<(u8, u8)>> {
    if bits & 0x01 == 0 {
        return Ok(None);
    }
    let effect = u8::read_options(reader, endian, ())?;
    let value = if bits & 0x02 != 0 {
        u8::read_options(reader, endian, ())?
    } else {
        0
    };
    Ok(Some((effect, value)))
}

#[binrw::parser(reader, endian)]
fn pattern_parser(pattern_length: u16) -> binrw::BinResult<Vec<FurPatternRow>> {
    let mut index = 0;
    let mut result = vec![];
    while index < pattern_length as u64 {
        let byte = u8::read_options(reader, endian, ())?;
        if byte == 0xff {
            break;
        }
        if byte & 0x80 != 0 {
            let skip = (byte & 0x7f) + 2;
            index += skip as u64;
            continue;
        }
        if byte == 0 {
            index += 1;
            continue;
        }
        let fx1 = if byte & 0x20 != 0 {
            u8::read_options(reader, endian, ())?
        } else {
            0
        };
        let fx2 = if byte & 0x40 != 0 {
            u8::read_options(reader, endian, ())?
        } else {
            0
        };
        let mut row = FurPatternRow {
            index,
            note: None,
            instrument: None,
            volume: None,
            effects: vec![],
        };
        if byte & 0x01 != 0 {
            let note = u8::read_options(reader, endian, ())?;
            row.note = Some(note);
        }
        if byte & 0x02 != 0 {
            let instrument = u8::read_options(reader, endian, ())?;
            row.instrument = Some(instrument);
        }
        if byte & 0x04 != 0 {
            let volume = u8::read_options(reader, endian, ())?;
            row.volume = Some(volume);
        }
        if let Some(effect) = effect_parser(reader, endian, ((byte >> 3) & 0x03,))? {
            row.effects.push(effect);
        }
        if let Some(effect) = effect_parser(reader, endian, (fx1 & 0x03,))? {
            row.effects.push(effect);
        }
        if let Some(effect) = effect_parser(reader, endian, (fx1 >> 2 & 0x03,))? {
            row.effects.push(effect);
        }
        if let Some(effect) = effect_parser(reader, endian, (fx1 >> 4 & 0x03,))? {
            row.effects.push(effect);
        }
        if let Some(effect) = effect_parser(reader, endian, (fx1 >> 6 & 0x03,))? {
            row.effects.push(effect);
        }
        if let Some(effect) = effect_parser(reader, endian, (fx2 & 0x03,))? {
            row.effects.push(effect);
        }
        if let Some(effect) = effect_parser(reader, endian, (fx2 >> 2 & 0x03,))? {
            row.effects.push(effect);
        }
        if let Some(effect) = effect_parser(reader, endian, (fx2 >> 4 & 0x03,))? {
            row.effects.push(effect);
        }
        if let Some(effect) = effect_parser(reader, endian, (fx2 >> 6 & 0x03,))? {
            row.effects.push(effect);
        }
        result.push(row);
        index += 1;
    }
    Ok(result)
}

#[derive(Debug)]
pub struct FurPatternRow {
    pub index: u64,
    pub note: Option<u8>,
    pub instrument: Option<u8>,
    pub volume: Option<u8>,
    pub effects: Vec<(u8, u8)>,
}
