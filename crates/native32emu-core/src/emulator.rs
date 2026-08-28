// Core emulator implementation for libretro.

use crate::action_vm::{ActionProp, ActionVM, VmHost};
use crate::audio_engine::AudioEngine;
use crate::cheats::CheatManager;
use crate::content_loader::{ContentLoadKind, ContentLoader};
use crate::file_loader::{FrameObject, Native32Reader, ObjectType};
use crate::frame_player::FramePlayer;
use crate::input_handler::InputHandler;
use crate::renderer::Renderer;
use crate::save_manager::SaveManager;
use crate::sprite_system::{MovieState, SpriteSystem};
use anyhow::{Context, Result};
use std::path::{Component, Path, PathBuf};

fn timeline_sound_value(index: u16, loop_count: i16) -> u16 {
    let repeat = if loop_count == i16::MAX {
        0xFF
    } else {
        loop_count.clamp(0, 0xFE) as u16
    };
    (repeat << 8) | (index & 0xFF)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReturnFrame {
    path: PathBuf,
}

/// The platform-independent emulator core.
///
/// This holds all the game simulation state and is shared by both the
/// standalone (minifb window) front-end and the libretro core. Platform
/// specifics (window management, audio output device, input source) live in the
/// respective front-ends; the core only consumes already-decoded input button
/// codes and produces a framebuffer plus audio samples.
pub struct Emulator {
    pub filename: PathBuf,
    /// Stable root used to encode content paths inside portable save states.
    content_root: PathBuf,
    pub reader: Native32Reader,
    pub sprites: SpriteSystem,
    pub frame_player: FramePlayer,
    pub vm: ActionVM,
    pub audio: AudioEngine,
    pub renderer: Renderer,
    pub input: InputHandler,
    pub save_manager: SaveManager,
    pub cheats: CheatManager,
    pub content_loader: ContentLoader,
    /// Front-end menu (FHUI) directory browser for the game-list host calls.
    pub file_browser: crate::file_browser::FileBrowser,
    /// Navigation context saved/restored by the menu (GetContext/SaveContext).
    pub menu_context: Option<String>,
    pub cur_frame_objects: Vec<FrameObject>,
    pub tick_count: u64,
    pub time_ms: u32,
    /// MPEG-1 cutscene videos queued by SSL_PlayNext, played before the next
    /// SSL content loads.
    pub pending_videos: Vec<String>,
    /// Active cutscene player, if a video is currently playing.
    pub video_player: Option<crate::mpeg::VideoPlayer>,
    active_video_name: Option<String>,
    /// Parent SMF applications that can be restored by the shared back action.
    return_stack: Vec<ReturnFrame>,
    /// When true, cutscene videos are skipped automatically as soon as they
    /// become active, instead of waiting for the user to press A/B.
    pub auto_skip_cutscenes: bool,
    /// Temporary directory handle for ZIP extraction. When this field is
    /// dropped (e.g. when the Emulator is dropped), the directory is deleted.
    _temp_dir: Option<tempfile::TempDir>,
}

impl Emulator {
    /// Create a new emulator from a game file path.
    ///
    /// Supports .smf, .sgm, .ssl, and .zip files. For .zip files, extracts
    /// the archive and loads FHUI.smf or NA32UI.smf from its root.
    pub fn from_path(path: PathBuf, volume: u32) -> Result<Self> {
        // Check if this is a ZIP file
        let is_zip = is_zip_file(&path);
        let (game_path, _temp_dir) = if is_zip {
            let (td, p) = crate::archive_loader::load_zip_game(&path)?;
            (p, Some(td))
        } else {
            (path, None)
        };

        let data = std::fs::read(&game_path)
            .with_context(|| format!("Failed to read game file: {}", game_path.display()))?;

        let mut reader = Native32Reader::new(data);
        reader.init().context("Failed to initialize game file")?;

        let resolution = reader.resolution;
        let colorspace = reader.colorspace;

        let save_manager = SaveManager::new(&game_path);

        let content_root = game_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_path_buf();

        Ok(Self {
            filename: game_path,
            content_root,
            reader,
            sprites: SpriteSystem::new(),
            frame_player: FramePlayer::new(),
            vm: ActionVM::new(),
            audio: AudioEngine::new(colorspace, volume),
            renderer: Renderer::new(resolution.0, resolution.1),
            input: InputHandler::new(),
            save_manager,
            cheats: CheatManager::new(),
            content_loader: ContentLoader::new(),
            file_browser: crate::file_browser::FileBrowser::new(),
            menu_context: None,
            cur_frame_objects: Vec::new(),
            tick_count: 0,
            time_ms: 0,
            pending_videos: Vec::new(),
            video_player: None,
            active_video_name: None,
            return_stack: Vec::new(),
            auto_skip_cutscenes: false,
            _temp_dir,
        })
    }

    /// Get the game resolution.
    pub fn get_resolution(&self) -> (u32, u32) {
        self.reader.resolution
    }

    /// Get the audio sample rate based on colorspace.
    pub fn get_audio_sample_rate(&self) -> f64 {
        match self.reader.colorspace {
            crate::file_loader::Colorspace::YUV => 11025.0,
            crate::file_loader::Colorspace::ARGB => 22050.0,
        }
    }

    /// Whether the emulator can return to a parent SMF application.
    pub fn can_return_to_parent(&self) -> bool {
        !self.return_stack.is_empty()
    }

    /// Return to the SMF application that launched the current child.
    ///
    /// Returns `true` after reloading the parent and `false` when the frontend
    /// should handle the back action by ending the current emulation session.
    pub fn try_return_to_parent(&mut self) -> Result<bool> {
        let Some(frame) = self.return_stack.last().cloned() else {
            return Ok(false);
        };

        self.load_path(frame.path)?;
        self.return_stack.pop();
        Ok(true)
    }

    /// Reload the emulator from the given file path, performing a full state
    /// reset and starting a new navigation session.
    pub fn reload_from_path(&mut self, path: PathBuf) -> Result<()> {
        self.load_path(path)?;
        self.return_stack.clear();
        Ok(())
    }

    fn load_path(&mut self, path: PathBuf) -> Result<()> {
        let data = std::fs::read(&path)
            .with_context(|| format!("Failed to read game file: {}", path.display()))?;
        let mut reader = Native32Reader::new(data);
        reader.init()?;

        self.audio.stop_all();
        self.renderer.clear_sprite_overrides();
        self.pending_videos.clear();
        self.video_player = None;
        self.active_video_name = None;
        self.tick_count = 0;
        self.time_ms = 0;
        self.sprites = SpriteSystem::new();
        self.frame_player = FramePlayer::new();
        self.vm = ActionVM::new();
        self.content_loader = ContentLoader::new();
        self.cur_frame_objects.clear();
        self.menu_context = None;
        self.filename = path;
        self.reader = reader;
        self.audio = AudioEngine::new(self.reader.colorspace, (self.audio.volume * 100.0) as u32);
        self.save_manager = SaveManager::new(&self.filename);

        Ok(())
    }

    /// Set button state from libretro input.
    pub fn set_buttons(&mut self, keycodes: &[u16]) {
        self.input.set_buttons(keycodes);
    }

    /// Enable or disable automatic skipping of cutscene videos.
    pub fn set_auto_skip_cutscenes(&mut self, enabled: bool) {
        self.auto_skip_cutscenes = enabled;
    }

    /// Load a new frame and set up sprites.
    pub fn load_frame(&mut self, frame: u32) {
        if let Some(objects) = self.reader.get_frame(frame) {
            self.cur_frame_objects = objects.clone();
            self.sprites.update_for_frame(&objects);
            for sound in objects
                .iter()
                .filter(|object| object.obj_type == ObjectType::Sound)
            {
                let sound_value = timeline_sound_value(sound.index, sound.x);
                self.audio.play_sound(&mut self.reader, sound_value, "");
            }
        } else {
            log::warn!("Failed to load frame {}", frame);
            self.cur_frame_objects.clear();
        }
    }

    /// Run one tick of the emulation (called at 30fps).
    pub fn tick(&mut self) {
        self.tick_count += 1;

        // While a cutscene is playing (or queued), drive video playback instead
        // of the normal timeline.
        if self.is_cutscene_active() {
            self.cutscene_tick();
            self.apply_cheats();
            self.time_ms += 1000 / 30;
            return;
        }

        // Advance main timeline
        self.frame_player.tick();

        // Handle pending frame switch
        if let Some(next) = self.frame_player.take_next_frame() {
            self.frame_player.current_frame = next;
            self.load_frame(next);

            // Execute frame actions first
            let frame_action_indices: Vec<u32> = self
                .cur_frame_objects
                .iter()
                .filter(|o| o.obj_type == ObjectType::Action)
                .map(|o| o.index as u32)
                .collect();
            let self_ptr = self as *mut Emulator;
            unsafe {
                for action_idx in frame_action_indices {
                    (*self_ptr)
                        .vm
                        .run(&mut (*self_ptr).reader, &mut *self_ptr, action_idx, "");
                }
            }
        }

        // Advance movie frames
        let tick = self.tick_count;

        // Determine which movies need frame advancement
        let movie_advancements: Vec<(String, isize)> = {
            let mut advancements = Vec::new();
            for (name, movie) in self.sprites.iter_mut() {
                if !movie.playing || movie.next_frame.is_some() {
                    continue;
                }
                let movie_frames = self.reader.get_movie(movie.movie);
                if movie_frames.is_empty() {
                    continue;
                }
                if tick.is_multiple_of(2) {
                    if movie.frame < movie_frames.len() - 1 {
                        advancements.push((name.clone(), movie.frame as isize + 1));
                    } else {
                        advancements.push((name.clone(), 0)); // loop
                    }
                }
            }
            advancements
        };

        // Apply advancements
        for (name, next_frame) in movie_advancements {
            if let Some(movie) = self.sprites.get_mut(&name) {
                movie.next_frame = Some(next_frame);
            }
        }

        // Process pending movie frame switches
        let names: Vec<String> = self.sprites.sprites.keys().cloned().collect();
        for name in &names {
            let next = self.sprites.get(name).and_then(|m| m.next_frame);
            if let Some(next_frame) = next {
                if let Some(movie) = self.sprites.get_mut(name) {
                    if next_frame == -1 {
                        movie.frame = 0;
                    } else if (next_frame as usize) < self.reader.get_movie(movie.movie).len() {
                        movie.frame = next_frame as usize;
                    }
                    movie.next_frame = None;
                }

                let frame_data = {
                    let movie = match self.sprites.get(name) {
                        Some(m) => m,
                        None => continue,
                    };
                    let movie_frames = self.reader.get_movie(movie.movie);
                    if movie.frame < movie_frames.len() {
                        let mf = movie_frames[movie.frame];
                        Some((mf.sound, mf.action))
                    } else {
                        None
                    }
                };

                if let Some((sound, action)) = frame_data {
                    // Play sound if present and track it on the movie
                    if sound != 0 {
                        if let Some(channel) = self.audio.play_sound(&mut self.reader, sound, name)
                        {
                            if let Some(movie) = self.sprites.get_mut(name) {
                                movie.sound_channel = Some(channel);
                            }
                        }
                    }

                    // Execute movie frame action
                    if action != 0 {
                        let action_idx = action as u32;
                        let self_ptr = self as *mut Emulator;
                        unsafe {
                            (*self_ptr).vm.run(
                                &mut (*self_ptr).reader,
                                &mut *self_ptr,
                                action_idx,
                                name,
                            );
                        }
                    }
                }
            }
        }

        // Release movies whose sound channel reached the end.
        for (_, movie) in self.sprites.iter_mut() {
            if movie
                .sound_channel
                .is_some_and(|channel| !self.audio.is_channel_playing(channel))
            {
                movie.sound_channel = None;
            }
        }

        self.handle_buttons();

        // Handle queued content switching.
        if self.content_loader.has_pending() && !self.is_cutscene_active() {
            if let Some((filename, kind)) = self.content_loader.take_pending() {
                if let Err(e) = self.switch_content(&filename, kind) {
                    log::error!("Failed to switch content: {}", e);
                }
            }
        }

        self.apply_cheats();
        self.time_ms += 1000 / 30;
    }

    pub fn set_cheat_debug_logging(&mut self, enabled: bool, interval_frames: u64) {
        self.cheats.set_debug_logging(enabled, interval_frames);
    }

    pub fn set_cheat_debug_variable_filter(&mut self, filter: Option<&str>) {
        self.cheats.set_debug_variable_filter(filter);
    }

    fn apply_cheats(&mut self) {
        self.cheats
            .apply(&mut self.vm, &mut self.sprites, &mut self.frame_player);
        self.cheats
            .maybe_log_targets(self.tick_count, &self.vm, &self.sprites, &self.frame_player);
    }

    /// Whether a cutscene video is currently playing or queued.
    pub fn is_cutscene_active(&self) -> bool {
        self.video_player.is_some() || !self.pending_videos.is_empty()
    }

    /// Queue MPEG-1 cutscene videos (SSL_PlayNext pre-content) for playback.
    pub fn queue_videos(&mut self, names: &[&str]) {
        for name in names {
            let normalized = normalize_content_path(name);
            if !normalized.is_empty() {
                self.pending_videos.push(normalized);
            }
        }
    }

    /// Skip the current (and any queued) cutscene videos.
    pub fn skip_cutscene(&mut self) {
        self.pending_videos.clear();
        self.video_player = None;
        self.active_video_name = None;
        self.audio.stop_all();
    }

    /// Advance cutscene playback by one 30fps tick.
    fn cutscene_tick(&mut self) {
        // Load the next queued video if none is playing.
        if self.video_player.is_none() {
            if self.pending_videos.is_empty() {
                return;
            }
            let name = self.pending_videos.remove(0);
            if !self.start_video(&name) {
                // Could not start this one; the next tick tries the next entry
                // (or finishes the cutscene and loads the SSL content).
                return;
            }
        }

        let (w, h) = (self.renderer.width as usize, self.renderer.height as usize);
        if let Some(player) = self.video_player.as_mut() {
            player.advance_and_render(1.0 / 30.0, &mut self.renderer.buffer, w, h);
            if player.is_finished() {
                self.video_player = None;
                self.active_video_name = None;
                self.audio.stop_all();
            }
        }
    }

    /// Resolve, load and start a cutscene video. Returns false on any failure
    /// (missing file, bad data) so the caller can fall through.
    fn start_video(&mut self, name: &str) -> bool {
        let path = match ContentLoader::find_content_file(&self.filename, name) {
            Some(p) => p,
            None => {
                log::warn!("Cutscene video not found: {}", name);
                return false;
            }
        };
        log::info!("Playing cutscene: {}", path.display());

        let data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Failed to read cutscene {}: {}", path.display(), e);
                return false;
            }
        };

        let streams = crate::mpeg::demux_all(data);
        if streams.video.is_empty() {
            log::warn!("Cutscene has no video stream: {}", path.display());
            return false;
        }

        // Decode and start the audio track up front (if present).
        if !streams.audio.is_empty() {
            let mut audio = crate::mpeg::Audio::new(streams.audio);
            if audio.has_header() {
                let rate = audio.samplerate();
                let mut pcm: Vec<f32> = Vec::new();
                while let Some(s) = audio.decode() {
                    pcm.extend_from_slice(&s.interleaved);
                }
                self.audio.stop_all();
                self.audio.play_pcm_stream(pcm, 2, rate);
            }
        }

        match crate::mpeg::VideoPlayer::new(streams.video) {
            Some(player) => {
                self.video_player = Some(player);
                self.active_video_name = Some(name.to_string());
                true
            }
            None => {
                log::warn!("Cutscene video has no sequence header: {}", path.display());
                false
            }
        }
    }

    /// Handle button presses from libretro input.
    pub fn handle_buttons(&mut self) {
        let pressed = self.input.get_pressed_buttons();
        let button_events: Vec<(u32, Vec<(u16, u16)>)> = self
            .cur_frame_objects
            .iter()
            .filter(|o| o.obj_type == ObjectType::Button)
            .map(|o| {
                (
                    o.index as u32,
                    self.reader.get_button_events(o.index as u32),
                )
            })
            .collect();

        for (_button_idx, events) in &button_events {
            for (keycode, action_idx) in events {
                if pressed.contains(keycode) {
                    let self_ptr = self as *mut Emulator;
                    unsafe {
                        (*self_ptr).vm.run(
                            &mut (*self_ptr).reader,
                            &mut *self_ptr,
                            *action_idx as u32,
                            "",
                        );
                    }
                }
            }
        }
    }

    /// Draw the current frame to the renderer's buffer.
    pub fn draw(&mut self) {
        // During a cutscene the framebuffer is produced by the video player.
        if self.is_cutscene_active() {
            return;
        }
        self.renderer
            .draw_frame(&mut self.reader, &self.sprites, &self.cur_frame_objects);
    }

    /// Load a menu image from a `.dat` file and bind it to a menu sprite.
    ///
    /// `spec` has the form "<spriteName>+<flag>+<picPath>" where `<picPath>` is
    /// a root-relative game path without extension (e.g. "/EACT    /EBBLADE").
    /// The flag selects which embedded image to use: "D" -> the game-name
    /// banner (grid items), "J" -> the preview screenshot (info pane). Once
    /// decoded the image is registered as an override for the named sprite so
    /// the renderer draws it.
    fn load_menu_image(&mut self, spec: &str) {
        let segments: Vec<&str> = spec.splitn(3, '+').collect();
        if segments.len() < 3 {
            log::warn!("LoadImage: malformed spec '{}'", spec);
            return;
        }
        let sprite_name = segments[0];
        let which = crate::dat_loader::DatImage::from_flag(segments[1]);
        let pic_path = segments[2];

        // Normalize the Native32 menu path (drop the leading slash, trim the
        // space-padded directory components) before resolving on disk.
        let normalized = normalize_content_path(&format!("{}.dat", pic_path));
        let dat_path = match ContentLoader::find_content_file(&self.filename, &normalized) {
            Some(p) => p,
            None => {
                log::debug!("LoadImage: thumbnail not found for '{}'", pic_path);
                return;
            }
        };

        let data = match std::fs::read(&dat_path) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("LoadImage: failed to read {}: {}", dat_path.display(), e);
                return;
            }
        };

        match crate::dat_loader::decode_image(&data, self.reader.colorspace, which) {
            Some(img) => {
                // Tie the thumbnail's visibility to its owning menu panel so it
                // hides when the view switches: the list-item name banners
                // (gName*) follow the list panel (listA0), the info preview
                // (gInfo) follows the info panel (infoA0). Without this the
                // flat sprite model keeps these always-visible overrides on
                // screen across a list <-> info switch.
                let visibility_leader = if sprite_name.starts_with("gName") {
                    Some("listA0".to_string())
                } else if sprite_name.starts_with("gInfo") {
                    Some("infoA0".to_string())
                } else {
                    None
                };
                self.renderer
                    .set_sprite_override(sprite_name.to_string(), img, visibility_leader);
            }
            None => log::debug!("LoadImage: could not decode image '{}'", pic_path),
        }
    }

    /// Switch to a new content file.
    fn switch_content(&mut self, filename: &str, kind: ContentLoadKind) -> anyhow::Result<()> {
        let fullpath = ContentLoader::find_content_file(&self.filename, filename)
            .ok_or_else(|| anyhow::anyhow!("Content file not found: {}", filename))?;

        log::info!("Loading content: {}", fullpath.display());

        // Validate the new content before changing the running application.
        let data = std::fs::read(&fullpath)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", fullpath.display(), e))?;
        let mut reader = Native32Reader::new(data);
        reader.init()?;

        let new_content_root = if fullpath.starts_with(&self.content_root) {
            self.content_root.clone()
        } else {
            self.content_root
                .ancestors()
                .find(|ancestor| fullpath.starts_with(ancestor))
                .context("switched content has no common root with the initial content")?
                .to_path_buf()
        };
        let replace_save_manager =
            !has_extension(&self.filename, "ssl") || !has_extension(&fullpath, "ssl");
        let return_frame =
            should_push_return_frame(kind, &self.filename, &fullpath).then(|| ReturnFrame {
                path: self.filename.clone(),
            });

        // Stop all sounds
        self.audio.stop_all();

        // Drop any front-end menu thumbnail overrides from the previous content.
        self.renderer.clear_sprite_overrides();

        // Reset state
        self.tick_count = 0;
        self.time_ms = 0;
        self.sprites = SpriteSystem::new();
        self.frame_player = FramePlayer::new();
        self.vm = ActionVM::new();
        self.content_loader = ContentLoader::new();

        // Consecutive SSL scenes belong to one game and share its save data.
        if replace_save_manager {
            self.save_manager = SaveManager::new(&fullpath);
        }
        self.content_root = new_content_root;
        if let Some(frame) = return_frame {
            self.return_stack.push(frame);
            self.menu_context = None;
        }
        self.filename = fullpath;
        self.reader = reader;
        self.audio = AudioEngine::new(self.reader.colorspace, (self.audio.volume * 100.0) as u32);
        self.cur_frame_objects.clear();

        Ok(())
    }

    /// Get the framebuffer as a slice of u32 (XRGB8888).
    pub fn get_framebuffer(&self) -> &[u32] {
        &self.renderer.buffer
    }

    /// Get pending audio samples as i16 interleaved stereo.
    pub fn get_pending_audio_samples(&mut self) -> Vec<i16> {
        self.audio.get_pending_samples()
    }

    /// Reset the emulator state.
    pub fn reset(&mut self) {
        self.tick_count = 0;
        self.time_ms = 0;
        self.sprites = SpriteSystem::new();
        self.frame_player = FramePlayer::new();
        self.vm = ActionVM::new();
        self.pending_videos.clear();
        self.video_player = None;
        self.active_video_name = None;
        self.audio.stop_all();
    }

    /// Get the fixed size needed for libretro serialization.
    pub fn serialize_size(&self) -> usize {
        crate::save_state::SERIALIZED_SIZE
    }

    /// Serialize the emulator state to a buffer.
    pub fn serialize(&self, buffer: &mut [u8]) -> Result<()> {
        use crate::save_state::{EmulatorState, ReturnFrameState, VideoState};

        let content_path =
            portable_content_path(&self.filename, &self.content_root, "current content")?;
        let return_stack = self
            .return_stack
            .iter()
            .map(|frame| {
                Ok(ReturnFrameState {
                    content_path: portable_content_path(
                        &frame.path,
                        &self.content_root,
                        "return target",
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let state = EmulatorState {
            content_path,
            content_crc32: crc32fast::hash(&self.reader.data),
            frame_player: self.frame_player.clone(),
            sprites: self.sprites.clone(),
            vm_vars: self.vm.vars.clone(),
            rng_state: self.vm.rng_state,
            audio: self.audio.save_state(),
            renderer: self.renderer.save_state(),
            content_loader: self.content_loader.clone(),
            menu_context: self.menu_context.clone(),
            return_stack: Some(return_stack),
            tick_count: self.tick_count,
            time_ms: self.time_ms,
            pending_videos: self.pending_videos.clone(),
            video: self
                .active_video_name
                .as_ref()
                .zip(self.video_player.as_ref())
                .map(|(name, player)| VideoState {
                    name: name.clone(),
                    elapsed: player.elapsed(),
                }),
        };
        crate::save_state::encode(&state, buffer)
    }

    /// Deserialize the emulator state from a buffer.
    pub fn deserialize(&mut self, buffer: &[u8]) -> Result<()> {
        let state = crate::save_state::decode(buffer)?;
        let content_path =
            resolve_state_content_path(&self.content_root, &state.content_path, "current content")?;
        let return_stack = if let Some(saved_stack) = &state.return_stack {
            saved_stack
                .iter()
                .map(|frame| {
                    let path = resolve_state_content_path(
                        &self.content_root,
                        &frame.content_path,
                        "return target",
                    )?;
                    if !has_extension(&path, "smf") {
                        anyhow::bail!("save-state return target is not an SMF file");
                    }
                    Ok(ReturnFrame { path })
                })
                .collect::<Result<Vec<_>>>()?
        } else if !self.return_stack.is_empty() {
            // Loading an old state during an active session keeps its known parents.
            self.return_stack.clone()
        } else if self._temp_dir.is_some() && self.filename != content_path {
            // Old ZIP save states relied on the package entry point implicitly.
            vec![ReturnFrame {
                path: self.filename.clone(),
            }]
        } else {
            Vec::new()
        };
        let data = std::fs::read(&content_path).with_context(|| {
            format!(
                "failed to read save-state content: {}",
                content_path.display()
            )
        })?;
        if crc32fast::hash(&data) != state.content_crc32 {
            anyhow::bail!("save state belongs to different content data");
        }
        let mut reader = Native32Reader::new(data);
        reader
            .init()
            .context("failed to initialize save-state content")?;
        if reader.resolution != (self.renderer.width, self.renderer.height) {
            anyhow::bail!("save-state content resolution does not match the loaded game");
        }
        if state.video.as_ref().is_some_and(|video| {
            !video.elapsed.is_finite() || video.elapsed < 0.0 || video.name.is_empty()
        }) {
            anyhow::bail!("save state contains invalid video playback state");
        }

        self.audio.stop_all();
        self.filename = content_path;
        self.reader = reader;
        self.save_manager = SaveManager::new(&self.filename);
        self.frame_player = state.frame_player;
        self.sprites = state.sprites;
        self.vm.vars = state.vm_vars;
        self.vm.rng_state = state.rng_state;
        self.content_loader = state.content_loader;
        self.menu_context = state.menu_context;
        self.return_stack = return_stack;
        self.tick_count = state.tick_count;
        self.time_ms = state.time_ms;
        self.pending_videos = state.pending_videos;
        self.video_player = None;
        self.active_video_name = None;
        self.renderer.restore_state(state.renderer);
        self.cur_frame_objects = if self.frame_player.current_frame == 0 {
            Vec::new()
        } else {
            self.reader
                .get_frame(self.frame_player.current_frame)
                .unwrap_or_default()
        };

        if let Some(video) = state.video {
            if !self.start_video(&video.name) {
                anyhow::bail!("failed to restore active cutscene {}", video.name);
            }
            if let Some(player) = self.video_player.as_mut() {
                player.advance_and_render(
                    video.elapsed,
                    &mut self.renderer.buffer,
                    self.renderer.width as usize,
                    self.renderer.height as usize,
                );
            }
        }
        self.audio.colorspace = self.reader.colorspace;
        self.audio.restore_state(state.audio);
        self.input.set_buttons(&[]);
        Ok(())
    }
}

/// VmHost trait implementation for the Emulator.
impl VmHost for Emulator {
    fn stop(&mut self, target: &str) {
        if target.is_empty() {
            self.frame_player.playing = false;
        } else if let Some(movie) = self.sprites.get_mut(target) {
            movie.playing = false;
        }
    }

    fn play(&mut self, target: &str) {
        if target.is_empty() {
            self.frame_player.playing = true;
        } else if let Some(movie) = self.sprites.get_mut(target) {
            movie.playing = true;
        }
    }

    fn get_frame(&mut self, target: &str) -> u32 {
        if target.is_empty() {
            self.frame_player.current_frame
        } else if let Some(movie) = self.sprites.get(target) {
            movie.frame as u32 + 1
        } else {
            0
        }
    }

    fn goto_frame(&mut self, target: &str, frame: u32, playing: bool) {
        if target.is_empty() {
            self.frame_player.goto(frame, playing);
        } else if let Some(movie) = self.sprites.get_mut(target) {
            movie.next_frame = Some(frame as isize - 1);
            movie.playing = playing;
        }
    }

    fn stop_sounds(&mut self, target: &str) {
        if target.is_empty() {
            self.audio.stop_all();
            for (_, movie) in self.sprites.iter_mut() {
                movie.sound_channel = None;
            }
        } else {
            self.audio.stop_for_movie(target);
            if let Some(movie) = self.sprites.get_mut(target) {
                movie.sound_channel = None;
            }
        }
    }

    fn set_property(&mut self, target: &str, prop: ActionProp, value: &str) {
        if let Some(movie) = self.sprites.get_mut(target) {
            match prop {
                ActionProp::X => movie.x = str_to_float(value) as i16,
                ActionProp::Y => movie.y = str_to_float(value) as i16,
                ActionProp::Visible => movie.visible = str_to_float(value) != 0.0,
                ActionProp::CurrentFrame => {
                    movie.next_frame = Some(str_to_float(value) as isize);
                }
                ActionProp::Name => {
                    let old_name = target.to_string();
                    if let Some(state) = self.sprites.remove(&old_name) {
                        self.sprites.insert(value.to_string(), state);
                    }
                }
                _ => {
                    log::warn!("Unsupported SetProperty({:?})", prop);
                }
            }
        }
    }

    fn get_property(&mut self, target: &str, prop: ActionProp) -> String {
        if let Some(movie) = self.sprites.get(target) {
            match prop {
                ActionProp::X => movie.x.to_string(),
                ActionProp::Y => movie.y.to_string(),
                ActionProp::Visible => (movie.visible as i32).to_string(),
                ActionProp::CurrentFrame => {
                    if let Some(nf) = movie.next_frame {
                        (nf.max(0) as u32 + 1).to_string()
                    } else if movie.playing {
                        (movie.frame as u32 + 2).to_string()
                    } else {
                        (movie.frame as u32 + 1).to_string()
                    }
                }
                ActionProp::TotalFrames => {
                    let frames = self.reader.get_movie(movie.movie);
                    frames.len().to_string()
                }
                ActionProp::Name => target.to_string(),
                _ => "0".to_string(),
            }
        } else {
            "0".to_string()
        }
    }

    fn clone_sprite(&mut self, src: &str, dest: &str, depth: i32) {
        if let Some(orig) = self.sprites.get(src).cloned() {
            let mut new_state = MovieState::new(orig.movie, orig.x, orig.y, depth as u16);
            new_state.frame = 0;
            new_state.visible = true;
            new_state.playing = true;
            new_state.cloned = true;
            new_state.next_frame = Some(0);
            self.sprites.insert(dest.to_string(), new_state);
        }
    }

    fn remove_sprite(&mut self, name: &str) {
        if let Some(movie) = self.sprites.remove(name) {
            if movie.sound_channel.is_some() {
                self.audio.stop_for_movie(name);
            }
        }
    }

    fn call(&mut self, frame: u32) {
        if let Some(objects) = self.reader.get_frame(frame) {
            let action_indices: Vec<u32> = objects
                .iter()
                .filter(|o| o.obj_type == ObjectType::Action)
                .map(|o| o.index as u32)
                .collect();
            let self_ptr = self as *mut Emulator;
            unsafe {
                for action_idx in action_indices {
                    (*self_ptr)
                        .vm
                        .run(&mut (*self_ptr).reader, &mut *self_ptr, action_idx, "");
                }
            }
        }
    }

    fn get_time(&self) -> u32 {
        self.time_ms
    }

    fn get_url(&mut self, url: &str, target: &str) {
        let parts: Vec<&str> = target.split('+').collect();
        // The front-end menu (FHUI) issues file-system and navigation host
        // calls where the command is the first '+'-segment of the target and
        // the result variable name is the `url`. The in-game SSL/NAV calls
        // instead carry the command in the second segment, so dispatch on the
        // first segment and fall through to the SSL/NAV handler.
        let cmd = parts[0];
        // Everything after the first '+' (used as a path/argument by the
        // front-end file-system commands; paths never contain '+').
        let arg = target.split_once('+').map(|x| x.1).unwrap_or("");

        match cmd {
            "GetFileNum" => {
                let count = self.file_browser.file_count(&self.filename, arg);
                self.vm.vars.insert(url.to_lowercase(), count.to_string());
            }
            "GetFirstFile" => {
                let name = self.file_browser.first_file(&self.filename, arg);
                self.vm.vars.insert(url.to_lowercase(), name);
            }
            "GetNextFile" => {
                let name = self.file_browser.next_file();
                self.vm.vars.insert(url.to_lowercase(), name);
            }
            "GetContext" => {
                // Restore the navigation context saved before launching a game;
                // "NULL" tells the menu to start from its default state.
                let value = self
                    .menu_context
                    .clone()
                    .unwrap_or_else(|| "NULL".to_string());
                self.vm.vars.insert(url.to_lowercase(), value);
            }
            "SaveContext" => {
                self.menu_context = Some(url.to_string());
            }
            "FHUI_StrSub" => {
                // FHUI_StrSub+<str>+<delim>+<field>+... : split <str> on the
                // 1-character <delim> and return the 1-based <field>.
                let value = fhui_str_sub(&parts);
                self.vm.vars.insert(url.to_lowercase(), value);
            }
            "StartGame" => {
                // `url` is a root-relative game path without extension
                // (e.g. "/EACT    /EBBLADE"); load the matching .smf game.
                log::info!("StartGame({})", url);
                self.content_loader.queue_child(&format!("{}.smf", url));
            }
            "LoadImage" => {
                // url = "<spriteName>+D+<picPath>" : load the thumbnail stored
                // in <picPath>.dat and bind it to the named menu sprite.
                self.load_menu_image(url);
            }
            "FormateStr" => {
                // In FHUI this only ever formats an empty string to clear a
                // text label; the visible game names are pre-rendered banner
                // graphics loaded from the `.dat` files via LoadImage("D").
                // Arbitrary text rendering would need a font subsystem that
                // does not exist yet.
                log::debug!("Ignoring FormateStr('{}')", url);
            }
            "SSL" | "NAV" => match parts.get(1).copied().unwrap_or("") {
                "SSL_PlayNext" => {
                    log::info!("SSL_PlayNext({}, {})", url, target);
                    let url_parts: Vec<&str> = url.split('+').collect();
                    // All parts except the last are MPEG-1 pre-content (logo /
                    // cutscene videos) to play before loading the final SSL content.
                    if url_parts.len() > 1 {
                        let pre = &url_parts[..url_parts.len() - 1];
                        self.queue_videos(pre);
                    }
                    if let Some(last) = url_parts.last() {
                        self.content_loader.queue_load(last);
                    }
                }
                "SSL_PlayPlan" => {
                    log::info!("Ignoring SSL_PlayPlan('{}')", url);
                }
                "SSL_PlayProg" => {
                    log::info!("Ignoring SSL_PlayProg('{}')", url);
                }
                "SSL_GetSSLData" => {
                    log::info!("SSL_GetSSLData");
                    if parts.len() >= 3 {
                        let success_var = parts[2];
                        match self.save_manager.load() {
                            Some(data) => {
                                self.vm.vars.insert(url.to_lowercase(), data);
                                self.vm
                                    .vars
                                    .insert(success_var.to_lowercase(), "S".to_string());
                            }
                            None => {
                                self.vm
                                    .vars
                                    .insert(success_var.to_lowercase(), "N".to_string());
                            }
                        }
                    }
                }
                "SSL_SaveSSLData" => {
                    log::info!("SSL_SaveSSLData");
                    if parts.len() >= 3 {
                        let success_var = parts[2];
                        if self.save_manager.save(url) {
                            self.vm
                                .vars
                                .insert(success_var.to_lowercase(), "S".to_string());
                        }
                    }
                }
                "NAV_ScreenMove" => {
                    let coords: Vec<&str> = url.split('+').collect();
                    if coords.len() >= 2 {
                        let dx = coords[0].parse::<i32>().unwrap_or(0);
                        let dy = coords[1].parse::<i32>().unwrap_or(0);
                        self.renderer.screen_x = dx;
                        self.renderer.screen_y = dy;
                    }
                }
                "NAV_SelectNES" => {
                    // NES ROM browsing is handled by the original platform's NES
                    // emulator, which is out of scope for this core.
                    log::debug!("Ignoring NAV_SelectNES('{}')", url);
                }
                "NAV_SelectNa32" => {
                    // The platform selector hands control to the sibling
                    // Native32 menu after its selection animation finishes.
                    log::info!("NAV_SelectNa32");
                    self.content_loader.queue_load("NA32UI.smf");
                }
                "NAV_SelectKOK" => {
                    // Karaoke requires a separate firmware service that is not
                    // part of the Native32 runtime. Restart the selector because
                    // its confirmation animation no longer accepts input.
                    log::warn!("NAV_SelectKOK is unsupported; restarting the selector");
                    self.content_loader.queue_load("SELECT.SSL");
                }
                other => {
                    log::warn!("Unhandled GetUrl2('{}', '{}') [{}]", url, target, other);
                }
            },
            _ => {
                log::warn!("Unhandled GetUrl2('{}', '{}')", url, target);
            }
        }
    }

    fn run_frame_actions(&mut self, frame: u32) {
        if let Some(objects) = self.reader.get_frame(frame) {
            let action_indices: Vec<u32> = objects
                .iter()
                .filter(|o| o.obj_type == ObjectType::Action)
                .map(|o| o.index as u32)
                .collect();
            let self_ptr = self as *mut Emulator;
            unsafe {
                for action_idx in action_indices {
                    (*self_ptr)
                        .vm
                        .run(&mut (*self_ptr).reader, &mut *self_ptr, action_idx, "");
                }
            }
        }
    }
}

fn str_to_float(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    s.parse::<f64>().unwrap_or(0.0)
}

/// FHUI_StrSub host helper: split the source string `parts[1]` on the
/// delimiter `parts[2]` and return its 1-based field `parts[3]`.
fn fhui_str_sub(parts: &[&str]) -> String {
    if parts.len() < 4 {
        return String::new();
    }
    let source = parts[1];
    let delim = parts[2];
    let field: usize = parts[3].trim().parse().unwrap_or(0);
    if field == 0 {
        return String::new();
    }
    let fields: Vec<&str> = if delim.is_empty() {
        vec![source]
    } else {
        source.split(delim).collect()
    };
    fields
        .get(field - 1)
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn portable_content_path(path: &Path, root: &Path, label: &str) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .with_context(|| format!("{label} is outside the save-state content root"))?;
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        anyhow::bail!("{label} path cannot be represented safely");
    }
    Ok(relative
        .to_str()
        .with_context(|| format!("{label} path is not valid UTF-8"))?
        .replace('\\', "/"))
}

fn resolve_state_content_path(root: &Path, value: &str, label: &str) -> Result<PathBuf> {
    let relative = Path::new(value);
    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        anyhow::bail!("save state contains an unsafe {label} path");
    }
    Ok(root.join(relative))
}

/// Normalize a Native32 content path: split on '/', trim each component
/// (games pad directory names with trailing spaces), drop empties, rejoin.
fn normalize_content_path(filename: &str) -> String {
    filename
        .split('/')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn has_extension(path: &Path, expected: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case(expected))
}

fn should_push_return_frame(kind: ContentLoadKind, current: &Path, target: &Path) -> bool {
    kind == ContentLoadKind::LaunchChild
        && current != target
        && has_extension(current, "smf")
        && has_extension(target, "smf")
}

/// Check if a file is a ZIP archive by reading its magic bytes.
///
/// ZIP files start with "PK\x03\x04" (local file header signature).
fn is_zip_file(path: &PathBuf) -> bool {
    // First check the extension
    if let Some(ext) = path.extension() {
        if ext.to_string_lossy().eq_ignore_ascii_case("zip") {
            return true;
        }
    }

    // Fallback: check magic bytes
    if let Ok(mut file) = std::fs::File::open(path) {
        let mut magic = [0u8; 4];
        if std::io::Read::read_exact(&mut file, &mut magic).is_ok() {
            return magic == [0x50, 0x4B, 0x03, 0x04]; // PK\x03\x04
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_smf_launch_creates_return_frame() {
        assert!(should_push_return_frame(
            ContentLoadKind::LaunchChild,
            Path::new("menu.SMF"),
            Path::new("games/child.smf")
        ));
    }

    #[test]
    fn replacement_and_ssl_transitions_do_not_create_return_frames() {
        assert!(!should_push_return_frame(
            ContentLoadKind::Replace,
            Path::new("menu.smf"),
            Path::new("child.smf")
        ));
        assert!(!should_push_return_frame(
            ContentLoadKind::LaunchChild,
            Path::new("menu.smf"),
            Path::new("scene.ssl")
        ));
        assert!(!should_push_return_frame(
            ContentLoadKind::LaunchChild,
            Path::new("menu.smf"),
            Path::new("menu.smf")
        ));
    }
}
