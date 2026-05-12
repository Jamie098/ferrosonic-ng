use strum_macros::{Display, EnumIter};

#[derive(Display, EnumIter, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BrowseTab {
    #[default]
    Songs,
    Albums,
}

#[derive(Display, EnumIter, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongOption {
    All,
    Starred,
    Random,
}
