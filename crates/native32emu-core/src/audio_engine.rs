// Audio engine: handles MP3 and raw PCM audio playback.

use crate::file_loader::{AudioFormat, Colorspace, Native32Reader};
#[cfg(not(feature = "standalone"))]
use symphonia::core::{
    codecs::{audio::AudioDecoderOptions, CodecParameters},
    errors::Error as SymphoniaError,
    formats::{probe::Hint, FormatOptions, TrackType},
    io::MediaSourceStream,
    meta::MetadataOptions,
};

#[cfg(feature = "standalone")]
use rodio::Source;
#[cfg(feature = "standalone")]
use std::num::NonZero;

const MAX_SOUND_EFFECTS: usize = 8;

pub struct AudioEngine {
    // Standalone mode: rodio audio
    #[cfg(feature = "standalone")]
    _mixer_device: Option<rodio::MixerDeviceSink>,
    #[cfg(feature = "standalone")]
    mixer: Option<rodio::mixer::Mixer>,
    #[cfg(feature = "standalone")]
    music_player: Option<StandaloneChannel>,
    #[cfg(feature = "standalone")]
    sound_players: Vec<StandaloneChannel>,

    pub volume: f32,
    pub colorspace: Colorspace,
    next_channel_id: usize,
    sample_frame_remainder: u32,

    #[cfg(not(feature = "standalone"))]
    channels: Vec<PlaybackChannel>,
    tone_phase: f64,
    tone_active: bool,
}

#[cfg(feature = "standalone")]
struct StandaloneChannel {
    id: usize,
    owner: String,
    player: rodio::Player,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
enum SavedAudioSource {
    Mp3(Vec<u8>),
    Raw(Vec<u8>),
    DecodedPcm(Vec<i16>),
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct SavedChannel {
    id: usize,
    owner: String,
    source: SavedAudioSource,
    position: usize,
    loops_remaining: Option<u32>,
    is_music: bool,
}

#[cfg(not(feature = "standalone"))]
struct PlaybackChannel {
    state: SavedChannel,
    samples: Vec<i16>,
    finished: bool,
}

#[cfg(not(feature = "standalone"))]
impl PlaybackChannel {
    fn next_frame(&mut self) -> Option<(i16, i16)> {
        loop {
            if self.state.position * 2 < self.samples.len() {
                let offset = self.state.position * 2;
                self.state.position += 1;
                return Some((self.samples[offset], self.samples[offset + 1]));
            }
            if self.samples.is_empty() {
                self.finished = true;
                return None;
            }
            match self.state.loops_remaining {
                Some(remaining) if remaining > 0 => {
                    self.state.loops_remaining = Some(remaining - 1);
                    self.state.position = 0;
                }
                Some(_) => {
                    self.finished = true;
                    return None;
                }
                None => {
                    self.state.position = 0;
                }
            }
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct AudioState {
    pub volume: f32,
    channels: Vec<SavedChannel>,
    next_channel_id: usize,
    sample_frame_remainder: u32,
    pub tone_phase: f64,
    pub tone_active: bool,
}

impl AudioEngine {
    pub fn new(colorspace: Colorspace, volume: u32) -> Self {
        #[cfg(feature = "standalone")]
        {
            let (mixer_device, mixer) = match rodio::DeviceSinkBuilder::open_default_sink() {
                Ok(mds) => {
                    let mixer = mds.mixer().clone();
                    (Some(mds), Some(mixer))
                }
                Err(e) => {
                    log::warn!("Failed to initialize audio: {}", e);
                    (None, None)
                }
            };

            Self {
                _mixer_device: mixer_device,
                mixer,
                music_player: None,
                sound_players: Vec::new(),
                volume: volume as f32 / 100.0,
                colorspace,
                next_channel_id: 1,
                sample_frame_remainder: 0,
                tone_phase: 0.0,
                tone_active: false,
            }
        }

        #[cfg(not(feature = "standalone"))]
        {
            Self {
                volume: volume as f32 / 100.0,
                colorspace,
                next_channel_id: 1,
                sample_frame_remainder: 0,
                channels: Vec::new(),
                tone_phase: 0.0,
                tone_active: false,
            }
        }
    }

    /// Get pending audio samples for libretro mode.
    /// Returns interleaved stereo i16 samples.
    /// This should be called once per frame in retro_run().
    pub fn get_pending_samples(&mut self) -> Vec<i16> {
        let sample_rate = self.output_sample_rate();
        self.sample_frame_remainder += sample_rate;
        let frames_per_video_frame = (self.sample_frame_remainder / 30) as usize;
        self.sample_frame_remainder %= 30;
        let values_per_video_frame = frames_per_video_frame * 2;

        #[cfg(feature = "standalone")]
        let mut result = vec![0i16; values_per_video_frame];

        #[cfg(not(feature = "standalone"))]
        let mut result = {
            let mut mixed = vec![(0i32, 0i32); frames_per_video_frame];
            for channel in &mut self.channels {
                for frame in &mut mixed {
                    let Some((left, right)) = channel.next_frame() else {
                        break;
                    };
                    frame.0 += left as i32;
                    frame.1 += right as i32;
                }
            }
            self.channels.retain(|channel| !channel.finished);

            let mut output = Vec::with_capacity(values_per_video_frame);
            for (left, right) in mixed {
                output.push(
                    (left as f32 * self.volume).clamp(i16::MIN as f32, i16::MAX as f32) as i16,
                );
                output.push(
                    (right as f32 * self.volume).clamp(i16::MIN as f32, i16::MAX as f32) as i16,
                );
            }
            output
        };

        if self.tone_active {
            for frame in result.as_chunks_mut::<2>().0 {
                let sample = (self.tone_phase * 2.0 * std::f64::consts::PI * 440.0
                    / sample_rate as f64)
                    .sin();
                let sample_i16 = (sample * 16000.0 * self.volume as f64) as i16;
                frame[0] = frame[0].saturating_add(sample_i16);
                frame[1] = frame[1].saturating_add(sample_i16);
                self.tone_phase += 1.0;
                if self.tone_phase >= sample_rate as f64 {
                    self.tone_phase -= sample_rate as f64;
                }
            }
        }

        result
    }

    /// Start a test tone (for debugging).
    pub fn start_tone(&mut self) {
        self.tone_active = true;
        self.tone_phase = 0.0;
    }

    /// Stop the test tone.
    pub fn stop_tone(&mut self) {
        self.tone_active = false;
    }

    fn output_sample_rate(&self) -> u32 {
        match self.colorspace {
            Colorspace::YUV => 11025,
            Colorspace::ARGB => 22050,
        }
    }

    fn allocate_channel_id(&mut self) -> usize {
        let id = self.next_channel_id;
        self.next_channel_id = self.next_channel_id.wrapping_add(1).max(1);
        id
    }

    /// Play a sound and return its channel identifier.
    pub fn play_sound(
        &mut self,
        reader: &mut Native32Reader,
        sound_value: u16,
        movie_name: &str,
    ) -> Option<usize> {
        let repeat = ((sound_value >> 8) & 0xFF) as u8;
        let index = (sound_value & 0xFF) as u32;
        if index == 0 {
            return None;
        }

        let sound = match reader.get_sound(index) {
            Some(sound) => sound,
            None => {
                log::warn!("Failed to load sound {}", index);
                return None;
            }
        };

        match sound.format {
            AudioFormat::MP3 => self.play_mp3(&sound.data, repeat, movie_name),
            AudioFormat::Raw => self.play_raw(&sound.data, repeat, movie_name),
        }
    }

    #[cfg(not(feature = "standalone"))]
    fn play_mp3(&mut self, data: &[u8], repeat: u8, movie_name: &str) -> Option<usize> {
        let samples = match decode_mp3(data, self.output_sample_rate()) {
            Ok(samples) => samples,
            Err(error) => {
                log::warn!("Failed to decode MP3: {error}");
                return None;
            }
        };
        self.add_channel(
            samples,
            SavedAudioSource::Mp3(data.to_vec()),
            repeat,
            movie_name,
            true,
        )
    }

    #[cfg(not(feature = "standalone"))]
    fn play_raw(&mut self, data: &[u8], repeat: u8, movie_name: &str) -> Option<usize> {
        let samples = raw_pcm_to_stereo(data);
        self.add_channel(
            samples,
            SavedAudioSource::Raw(data.to_vec()),
            repeat,
            movie_name,
            false,
        )
    }

    #[cfg(not(feature = "standalone"))]
    fn add_channel(
        &mut self,
        samples: Vec<i16>,
        source: SavedAudioSource,
        repeat: u8,
        owner: &str,
        is_music: bool,
    ) -> Option<usize> {
        if samples.is_empty() {
            return None;
        }
        self.channels.retain(|channel| !channel.finished);
        if is_music {
            self.channels.retain(|channel| !channel.state.is_music);
        } else if self
            .channels
            .iter()
            .filter(|channel| !channel.state.is_music)
            .count()
            >= MAX_SOUND_EFFECTS
        {
            return None;
        }

        let id = self.allocate_channel_id();
        self.channels.push(PlaybackChannel {
            state: SavedChannel {
                id,
                owner: owner.to_string(),
                source,
                position: 0,
                loops_remaining: (repeat != 0xFF).then_some(repeat as u32),
                is_music,
            },
            samples,
            finished: false,
        });
        self.tone_active = false;
        Some(id)
    }

    pub(crate) fn save_state(&self) -> AudioState {
        #[cfg(feature = "standalone")]
        let channels = Vec::new();
        #[cfg(not(feature = "standalone"))]
        let channels = self
            .channels
            .iter()
            .filter(|channel| !channel.finished)
            .map(|channel| channel.state.clone())
            .collect();

        AudioState {
            volume: self.volume,
            channels,
            next_channel_id: self.next_channel_id,
            sample_frame_remainder: self.sample_frame_remainder,
            tone_phase: self.tone_phase,
            tone_active: self.tone_active,
        }
    }

    pub(crate) fn restore_state(&mut self, state: AudioState) {
        self.stop_all();
        self.volume = state.volume.clamp(0.0, 1.0);
        self.next_channel_id = state.next_channel_id.max(1);
        self.sample_frame_remainder = state.sample_frame_remainder % 30;
        self.tone_phase = state.tone_phase;
        self.tone_active = state.tone_active;

        #[cfg(not(feature = "standalone"))]
        for saved in state.channels {
            let samples = match &saved.source {
                SavedAudioSource::Mp3(data) => match decode_mp3(data, self.output_sample_rate()) {
                    Ok(samples) => samples,
                    Err(error) => {
                        log::warn!("Failed to restore MP3 channel: {error}");
                        continue;
                    }
                },
                SavedAudioSource::Raw(data) => raw_pcm_to_stereo(data),
                SavedAudioSource::DecodedPcm(samples) => samples.clone(),
            };
            let mut saved = saved;
            saved.position = saved.position.min(samples.len() / 2);
            self.channels.push(PlaybackChannel {
                state: saved,
                samples,
                finished: false,
            });
        }
    }

    #[cfg(feature = "standalone")]
    fn play_mp3(&mut self, data: &[u8], repeat: u8, movie_name: &str) -> Option<usize> {
        let mixer = self.mixer.clone()?;
        if let Some(channel) = self.music_player.take() {
            channel.player.stop();
        }

        let player = rodio::Player::connect_new(&mixer);
        player.set_volume(self.volume);
        let cursor = std::io::Cursor::new(data.to_vec());
        let decoder = match rodio::Decoder::new_mp3(cursor) {
            Ok(decoder) => decoder,
            Err(error) => {
                log::warn!("Failed to decode MP3: {error}");
                return None;
            }
        };
        let buffered = decoder.buffered();
        if repeat == 0xFF {
            player.append(buffered.repeat_infinite());
        } else {
            player.append(buffered.clone());
            for _ in 0..repeat {
                player.append(buffered.clone());
            }
        }

        let id = self.allocate_channel_id();
        self.music_player = Some(StandaloneChannel {
            id,
            owner: movie_name.to_string(),
            player,
        });
        self.tone_active = false;
        Some(id)
    }

    #[cfg(feature = "standalone")]
    fn play_raw(&mut self, data: &[u8], repeat: u8, movie_name: &str) -> Option<usize> {
        let mixer = self.mixer.clone()?;
        self.sound_players.retain(|channel| !channel.player.empty());
        if self.sound_players.len() >= MAX_SOUND_EFFECTS {
            return None;
        }

        let samples: Vec<f32> = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
            .collect();
        if samples.is_empty() {
            return None;
        }
        let source = rodio::buffer::SamplesBuffer::new(
            NonZero::<u16>::new(1).unwrap(),
            NonZero::<u32>::new(self.output_sample_rate()).unwrap(),
            samples,
        );
        let player = rodio::Player::connect_new(&mixer);
        player.set_volume(self.volume);
        let buffered = source.buffered();
        if repeat == 0xFF {
            player.append(buffered.repeat_infinite());
        } else {
            player.append(buffered.clone());
            for _ in 0..repeat {
                player.append(buffered.clone());
            }
        }

        let id = self.allocate_channel_id();
        self.sound_players.push(StandaloneChannel {
            id,
            owner: movie_name.to_string(),
            player,
        });
        self.tone_active = false;
        Some(id)
    }

    /// Play a fully-decoded interleaved PCM stream (used for cutscene audio).
    /// `samples` are normalized floats interleaved by `channels`.
    #[cfg(feature = "standalone")]
    pub fn play_pcm_stream(&mut self, samples: Vec<f32>, channels: u16, sample_rate: u32) {
        if samples.is_empty() || channels == 0 || sample_rate == 0 {
            return;
        }
        let Some(mixer) = self.mixer.clone() else {
            return;
        };
        if let Some(channel) = self.music_player.take() {
            channel.player.stop();
        }
        let source = rodio::buffer::SamplesBuffer::new(
            NonZero::<u16>::new(channels).unwrap(),
            NonZero::<u32>::new(sample_rate).unwrap(),
            samples,
        );
        let player = rodio::Player::connect_new(&mixer);
        player.set_volume(self.volume);
        player.append(source);
        let id = self.allocate_channel_id();
        self.music_player = Some(StandaloneChannel {
            id,
            owner: "__cutscene__".to_string(),
            player,
        });
    }

    #[cfg(not(feature = "standalone"))]
    pub fn play_pcm_stream(&mut self, samples: Vec<f32>, channels: u16, sample_rate: u32) {
        let output = resample_to_stereo(
            &samples,
            channels as usize,
            sample_rate,
            self.output_sample_rate(),
        );
        let _ = self.add_channel(
            output.clone(),
            SavedAudioSource::DecodedPcm(output),
            0,
            "__cutscene__",
            true,
        );
    }

    /// Stop all currently playing sounds.
    pub fn stop_all(&mut self) {
        #[cfg(feature = "standalone")]
        {
            if let Some(channel) = self.music_player.take() {
                channel.player.stop();
            }
            for channel in self.sound_players.drain(..) {
                channel.player.stop();
            }
        }
        #[cfg(not(feature = "standalone"))]
        self.channels.clear();

        self.tone_active = false;
    }

    /// Stop sounds owned by the given movie.
    pub fn stop_for_movie(&mut self, movie_name: &str) {
        #[cfg(feature = "standalone")]
        {
            if self
                .music_player
                .as_ref()
                .is_some_and(|channel| channel.owner == movie_name)
            {
                if let Some(channel) = self.music_player.take() {
                    channel.player.stop();
                }
            }
            let mut retained = Vec::with_capacity(self.sound_players.len());
            for channel in self.sound_players.drain(..) {
                if channel.owner == movie_name {
                    channel.player.stop();
                } else {
                    retained.push(channel);
                }
            }
            self.sound_players = retained;
        }
        #[cfg(not(feature = "standalone"))]
        self.channels
            .retain(|channel| channel.state.owner != movie_name);
    }

    pub fn is_channel_playing(&self, channel_id: usize) -> bool {
        #[cfg(feature = "standalone")]
        {
            self.music_player
                .as_ref()
                .is_some_and(|channel| channel.id == channel_id && !channel.player.empty())
                || self
                    .sound_players
                    .iter()
                    .any(|channel| channel.id == channel_id && !channel.player.empty())
        }
        #[cfg(not(feature = "standalone"))]
        {
            self.channels
                .iter()
                .any(|channel| channel.state.id == channel_id && !channel.finished)
        }
    }

    pub fn is_playing(&self) -> bool {
        #[cfg(feature = "standalone")]
        {
            self.music_player
                .as_ref()
                .is_some_and(|channel| !channel.player.empty())
                || self
                    .sound_players
                    .iter()
                    .any(|channel| !channel.player.empty())
                || self.tone_active
        }
        #[cfg(not(feature = "standalone"))]
        {
            self.channels.iter().any(|channel| !channel.finished) || self.tone_active
        }
    }

    /// Set the volume (0-100).
    pub fn set_volume(&mut self, volume: u32) {
        self.volume = volume as f32 / 100.0;
        #[cfg(feature = "standalone")]
        {
            if let Some(channel) = &self.music_player {
                channel.player.set_volume(self.volume);
            }
            for channel in &self.sound_players {
                channel.player.set_volume(self.volume);
            }
        }
    }
}

#[cfg(not(feature = "standalone"))]
fn raw_pcm_to_stereo(data: &[u8]) -> Vec<i16> {
    let mut output = Vec::with_capacity(data.len());
    for chunk in data.as_chunks::<2>().0 {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
        output.push(sample);
        output.push(sample);
    }
    output
}

#[cfg(not(feature = "standalone"))]
fn resample_to_stereo(
    samples: &[f32],
    channels: usize,
    input_rate: u32,
    output_rate: u32,
) -> Vec<i16> {
    if samples.is_empty() || channels == 0 || input_rate == 0 || output_rate == 0 {
        return Vec::new();
    }
    let input_frames = samples.len() / channels;
    if input_frames == 0 {
        return Vec::new();
    }
    let output_frames =
        (input_frames as u64 * output_rate as u64).div_ceil(input_rate as u64) as usize;
    let mut output = Vec::with_capacity(output_frames * 2);

    for output_frame in 0..output_frames {
        let position = output_frame as u64 * input_rate as u64;
        let source_frame = (position / output_rate as u64) as usize;
        let next_frame = (source_frame + 1).min(input_frames - 1);
        let fraction = (position % output_rate as u64) as f32 / output_rate as f32;

        for channel in 0..2 {
            let source_channel = channel.min(channels - 1);
            let first = samples[source_frame.min(input_frames - 1) * channels + source_channel];
            let second = samples[next_frame * channels + source_channel];
            let sample = first + (second - first) * fraction;
            output.push((sample * 32767.0).clamp(i16::MIN as f32, i16::MAX as f32) as i16);
        }
    }
    output
}

#[cfg(not(feature = "standalone"))]
fn decode_mp3(data: &[u8], output_rate: u32) -> anyhow::Result<Vec<i16>> {
    let source = Box::new(std::io::Cursor::new(data.to_vec()));
    let stream = MediaSourceStream::new(source, Default::default());
    let mut hint = Hint::new();
    hint.with_extension("mp3");
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            stream,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| anyhow::anyhow!("failed to probe MP3 stream: {error}"))?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| anyhow::anyhow!("MP3 stream has no audio track"))?;
    let track_id = track.id;
    let codec_params = track
        .codec_params
        .clone()
        .ok_or_else(|| anyhow::anyhow!("MP3 track has no codec parameters"))?;
    let audio_codec_params = match codec_params {
        CodecParameters::Audio(params) => params,
        _ => return Err(anyhow::anyhow!("MP3 track is not an audio track")),
    };
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_codec_params, &AudioDecoderOptions::default())
        .map_err(|error| anyhow::anyhow!("failed to create MP3 decoder: {error}"))?;

    let mut decoded = Vec::new();
    let mut sample_rate = 0;
    let mut channel_count = 0;
    let mut temp_buf = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break, // End of stream
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                return Err(anyhow::anyhow!("MP3 stream changed tracks while decoding"));
            }
            Err(error) => return Err(anyhow::anyhow!("failed to read MP3 packet: {error}")),
        };
        if packet.track_id != track_id {
            continue;
        }
        let audio = match decoder.decode(&packet) {
            Ok(audio) => audio,
            Err(SymphoniaError::DecodeError(error)) => {
                log::debug!("Skipping malformed MP3 packet: {error}");
                continue;
            }
            Err(SymphoniaError::IoError(error))
                if error.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(error) => return Err(anyhow::anyhow!("failed to decode MP3 packet: {error}")),
        };
        let spec = audio.spec();
        if sample_rate == 0 {
            sample_rate = spec.rate();
            channel_count = spec.channels().count();
        } else if sample_rate != spec.rate() || channel_count != spec.channels().count() {
            return Err(anyhow::anyhow!(
                "MP3 stream changed audio format while decoding"
            ));
        }
        // Use temp_buf to accumulate samples, then extend decoded
        temp_buf.clear();
        audio.copy_to_vec_interleaved(&mut temp_buf);
        decoded.extend_from_slice(&temp_buf);
    }

    if sample_rate == 0 || channel_count == 0 || decoded.is_empty() {
        return Err(anyhow::anyhow!("MP3 stream decoded to no samples"));
    }
    Ok(resample_to_stereo(
        &decoded,
        channel_count,
        sample_rate,
        output_rate,
    ))
}
#[cfg(all(test, not(feature = "standalone")))]
mod tests {
    use super::*;

    fn engine() -> AudioEngine {
        AudioEngine::new(Colorspace::YUV, 100)
    }

    #[test]
    fn emits_exact_fractional_sample_rate_over_two_frames() {
        let mut audio = engine();
        let first = audio.get_pending_samples();
        let second = audio.get_pending_samples();

        assert_eq!(first.len() / 2, 367);
        assert_eq!(second.len() / 2, 368);
        assert_eq!((first.len() + second.len()) / 2, 735);
    }

    #[test]
    fn playback_channel_honors_finite_and_infinite_loops() {
        let state = SavedChannel {
            id: 1,
            owner: "finite".to_string(),
            source: SavedAudioSource::DecodedPcm(vec![100, 100]),
            position: 0,
            loops_remaining: Some(2),
            is_music: false,
        };
        let mut finite = PlaybackChannel {
            state,
            samples: vec![100, 100],
            finished: false,
        };
        assert_eq!(finite.next_frame(), Some((100, 100)));
        assert_eq!(finite.next_frame(), Some((100, 100)));
        assert_eq!(finite.next_frame(), Some((100, 100)));
        assert_eq!(finite.next_frame(), None);

        let mut infinite = PlaybackChannel {
            state: SavedChannel {
                id: 2,
                owner: "infinite".to_string(),
                source: SavedAudioSource::DecodedPcm(vec![200, 200]),
                position: 0,
                loops_remaining: None,
                is_music: true,
            },
            samples: vec![200, 200],
            finished: false,
        };
        for _ in 0..100 {
            assert_eq!(infinite.next_frame(), Some((200, 200)));
        }
    }

    #[test]
    fn mixes_background_music_and_sound_effects_without_interrupting_music() {
        let mut audio = engine();
        let music = vec![1000, 1000];
        let effect = vec![2000, 2000];
        let music_id = audio
            .add_channel(
                music.clone(),
                SavedAudioSource::DecodedPcm(music),
                0xFF,
                "music",
                true,
            )
            .unwrap();
        let effect_id = audio
            .add_channel(
                effect.clone(),
                SavedAudioSource::DecodedPcm(effect),
                0,
                "effect",
                false,
            )
            .unwrap();

        let output = audio.get_pending_samples();
        assert_eq!(&output[..4], &[3000, 3000, 1000, 1000]);
        assert!(audio.is_channel_playing(music_id));
        assert!(!audio.is_channel_playing(effect_id));
    }

    #[test]
    fn save_state_restores_channel_position_and_looping() {
        let mut audio = engine();
        let music = vec![1000, 1000, 2000, 2000];
        let channel_id = audio
            .add_channel(
                music.clone(),
                SavedAudioSource::DecodedPcm(music),
                0xFF,
                "music",
                true,
            )
            .unwrap();
        let _ = audio.get_pending_samples();
        let state = audio.save_state();

        let mut restored = engine();
        restored.restore_state(state);
        assert!(restored.is_channel_playing(channel_id));
        assert!(restored
            .get_pending_samples()
            .iter()
            .any(|sample| *sample != 0));
    }

    #[test]
    fn resamples_mono_audio_to_stereo_without_changing_duration() {
        let input = vec![0.0, 0.25, 0.5, 1.0];
        let output = resample_to_stereo(&input, 1, 22050, 11025);

        assert_eq!(output.len(), 4);
        assert_eq!(output[0], output[1]);
        assert_eq!(output[2], output[3]);
    }
}
