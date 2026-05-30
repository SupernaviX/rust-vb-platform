use std::{
    collections::{BTreeMap, HashSet},
    time::Duration,
};

use anyhow::Result;

use crate::{
    assets::{
        ChannelData, WaveformSetData,
        ir::{
            ControlEffect, Effect, Instrument, InstrumentMacro, IrInfo, NoteEvent, PanningEffect,
            PatternRow, PitchEffect, VolumeEffect,
        },
        sound::{ChannelBuilder, ChannelPlayer, Moment},
    },
    config::ChannelEffects,
};

pub fn decode(
    info: IrInfo,
    waveforms: &mut WaveformSetData,
    looping: bool,
) -> Result<Vec<ChannelData>> {
    let mut clock = Clock::new(&info);
    let mut channels = BTreeMap::new();
    for (i, c) in &info.channels {
        let channel = Channel::new(*i as usize, c.effects.clone());
        channels.insert(*i, channel);
    }
    let orders_length = info
        .channels
        .values()
        .map(|c| c.order.len())
        .max()
        .unwrap_or_default();
    let mut orders_seen = HashSet::new();
    let mut order = 0;

    let mut start_row = 0;
    let mut tick = 0;
    'outer: loop {
        if !orders_seen.insert(order) {
            for channel in channels.values_mut() {
                channel.jump_to(order);
            }
            break;
        }
        for channel in channels.values_mut() {
            channel.start_pattern(order);
        }
        let start = start_row;
        let mut ran_up_to = start;
        start_row = 0;
        for (row_index, rows) in gather_rows(&info, order) {
            if row_index < start {
                continue;
            }

            let rows_elapsed = row_index - ran_up_to;
            if rows_elapsed > 0 {
                tick += info.ticks_per_row as u64 * rows_elapsed;
                for channel in channels.values_mut() {
                    channel.advance_time(tick, &clock, waveforms)?;
                }
            }
            ran_up_to = row_index + 1;

            let mut next = NextAction::Continue;
            for (channel_index, row) in rows.patterns {
                let channel = channels.get_mut(&channel_index).unwrap();
                channel.handle_row(&row, &info);
            }
            for effect in rows.control {
                match effect {
                    ControlEffect::Jump { order, row } => {
                        next = next.max(NextAction::Jump { order, row })
                    }
                    ControlEffect::JumpToNextPattern { row } => {
                        next = next.max(NextAction::Jump {
                            order: order + 1,
                            row,
                        });
                    }
                    ControlEffect::SetVirtualTempoNumerator(n) => {
                        clock.set_virtual_numerator(tick, n as u16);
                    }
                    ControlEffect::SetVirtualTempoDenominator(d) => {
                        clock.set_virtual_denominator(tick, d as u16);
                    }
                    ControlEffect::StopSong => {
                        next = next.max(NextAction::Stop);
                    }
                }
            }
            tick += info.ticks_per_row as u64;
            for channel in channels.values_mut() {
                channel.advance_time(tick, &clock, waveforms)?;
            }

            match next {
                NextAction::Continue => {}
                NextAction::Jump { order: o, row } => {
                    order = o;
                    start_row = row;
                    continue 'outer;
                }
                NextAction::Stop => {
                    break 'outer;
                }
            }
        }
        if ran_up_to < info.pattern_length as u64 {
            tick += info.ticks_per_row as u64 * (info.pattern_length as u64 - ran_up_to);
            for channel in channels.values_mut() {
                channel.advance_time(tick, &clock, waveforms)?;
            }
        }

        if order == orders_length - 1 {
            if looping {
                order = 0
            } else {
                break;
            }
        } else {
            order += 1;
        }
    }

    Ok(channels
        .into_values()
        .flat_map(|c| c.build(&info.name))
        .collect())
}

fn gather_rows(info: &IrInfo, order: usize) -> BTreeMap<u64, Rows> {
    let mut rows: BTreeMap<u64, Rows> = BTreeMap::new();
    for (index, channel) in &info.channels {
        let pattern_index = channel.order[order];
        let Some(pattern) = channel.patterns.get(&pattern_index) else {
            panic!("unrecognized pattern {pattern_index} in channel {index}");
        };
        for (row_index, row) in &pattern.data {
            rows.entry(*row_index)
                .or_default()
                .patterns
                .push((*index, row.clone()));
        }
    }
    for (row_index, effects) in &info.control[order] {
        rows.entry(*row_index).or_default().control = effects.clone();
    }
    rows
}

#[derive(Default)]
struct Rows {
    patterns: Vec<(u8, PatternRow)>,
    control: Vec<ControlEffect>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum NextAction {
    Continue,
    Jump { order: usize, row: u64 },
    Stop,
}

struct Clock {
    tempos: BTreeMap<u64, Tempo>,
}
impl Clock {
    fn new(info: &IrInfo) -> Self {
        let initial_tempo = Tempo::new(
            Moment::START,
            info.ticks_per_second,
            info.virtual_tempo_numerator,
            info.virtual_tempo_denominator,
        );
        let mut tempos = BTreeMap::new();
        tempos.insert(0, initial_tempo);
        Self { tempos }
    }

    fn moment(&self, tick: u64) -> Moment {
        let (tempo, elapsed) = self.tempo_at(tick);
        tempo.start + (tempo.per_tick * elapsed as u32)
    }

    fn set_virtual_numerator(&mut self, tick: u64, value: u16) {
        let (tempo, elapsed) = self.tempo_at(tick);
        if tempo.virtual_numerator == value {
            return;
        }
        let start = tempo.start + (tempo.per_tick * elapsed as u32);
        let new_tempo = Tempo::new(
            start,
            tempo.ticks_per_second,
            value,
            tempo.virtual_denominator,
        );
        self.tempos.insert(tick, new_tempo);
    }

    fn set_virtual_denominator(&mut self, tick: u64, value: u16) {
        let (tempo, elapsed) = self.tempo_at(tick);
        if tempo.virtual_denominator == value {
            return;
        }
        let start = tempo.start + (tempo.per_tick * elapsed as u32);
        let new_tempo = Tempo::new(
            start,
            tempo.ticks_per_second,
            tempo.virtual_numerator,
            value,
        );
        self.tempos.insert(tick, new_tempo);
    }

    fn tempo_at(&self, tick: u64) -> (&Tempo, u64) {
        let (start_tick, tempo) = self.tempos.range(..=tick).next_back().unwrap();
        let elapsed = tick - *start_tick;
        (tempo, elapsed)
    }
}

struct Tempo {
    start: Moment,
    ticks_per_second: f32,
    virtual_numerator: u16,
    virtual_denominator: u16,
    per_tick: Duration,
}
impl Tempo {
    fn new(
        start: Moment,
        ticks_per_second: f32,
        virtual_numerator: u16,
        virtual_denominator: u16,
    ) -> Self {
        Self {
            start,
            ticks_per_second,
            virtual_numerator,
            virtual_denominator,
            per_tick: Duration::from_secs_f32(
                virtual_denominator as f32 / (virtual_numerator as f32 * ticks_per_second),
            ),
        }
    }
}

struct Channel {
    channel: usize,
    player: ChannelPlayer,
    state: ChannelState,
    next_tick: u64,
}
impl Channel {
    fn new(channel: usize, effects: ChannelEffects) -> Self {
        let mut player = ChannelPlayer::new(effects, false);
        player.set_envelope(15);
        player.set_volume((15, 15));
        player.advance_time(Moment::START);
        Self {
            channel,
            player,
            state: ChannelState::new(),
            next_tick: 0,
        }
    }
    fn handle_row(&mut self, row: &PatternRow, info: &IrInfo) {
        self.state.handle_row(row, info);
    }
    fn advance_time(
        &mut self,
        tick: u64,
        clock: &Clock,
        waveforms: &mut WaveformSetData,
    ) -> Result<()> {
        for (new_tick, update) in (self.next_tick..).zip(self.state.advance(tick - self.next_tick))
        {
            self.player.advance_time(clock.moment(new_tick));
            update.apply(&mut self.player, waveforms)?;
        }
        self.next_tick = tick;
        Ok(())
    }
    fn start_pattern(&mut self, order: usize) {
        self.player.start_pattern(order as u8);
    }
    fn jump_to(&mut self, order: usize) {
        self.player.go_to_pattern(order as u8);
    }
    fn build(self, name: &str) -> Option<ChannelData> {
        if self.state.empty {
            return None;
        }
        let builder = ChannelBuilder {
            name: format!("{name}_{}", self.channel),
            player: self.player,
        };
        Some(builder.build())
    }
}

struct ChannelState {
    panning: PanningCursor,
    volume: VolumeCursor,
    pitch: PitchCursor,
    waveform: WaveformCursor,
    empty: bool,
}
impl ChannelState {
    fn new() -> Self {
        Self {
            panning: PanningCursor::new(),
            volume: VolumeCursor::new(),
            pitch: PitchCursor::new(),
            waveform: WaveformCursor::new(),
            empty: true,
        }
    }

    fn advance(&mut self, ticks: u64) -> Vec<ChannelUpdate> {
        let mut updates = vec![];
        for _ in 0..ticks {
            let note_event = self.pitch.next_note_event();
            match &note_event {
                Some(NoteEvent::Release) => {
                    self.volume.release_macros();
                    self.pitch.release_macros();
                    self.waveform.release_macros();
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
                waveform: self.waveform.next(),
                note_event,
            });
        }
        updates
    }

    fn handle_row(&mut self, row: &PatternRow, info: &IrInfo) {
        if let Some(vol) = row.volume {
            self.volume.set(vol);
        }
        if let Some(instrument) = row.instrument {
            let instr = &info.instruments[instrument];
            self.volume.load_instrument(instr);
            self.waveform.load_instrument(instr);
            self.pitch.load_instrument(instr);
        }
        for effect in &row.effects {
            if let Effect::Volume(e) = effect {
                self.volume.load_effect(e);
            }
            if let Effect::Panning(e) = effect {
                self.panning.load_effect(e);
            }
        }
        self.pitch.load(row);
    }
}

#[derive(Debug, Clone)]
pub struct ChannelUpdate {
    volume: Option<(f64, f64)>,
    envelope: Option<f64>,
    pitch_shift: f64,
    waveform: Option<[u8; 32]>,
    note_event: Option<NoteEvent>,
}
impl ChannelUpdate {
    pub fn apply(&self, player: &mut ChannelPlayer, waveforms: &mut WaveformSetData) -> Result<()> {
        if let Some((left, right)) = self.volume {
            let volume = ((left * 15.0) as u8, (right * 15.0) as u8);
            player.set_volume(volume);
        }
        if let Some(envelope) = self.envelope {
            let envelope = (envelope * 15.0) as u8;
            player.set_envelope(envelope);
        }
        player.set_pitch_shift(self.pitch_shift);
        if let Some(waveform) = self.waveform {
            let index = waveforms.add_waveform(waveform)?;
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

struct VolumeCursor {
    value: Option<f64>,
    fixed: Option<f64>,
    instrument: Option<MacroCursor<f64>>,
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
            let mut new_target = ins;
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

    fn set(&mut self, value: f64) {
        self.fixed = Some(value);
    }

    fn load_instrument(&mut self, instr: &Instrument) {
        self.instrument = instr.volume_macro.as_ref().map(MacroCursor::load);
    }

    fn load_effect(&mut self, effect: &VolumeEffect) {
        match effect {
            VolumeEffect::VolumeSlide(speed) => {
                if *speed == 0.0 {
                    self.slide_speed = None;
                } else {
                    self.slide_speed = Some(*speed);
                    self.fixed = None;
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

    fn load_effect(&mut self, effect: &PanningEffect) {
        match effect {
            PanningEffect::SetPanning(left, right) => self.value = Some((*left, *right)),
            PanningEffect::SetVolumeLeft(value) => {
                let left = *value;
                let right = self.value.map(|v| v.1).unwrap_or(1.0);
                self.value = Some((left, right));
            }
            PanningEffect::SetVolumeRight(value) => {
                let left = self.value.map(|v| v.0).unwrap_or(1.0);
                let right = *value;
                self.value = Some((left, right));
            }
        }
    }
}

struct WaveformCursor {
    value: Option<[u8; 32]>,
    instrument: Option<MacroCursor<[u8; 32]>>,
}
impl WaveformCursor {
    fn new() -> Self {
        Self {
            value: None,
            instrument: None,
        }
    }

    fn next(&mut self) -> Option<[u8; 32]> {
        if let Some(wav) = self.instrument.as_mut().and_then(|i| i.next()) {
            Some(wav)
        } else {
            self.value.take()
        }
    }

    fn load_instrument(&mut self, instr: &Instrument) {
        self.value = instr.waveform;
        self.instrument = instr.waveform_macro.as_ref().map(MacroCursor::load);
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

    fn load_instrument(&mut self, instr: &Instrument) {
        self.instrument_arpeggio = instr.arpeggio_macro.as_ref().map(MacroCursor::load);
    }

    fn load(&mut self, row: &PatternRow) {
        for effect in &row.effects {
            let Effect::Pitch(effect) = effect else {
                continue;
            };
            match *effect {
                PitchEffect::Arpeggio(x, y) => {
                    if x == 0 && y == 0 {
                        self.arpeggio_effect = None;
                    } else {
                        self.arpeggio_effect = Some(ArpeggioEffect::new(self.arpeggio_speed, x, y));
                    }
                }
                PitchEffect::PitchSlide(speed) => self.load_pitch_slide(speed),
                PitchEffect::Portamento(speed) => self.load_portamento(speed, row.note),
                PitchEffect::Vibrato(speed, depth) => {
                    if speed == 0 {
                        self.vibrato_effect = None;
                    } else {
                        self.vibrato_effect = Some(VibratoEffect::new(speed, depth));
                    }
                }
                PitchEffect::ArpeggioSpeed(speed) => {
                    self.arpeggio_speed = speed;
                    if let Some(arp) = self.arpeggio_effect.as_mut() {
                        arp.speed = speed;
                    }
                }
                PitchEffect::NoteCut(ticks) => self.cut_delay = Some(ticks),
                PitchEffect::NoteRelease(ticks) => self.release_delay = Some(ticks),
            }
        }
        self.note_event = row.note;
        if let Some(NoteEvent::Start(note)) = self.note_event {
            self.last_note = Some(note);
        }
    }

    fn load_pitch_slide(&mut self, speed: f64) {
        if speed == 0.0 {
            self.slide_effect = None;
        } else {
            self.slide_effect = Some(PitchSlide {
                value: 0.0,
                target: None,
                speed,
            })
        }
    }

    fn load_portamento(&mut self, speed: f64, next_note: Option<NoteEvent>) {
        if speed != 0.0
            && let (Some(NoteEvent::Start(next)), Some(prev)) = (next_note, self.last_note)
        {
            let delta = (prev as f64 - next as f64) / 128.0;
            let speed = if delta < 0.0 { speed } else { -speed };
            self.slide_effect = Some(PitchSlide {
                value: delta,
                target: Some(0.0),
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
    speed: f64,
    value: f64,
    target: Option<f64>,
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
        result
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
    T: Clone,
{
    fn load(body: &InstrumentMacro<T>) -> Self {
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
