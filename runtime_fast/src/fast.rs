use super::{forkcli, shm_branches};

use std::sync::Once;

static START: Once = Once::new();

#[ctor]
fn fast_init() {
    START.call_once(|| {
        shm_branches::map_branch_counting_shm();
        forkcli::start_forkcli();
    });
}
