use std::collections::HashMap;

pub struct Runtime {
    pub name: String,
    pub source_file_name: String,
    pub is_compiled: bool,
}
pub type Seconds = f32;
pub type WholeSeconds = u32;
pub type Kilobytes = u32;
pub type Metadata = HashMap<u32, Runtime>;
