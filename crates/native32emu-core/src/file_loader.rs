// File loader: reads game files, skips thumbnails, locates headers, parses resource tables.

use std::collections::HashMap;
use std::io::Read;

use crate::error::{EmuError, Result};
use crate::header_decryptor::decrypt_header;
use crate::image_decoder::{decode_image_argb, decode_image_yuv, RgbaImage};
use flate2::read::DeflateDecoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    Image = 1,
    Movie = 2,
    Button = 3,
    Action = 4,
    Sound = 5,
}

impl ObjectType {
    pub fn from_u16(val: u16) -> Option<Self> {
        match val {
            1 => Some(ObjectType::Image),
            2 => Some(ObjectType::Movie),
            3 => Some(ObjectType::Button),
            4 => Some(ObjectType::Action),
            5 => Some(ObjectType::Sound),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FrameObject {
    pub obj_type: ObjectType,
    pub index: u16,
    pub x: i16,
    pub y: i16,
    pub depth: u16,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct MovieFrame {
    pub image: u16,
    pub x: i16,
    pub y: i16,
    pub action: u16,
    pub sound: u16,
    pub reserved: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    MP3,
    Raw,
}

#[derive(Debug, Clone)]
pub struct SoundData {
    pub format: AudioFormat,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Colorspace {
    YUV,
    ARGB,
}

pub struct Native32Reader {
    pub data: Vec<u8>,
    pub idx: usize,
    pub colorspace: Colorspace,
    pub resolution: (u32, u32),
    pub base: usize,
    // Resource table offsets (relative to base)
    pub frame_idx: u32,
    pub image_idx: u32,
    pub action_idx: u32,
    pub movie_idx: u32,
    pub button_idx: u32,
    pub button_cond_idx: u32,
    pub mp3_offset: u32,
    pub sound_table: usize,
    // Caches
    actions_cache: Vec<Option<(crate::actions::Action, Option<ActionPayload>)>>,
    images_cache: HashMap<u32, Option<RgbaImage>>,
    frames_cache: HashMap<u32, Vec<FrameObject>>,
    movies_cache: HashMap<u32, Vec<MovieFrame>>,
    sound_cache: HashMap<u32, SoundData>,
    button_events_cache: HashMap<u32, Vec<(u16, u16)>>,
}

#[derive(Debug, Clone)]
pub enum ActionPayload {
    String(String),
    Integer(i16),
}

fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_i16_le(data: &[u8], offset: usize) -> i16 {
    i16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32_le(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

impl Native32Reader {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            idx: 0,
            colorspace: Colorspace::YUV,
            resolution: (320, 240),
            base: 0,
            frame_idx: 0,
            image_idx: 0,
            action_idx: 0,
            movie_idx: 0,
            button_idx: 0,
            button_cond_idx: 0,
            mp3_offset: 0,
            sound_table: 0,
            actions_cache: vec![None], // 1-based index, slot 0 unused
            images_cache: HashMap::new(),
            frames_cache: HashMap::new(),
            movies_cache: HashMap::new(),
            sound_cache: HashMap::new(),
            button_events_cache: HashMap::new(),
        }
    }

    fn get_str(&self, offset: usize) -> String {
        let mut s = String::new();
        let mut pos = offset;
        while pos < self.data.len() && self.data[pos] != 0 {
            s.push(self.data[pos] as char);
            pos += 1;
        }
        s
    }

    /// Skip the optional SWFT thumbnail at the beginning of the file.
    pub fn skip_thumbnail(&mut self) {
        if self.idx + 4 <= self.data.len() && &self.data[self.idx..self.idx + 4] == b"SWFT" {
            self.idx += 4;
            // Read thumbnail header: colorspace(4) + flags(4) + width(2) + height(2) + size(4)
            if self.idx + 0x10 <= self.data.len() {
                let size = read_u32_le(&self.data, self.idx + 0x0c) as usize;
                log::info!("Skipping SWFT thumbnail ({} bytes)", 0x10 + size);
                self.idx += 0x10 + size;
            }
        }
    }

    /// Scan for the `_YUV` or `ARGB` colorspace marker.
    pub fn find_header(&mut self) -> Result<()> {
        while self.idx + 4 <= self.data.len() {
            let magic = &self.data[self.idx..self.idx + 4];
            if magic == b"_YUV" {
                self.colorspace = Colorspace::YUV;
                log::info!("Found _YUV Native32 header at 0x{:x}", self.idx);
                return Ok(());
            } else if magic == b"ARGB" {
                self.colorspace = Colorspace::ARGB;
                log::info!("Found ARGB Native32 header at 0x{:x}", self.idx);
                return Ok(());
            }
            self.idx += 1;
        }
        Err(EmuError::HeaderNotFound)
    }

    /// Parse the header: generator string, base offset, encrypted section, resource table offsets.
    pub fn process_header(&mut self) -> Result<()> {
        // Parse generator string (48 bytes starting at colorspace+4)
        let gen_start = self.idx + 0x04;
        if gen_start + 0x30 <= self.data.len() {
            let gen_bytes = &self.data[gen_start..gen_start + 0x30];
            let gen_str = String::from_utf8_lossy(gen_bytes)
                .trim_end_matches('\0')
                .to_string();
            log::info!("Generator: {}", gen_str);

            // Try to parse Resolution_WxH
            if let Some((w, h)) = parse_resolution(&gen_str) {
                if w > 0 && h > 0 {
                    self.resolution = (w, h);
                    log::info!("Resolution: {}x{}", w, h);
                }
            }
        }

        // Base offset is at colorspace + 0x60
        self.base = self.idx + 0x60;
        if self.base + 0x38 > self.data.len() {
            return Err(EmuError::InvalidFile("Header extends beyond file".into()));
        }

        // Read header fields before encrypted section:
        // 8 bytes: fps_color_size(2) + action_stack_var(2) + button_movieclip(2) + buffer_sound(2)
        // 16 bytes: load_addr(4) + binary_size(4) + mp3_offset(4) + mp3_length(4)
        // Total: 24 = 0x18 bytes
        let packed_block_count = (read_u16_le(&self.data, self.base) & 0x00FF) as usize;
        let binary_size = read_u32_le(&self.data, self.base + 0x0c) as usize;
        self.mp3_offset = read_u32_le(&self.data, self.base + 0x10);

        // Decrypt the 32-byte encrypted header at base + 0x18
        let enc_start = self.base + 0x18;
        if enc_start + 0x20 > self.data.len() {
            return Err(EmuError::InvalidFile(
                "Encrypted header extends beyond file".into(),
            ));
        }
        let encrypted = &self.data[enc_start..enc_start + 0x20];
        let decrypted = decrypt_header(encrypted).ok_or(EmuError::DecryptionFailed)?;

        // Parse resource table offsets from decrypted header
        // Layout: unkh(4) + magic(4) + frame(4) + image(4) + action(4) + movie(4) + button(4) + button_cond(4)
        self.frame_idx = read_u32_le(&decrypted, 0x08);
        self.image_idx = read_u32_le(&decrypted, 0x0c);
        self.action_idx = read_u32_le(&decrypted, 0x10);
        self.movie_idx = read_u32_le(&decrypted, 0x14);
        self.button_idx = read_u32_le(&decrypted, 0x18);
        self.button_cond_idx = read_u32_le(&decrypted, 0x1c);

        self.expand_packed_assets(packed_block_count, binary_size)?;

        log::info!("Frame table: 0x{:08x}", self.frame_idx);
        log::info!("Image table: 0x{:08x}", self.image_idx);
        log::info!("Action table: 0x{:08x}", self.action_idx);
        log::info!("Movie table: 0x{:08x}", self.movie_idx);
        log::info!("Button table: 0x{:08x}", self.button_idx);
        log::info!("Button cond table: 0x{:08x}", self.button_cond_idx);

        // Skip cursor data (2 bytes width + 2 bytes height + pixel data)
        let cursor_pos = enc_start + 0x20; // after the 32-byte encrypted header
        if cursor_pos + 4 <= self.data.len() {
            let cursor_w = read_u16_le(&self.data, cursor_pos) as usize;
            let cursor_h = read_u16_le(&self.data, cursor_pos + 2) as usize;
            let cursor_size = 2 * cursor_w * cursor_h;
            self.sound_table = cursor_pos + 4 + cursor_size;
            log::info!("Sound table at 0x{:x}", self.sound_table);
        }

        Ok(())
    }

    fn expand_packed_assets(&mut self, block_count: usize, binary_size: usize) -> Result<()> {
        if block_count == 0 {
            return Ok(());
        }

        let image_table = self
            .base
            .checked_add(self.image_idx as usize)
            .ok_or_else(|| EmuError::InvalidFile("Image table offset overflow".into()))?;
        let first_image_ptr = self
            .data
            .get(image_table..image_table + 4)
            .ok_or_else(|| EmuError::InvalidFile("Packed image table is truncated".into()))?;
        let first_image_offset = u32::from_le_bytes(first_image_ptr.try_into().unwrap()) as usize;
        if first_image_offset == u32::MAX as usize {
            return Err(EmuError::InvalidFile(
                "Packed asset stream has no first image".into(),
            ));
        }

        let asset_start = self
            .base
            .checked_add(first_image_offset)
            .ok_or_else(|| EmuError::InvalidFile("Packed asset offset overflow".into()))?;
        let max_expanded_size = binary_size.checked_sub(first_image_offset).ok_or_else(|| {
            EmuError::InvalidFile("Packed asset offset exceeds binary size".into())
        })?;
        let packed_offset = first_image_offset
            .checked_add(0x7FF)
            .ok_or_else(|| EmuError::InvalidFile("Packed asset alignment overflow".into()))?
            & !0x7FF;
        let mut block_start = self
            .base
            .checked_add(packed_offset)
            .ok_or_else(|| EmuError::InvalidFile("Packed block offset overflow".into()))?;
        let mut expanded = Vec::new();

        for block_index in 0..block_count {
            let size_bytes = self.data.get(block_start..block_start + 4).ok_or_else(|| {
                EmuError::InvalidFile(format!(
                    "Packed asset block {} header is truncated",
                    block_index + 1
                ))
            })?;
            let block_size = u32::from_le_bytes(size_bytes.try_into().unwrap()) as usize;
            if block_size <= 4 {
                return Err(EmuError::InvalidFile(format!(
                    "Packed asset block {} has invalid size {block_size}",
                    block_index + 1
                )));
            }
            let block_end = block_start.checked_add(block_size).ok_or_else(|| {
                EmuError::InvalidFile(format!(
                    "Packed asset block {} size overflow",
                    block_index + 1
                ))
            })?;
            let compressed = self.data.get(block_start + 4..block_end).ok_or_else(|| {
                EmuError::InvalidFile(format!(
                    "Packed asset block {} extends beyond the file",
                    block_index + 1
                ))
            })?;
            let remaining_capacity = max_expanded_size.saturating_sub(expanded.len());
            DeflateDecoder::new(compressed)
                .take(remaining_capacity.saturating_add(1) as u64)
                .read_to_end(&mut expanded)
                .map_err(|error| {
                    EmuError::InvalidFile(format!(
                        "Failed to decompress packed asset block {}: {error}",
                        block_index + 1
                    ))
                })?;
            if expanded.len() > max_expanded_size {
                return Err(EmuError::InvalidFile(
                    "Packed assets exceed the declared binary size".into(),
                ));
            }
            block_start = block_end;
        }

        if block_start != self.data.len() {
            return Err(EmuError::InvalidFile(format!(
                "Packed asset blocks end at 0x{block_start:x}, file ends at 0x{:x}",
                self.data.len()
            )));
        }
        if asset_start > self.data.len() {
            return Err(EmuError::InvalidFile(
                "Packed asset destination extends beyond the file".into(),
            ));
        }

        let expanded_size = expanded.len();
        self.data.truncate(asset_start);
        self.data.extend_from_slice(&expanded);
        log::info!(
            "Expanded {block_count} packed asset block(s) to {expanded_size} bytes at 0x{asset_start:x}"
        );
        Ok(())
    }

    /// Initialize: skip thumbnail, find header, process header.
    pub fn init(&mut self) -> Result<()> {
        self.skip_thumbnail();
        self.find_header()?;
        self.process_header()?;
        Ok(())
    }

    /// Disassemble a single action instruction at the given 1-based index.
    fn disassemble_action(
        &self,
        index: u32,
    ) -> Option<(crate::actions::Action, Option<ActionPayload>)> {
        let ptr = self.base + self.action_idx as usize + ((index - 1) * 8) as usize;
        if ptr + 8 > self.data.len() {
            return None;
        }
        let opcode = read_u32_le(&self.data, ptr);
        let payload_val = read_u32_le(&self.data, ptr + 4);

        let act = crate::actions::Action::from_u32(opcode)?;

        if payload_val == 0 || act == crate::actions::Action::End {
            return Some((act, None));
        }

        let payload_idx = self.base + payload_val as usize;
        if payload_idx >= self.data.len() {
            return Some((act, None));
        }

        match act {
            crate::actions::Action::If
            | crate::actions::Action::GotoFrame
            | crate::actions::Action::GotoFrame2
            | crate::actions::Action::Jump => {
                if payload_idx + 2 <= self.data.len() {
                    let val = read_i16_le(&self.data, payload_idx);
                    Some((act, Some(ActionPayload::Integer(val))))
                } else {
                    Some((act, None))
                }
            }
            _ => {
                let s = self.get_str(payload_idx);
                Some((act, Some(ActionPayload::String(s))))
            }
        }
    }

    /// Get an action by 1-based index (with lazy caching).
    pub fn get_action(
        &mut self,
        index: u32,
    ) -> Option<(crate::actions::Action, Option<ActionPayload>)> {
        while index as usize >= self.actions_cache.len() {
            let i = self.actions_cache.len() as u32;
            let action = self.disassemble_action(i);
            self.actions_cache.push(action);
        }
        self.actions_cache[index as usize].clone()
    }

    /// Get an action by 1-based index (immutable, assumes already cached).
    pub fn get_action_cached(
        &self,
        index: u32,
    ) -> Option<&(crate::actions::Action, Option<ActionPayload>)> {
        if (index as usize) < self.actions_cache.len() {
            self.actions_cache[index as usize].as_ref()
        } else {
            None
        }
    }

    /// Parse a frame's object list by 1-based frame index.
    pub fn get_frame(&mut self, frame: u32) -> Option<Vec<FrameObject>> {
        if let Some(cached) = self.frames_cache.get(&frame) {
            return Some(cached.clone());
        }

        let ptr_idx = self.base + self.frame_idx as usize + 4 * (frame - 1) as usize;
        if ptr_idx + 4 > self.data.len() {
            return None;
        }
        let offset = read_u32_le(&self.data, ptr_idx);
        if offset == 0 || offset as usize > self.data.len() {
            return None;
        }

        let mut objects = Vec::new();
        let mut i = self.base + offset as usize;
        while i + 0x10 <= self.data.len() {
            let obj_type = read_u16_le(&self.data, i);
            if obj_type == 0x0000 || obj_type == 0xFFFF {
                break;
            }
            let index = read_u16_le(&self.data, i + 2);
            let x = read_i16_le(&self.data, i + 4);
            let y = read_i16_le(&self.data, i + 6);
            let depth = read_u16_le(&self.data, i + 8);
            let name_ptr = read_u32_le(&self.data, i + 12);

            let name = if name_ptr != 0 {
                Some(self.get_str(self.base + name_ptr as usize))
            } else {
                None
            };

            if let Some(ot) = ObjectType::from_u16(obj_type) {
                objects.push(FrameObject {
                    obj_type: ot,
                    index,
                    x,
                    y,
                    depth,
                    name,
                });
            }
            i += 0x10;
        }

        self.frames_cache.insert(frame, objects.clone());
        Some(objects)
    }

    /// Parse a movie's frame list by 1-based movie index.
    pub fn get_movie(&mut self, movie: u32) -> Vec<MovieFrame> {
        if let Some(cached) = self.movies_cache.get(&movie) {
            return cached.clone();
        }

        let idx_ptr = self.base + self.movie_idx as usize + 4 * (movie - 1) as usize;
        if idx_ptr + 4 > self.data.len() {
            return Vec::new();
        }
        let ptr = read_u32_le(&self.data, idx_ptr) as usize + self.base;

        let mut frames = Vec::new();
        let mut pos = ptr;
        while pos + 0x0C <= self.data.len() {
            let img = read_u16_le(&self.data, pos);
            if img == 0xFFFF || img == 0x0000 {
                break;
            }
            if img as usize >= self.data.len() {
                break;
            }
            frames.push(MovieFrame {
                image: img,
                x: read_i16_le(&self.data, pos + 2),
                y: read_i16_le(&self.data, pos + 4),
                action: read_u16_le(&self.data, pos + 6),
                sound: read_u16_le(&self.data, pos + 8),
                reserved: read_u16_le(&self.data, pos + 10),
            });
            pos += 0x0C;
        }

        self.movies_cache.insert(movie, frames.clone());
        frames
    }

    /// Get decoded image by 1-based index (with lazy caching).
    pub fn get_image(&mut self, index: u32) -> Option<RgbaImage> {
        if let Some(cached) = self.images_cache.get(&index) {
            return cached.clone();
        }

        let ptr = self.base + self.image_idx as usize + 4 * (index - 1) as usize;
        if ptr + 4 > self.data.len() {
            self.images_cache.insert(index, None);
            return None;
        }
        let img_offset = read_u32_le(&self.data, ptr);
        if img_offset == 0xFFFFFFFF {
            self.images_cache.insert(index, None);
            return None;
        }

        let img_start = self.base + img_offset as usize;
        if img_start + 8 > self.data.len() {
            self.images_cache.insert(index, None);
            return None;
        }

        let img_size = read_u32_le(&self.data, img_start + 4) as usize;
        let img_end = std::cmp::min(img_start + 8 + img_size, self.data.len());

        let img_data = &self.data[img_start..img_end];
        let result = match self.colorspace {
            Colorspace::ARGB => decode_image_argb(img_data),
            Colorspace::YUV => decode_image_yuv(img_data),
        };

        self.images_cache.insert(index, result.clone());
        result
    }

    /// Get sound data by 1-based index.
    pub fn get_sound(&mut self, idx: u32) -> Option<SoundData> {
        if let Some(cached) = self.sound_cache.get(&idx) {
            return Some(cached.clone());
        }

        let table_offset = usize::try_from(idx.checked_sub(1)?).ok()?.checked_mul(4)?;
        let table_idx = self.sound_table.checked_add(table_offset)?;
        self.data.get(table_idx..table_idx.checked_add(4)?)?;
        let ptr = read_u32_le(&self.data, table_idx);
        let flags = ptr & 0xF0000000;
        let addr = (ptr & 0x0FFFFFFF) as usize;

        let result = if flags == 0xF0000000 {
            // MP3 audio
            let begin = self
                .base
                .checked_add(self.mp3_offset as usize)?
                .checked_add(addr)?;
            self.data.get(begin..begin.checked_add(6)?)?;
            let size = read_u32_le(&self.data, begin) as usize;
            let data_start = begin.checked_add(6)?;
            let data_end = data_start.checked_add(size)?;
            let data = self.data.get(data_start..data_end)?;
            SoundData {
                format: AudioFormat::MP3,
                data: data.to_vec(),
            }
        } else if flags == 0x00000000 {
            // Raw PCM
            let begin = self.base.checked_add(addr)?;
            self.data.get(begin..begin.checked_add(4)?)?;
            let size = read_u32_le(&self.data, begin) as usize;
            let data_start = begin.checked_add(4)?;
            let data_end = data_start.checked_add(size)?;
            let raw_data = self.data.get(data_start..data_end)?;

            if self.colorspace == Colorspace::YUV {
                // Big-endian to little-endian conversion
                SoundData {
                    format: AudioFormat::Raw,
                    data: endian_swap_pcm16(raw_data),
                }
            } else {
                SoundData {
                    format: AudioFormat::Raw,
                    data: raw_data.to_vec(),
                }
            }
        } else {
            return None;
        };

        self.sound_cache.insert(idx, result.clone());
        Some(result)
    }

    /// Get button events by 1-based button index.
    pub fn get_button_events(&mut self, button: u32) -> Vec<(u16, u16)> {
        if let Some(cached) = self.button_events_cache.get(&button) {
            return cached.clone();
        }

        let cond_table_idx = self.base + self.button_cond_idx as usize + (button - 1) as usize * 4;
        if cond_table_idx + 4 > self.data.len() {
            return Vec::new();
        }
        let ptr = read_u32_le(&self.data, cond_table_idx) as usize + self.base;
        if ptr + 2 > self.data.len() {
            return Vec::new();
        }

        let total_act_len = read_u16_le(&self.data, ptr) as usize;
        let mut pos = ptr + 2;
        let mut i = 0;
        let mut events = Vec::new();

        while i < total_act_len && pos + 6 <= self.data.len() {
            let keycode = read_u16_le(&self.data, pos);
            let act_len = read_u16_le(&self.data, pos + 2);
            let event = read_u16_le(&self.data, pos + 4);
            events.push((keycode, event));
            i += act_len as usize;
            pos += 6;
        }

        self.button_events_cache.insert(button, events.clone());
        events
    }

    /// Pre-cache all actions for the entire file.
    pub fn cache_all_actions(&mut self) {
        let mut i = 1u32;
        loop {
            if self.disassemble_action(i).is_none() {
                break;
            }
            // Already cached by disassemble_action call above
            i += 1;
            // Ensure cache is populated
            while (i as usize) >= self.actions_cache.len() {
                let idx = self.actions_cache.len() as u32;
                let action = self.disassemble_action(idx);
                self.actions_cache.push(action);
            }
            // Check if the last one was None
            if self.actions_cache.last().is_none_or(|a| a.is_none()) {
                break;
            }
        }
    }
}

fn parse_resolution(gen_str: &str) -> Option<(u32, u32)> {
    // Match "Resolution_<width>_<height>"
    if let Some(rest) = gen_str.strip_prefix("Resolution_") {
        let parts: Vec<&str> = rest.split('_').collect();
        if parts.len() >= 2 {
            if let (Ok(w), Ok(h)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                return Some((w, h));
            }
        }
    }
    None
}

fn endian_swap_pcm16(data: &[u8]) -> Vec<u8> {
    let len = data.len() & 0xFFFFFFFE;
    let mut result = vec![0u8; len];
    for (out, input) in result
        .as_chunks_mut::<2>()
        .0
        .iter_mut()
        .zip(data[..len].as_chunks::<2>().0.iter())
    {
        out[0] = input[1];
        out[1] = input[0];
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::DeflateEncoder;
    use flate2::Compression;
    use std::io::Write;

    // === parse_resolution tests ===

    #[test]
    fn test_parse_resolution_valid() {
        assert_eq!(parse_resolution("Resolution_320_240"), Some((320, 240)));
        assert_eq!(parse_resolution("Resolution_640_480"), Some((640, 480)));
        assert_eq!(parse_resolution("Resolution_1920_1080"), Some((1920, 1080)));
    }

    #[test]
    fn test_parse_resolution_invalid_prefix() {
        assert_eq!(parse_resolution("Garbage_320_240"), None);
        assert_eq!(parse_resolution("resolution_320_240"), None); // case-sensitive
    }

    #[test]
    fn test_parse_resolution_missing_parts() {
        assert_eq!(parse_resolution("Resolution_"), None);
        assert_eq!(parse_resolution("Resolution_320"), None);
        assert_eq!(parse_resolution("Resolution_abc_240"), None);
        assert_eq!(parse_resolution("Resolution_320_xyz"), None);
    }

    #[test]
    fn test_parse_resolution_empty() {
        assert_eq!(parse_resolution(""), None);
    }

    #[test]
    fn test_parse_resolution_zero_dimensions() {
        // 0 is technically valid parse, caller checks for > 0
        assert_eq!(parse_resolution("Resolution_0_0"), Some((0, 0)));
    }

    // === endian_swap_pcm16 tests ===

    #[test]
    fn test_endian_swap_pcm16_two_bytes() {
        // 2 bytes [0x12, 0x34] stay 2 bytes with endian swap
        let input = vec![0x12, 0x34];
        let output = endian_swap_pcm16(&input);
        assert_eq!(output.len(), 2);
        assert_eq!(output, vec![0x34, 0x12]);
    }

    #[test]
    fn test_endian_swap_pcm16_four_bytes() {
        let input = vec![0xAA, 0xBB, 0xCC, 0xDD];
        let output = endian_swap_pcm16(&input);
        assert_eq!(output.len(), 4);
        // Pairs: (0xAA, 0xBB), (0xCC, 0xDD)
        assert_eq!(output, vec![0xBB, 0xAA, 0xDD, 0xCC]);
    }

    #[test]
    fn test_endian_swap_pcm16_empty() {
        let output = endian_swap_pcm16(&[]);
        assert!(output.is_empty());
    }

    #[test]
    fn test_endian_swap_pcm16_odd_length() {
        // Odd length: last byte is ignored (len & 0xFFFFFFFE)
        let input = vec![0x12, 0x34, 0x56];
        let output = endian_swap_pcm16(&input);
        // len = 2 (0x56 ignored)
        assert_eq!(output.len(), 2);
        assert_eq!(output, vec![0x34, 0x12]);
    }

    // === ObjectType::from_u16 tests ===

    #[test]
    fn test_object_type_from_u16_valid() {
        assert_eq!(ObjectType::from_u16(1), Some(ObjectType::Image));
        assert_eq!(ObjectType::from_u16(2), Some(ObjectType::Movie));
        assert_eq!(ObjectType::from_u16(3), Some(ObjectType::Button));
        assert_eq!(ObjectType::from_u16(4), Some(ObjectType::Action));
        assert_eq!(ObjectType::from_u16(5), Some(ObjectType::Sound));
    }

    #[test]
    fn test_object_type_from_u16_invalid() {
        assert_eq!(ObjectType::from_u16(0), None);
        assert_eq!(ObjectType::from_u16(6), None);
        assert_eq!(ObjectType::from_u16(0xFFFF), None);
    }

    // === read helper tests ===

    #[test]
    fn test_read_u16_le() {
        let data = [0x34, 0x12, 0x78, 0x56];
        assert_eq!(read_u16_le(&data, 0), 0x1234);
        assert_eq!(read_u16_le(&data, 2), 0x5678);
    }

    #[test]
    fn test_read_i16_le() {
        let data = [0xFF, 0xFF]; // -1 in i16
        assert_eq!(read_i16_le(&data, 0), -1);
        let data = [0x01, 0x00]; // 1
        assert_eq!(read_i16_le(&data, 0), 1);
    }

    #[test]
    fn test_read_u32_le() {
        let data = [0x78, 0x56, 0x34, 0x12];
        assert_eq!(read_u32_le(&data, 0), 0x12345678);
    }

    // === Native32Reader basic tests ===

    #[test]
    fn test_reader_new_defaults() {
        let reader = Native32Reader::new(vec![0u8; 100]);
        assert_eq!(reader.colorspace, Colorspace::YUV);
        assert_eq!(reader.resolution, (320, 240));
        assert_eq!(reader.idx, 0);
        assert_eq!(reader.base, 0);
    }

    #[test]
    fn test_expand_multiple_packed_asset_blocks() {
        let base = 0x20;
        let image_idx = 0x40;
        let first_image_offset = 0x83;
        let packed_start = base + 0x800;
        let chunks = [
            b"first asset block".as_slice(),
            b"second asset block".as_slice(),
        ];
        let mut data = vec![0u8; packed_start];
        data[base + image_idx..base + image_idx + 4]
            .copy_from_slice(&(first_image_offset as u32).to_le_bytes());

        for chunk in chunks {
            let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
            encoder.write_all(chunk).unwrap();
            let compressed = encoder.finish().unwrap();
            let block_size = (compressed.len() + 4) as u32;
            data.extend_from_slice(&block_size.to_le_bytes());
            data.extend_from_slice(&compressed);
        }

        let mut reader = Native32Reader::new(data);
        reader.base = base;
        reader.image_idx = image_idx as u32;
        reader
            .expand_packed_assets(2, first_image_offset + 36)
            .unwrap();

        let asset_start = base + first_image_offset;
        assert_eq!(
            &reader.data[asset_start..],
            b"first asset blocksecond asset block"
        );
    }

    #[test]
    fn test_reader_skip_thumbnail_no_swft() {
        let data = vec![0u8; 100];
        let mut reader = Native32Reader::new(data);
        reader.skip_thumbnail();
        assert_eq!(reader.idx, 0); // no SWFT header, idx unchanged
    }

    #[test]
    fn test_reader_skip_thumbnail_with_swft() {
        let mut data = vec![0u8; 256];
        // Write "SWFT" magic
        data[0..4].copy_from_slice(b"SWFT");
        // Thumbnail size at offset 0x0c (relative to idx+4): 16 bytes
        data[0x10..0x14].copy_from_slice(&16u32.to_le_bytes());

        let mut reader = Native32Reader::new(data);
        reader.skip_thumbnail();
        // Should skip: 4 (SWFT) + 0x10 (header) + 16 (size) = 36
        assert_eq!(reader.idx, 36);
    }

    #[test]
    fn test_reader_find_header_yuv() {
        let mut data = vec![0u8; 256];
        data[10..14].copy_from_slice(b"_YUV");
        let mut reader = Native32Reader::new(data);
        reader.idx = 10;
        assert!(reader.find_header().is_ok());
        assert_eq!(reader.colorspace, Colorspace::YUV);
    }

    #[test]
    fn test_reader_find_header_argb() {
        let mut data = vec![0u8; 256];
        data[20..24].copy_from_slice(b"ARGB");
        let mut reader = Native32Reader::new(data);
        reader.idx = 20;
        assert!(reader.find_header().is_ok());
        assert_eq!(reader.colorspace, Colorspace::ARGB);
    }

    #[test]
    fn test_reader_find_header_not_found() {
        let data = vec![0u8; 64];
        let mut reader = Native32Reader::new(data);
        let result = reader.find_header();
        assert!(result.is_err());
    }

    #[test]
    fn test_reader_get_str() {
        let data = b"hello\0world".to_vec();
        let reader = Native32Reader::new(data);
        assert_eq!(reader.get_str(0), "hello");
        assert_eq!(reader.get_str(6), "world");
    }

    #[test]
    fn test_reader_get_str_at_end_of_data() {
        let data = b"test".to_vec();
        let reader = Native32Reader::new(data);
        assert_eq!(reader.get_str(0), "test");
    }

    #[test]
    fn test_frame_object_debug() {
        let obj = FrameObject {
            obj_type: ObjectType::Image,
            index: 5,
            x: 10,
            y: 20,
            depth: 0,
            name: Some("test".to_string()),
        };
        let debug = format!("{:?}", obj);
        assert!(debug.contains("Image"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn test_movie_frame_copy() {
        let mf = MovieFrame {
            image: 1,
            x: 10,
            y: 20,
            action: 0,
            sound: 0,
            reserved: 0,
        };
        let mf2 = mf; // Copy
        assert_eq!(mf2.image, 1);
        assert_eq!(mf2.x, 10);
    }

    #[test]
    fn test_colorspace_equality() {
        assert_eq!(Colorspace::YUV, Colorspace::YUV);
        assert_eq!(Colorspace::ARGB, Colorspace::ARGB);
        assert_ne!(Colorspace::YUV, Colorspace::ARGB);
    }

    #[test]
    fn test_audio_format_equality() {
        assert_eq!(AudioFormat::MP3, AudioFormat::MP3);
        assert_eq!(AudioFormat::Raw, AudioFormat::Raw);
        assert_ne!(AudioFormat::MP3, AudioFormat::Raw);
    }

    #[test]
    fn test_action_payload_variants() {
        let s = ActionPayload::String("hello".to_string());
        let i = ActionPayload::Integer(42);
        match s {
            ActionPayload::String(v) => assert_eq!(v, "hello"),
            _ => panic!("expected String"),
        }
        match i {
            ActionPayload::Integer(v) => assert_eq!(v, 42),
            _ => panic!("expected Integer"),
        }
    }
}
