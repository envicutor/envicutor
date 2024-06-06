use serde::Deserialize;

use crate::limits::{Limits, MandatoryLimits};

#[derive(Deserialize)]
pub struct AddRuntimeRequest {
    pub name: String,
    pub description: String,
    pub nix_shell: String,
    pub compile_command: String,
    pub run_command: String,
    pub source_file_name: String,
    limits: Option<Limits>,
}

impl AddRuntimeRequest {
    pub fn get_limits(&self, system_limits: &MandatoryLimits) -> Result<MandatoryLimits, String> {
        let res = match &self.limits {
            Some(req_limits) => {
                if let Some(wall_time) = req_limits.wall_time {
                    if wall_time > system_limits.wall_time {
                        return Err(format!(
                            "wall_time can't exceed {} seconds",
                            system_limits.wall_time
                        ));
                    }
                }
                if let Some(cpu_time) = req_limits.cpu_time {
                    if cpu_time > system_limits.cpu_time {
                        return Err(format!(
                            "cpu_time can't exceed {} seconds",
                            system_limits.cpu_time
                        ));
                    }
                }
                if let Some(memory) = req_limits.memory {
                    if memory > system_limits.memory {
                        return Err(format!(
                            "memory can't exceed {} kilobytes",
                            system_limits.memory
                        ));
                    }
                }
                if let Some(extra_time) = req_limits.extra_time {
                    if extra_time > system_limits.extra_time {
                        return Err(format!(
                            "extra_time can't exceed {} seconds",
                            system_limits.extra_time
                        ));
                    }
                }
                if let Some(max_open_files) = req_limits.max_open_files {
                    if max_open_files > system_limits.max_open_files {
                        return Err(format!(
                            "max_open_files can't exceed {}",
                            system_limits.max_open_files
                        ));
                    }
                }
                if let Some(max_file_size) = req_limits.max_file_size {
                    if max_file_size > system_limits.max_file_size {
                        return Err(format!(
                            "max_file_size can't exceed {} kilobytes",
                            system_limits.max_file_size
                        ));
                    }
                }
                if let Some(max_number_of_processes) = req_limits.max_number_of_processes {
                    if max_number_of_processes > system_limits.max_number_of_processes {
                        return Err(format!(
                            "max_number_of_processes can't exceed {}",
                            system_limits.max_number_of_processes
                        ));
                    }
                }
                Ok(MandatoryLimits {
                    wall_time: req_limits.wall_time.unwrap_or(system_limits.wall_time),
                    cpu_time: req_limits.cpu_time.unwrap_or(system_limits.cpu_time),
                    memory: req_limits.memory.unwrap_or(system_limits.memory),
                    extra_time: req_limits.extra_time.unwrap_or(system_limits.extra_time),
                    max_open_files: req_limits
                        .max_open_files
                        .unwrap_or(system_limits.max_open_files),
                    max_file_size: req_limits
                        .max_file_size
                        .unwrap_or(system_limits.max_file_size),
                    max_number_of_processes: req_limits
                        .max_number_of_processes
                        .unwrap_or(system_limits.max_number_of_processes),
                })
            }
            None => Ok(system_limits.clone()),
        };
        res
    }
}
