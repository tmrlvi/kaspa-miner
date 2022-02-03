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

#[derive(clap::Args, Debug)]
pub struct OpenCLOpt {
    #[clap(long = "opencl-platform", help = "Which OpenCL platform to use (limited to one per executable)")]
    pub opencl_platform: Option<u16>,
    #[clap(long = "opencl-device", use_delimiter = true, help = "Which OpenCL GPUs to use on a specific platform")]
    pub opencl_device: Option<Vec<u16>>,
    #[clap(long = "opencl-workload", help = "Ratio of nonces to GPU possible parrallel run in OpenCL [default: 512]")]
    pub opencl_workload: Option<Vec<f32>>,
    #[clap(
        long = "opencl-workload-absolute",
        help = "The values given by workload are not ratio, but absolute number of nonces in OpenCL [default: false]"
    )]
    pub opencl_workload_absolute: bool,
    #[clap(long = "opencl-enable", help = "Enable opencl, and take all devices of the chosen platform")]
    pub opencl_enable: bool,
    #[clap(long = "opencl-amd-binary", help = "Disable fetching of precompiled AMD kernel (if exists)")]
    pub opencl_amd_binary: bool,
    #[clap(
        long = "experimental-amd",
        help = "Uses SMID instructions in AMD. Miner will crash if instruction is not supported"
    )]
    pub experimental_amd: bool,
    #[clap(
        long = "nonce-gen",
        help = "The random method used to generate nonces. Options: (i) xoshiro - each thread in GPU will have its own random state, creating a (pseudo-)independent xoshiro sequence (ii) lean - each GPU will have a single random nonce, and each GPU thread will work on nonce + thread id.",
        default_value = "lean"
    )]
    pub nonce_gen: NonceGenEnum,
}
