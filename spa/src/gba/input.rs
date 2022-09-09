use crate::common::peripheral::joypad::Buttons;

pub enum Button {
    A,
    B,
    Start,
    Select,
    Left,
    Right,
    Up,
    Down,
    L,
    R
}

impl From<crate::gba::Button> for Buttons {
    fn from(b: crate::gba::Button) -> Buttons {
        use crate::gba::Button::*;
        match b {
            A       => Buttons::A,
            B       => Buttons::B,
            Select  => Buttons::SELECT,
            Start   => Buttons::START,
            L       => Buttons::L,
            R       => Buttons::R,
            Left    => Buttons::LEFT,
            Right   => Buttons::RIGHT,
            Up      => Buttons::UP,
            Down    => Buttons::DOWN
        }
    }
}
