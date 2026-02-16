use crate::assets::{
    WaveformSetData,
    fur::{
        find_wavetable,
        parser::{FurEffect, FurInfoBlock, FurInstrument, FurMacro, FurMacroBody, FurPatternRow},
    },
    sound::ChannelPlayer,
};
use anyhow::Result;
use binrw::BinRead;

pub struct FurChannelState {
    panning: PanningCursor,
    volume: VolumeCursor,
    pitch: PitchCursor,
    wavedata_index: WavedataIndexCursor,
    empty: bool,
}

impl FurChannelState {
    pub fn new() -> Self {
        Self {
            panning: PanningCursor::new(),
            volume: VolumeCursor::new(),
            pitch: PitchCursor::new(),
            wavedata_index: WavedataIndexCursor::new(),
            empty: true,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.empty
    }

    pub fn advance(&mut self, ticks: u64) -> Vec<ChannelUpdate> {
        let mut updates = vec![];
        for _ in 0..ticks {
            let note_event = self.pitch.next_note_event();
            match &note_event {
                Some(NoteEvent::Release) => {
                    self.volume.release_macros();
                    self.pitch.release_macros();
                    self.wavedata_index.release_macros();
                }
                Some(NoteEvent::Start(_)) => {
                    self.empty = false;
                }
                _ => {}
            }
            updates.push(ChannelUpdate {
                volume: self.panning.next(),
                envelope: self.volume.next(),
                pitch_shift: self.pitch.next_shift(),
                wavedata_index: self.wavedata_index.next(),
                note_event,
            });
        }
        updates
    }

    pub fn handle_row(&mut self, row: &FurPatternRow, info: &FurInfoBlock) {
        if let Some(vol) = row.volume {
            self.volume.set(vol);
        }
        if let Some(instrument) = row.instrument {
            let instr = &info.instruments[instrument as usize].value;
            self.volume.load_instrument(instr);
            self.wavedata_index.load_instrument(instr);
            self.pitch.load_instrument(instr);
        }
        for effect in &row.effects {
            self.volume.load_effect(*effect);
            self.panning.load_effect(*effect);
        }
        self.pitch.load(row, info);
    }
}

#[derive(Debug, Clone)]
pub struct ChannelUpdate {
    volume: Option<(f64, f64)>,
    envelope: Option<f64>,
    pitch_shift: f64,
    wavedata_index: Option<usize>,
    note_event: Option<NoteEvent>,
}
impl ChannelUpdate {
    pub fn apply(
        &self,
        player: &mut ChannelPlayer,
        info: &FurInfoBlock,
        waveforms: &mut WaveformSetData,
    ) -> Result<()> {
        if let Some((left, right)) = self.volume {
            let volume = ((left * 15.0) as u8, (right * 15.0) as u8);
            player.set_volume(volume);
        }
        if let Some(envelope) = self.envelope {
            let envelope = (envelope * 15.0) as u8;
            player.set_envelope(envelope);
        }
        player.set_pitch_shift(self.pitch_shift);
        if let Some(wavedata_index) = self.wavedata_index {
            let wavedata = find_wavetable(info, wavedata_index).expect("Invalid wavetable");
            let index = waveforms.add_waveform(wavedata)?;
            player.set_waveform(index);
        }
        match self.note_event {
            Some(NoteEvent::Start(key)) => {
                player.start_note(key);
            }
            Some(NoteEvent::Stop) => {
                player.stop_note();
            }
            Some(NoteEvent::Release) => {}
            None => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NoteEvent {
    Release,
    Stop,
    Start(u8),
}

struct VolumeCursor {
    value: Option<f64>,
    fixed: Option<f64>,
    instrument: Option<MacroCursor<u8>>,
    slide_speed: Option<f64>,
}
impl VolumeCursor {
    fn new() -> Self {
        Self {
            value: None,
            fixed: None,
            instrument: None,
            slide_speed: None,
        }
    }

    fn next(&mut self) -> Option<f64> {
        let mut target = self.value;
        if let Some(ins) = self.instrument.as_mut().and_then(|i| i.next()) {
            let mut new_target = ins as f64 / 15.0;
            if let Some(fixed) = self.fixed {
                new_target *= fixed;
            }
            target = Some(new_target);
        } else if let Some(fixed) = self.fixed {
            target = Some(fixed);
        }
        let target = target?;
        if let Some(slide_speed) = self.slide_speed {
            let value = self.value.unwrap_or(1.0);
            if slide_speed > 0.0 {
                self.value = Some(target.min(value + slide_speed))
            } else {
                self.value = Some(target.max(value + slide_speed))
            }
        } else {
            self.value = Some(target);
        }
        self.value
    }

    fn set(&mut self, value: u8) {
        self.fixed = Some(value as f64 / 15.0);
    }

    fn load_instrument(&mut self, instr: &FurInstrument) {
        self.instrument = None;
        if let Some(macros) = instr.macros() {
            for m in macros {
                if let FurMacro::Volume(body) = m {
                    self.instrument = Some(MacroCursor::load(body));
                }
            }
        }
    }

    fn load_effect(&mut self, effect: FurEffect) {
        if let FurEffect::VolumeSlide(up, down) = effect {
            let speed = (up as i16 - down as i16) as f64 / 64.0;
            if speed == 0.0 {
                self.slide_speed = None;
            } else {
                self.slide_speed = Some(speed);
                self.fixed = None;
            }
        }
    }

    fn release_macros(&mut self) {
        if let Some(ins) = self.instrument.as_mut() {
            ins.release();
        }
    }
}

struct PanningCursor {
    value: Option<(f64, f64)>,
}
impl PanningCursor {
    fn new() -> Self {
        Self { value: None }
    }

    fn next(&mut self) -> Option<(f64, f64)> {
        self.value
    }

    fn load_effect(&mut self, effect: FurEffect) {
        match effect {
            FurEffect::SetPanning(left, right) => {
                self.value = Some((left as f64 / 15.0, right as f64 / 15.0));
            }
            FurEffect::SetVolumeLeft(value) => {
                let left = value as f64 / 15.0;
                let right = self.value.map(|v| v.1).unwrap_or(1.0);
                self.value = Some((left, right));
            }
            FurEffect::SetVolumeRight(value) => {
                let left = self.value.map(|v| v.0).unwrap_or(1.0);
                let right = value as f64 / 15.0;
                self.value = Some((left, right));
            }
            _ => {}
        }
    }
}

struct WavedataIndexCursor {
    value: Option<usize>,
    instrument: Option<MacroCursor<u8>>,
}
impl WavedataIndexCursor {
    fn new() -> Self {
        Self {
            value: None,
            instrument: None,
        }
    }

    fn next(&mut self) -> Option<usize> {
        if let Some(wav) = self.instrument.as_mut().and_then(|i| i.next()) {
            Some(wav as usize)
        } else {
            self.value.take()
        }
    }

    fn load_instrument(&mut self, instr: &FurInstrument) {
        if let Some(synth) = instr.wavetable_synth_data() {
            self.value = Some(synth.first_wave as usize);
        };
        self.instrument = None;
        if let Some(macros) = instr.macros() {
            for m in macros {
                if let FurMacro::Waveform(body) = m {
                    self.instrument = Some(MacroCursor::load(body));
                }
            }
        }
    }

    fn release_macros(&mut self) {
        if let Some(ins) = self.instrument.as_mut() {
            ins.release();
        }
    }
}

struct PitchCursor {
    instrument_arpeggio: Option<MacroCursor<i8>>,
    arpeggio_effect: Option<ArpeggioEffect>,
    arpeggio_speed: u8,
    vibrato_effect: Option<VibratoEffect>,
    slide_effect: Option<PitchSlide>,
    last_note: Option<u8>,
    release_delay: Option<u8>,
    cut_delay: Option<u8>,
    note_event: Option<NoteEvent>,
}
impl PitchCursor {
    fn new() -> Self {
        Self {
            instrument_arpeggio: None,
            arpeggio_effect: None,
            arpeggio_speed: 1,
            vibrato_effect: None,
            slide_effect: None,
            last_note: None,
            release_delay: None,
            cut_delay: None,
            note_event: None,
        }
    }

    fn next_shift(&mut self) -> f64 {
        let mut value = 0.0;
        if let Some(ins) = self.instrument_arpeggio.as_mut().and_then(|i| i.next()) {
            value += ins as f64;
        }
        if let Some(arp) = self.arpeggio_effect.as_mut().map(|m| m.next()) {
            value += arp;
        }
        if let Some(vib) = self.vibrato_effect.as_mut().map(|m| m.next()) {
            value += vib;
        }
        if let Some(sld) = self.slide_effect.as_mut().map(|m| m.next()) {
            value += sld;
        }
        value
    }

    fn next_note_event(&mut self) -> Option<NoteEvent> {
        if let Some(delay) = &mut self.release_delay {
            *delay -= 1;
            if *delay == 0 {
                self.release_delay = None;
                self.note_event = self.note_event.max(Some(NoteEvent::Release));
            }
        }
        if let Some(delay) = &mut self.cut_delay {
            *delay -= 1;
            if *delay == 0 {
                self.cut_delay = None;
                self.note_event = self.note_event.max(Some(NoteEvent::Stop));
            }
        }
        self.note_event.take()
    }

    fn load_instrument(&mut self, instr: &FurInstrument) {
        self.instrument_arpeggio = None;
        if let Some(macros) = instr.macros() {
            for m in macros {
                if let FurMacro::Arpeggio(body) = m {
                    self.instrument_arpeggio = Some(MacroCursor::load(body));
                }
            }
        }
    }

    fn load(&mut self, row: &FurPatternRow, info: &FurInfoBlock) {
        for effect in &row.effects {
            match *effect {
                FurEffect::Arpeggio(x, y) => {
                    if x == 0 && y == 0 {
                        self.arpeggio_effect = None;
                    } else {
                        self.arpeggio_effect = Some(ArpeggioEffect::new(self.arpeggio_speed, x, y));
                    }
                }
                FurEffect::PitchSlideUp(speed) => self.load_pitch_slide(info, speed as i16),
                FurEffect::PitchSlideDown(speed) => self.load_pitch_slide(info, -(speed as i16)),
                FurEffect::Portamento(speed) => self.load_portamento(info, speed as i16, row.note),
                FurEffect::Vibrato(speed, depth) => {
                    if speed == 0 {
                        self.vibrato_effect = None;
                    } else {
                        self.vibrato_effect = Some(VibratoEffect::new(speed, depth));
                    }
                }
                FurEffect::ArpeggioSpeed(speed) => {
                    self.arpeggio_speed = speed;
                    if let Some(arp) = self.arpeggio_effect.as_mut() {
                        arp.speed = speed;
                    }
                }
                FurEffect::NoteCut(ticks) => self.cut_delay = Some(ticks),
                FurEffect::NoteRelease(ticks) => self.release_delay = Some(ticks),
                _ => {}
            }
        }
        if let Some(note) = row.note {
            if note == 182 || note == 181 {
                // macro release or note release
                self.note_event = Some(NoteEvent::Release);
            } else if note == 180 {
                // note off
                self.note_event = Some(NoteEvent::Stop);
            } else {
                self.last_note = Some(note);
                self.note_event = Some(NoteEvent::Start(note - 48));
            }
        }
    }

    fn load_pitch_slide(&mut self, info: &FurInfoBlock, speed: i16) {
        assert_eq!(info.linear_pitch, 1);
        let speed = info.pitch_slide_speed as i16 * speed;
        if speed == 0 {
            self.slide_effect = None;
        } else {
            self.slide_effect = Some(PitchSlide {
                value: 0,
                target: None,
                speed,
            })
        }
    }

    fn load_portamento(&mut self, info: &FurInfoBlock, speed: i16, next_note: Option<u8>) {
        assert_eq!(info.linear_pitch, 1);
        let speed = info.pitch_slide_speed as i16 * speed;
        if speed != 0
            && let (Some(next), Some(prev)) = (next_note, self.last_note)
        {
            let delta = prev as i16 - next as i16;
            let speed = if delta < 0 { speed } else { -speed };
            self.slide_effect = Some(PitchSlide {
                value: delta,
                target: Some(0),
                speed,
            })
        } else {
            self.slide_effect = None;
        }
    }

    fn release_macros(&mut self) {
        if let Some(ins) = self.instrument_arpeggio.as_mut() {
            ins.release();
        }
    }
}

struct ArpeggioEffect {
    index: u8,
    delay: u8,
    speed: u8,
    x: u8,
    y: u8,
}
impl ArpeggioEffect {
    fn new(speed: u8, x: u8, y: u8) -> Self {
        Self {
            index: 0,
            delay: speed - 1,
            speed: speed - 1,
            x,
            y,
        }
    }

    fn next(&mut self) -> f64 {
        let value = match self.index {
            2 => self.y,
            1 => self.x,
            _ => 0,
        };
        if self.delay == 0 {
            self.delay = self.speed;
            self.index += 1;
            if self.index > 2 {
                self.index = 0;
            }
        } else {
            self.delay -= 1;
        }
        value as f64
    }
}

struct VibratoEffect {
    index: u8,
    speed: u8,
    depth: u8,
}
impl VibratoEffect {
    fn new(speed: u8, depth: u8) -> Self {
        Self {
            index: 0,
            speed,
            depth,
        }
    }

    fn next(&mut self) -> f64 {
        // The vibrato pitch shift is controlled by a sine wave,
        // with period of 64/speed steps and amplitude depth/16 semitones.
        let t = self.index as f64 * std::f64::consts::TAU / 64.0;
        let value = t.sin() * self.depth as f64 / 16.0;
        self.index += self.speed;
        while self.index > 64 {
            self.index -= 64;
        }
        value
    }
}

struct PitchSlide {
    speed: i16,
    value: i16,
    target: Option<i16>,
}
impl PitchSlide {
    fn next(&mut self) -> f64 {
        let result = self.value;
        let mut new_value = result + self.speed;
        if let Some(target) = self.target {
            if self.value <= target {
                new_value = new_value.min(target);
            }
            if self.value >= target {
                new_value = new_value.max(target);
            }
        }
        self.value = new_value;
        result as f64 / 128.0
    }
}

#[derive(Debug)]
struct MacroCursor<T> {
    data: Vec<T>,
    index: usize,
    delay: u64,
    speed: u64,
    release: Option<usize>,
    loop_to: Option<usize>,
}
impl<T> MacroCursor<T>
where
    T: BinRead + Copy + std::fmt::Debug,
    for<'a> T::Args<'a>: Default + Clone,
{
    fn load(body: &FurMacroBody<T>) -> Self {
        let speed = body.macro_speed as u64 - 1;
        Self {
            data: body.data.clone(),
            index: 0,
            delay: body.macro_delay as u64 + speed,
            speed,
            loop_to: body
                .macro_loop
                .try_into()
                .ok()
                .filter(|l| *l < body.data.len()),
            release: body
                .macro_release
                .try_into()
                .ok()
                .filter(|r| *r < body.data.len()),
        }
    }

    fn release(&mut self) {
        self.release = None;
    }
}

impl<T> Iterator for MacroCursor<T>
where
    T: Copy,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        let value = *self.data.get(self.index)?;
        if self.delay == 0 {
            self.delay = self.speed;
            if self.release != Some(self.index) {
                if self.index + 1 < self.data.len() {
                    self.index += 1;
                } else if let Some(to) = self.loop_to {
                    self.index = to;
                }
            }
        } else {
            self.delay -= 1;
        }
        Some(value)
    }
}
