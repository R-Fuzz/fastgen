// branch.rs
pub const MAP_SIZE_POW2: usize = 20;
pub const BRANCHES_SIZE: usize = 1 << MAP_SIZE_POW2;
pub const TMOUT_SKIP: usize = 3;
pub const TIME_LIMIT: u64 = 1;
pub const MEM_LIMIT: u64 = 200; // MB
pub const TIME_LIMIT_TRACK: u64 = 12;
pub const MEM_LIMIT_TRACK: u64 = 0;

pub const MAX_INVARIABLE_NUM: usize = 16;
pub const MAX_INPUT_LEN: usize = 15000;
pub const SAVING_WHOLE: bool = false;
pub const USE_CODECACHE: bool = true;
