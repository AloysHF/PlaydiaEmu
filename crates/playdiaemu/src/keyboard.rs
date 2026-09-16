use std::str::FromStr;

use minifb::{Key, Window};
use playdiaemu_core::InputButtons;

const DEFAULT_MAPPINGS: &[(PlaydiaButton, Key)] = &[
    (PlaydiaButton::Up, Key::Up),
    (PlaydiaButton::Down, Key::Down),
    (PlaydiaButton::Left, Key::Left),
    (PlaydiaButton::Right, Key::Right),
    (PlaydiaButton::A, Key::Z),
    (PlaydiaButton::B, Key::X),
    (PlaydiaButton::Start, Key::Enter),
    (PlaydiaButton::Select, Key::RightShift),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaydiaButton {
    Up,
    Down,
    Left,
    Right,
    A,
    B,
    Start,
    Select,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemapSpec {
    button: PlaydiaButton,
    key: Key,
}

impl FromStr for RemapSpec {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (button, key) = value
            .split_once(':')
            .ok_or_else(|| "expected BUTTON:KEY, for example a:space".to_string())?;
        Ok(Self {
            button: parse_button(button.trim())?,
            key: parse_key(key.trim())?,
        })
    }
}

pub struct KeyboardMapper {
    mappings: Vec<(PlaydiaButton, Key)>,
    swap_ab: bool,
}

impl KeyboardMapper {
    pub fn new(remappings: &[RemapSpec], swap_ab: bool) -> Self {
        let mut mappings = DEFAULT_MAPPINGS.to_vec();
        for remapping in remappings {
            mappings.retain(|(button, _)| *button != remapping.button);
            mappings.push((remapping.button, remapping.key));
        }
        Self { mappings, swap_ab }
    }

    pub fn pressed_buttons(&self, window: &Window) -> InputButtons {
        self.buttons_from_key_state(|key| window.is_key_down(key))
    }

    fn buttons_from_key_state(&self, mut is_down: impl FnMut(Key) -> bool) -> InputButtons {
        let mut buttons = InputButtons::default();
        for (button, key) in &self.mappings {
            if !is_down(*key) {
                continue;
            }
            match button {
                PlaydiaButton::Up => buttons.up = true,
                PlaydiaButton::Down => buttons.down = true,
                PlaydiaButton::Left => buttons.left = true,
                PlaydiaButton::Right => buttons.right = true,
                PlaydiaButton::A => buttons.a = true,
                PlaydiaButton::B => buttons.b = true,
                PlaydiaButton::Start => buttons.start = true,
                PlaydiaButton::Select => buttons.select = true,
            }
        }
        if self.swap_ab {
            std::mem::swap(&mut buttons.a, &mut buttons.b);
        }
        buttons
    }
}

fn parse_button(name: &str) -> Result<PlaydiaButton, String> {
    match name.to_ascii_lowercase().as_str() {
        "up" => Ok(PlaydiaButton::Up),
        "down" => Ok(PlaydiaButton::Down),
        "left" => Ok(PlaydiaButton::Left),
        "right" => Ok(PlaydiaButton::Right),
        "a" => Ok(PlaydiaButton::A),
        "b" => Ok(PlaydiaButton::B),
        "start" => Ok(PlaydiaButton::Start),
        "select" => Ok(PlaydiaButton::Select),
        _ => Err(format!(
            "unknown Playdia button '{name}'; expected up, down, left, right, a, b, start, or select"
        )),
    }
}

fn parse_key(name: &str) -> Result<Key, String> {
    let key = match name.to_ascii_lowercase().as_str() {
        "a" => Key::A,
        "b" => Key::B,
        "c" => Key::C,
        "d" => Key::D,
        "e" => Key::E,
        "f" => Key::F,
        "g" => Key::G,
        "h" => Key::H,
        "i" => Key::I,
        "j" => Key::J,
        "k" => Key::K,
        "l" => Key::L,
        "m" => Key::M,
        "n" => Key::N,
        "o" => Key::O,
        "p" => Key::P,
        "q" => Key::Q,
        "r" => Key::R,
        "s" => Key::S,
        "t" => Key::T,
        "u" => Key::U,
        "v" => Key::V,
        "w" => Key::W,
        "x" => Key::X,
        "y" => Key::Y,
        "z" => Key::Z,
        "0" => Key::Key0,
        "1" => Key::Key1,
        "2" => Key::Key2,
        "3" => Key::Key3,
        "4" => Key::Key4,
        "5" => Key::Key5,
        "6" => Key::Key6,
        "7" => Key::Key7,
        "8" => Key::Key8,
        "9" => Key::Key9,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        "up" => Key::Up,
        "down" => Key::Down,
        "left" => Key::Left,
        "right" => Key::Right,
        "space" => Key::Space,
        "enter" | "return" => Key::Enter,
        "backspace" => Key::Backspace,
        "tab" => Key::Tab,
        "delete" => Key::Delete,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" => Key::PageUp,
        "pagedown" => Key::PageDown,
        "leftshift" => Key::LeftShift,
        "rightshift" => Key::RightShift,
        "leftctrl" => Key::LeftCtrl,
        "rightctrl" => Key::RightCtrl,
        "leftalt" => Key::LeftAlt,
        "rightalt" => Key::RightAlt,
        "escape" | "esc" => return Err("escape is reserved for exiting the emulator".to_string()),
        _ => return Err(format!("unknown key '{name}'")),
    };
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remapping_replaces_the_default_key() {
        let mapper = KeyboardMapper::new(&["a:space".parse().unwrap()], false);
        assert_eq!(
            mapper.buttons_from_key_state(|key| key == Key::Space),
            InputButtons {
                a: true,
                ..Default::default()
            }
        );
        assert_eq!(
            mapper.buttons_from_key_state(|key| key == Key::Z),
            InputButtons::default()
        );
    }

    #[test]
    fn swap_ab_exchanges_face_buttons() {
        let mapper = KeyboardMapper::new(&[], true);
        assert_eq!(
            mapper.buttons_from_key_state(|key| key == Key::X),
            InputButtons {
                a: true,
                ..Default::default()
            }
        );
        assert_eq!(
            mapper.buttons_from_key_state(|key| key == Key::Z),
            InputButtons {
                b: true,
                ..Default::default()
            }
        );
    }

    #[test]
    fn parser_accepts_playdia_buttons_and_rejects_escape() {
        for button in ["up", "down", "left", "right", "a", "b", "start", "select"] {
            assert!(format!("{button}:space").parse::<RemapSpec>().is_ok());
        }
        assert!("a:escape".parse::<RemapSpec>().is_err());
    }
}
