use crate::Error;
use std::str::FromStr;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum NonceGenEnum {
    Lean,
    Xoshiro,
}

impl FromStr for NonceGenEnum {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "lean" => Ok(Self::Lean),
            "xoshiro" => Ok(Self::Xoshiro),
            _ => Err("Unknown string".into()),
        }
    }
}

#[cfg(feature = "overclock")]
#[derive(clap::Args, Debug, Default)]
pub struct OverClock {
    #[clap(long = "cuda-lock-mem-clocks", use_delimiter = true, help = "Lock mem clocks eg: ,810, [default: 0]")]
    pub cuda_lock_mem_clocks: Option<Vec<u32>>,
    #[clap(long = "cuda-lock-core-clocks", use_delimiter = true, help = "Lock core clocks eg: ,1200, [default: 0]")]
    pub cuda_lock_core_clocks: Option<Vec<u32>>,
    #[clap(long = "cuda-power-limits", use_delimiter = true, help = "Lock power limits eg: ,150, [default: 0]")]
    pub cuda_power_limits: Option<Vec<u32>>,
}

#[derive(clap::Args, Debug)]
pub struct CudaOpt {
    #[clap(long = "cuda-device", use_delimiter = true, help = "Which CUDA GPUs to use [default: all]")]
    pub cuda_device: Option<Vec<u16>>,
    #[clap(long = "cuda-workload", help = "Ratio of nonces to GPU possible parrallel run [default: 64]")]
    pub cuda_workload: Option<Vec<f32>>,
    #[clap(
        long = "cuda-workload-absolute",
        help = "The values given by workload are not ratio, but absolute number of nonces [default: false]"
    )]
    pub cuda_workload_absolute: bool,
    #[clap(long = "cuda-disable", help = "Disable cuda workers")]
    pub cuda_disable: bool,
    #[clap(
        long = "cuda-no-blocking-sync",
        help = "Actively wait for result. Higher CPU usage, but less red blocks. Can have lower workload.",
        long_help = "Actively wait for GPU result. Increases CPU usage, but removes delays that might result in red blocks. Can have lower workload."
    )]
    pub cuda_no_blocking_sync: bool,
    #[clap(
        long = "nonce-gen",
        help = "The random method used to generate nonces. Options: (i) xoshiro - each thread in GPU will have its own random state, creating a (pseudo-)independent xoshiro sequence (ii) lean - each GPU will have a single random nonce, and each GPU thread will work on nonce + thread id.",
        default_value = "lean"
    )]
    pub nonce_gen: NonceGenEnum,
    #[clap(long = "cuda-no-binary", help = "Force use runtime compilation compilation")]
    pub cuda_no_binary: bool,

    #[cfg(feature = "overclock")]
    #[clap(flatten)]
    pub overclock: OverClock,
}
