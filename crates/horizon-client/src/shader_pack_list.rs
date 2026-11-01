//! Packs in `shaderpacks/`, listed when the Shader Packs page opens.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Classic,
    Horizon,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub kind: Kind,
    pub title: String,
    pub file: String,
}