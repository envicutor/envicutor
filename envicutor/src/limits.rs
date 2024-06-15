use serde::Deserialize;

use crate::units::{Kilobytes, Seconds};

pub trait GetLimits {
    fn get(&self, system_limits: &MandatoryLimits) -> Result<MandatoryLimits, String>;
}

#[derive(Deserialize)]
pub struct Limits {
    pub wall_time: Option<Seconds>,
    pub cpu_time: Option<Seconds>,
    pub memory: Option<Kilobytes>,
    pub extra_time: Option<Seconds>,
    pub max_open_files: Option<u32>,
    pub max_file_size: Option<Kilobytes>,
    pub max_number_of_processes: Option<u32>,
}

impl GetLimits for Option<Limits> {
    fn get(&self, system_limits: &MandatoryLimits) -> Result<MandatoryLimits, String> {
        match &self {
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
        }
    }
}

#[derive(Clone)]
pub struct MandatoryLimits {
    pub wall_time: Seconds,
    pub cpu_time: Seconds,
    pub memory: Kilobytes,
    pub extra_time: Seconds,
    pub max_open_files: u32,
    pub max_file_size: Kilobytes,
    pub max_number_of_processes: u32,
}

#[derive(Clone)]
pub struct SystemLimits {
    pub installation: MandatoryLimits,
}
