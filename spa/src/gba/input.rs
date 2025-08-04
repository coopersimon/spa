use crate::common::peripheral::joypad::Buttons;

impl From<crate::Button> for Buttons {
    fn from(b: crate::Button) -> Buttons {
        use crate::Button::*;
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
            Down    => Buttons::DOWN,
            _       => Buttons::empty()
        }
    }
}
