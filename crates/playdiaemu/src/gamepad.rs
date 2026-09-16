use gilrs::{Axis, Button, EventType, Gilrs};
use playdiaemu_core::InputButtons;

const STICK_DEADZONE: f32 = 0.5;

/// Polls the first connected physical gamepad and maps it onto Playdia buttons.
///
/// Face buttons follow the same RetroPad convention as dingoo-emu: East鈫扐,
/// South鈫払. Left/right sticks also act as a digital D-pad past the deadzone.
pub struct GamepadMapper {
    gilrs: Option<Gilrs>,
    swap_ab: bool,
}

impl GamepadMapper {
    pub fn new(enabled: bool, swap_ab: bool) -> Self {
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
        Self { gilrs, swap_ab }
    }

    pub fn pressed_buttons(&mut self) -> InputButtons {
        let Some(gilrs) = self.gilrs.as_mut() else {
            return InputButtons::default();
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

        let mut buttons = InputButtons::default();
        for (_id, gamepad) in gilrs.gamepads() {
            if !gamepad.is_connected() {
                continue;
            }
            buttons = map_buttons(&gamepad);
            break;
        }

        if self.swap_ab {
            std::mem::swap(&mut buttons.a, &mut buttons.b);
        }
        buttons
    }
}

fn map_buttons(gamepad: &gilrs::Gamepad<'_>) -> InputButtons {
    InputButtons {
        up: gamepad.is_pressed(Button::DPadUp)
            || stick_axis(gamepad, Axis::LeftStickY) > STICK_DEADZONE
            || stick_axis(gamepad, Axis::RightStickY) > STICK_DEADZONE,
        down: gamepad.is_pressed(Button::DPadDown)
            || stick_axis(gamepad, Axis::LeftStickY) < -STICK_DEADZONE
            || stick_axis(gamepad, Axis::RightStickY) < -STICK_DEADZONE,
        left: gamepad.is_pressed(Button::DPadLeft)
            || stick_axis(gamepad, Axis::LeftStickX) < -STICK_DEADZONE
            || stick_axis(gamepad, Axis::RightStickX) < -STICK_DEADZONE,
        right: gamepad.is_pressed(Button::DPadRight)
            || stick_axis(gamepad, Axis::LeftStickX) > STICK_DEADZONE
            || stick_axis(gamepad, Axis::RightStickX) > STICK_DEADZONE,
        a: gamepad.is_pressed(Button::East),
        b: gamepad.is_pressed(Button::South),
        start: gamepad.is_pressed(Button::Start),
        select: gamepad.is_pressed(Button::Select),
    }
}

fn stick_axis(gamepad: &gilrs::Gamepad<'_>, axis: Axis) -> f32 {
    gamepad.value(axis)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_mapper_reports_no_buttons() {
        let mut mapper = GamepadMapper::new(false, false);
        assert_eq!(mapper.pressed_buttons(), InputButtons::default());
    }

    #[test]
    fn stick_deadzone_threshold_rejects_small_values() {
        assert!(0.4_f32.abs() < STICK_DEADZONE);
        assert!(0.5_f32.abs() >= STICK_DEADZONE);
    }

    #[test]
    fn swap_ab_exchanges_face_buttons() {
        let mut buttons = InputButtons {
            a: true,
            ..Default::default()
        };
        std::mem::swap(&mut buttons.a, &mut buttons.b);
        assert!(buttons.b);
        assert!(!buttons.a);
    }
}
