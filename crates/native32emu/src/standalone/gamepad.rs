// Physical gamepad input: polls HID controllers via gilrs and maps them to
// Native32 keycodes. minifb only reports keyboard state, so this module is
// the standalone front-end's gamepad backend.
//
// Face buttons follow the same RetroPad convention as the libretro core and
// the default keyboard bindings: physical South (Xbox A / PS Cross) → Native32
// A, physical East (Xbox B / PS Circle) → Native32 B. Left/right sticks act
// as a digital D-pad once past the deadzone. Select is a host-level back
// action (return to parent SMF / exit), not a guest keycode.

use gilrs::{Axis, Button, EventType, Gilrs};

use native32emu_core::input_handler::{KEYCODE_A, KEYCODE_B};

const STICK_DEADZONE: f32 = 0.5;

/// Native32 directional keycodes (must match `input_handler::DEFAULT_KEY_MAP`).
const KEYCODE_LEFT: u16 = 0x0200;
const KEYCODE_RIGHT: u16 = 0x0400;
const KEYCODE_UP: u16 = 0x1c00;
const KEYCODE_DOWN: u16 = 0x1e00;

/// Snapshot of the first connected pad's relevant digital/axis state.
/// Extracted so mapping can be unit-tested without HID hardware.
#[derive(Clone, Copy, Default)]
struct PadState {
    dpad_up: bool,
    dpad_down: bool,
    dpad_left: bool,
    dpad_right: bool,
    south: bool,
    east: bool,
    select: bool,
    left_stick_x: f32,
    left_stick_y: f32,
    right_stick_x: f32,
    right_stick_y: f32,
}

/// Polls the first connected physical gamepad and maps it onto Native32 keys.
pub struct GamepadMapper {
    gilrs: Option<Gilrs>,
    /// Select state from the previous sample, used for rising-edge detection.
    select_was_down: bool,
    /// Select state from the most recent sample (after `pressed_keycodes`).
    select_is_down: bool,
}

impl GamepadMapper {
    pub fn new(enabled: bool) -> Self {
        let gilrs = if enabled {
            match Gilrs::new() {
                Ok(gilrs) => {
                    log::info!("Gamepad support enabled");
                    Some(gilrs)
                }
                Err(error) => {
                    log::warn!("Gamepad support unavailable: {error}");
                    None
                }
            }
        } else {
            None
        };
        Self {
            gilrs,
            select_was_down: false,
            select_is_down: false,
        }
    }

    /// Sample the first connected pad and return its pressed Native32 keycodes.
    ///
    /// Keycodes are discrete values (not a packed bitmask) because the Native32
    /// directional keycodes overlap. `--swap-ab` is applied later by the core's
    /// input handler, so this mapping stays physical-layout only.
    ///
    /// Must be called once per frame before [`select_just_pressed`](Self::select_just_pressed).
    pub fn pressed_keycodes(&mut self) -> Vec<u16> {
        let state = self.sample();
        self.select_was_down = self.select_is_down;
        self.select_is_down = state.select;
        map_keycodes(&state)
    }

    /// True only on the frame Select transitions from released to pressed.
    ///
    /// Used as the standalone host back action (same role as ESC / RetroPad
    /// Select in the libretro core). Call after [`pressed_keycodes`](Self::pressed_keycodes).
    pub fn select_just_pressed(&self) -> bool {
        self.select_is_down && !self.select_was_down
    }

    fn sample(&mut self) -> PadState {
        let Some(gilrs) = self.gilrs.as_mut() else {
            return PadState::default();
        };

        while let Some(event) = gilrs.next_event() {
            match event.event {
                EventType::Connected => {
                    log::info!("Gamepad connected: {}", gilrs.gamepad(event.id).name());
                }
                EventType::Disconnected => {
                    log::info!("Gamepad disconnected: {}", event.id);
                }
                _ => {}
            }
        }

        for (_id, gamepad) in gilrs.gamepads() {
            if !gamepad.is_connected() {
                continue;
            }
            return sample_pad(&gamepad);
        }
        PadState::default()
    }
}

fn sample_pad(gamepad: &gilrs::Gamepad<'_>) -> PadState {
    PadState {
        dpad_up: gamepad.is_pressed(Button::DPadUp),
        dpad_down: gamepad.is_pressed(Button::DPadDown),
        dpad_left: gamepad.is_pressed(Button::DPadLeft),
        dpad_right: gamepad.is_pressed(Button::DPadRight),
        south: gamepad.is_pressed(Button::South),
        east: gamepad.is_pressed(Button::East),
        select: gamepad.is_pressed(Button::Select),
        left_stick_x: gamepad.value(Axis::LeftStickX),
        left_stick_y: gamepad.value(Axis::LeftStickY),
        right_stick_x: gamepad.value(Axis::RightStickX),
        right_stick_y: gamepad.value(Axis::RightStickY),
    }
}

/// Map a pad snapshot to Native32 keycodes (physical layout, no A/B swap).
fn map_keycodes(state: &PadState) -> Vec<u16> {
    let mut buttons = Vec::new();

    if state.dpad_up || state.left_stick_y > STICK_DEADZONE || state.right_stick_y > STICK_DEADZONE
    {
        buttons.push(KEYCODE_UP);
    }
    if state.dpad_down
        || state.left_stick_y < -STICK_DEADZONE
        || state.right_stick_y < -STICK_DEADZONE
    {
        buttons.push(KEYCODE_DOWN);
    }
    if state.dpad_left
        || state.left_stick_x < -STICK_DEADZONE
        || state.right_stick_x < -STICK_DEADZONE
    {
        buttons.push(KEYCODE_LEFT);
    }
    if state.dpad_right
        || state.left_stick_x > STICK_DEADZONE
        || state.right_stick_x > STICK_DEADZONE
    {
        buttons.push(KEYCODE_RIGHT);
    }

    // RetroPad-compatible face buttons (matches the libretro core):
    // physical South → Native32 A, physical East → Native32 B.
    if state.south {
        buttons.push(KEYCODE_A);
    }
    if state.east {
        buttons.push(KEYCODE_B);
    }

    buttons
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_mapper_reports_no_buttons() {
        let mut mapper = GamepadMapper::new(false);
        assert!(mapper.pressed_keycodes().is_empty());
        assert!(!mapper.select_just_pressed());
    }

    #[test]
    fn idle_pad_maps_to_empty() {
        assert!(map_keycodes(&PadState::default()).is_empty());
    }

    #[test]
    fn face_buttons_follow_retropad_layout() {
        let south = map_keycodes(&PadState {
            south: true,
            ..Default::default()
        });
        assert_eq!(south, vec![KEYCODE_A]);

        let east = map_keycodes(&PadState {
            east: true,
            ..Default::default()
        });
        assert_eq!(east, vec![KEYCODE_B]);
    }

    #[test]
    fn dpad_maps_to_direction_keycodes() {
        let up = map_keycodes(&PadState {
            dpad_up: true,
            ..Default::default()
        });
        assert_eq!(up, vec![KEYCODE_UP]);

        let down_left = map_keycodes(&PadState {
            dpad_down: true,
            dpad_left: true,
            ..Default::default()
        });
        assert_eq!(down_left, vec![KEYCODE_DOWN, KEYCODE_LEFT]);
    }

    #[test]
    fn sticks_act_as_dpad_past_deadzone() {
        let up = map_keycodes(&PadState {
            left_stick_y: 0.9,
            ..Default::default()
        });
        assert_eq!(up, vec![KEYCODE_UP]);

        let left = map_keycodes(&PadState {
            right_stick_x: -0.75,
            ..Default::default()
        });
        assert_eq!(left, vec![KEYCODE_LEFT]);
    }

    #[test]
    fn stick_inside_deadzone_is_ignored() {
        let state = PadState {
            left_stick_x: 0.4,
            left_stick_y: -0.4,
            right_stick_x: -0.49,
            right_stick_y: 0.49,
            ..Default::default()
        };
        assert!(map_keycodes(&state).is_empty());
    }

    #[test]
    fn stick_at_deadzone_threshold_is_rejected() {
        // Strictly-greater-than comparison: the threshold itself stays idle.
        let state = PadState {
            left_stick_y: STICK_DEADZONE,
            ..Default::default()
        };
        assert!(map_keycodes(&state).is_empty());
    }

    #[test]
    fn stick_just_past_deadzone_is_accepted() {
        let state = PadState {
            left_stick_y: STICK_DEADZONE + 0.01,
            ..Default::default()
        };
        assert_eq!(map_keycodes(&state), vec![KEYCODE_UP]);
    }

    #[test]
    fn select_is_not_a_guest_keycode() {
        let state = PadState {
            select: true,
            ..Default::default()
        };
        assert!(map_keycodes(&state).is_empty());
    }

    #[test]
    fn combined_dpad_and_face_do_not_collide() {
        let buttons = map_keycodes(&PadState {
            dpad_right: true,
            south: true,
            east: true,
            ..Default::default()
        });
        assert_eq!(buttons, vec![KEYCODE_RIGHT, KEYCODE_A, KEYCODE_B]);
    }
}
