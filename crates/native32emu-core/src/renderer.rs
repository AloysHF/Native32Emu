// Renderer: draws the current frame's visible objects sorted by depth.

use std::collections::HashMap;
use std::path::Path;

use crate::file_loader::{FrameObject, Native32Reader, ObjectType};
use crate::image_decoder::RgbaImage;
use crate::sprite_system::SpriteSystem;

/// What a draw entry should blit: either an image from the game's image table,
/// or a runtime-supplied override image (used by the front-end menu thumbnails).
enum DrawSource {
    Image(u32),
    Override(String),
}

pub struct DrawEntry {
    source: DrawSource,
    pub x: i32,
    pub y: i32,
    pub depth: u32,
}

/// A runtime image override bound to a menu sprite.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct SpriteOverride {
    image: RgbaImage,
    /// Optional "visibility leader" sprite. When set, the override is only
    /// drawn if that sprite exists and is currently visible. This models the
    /// FHUI menu's panel parent-child visibility: the list-item name banners
    /// follow the list panel, the info preview follows the info panel, so each
    /// thumbnail hides together with its owning view instead of lingering when
    /// the view switches.
    visibility_leader: Option<String>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct RendererState {
    pub screen_x: i32,
    pub screen_y: i32,
    sprite_overrides: HashMap<String, SpriteOverride>,
}

pub struct Renderer {
    pub screen_x: i32,
    pub screen_y: i32,
    pub buffer: Vec<u32>,
    pub width: u32,
    pub height: u32,
    /// Runtime image overrides keyed by sprite name (front-end menu thumbnails
    /// loaded from `.dat` files via the LoadImage host call).
    sprite_overrides: HashMap<String, SpriteOverride>,
}

impl Renderer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            screen_x: 0,
            screen_y: 0,
            buffer: vec![0xFF000000; (width * height) as usize],
            width,
            height,
            sprite_overrides: HashMap::new(),
        }
    }

    /// Bind a decoded image to a sprite name so it is drawn in place of the
    /// sprite's normal movie-frame image. `visibility_leader` optionally ties
    /// the override's visibility to another sprite (the owning menu panel).
    pub fn set_sprite_override(
        &mut self,
        name: String,
        image: RgbaImage,
        visibility_leader: Option<String>,
    ) {
        self.sprite_overrides.insert(
            name,
            SpriteOverride {
                image,
                visibility_leader,
            },
        );
    }

    /// Drop one sprite image override (e.g. clearing a menu thumbnail).
    pub fn remove_sprite_override(&mut self, name: &str) {
        self.sprite_overrides.remove(name);
    }

    /// Drop all sprite image overrides (e.g. when switching content).
    pub fn clear_sprite_overrides(&mut self) {
        self.sprite_overrides.clear();
    }

    /// Number of active sprite image overrides (for diagnostics/tests).
    pub fn sprite_override_count(&self) -> usize {
        self.sprite_overrides.len()
    }

    pub(crate) fn save_state(&self) -> RendererState {
        RendererState {
            screen_x: self.screen_x,
            screen_y: self.screen_y,
            sprite_overrides: self.sprite_overrides.clone(),
        }
    }

    pub(crate) fn restore_state(&mut self, state: RendererState) {
        self.screen_x = state.screen_x;
        self.screen_y = state.screen_y;
        self.sprite_overrides = state.sprite_overrides;
    }

    /// Resize the display buffer (e.g., when scaling changes).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.buffer = vec![0xFF000000; (width * height) as usize];
    }

    /// Draw the current frame to the internal buffer.
    pub fn draw_frame(
        &mut self,
        reader: &mut Native32Reader,
        sprites: &SpriteSystem,
        cur_frame: &[FrameObject],
    ) {
        // Clear to opaque black
        for pixel in self.buffer.iter_mut() {
            *pixel = 0xFF000000;
        }

        // Collect draw entries
        let mut drawlist: Vec<DrawEntry> = Vec::new();

        // Image objects from the current frame
        for obj in cur_frame {
            if obj.obj_type == ObjectType::Image {
                drawlist.push(DrawEntry {
                    source: DrawSource::Image(obj.index as u32),
                    x: obj.x as i32,
                    y: obj.y as i32,
                    depth: obj.depth as u32,
                });
            }
        }

        // Visible movie instances
        for (name, movie) in sprites.iter() {
            if !movie.visible {
                continue;
            }
            // A runtime override (menu thumbnail) takes precedence over the
            // sprite's normal movie-frame image.
            if let Some(ovr) = self.sprite_overrides.get(name) {
                // Hide the thumbnail together with its owning panel: if a
                // visibility leader is set and that panel sprite is currently
                // hidden, skip this override so it does not linger across a
                // list <-> info view switch.
                if let Some(leader) = &ovr.visibility_leader {
                    if let Some(panel) = sprites.get(leader) {
                        if !panel.visible {
                            continue;
                        }
                    }
                }
                let (fx, fy) = {
                    let movie_frames = reader.get_movie(movie.movie);
                    movie_frames
                        .get(movie.frame)
                        .map(|f| (f.x as i32, f.y as i32))
                        .unwrap_or((0, 0))
                };
                drawlist.push(DrawEntry {
                    source: DrawSource::Override(name.clone()),
                    x: movie.x as i32 + fx,
                    y: movie.y as i32 + fy,
                    depth: movie.depth as u32,
                });
                continue;
            }
            let movie_frames = reader.get_movie(movie.movie);
            if movie.frame < movie_frames.len() {
                let frame = &movie_frames[movie.frame];
                drawlist.push(DrawEntry {
                    source: DrawSource::Image(frame.image as u32),
                    x: movie.x as i32 + frame.x as i32,
                    y: movie.y as i32 + frame.y as i32,
                    depth: movie.depth as u32,
                });
            }
        }

        // Sort by depth (stable to preserve insertion order for equal depths)
        drawlist.sort_by_key(|d| d.depth);

        // Draw each entry
        for entry in &drawlist {
            let x = entry.x + self.screen_x;
            let y = entry.y + self.screen_y;
            match &entry.source {
                DrawSource::Image(index) => {
                    if let Some(img) = reader.get_image(*index) {
                        self.blit_image(&img, x, y);
                    }
                }
                DrawSource::Override(name) => {
                    if let Some(img) = self.sprite_overrides.get(name).map(|o| o.image.clone()) {
                        self.blit_image(&img, x, y);
                    }
                }
            }
        }
    }

    /// Blit an ARGB image to the buffer with transparency.
    fn blit_image(&mut self, img: &RgbaImage, dst_x: i32, dst_y: i32) {
        for sy in 0..img.height as i32 {
            let dy = dst_y + sy;
            if dy < 0 || dy >= self.height as i32 {
                continue;
            }
            for sx in 0..img.width as i32 {
                let dx = dst_x + sx;
                if dx < 0 || dx >= self.width as i32 {
                    continue;
                }
                let src_pixel = img.pixels[(sy as u32 * img.width + sx as u32) as usize];
                // Skip fully transparent pixels (alpha == 0)
                if (src_pixel >> 24) == 0 {
                    continue;
                }
                self.buffer[(dy as u32 * self.width + dx as u32) as usize] = src_pixel;
            }
        }
    }

    /// Save the current buffer as a PNG screenshot.
    pub fn save_screenshot(&self, path: &Path) -> anyhow::Result<()> {
        let mut img = image::RgbaImage::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                let pixel = self.buffer[(y * self.width + x) as usize];
                let r = ((pixel >> 16) & 0xFF) as u8;
                let g = ((pixel >> 8) & 0xFF) as u8;
                let b = (pixel & 0xFF) as u8;
                let a = ((pixel >> 24) & 0xFF) as u8;
                img.put_pixel(x, y, image::Rgba([r, g, b, a]));
            }
        }
        img.save(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_loader::Native32Reader;
    use crate::sprite_system::{MovieState, SpriteSystem};

    // Build a renderer with a single 1x1 opaque-red override bound to `sprite`,
    // optionally following `leader`. The override sprite is visible; `leader`
    // (if present) is created with the given visibility.
    fn setup(
        leader: Option<&str>,
        leader_visible: bool,
    ) -> (Renderer, Native32Reader, SpriteSystem) {
        let mut renderer = Renderer::new(4, 4);
        let img = RgbaImage {
            width: 1,
            height: 1,
            pixels: vec![0xFFFF0000],
        };
        renderer.set_sprite_override("gName0".to_string(), img, leader.map(|s| s.to_string()));

        let reader = Native32Reader::new(Vec::new());

        let mut sprites = SpriteSystem::new();
        // movie index 1 so the empty reader safely returns no frames.
        sprites.insert("gName0".to_string(), MovieState::new(1, 0, 0, 17));
        if let Some(name) = leader {
            let mut panel = MovieState::new(1, 0, 0, 15);
            panel.visible = leader_visible;
            sprites.insert(name.to_string(), panel);
        }
        (renderer, reader, sprites)
    }

    #[test]
    fn override_drawn_when_leader_visible() {
        let (mut renderer, mut reader, sprites) = setup(Some("listA0"), true);
        renderer.draw_frame(&mut reader, &sprites, &[]);
        assert_eq!(renderer.buffer[0], 0xFFFF0000, "override should be drawn");
    }

    #[test]
    fn override_hidden_when_leader_hidden() {
        let (mut renderer, mut reader, sprites) = setup(Some("listA0"), false);
        renderer.draw_frame(&mut reader, &sprites, &[]);
        assert_eq!(
            renderer.buffer[0], 0xFF000000,
            "override should be skipped while its panel is hidden"
        );
    }

    #[test]
    fn override_drawn_without_leader() {
        // No visibility leader: the override always draws (legacy behaviour).
        let (mut renderer, mut reader, sprites) = setup(None, false);
        renderer.draw_frame(&mut reader, &sprites, &[]);
        assert_eq!(renderer.buffer[0], 0xFFFF0000);
    }
}
