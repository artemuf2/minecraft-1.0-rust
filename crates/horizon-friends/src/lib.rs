//! Stub: Horizon-Lite does not support friends / invite codes.
pub const ADDRESS_PREFIX: &str = "friend:";

pub fn relay_url() -> Option<String> {
    None
}

pub mod invite {
    pub enum Parsed {
        Short(ShortCode),
        Long(String),
    }
    pub struct ShortCode {
        pub nameplate: String,
    }
    pub fn parse(_code: &str) -> Option<Parsed> {
        None
    }
}

pub mod guest {
    use super::PathMode;
    pub struct GuestConfig {
        pub code: String,
        pub relay: Option<String>,
        pub protocol: i32,
        pub game: String,
        pub path: PathMode,
    }
    pub struct Tunnel {
        pub direct: bool,
    }
    impl Tunnel {
        pub fn serve_loopback(self) -> Result<(u16, ()), String> {
            Err("friends not supported".to_string())
        }
    }
    pub fn join(_config: &GuestConfig) -> Result<Tunnel, String> {
        Err("friends not supported".to_string())
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum PathMode {
    #[default]
    Direct,
    Relay,
}

impl PathMode {
    pub fn from_env() -> Self {
        Self::Direct
    }
}