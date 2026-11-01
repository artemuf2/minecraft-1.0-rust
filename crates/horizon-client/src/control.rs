pub struct Request;
pub struct Server;
impl Server {
    pub fn local() -> Self { Self }
    pub fn default_path() -> std::path::PathBuf { std::path::PathBuf::new() }
}
#[derive(Default)]
pub struct FrameStats;