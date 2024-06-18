use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

use crate::globals::MAX_BOX_ID;

pub fn get_next_box_id(box_id: &Arc<AtomicU64>) -> u64 {
    box_id.fetch_add(1, Ordering::SeqCst) % MAX_BOX_ID
}
