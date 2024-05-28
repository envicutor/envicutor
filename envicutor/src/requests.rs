use serde::Deserialize;

use crate::limits::Limits;

#[derive(Deserialize)]
pub struct AddRuntimeRequest {
    pub name: String,
    pub description: String,
    pub nix_shell: String,
    pub compile_command: String,
    pub run_command: String,
    limits: Option<Limits>,
}
