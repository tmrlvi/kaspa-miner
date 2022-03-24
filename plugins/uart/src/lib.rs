#[macro_use]
extern crate kaspa_miner;

use std::cmp::min;
use clap::{ArgMatches, FromArgMatches};
use kaspa_miner::{Plugin, Worker, WorkerSpec};
use std::error::Error as StdError;

pub type Error = Box<dyn StdError + Send + Sync + 'static>;

mod cli;
mod worker;

use crate::cli::UartOpt;
use crate::worker::UartWorker;

const DEFAULT_WORKLOAD: usize = 10_000;

pub struct UartPlugin {
    specs: Vec<UartWorkerSpec>,
    _enabled: bool,
}

impl UartPlugin {
    fn new() -> Result<Self, Error> {
        Ok(Self { specs: Vec::new(), _enabled: true })
    }
}

impl Plugin for UartPlugin {
    fn name(&self) -> &'static str {
        "UART Worker"
    }

    fn enabled(&self) -> bool {
        self._enabled
    }

    fn get_worker_specs(&self) -> Vec<Box<dyn WorkerSpec>> {
        self.specs.iter().map(|spec| Box::new(spec.clone()) as Box<dyn WorkerSpec>).collect::<Vec<Box<dyn WorkerSpec>>>()
    }

    //noinspection RsTypeCheck
    fn process_option(&mut self, matches: &ArgMatches) -> Result<(), kaspa_miner::Error> {
        let opts: UartOpt = UartOpt::from_arg_matches(matches)?;
        if opts.uart_path.is_none() {
            self._enabled = false;
            return Ok(());
        }
        let paths = opts.uart_path.expect("Missing UART paths");
        let bauds = opts.uart_baud.expect("Missing UART bauds");

        let count = min(
            paths.len(),
            bauds.len(),
        );

        self.specs = (0..count)
            .map(|i| UartWorkerSpec {
                workload: match &opts.uart_workload {
                    Some(workload) if i < workload.len() => workload[i],
                    Some(workload) if !workload.is_empty() => *workload.last().unwrap(),
                    _ => DEFAULT_WORKLOAD,
                },
                path: paths[i].clone(),
                baud_rate: bauds[i]
            })
            .collect();
        Ok(())
    }
}

#[derive(Clone)]
struct UartWorkerSpec {
    workload: usize,
    path: String,
    baud_rate: u32,

}

impl WorkerSpec for UartWorkerSpec {
    fn build(&self) -> Box<dyn Worker> {
        Box::new(UartWorker::new(self.path.clone(), self.baud_rate, self.workload).unwrap())
    }
}

declare_plugin!(UartPlugin, UartPlugin::new, UartOpt);
