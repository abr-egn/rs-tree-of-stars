use std::default;

use mortal;

pub struct Screen(pub mortal::Screen);

impl default::Default for Screen {
    fn default() -> Self {
        Screen(mortal::Screen::new(mortal::PrepareConfig::default()).unwrap())
    }
}