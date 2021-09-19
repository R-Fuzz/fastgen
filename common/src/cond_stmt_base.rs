use crate::defs::*;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Copy, Serialize, Deserialize)]
#[repr(C)] // It should be repr C since we will used it in shared memory
pub struct CondStmtBase {
    pub cmpid: u32,
    pub context: u32,
    pub order: u32,
    pub condition: u64,
}

/*
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub struct CondStmtMb {
    pub base: CondStmtBase,
    pub magic_bytes: Option<(Vec<u8>, Vec<u8>)>,
}
*/

impl PartialEq for CondStmtBase {
    fn eq(&self, other: &CondStmtBase) -> bool {
        self.cmpid == other.cmpid && self.context == other.context && self.order == other.order
    }
}

impl Eq for CondStmtBase {}
