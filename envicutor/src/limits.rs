use serde::Deserialize;

type Seconds = u32;
type Kilobytes = u32;

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
