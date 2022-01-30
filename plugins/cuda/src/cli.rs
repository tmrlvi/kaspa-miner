#[derive(clap::Args, Debug)]
pub struct CudaOpt {
    #[clap(long = "cuda-device", use_delimiter = true, help = "Which CUDA GPUs to use [default: all]")]
    pub cuda_device: Option<Vec<u16>>,
    #[clap(long = "cuda-workload", help = "Ratio of nonces to GPU possible parrallel run [defualt: 16]")]
    pub cuda_workload: Option<Vec<f32>>,
    #[clap(
        long = "cuda-workload-absolute",
        help = "The values given by workload are not ratio, but absolute number of nonces [default: false]"
    )]
    pub cuda_workload_absolute: bool,
    #[clap(long = "cuda-disable", help = "Disable cuda workers")]
    pub cuda_disable: bool,
    #[clap(long = "cuda-blocking-sync", help = "Block threads when waiting for GPU result. Lowers CPU usage, but might cause delays resulting in red blocks. Requires higher workload.")]
    pub cuda_blocking_sync: bool,
}
