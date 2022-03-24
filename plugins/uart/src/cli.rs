#[derive(clap::Args, Debug)]
pub struct UartOpt {
     #[clap(long = "uart-path", use_delimiter = true, help = "Which CUDA GPUs to use [default: all]")]
     pub uart_path: Option<Vec<String>>,
     #[clap(long = "uart-baud", use_delimiter = true, help = "Which CUDA GPUs to use [default: all]")]
     pub uart_baud: Option<Vec<u32>>,
     #[clap(long = "uart-workload", help = "Ratio of nonces to GPU possible parrallel run [default: 64]")]
     pub uart_workload: Option<Vec<usize>>,
}
