pub enum PingStatus {
    Idle,
    Loading,
    Ok(String),
    Err(String),
}

pub struct App {
    pub should_quit: bool,
    pub ping_status: PingStatus,
}

impl App {
    pub fn new() -> Self {
        Self {
            should_quit: false,
            ping_status: PingStatus::Idle,
        }
    }
}
