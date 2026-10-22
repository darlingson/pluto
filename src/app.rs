#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Main,
}

#[derive(Debug)]
pub struct App {
    pub screen: Screen,
    pub running: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::Main,
            running: true,
        }
    }

    pub fn quit(&mut self) {
        self.running = false;
    }
}
