use crate::audio_engine::AudioState;
use crate::content_loader::ContentLoader;
use crate::frame_player::FramePlayer;
use crate::renderer::RendererState;
use crate::sprite_system::SpriteSystem;
use anyhow::{bail, Context, Result};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const MAGIC: &[u8; 8] = b"N32STATE";
const VERSION: u32 = 2;
const HEADER_SIZE: usize = 20;

/// libretro requires this value to remain constant for a loaded core.
pub(crate) const SERIALIZED_SIZE: usize = 32 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
pub(crate) struct VideoState {
    pub name: String,
    pub elapsed: f64,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct ReturnFrameState {
    pub content_path: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct EmulatorState {
    pub content_path: String,
    pub content_crc32: u32,
    pub frame_player: FramePlayer,
    pub sprites: SpriteSystem,
    pub vm_vars: HashMap<String, String>,
    pub rng_state: u64,
    pub audio: AudioState,
    pub renderer: RendererState,
    pub content_loader: ContentLoader,
    pub menu_context: Option<String>,
    /// None identifies states written before parent-SMF navigation was added.
    #[serde(default)]
    pub return_stack: Option<Vec<ReturnFrameState>>,
    pub tick_count: u64,
    pub time_ms: u32,
    pub pending_videos: Vec<String>,
    pub video: Option<VideoState>,
}

pub(crate) fn encode(state: &EmulatorState, output: &mut [u8]) -> Result<()> {
    if output.len() < SERIALIZED_SIZE {
        bail!(
            "save-state buffer is too small: got {}, need {}",
            output.len(),
            SERIALIZED_SIZE
        );
    }

    let payload = serde_json::to_vec(state).context("failed to encode save state")?;
    if payload.len() > SERIALIZED_SIZE - HEADER_SIZE {
        bail!("save state exceeds the fixed serialization capacity");
    }

    output.fill(0);
    output[..8].copy_from_slice(MAGIC);
    output[8..12].copy_from_slice(&VERSION.to_le_bytes());
    output[12..16].copy_from_slice(&(payload.len() as u32).to_le_bytes());
    output[16..20].copy_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    output[HEADER_SIZE..HEADER_SIZE + payload.len()].copy_from_slice(&payload);
    Ok(())
}

pub(crate) fn decode(input: &[u8]) -> Result<EmulatorState> {
    decode_value(input)
}

fn decode_value<T: DeserializeOwned>(input: &[u8]) -> Result<T> {
    if input.len() < HEADER_SIZE {
        bail!("save state is truncated");
    }
    if &input[..8] != MAGIC {
        bail!("invalid save-state signature");
    }

    let version = u32::from_le_bytes(input[8..12].try_into().unwrap());
    if version != VERSION {
        bail!("unsupported save-state version {version}");
    }

    let payload_len = u32::from_le_bytes(input[12..16].try_into().unwrap()) as usize;
    let expected_crc = u32::from_le_bytes(input[16..20].try_into().unwrap());
    let payload_end = HEADER_SIZE
        .checked_add(payload_len)
        .filter(|&end| end <= input.len() && end <= SERIALIZED_SIZE)
        .context("invalid save-state payload length")?;
    let payload = &input[HEADER_SIZE..payload_end];
    if crc32fast::hash(payload) != expected_crc {
        bail!("save-state checksum mismatch");
    }

    serde_json::from_slice(payload).context("failed to decode save state")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestValue {
        text: String,
        number: u64,
    }

    #[test]
    fn codec_round_trip_and_checksum() {
        let value = TestValue {
            text: "Native32".to_string(),
            number: 42,
        };
        let mut output = vec![0u8; SERIALIZED_SIZE];
        let payload = serde_json::to_vec(&value).unwrap();
        output[..8].copy_from_slice(MAGIC);
        output[8..12].copy_from_slice(&VERSION.to_le_bytes());
        output[12..16].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        output[16..20].copy_from_slice(&crc32fast::hash(&payload).to_le_bytes());
        output[HEADER_SIZE..HEADER_SIZE + payload.len()].copy_from_slice(&payload);

        assert_eq!(decode_value::<TestValue>(&output).unwrap(), value);
        output[HEADER_SIZE] ^= 1;
        assert!(decode_value::<TestValue>(&output).is_err());
    }
}
