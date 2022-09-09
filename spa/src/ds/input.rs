// Dealing with user input.

use crate::common::peripheral::joypad::Buttons;
use super::joypad::DSButtons;

pub enum Button {
    A,
    B,
    X,
    Y,
    Start,
    Select,
    Left,
    Right,
    Up,
    Down,
    L,
    R
}

#[derive(Clone)]
pub struct UserInput {
    pub buttons:    Buttons,
    pub ds_buttons: DSButtons,

    pub touchscreen:    Option<(f64, f64)>
}

impl Default for UserInput {
    fn default() -> Self {
        Self {
            buttons:    Buttons::from_bits_truncate(0xFFFF),
            ds_buttons: DSButtons::from_bits_truncate(0x4B),

            touchscreen:    None,
        }
    }
}

impl UserInput {
    pub fn set_button(&mut self, button: Button, pressed: bool) {
        use Button::*;
        match button {
            A => self.buttons.set(Buttons::A, !pressed),
            B => self.buttons.set(Buttons::B, !pressed),
            X => self.ds_buttons.set(DSButtons::X, !pressed),
            Y => self.ds_buttons.set(DSButtons::Y, !pressed),
            Start => self.buttons.set(Buttons::START, !pressed),
            Select => self.buttons.set(Buttons::SELECT, !pressed),
            Left => self.buttons.set(Buttons::LEFT, !pressed),
            Right => self.buttons.set(Buttons::RIGHT, !pressed),
            Up => self.buttons.set(Buttons::UP, !pressed),
            Down => self.buttons.set(Buttons::DOWN, !pressed),
            L => self.buttons.set(Buttons::L, !pressed),
            R => self.buttons.set(Buttons::R, !pressed)
        }
    }

    pub fn set_touchscreen(&mut self, coords: Option<(f64, f64)>) {
        self.ds_buttons.set(DSButtons::PEN_DOWN, !coords.is_some());
        self.touchscreen = coords;
    }
}