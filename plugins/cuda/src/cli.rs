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
    #[clap(long = "cuda-no-blocking-sync", help = "Actively wait for GPU result. Increases CPU usage, but removes delays that might result in red blocks. Can have lower workload.")]
    pub cuda_no_blocking_sync: bool,
    #[clap(long = "cuda-lock-mem-clocks", use_delimiter = true, help = "Lock mem clocks eg: ,810, [default: 0]")]
    pub cuda_lock_mem_clocks: Option<Vec<u32>>,
    #[clap(long = "cuda-lock-core-clocks", use_delimiter = true, help = "Lock core clocks eg: ,1200, [default: 0]")]
    pub cuda_lock_core_clocks: Option<Vec<u32>>,
}
