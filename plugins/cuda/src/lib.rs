#[macro_use]
extern crate kaspa_miner;

use clap::{ArgMatches, FromArgMatches};
use cust::prelude::*;
use kaspa_miner::{Plugin, Worker, WorkerSpec};
use log::LevelFilter;
use std::error::Error as StdError;

pub type Error = Box<dyn StdError + Send + Sync + 'static>;

mod cli;
mod worker;

use crate::cli::CudaOpt;
use crate::worker::CudaGPUWorker;

const DEFAULT_WORKLOAD_SCALE: f32 = 64.;

pub struct CudaPlugin {
    specs: Vec<CudaWorkerSpec>,
    _enabled: bool,
}

impl CudaPlugin {
    fn new() -> Result<Self, Error> {
        cust::init(CudaFlags::empty())?;
        env_logger::builder().filter_level(LevelFilter::Info).parse_default_env().init();
        Ok(Self { specs: Vec::new(), _enabled: false })
    }
}

impl Plugin for CudaPlugin {
    fn name(&self) -> &'static str {
        "CUDA Worker"
    }

    fn enabled(&self) -> bool {
        self._enabled
    }

    fn get_worker_specs(&self) -> Vec<Box<dyn WorkerSpec>> {
        self.specs.iter().map(|spec| Box::new(*spec) as Box<dyn WorkerSpec>).collect::<Vec<Box<dyn WorkerSpec>>>()
    }

    //noinspection RsTypeCheck
    fn process_option(&mut self, matches: &ArgMatches) -> Result<(), kaspa_miner::Error> {
        let opts: CudaOpt = CudaOpt::from_arg_matches(matches)?;

        self._enabled = !opts.cuda_disable;

        let gpus: Vec<u16> = match &opts.cuda_device {
            Some(devices) => devices.clone(),
            None => {
                let gpu_count = Device::num_devices().unwrap() as u16;
                (0..gpu_count).collect()
            }
        };

        self.specs = (0..gpus.len())
            .map(|i| CudaWorkerSpec {
                device_id: gpus[i] as u32,
                workload: match &opts.cuda_workload {
                    Some(workload) if i < workload.len() => workload[i],
                    Some(workload) if !workload.is_empty() => *workload.last().unwrap(),
                    _ => DEFAULT_WORKLOAD_SCALE,
                },
                is_absolute: opts.cuda_workload_absolute,
                blocking_sync: opts.cuda_blocking_sync,
            })
            .collect();
        Ok(())
    }
}

#[derive(Copy, Clone)]
struct CudaWorkerSpec {
    device_id: u32,
    workload: f32,
    is_absolute: bool,
    blocking_sync: bool,
}

impl WorkerSpec for CudaWorkerSpec {
    fn build(&self) -> Box<dyn Worker> {
        Box::new(CudaGPUWorker::new(self.device_id, self.workload, self.is_absolute, self.blocking_sync).unwrap())
    }
}

declare_plugin!(CudaPlugin, CudaPlugin::new, CudaOpt);
