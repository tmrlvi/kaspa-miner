use std::num::Wrapping;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crate::{pow, watch, Error};
use log::{error, info, warn};
use rand::{thread_rng, RngCore};
use tokio::sync::mpsc::Sender;
use tokio::task::{self, JoinHandle};
use tokio::time::MissedTickBehavior;

use crate::pow::BlockSeed;
use kaspa_miner::{PluginManager, WorkerSpec};

type MinerHandler = std::thread::JoinHandle<Result<(), Error>>;

#[derive(Clone)]
enum WorkerCommand {
    Job(Box<pow::State>),
    Close,
}

#[allow(dead_code)]
pub struct MinerManager {
    handles: Vec<MinerHandler>,
    block_channel: watch::Sender<Option<WorkerCommand>>,
    send_channel: Sender<BlockSeed>,
    logger_handle: JoinHandle<()>,
    is_synced: bool,
    hashes_tried: Arc<AtomicU64>,
    current_state_id: AtomicUsize,
}

impl Drop for MinerManager {
    fn drop(&mut self) {
        info!("Closing miner");
        self.logger_handle.abort();
        match self.block_channel.send(Some(WorkerCommand::Close)) {
            Ok(_) => {}
            Err(_) => warn!("All workers are already dead"),
        }
        info!("Waiting for threads to close");
        while !self.handles.is_empty() {
            let handle = self.handles.pop().expect("There should be at least one");
            info!("Waiting on handle...");
            match handle.join() {
                Ok(_) => {}
                Err(e) => panic!("Change failed to close gracefully: {:?}", e),
            };
            info!("Handle closed. {} left.", self.handles.len());
        }
        info!("Miner closed!");
    }
}

pub fn get_num_cpus(n_cpus: Option<u16>) -> u16 {
    n_cpus.unwrap_or_else(|| {
        num_cpus::get_physical().try_into().expect("Doesn't make sense to have more than 65,536 CPU cores")
    })
}

const LOG_RATE: Duration = Duration::from_secs(10);

impl MinerManager {
    pub fn new(send_channel: Sender<BlockSeed>, n_cpus: Option<u16>, manager: &PluginManager) -> Self {
        let hashes_tried = Arc::new(AtomicU64::new(0));
        let (send, recv) = watch::channel(None);
        let mut handles =
            Self::launch_cpu_threads(send_channel.clone(), Arc::clone(&hashes_tried), recv.clone(), n_cpus)
                .collect::<Vec<MinerHandler>>();
        if manager.has_specs() {
            handles.append(&mut Self::launch_gpu_threads(
                send_channel.clone(),
                Arc::clone(&hashes_tried),
                recv,
                manager,
            ));
        }
        Self {
            handles,
            block_channel: send,
            send_channel,
            logger_handle: task::spawn(Self::log_hashrate(Arc::clone(&hashes_tried))),
            is_synced: true,
            hashes_tried,
            current_state_id: AtomicUsize::new(0),
        }
    }

    fn launch_cpu_threads(
        send_channel: Sender<BlockSeed>,
        hashes_tried: Arc<AtomicU64>,
        work_channel: watch::Receiver<Option<WorkerCommand>>,
        n_cpus: Option<u16>,
    ) -> impl Iterator<Item = MinerHandler> {
        let n_cpus = get_num_cpus(n_cpus);
        info!("launching: {} cpu miners", n_cpus);
        (0..n_cpus)
            .map(move |_| Self::launch_cpu_miner(send_channel.clone(), work_channel.clone(), Arc::clone(&hashes_tried)))
    }

    fn launch_gpu_threads(
        send_channel: Sender<BlockSeed>,
        hashes_tried: Arc<AtomicU64>,
        work_channel: watch::Receiver<Option<WorkerCommand>>,
        manager: &PluginManager,
    ) -> Vec<MinerHandler> {
        let mut vec = Vec::<MinerHandler>::new();
        let specs = manager.build().unwrap();
        for spec in specs {
            vec.push(Self::launch_gpu_miner(
                send_channel.clone(),
                work_channel.clone(),
                Arc::clone(&hashes_tried),
                spec,
            ));
        }
        vec
    }

    pub async fn process_block(&mut self, block: Option<BlockSeed>) -> Result<(), Error> {
        let state = match block {
            Some(b) => {
                self.is_synced = true;
                let id = self.current_state_id.fetch_add(1, Ordering::SeqCst);
                Some(WorkerCommand::Job(Box::new(pow::State::new(id, b)?)))
            }
            None => {
                if !self.is_synced {
                    return Ok(());
                }
                self.is_synced = false;
                warn!("Kaspad is not synced, skipping current template");
                None
            }
        };

        self.block_channel.send(state).map_err(|_e| "Failed sending block to threads")?;
        Ok(())
    }

    #[allow(unreachable_code)]
    fn launch_gpu_miner(
        send_channel: Sender<BlockSeed>,
        mut block_channel: watch::Receiver<Option<WorkerCommand>>,
        hashes_tried: Arc<AtomicU64>,
        spec: Box<dyn WorkerSpec>,
    ) -> MinerHandler {
        std::thread::spawn(move || {
            let mut box_ = spec.build();
            let gpu_work = box_.as_mut();
            (|| {
                info!("Spawned Thread for GPU {}", gpu_work.id());
                let mut nonces = vec![0u64; 1];

                let mut state = None;

                loop {
                    nonces[0] = 0;
                    if state.is_none() {
                        state = match block_channel.wait_for_change() {
                            Ok(cmd) => match cmd {
                                Some(WorkerCommand::Job(s)) => Some(s),
                                Some(WorkerCommand::Close) => {
                                    info!("Closing thread for GPU {}", gpu_work.id());
                                    return Ok(());
                                }
                                None => None,
                            },
                            Err(e) => {
                                info!("{}: GPU thread crashed: {}", gpu_work.id(), e.to_string());
                                return Ok(());
                            }
                        };
                    }
                    let state_ref = match &state {
                        Some(s) => {
                            s.load_to_gpu(gpu_work);
                            s
                        },
                        None => continue,
                    };
                    state_ref.pow_gpu(gpu_work);
                    gpu_work.sync().unwrap();

                    gpu_work.copy_output_to(&mut nonces)?;
                    if nonces[0] != 0 {
                        if let Some(block_seed) = state_ref.generate_block_if_pow(nonces[0]) {
                            match send_channel.blocking_send(block_seed.clone()) {
                                Ok(()) => block_seed.report_block(),
                                Err(e) => error!("Failed submitting block: ({})", e.to_string()),
                            };
                            if let BlockSeed::FullBlock(_) = block_seed {
                                state = None;
                            }
                            nonces[0] = 0;
                            hashes_tried.fetch_add(gpu_work.get_workload().try_into().unwrap(), Ordering::AcqRel);
                            continue;
                        } else {
                            let hash = state_ref.calculate_pow(nonces[0]);
                            warn!("Something is wrong in GPU results! Got nonce {}, with hash real {:?}  (target: {}*2^196)", nonces[0], hash.0, state_ref.target.0[3]);
                            break;
                        }
                    }

                        /*
                        info!("Output should be: {:02X?}", state_ref.calculate_pow(nonces[0]).to_le_bytes());
                        info!("We got: {:02X?} (Nonces: {:02X?})", hashes[0], nonces[0].to_le_bytes());
                        assert!(state_ref.calculate_pow(nonces[0]).to_le_bytes() == hashes[0]);
                        */
                        /*
                        info!("Output should be: {}", state_ref.calculate_pow(nonces[nonces.len()-1]).0[3]);
                        info!("We got: {} (Nonces: {})", Uint256::from_le_bytes(hashes[nonces.len()-1]).0[3], nonces[nonces.len()-1]);
                        assert!(state_ref.calculate_pow(nonces[nonces.len()-1]).0[0] == Uint256::from_le_bytes(hashes[nonces.len()-1]).0[0]);
                         */
                        /*
                        if state_ref.calculate_pow(nonces[0]).0[0] != Uint256::from_le_bytes(hashes[0]).0[0] {
                            gpu_work.sync()?;
                            let mut nonce_vec = vec![nonces[0]; 1];
                            nonce_vec.append(&mut vec![0u64; gpu_work.workload-1]);
                            gpu_work.calculate_pow_hash(&state_ref.pow_hash_header, Some(&nonce_vec));
                            gpu_work.sync()?;
                            gpu_work.calculate_matrix_mul(&mut state_ref.matrix.clone().0.as_slice().as_dbuf().unwrap());
                            gpu_work.sync()?;
                            gpu_work.calculate_heavy_hash();
                            gpu_work.sync()?;
                            let mut hashes2  = vec![[0u8; 32]; out_size];
                            let mut nonces2= vec![0u64; out_size];
                            gpu_work.copy_output_to(&mut hashes2, &mut nonces2);
                            assert!(state_ref.calculate_pow(nonces[0]).to_le_bytes() == hashes2[0]);
                            assert!(nonces2[0] == nonces[0]);
                            assert!(hashes2 == hashes);
                            assert!(false);
                        }*/

                    hashes_tried.fetch_add(gpu_work.get_workload().try_into().unwrap(), Ordering::AcqRel);

                    {
                        if let Some(new_cmd) = block_channel.get_changed()? {
                            state = match new_cmd {
                                Some(WorkerCommand::Job(s)) => Some(s),
                                Some(WorkerCommand::Close) => {
                                    info!("Closing thread for GPU {}", gpu_work.id());
                                    return Ok(());
                                }
                                None => None,
                            };
                        }
                    }
                }
                Ok(())
            })()
            .map_err(|e: Error| {
                error!("{}: GPU thread crashed: {}", gpu_work.id(), e.to_string());
                e
            })
        })
    }

    #[allow(unreachable_code)]
    fn launch_cpu_miner(
        send_channel: Sender<BlockSeed>,
        mut block_channel: watch::Receiver<Option<WorkerCommand>>,
        hashes_tried: Arc<AtomicU64>,
    ) -> MinerHandler {
        let mut nonce = Wrapping(thread_rng().next_u64());
        let mut mask = Wrapping(0);
        let mut fixed = Wrapping(0);
        std::thread::spawn(move || {
            (|| {
                let mut state = None;

                loop {
                    if state.is_none() {
                        state = match block_channel.wait_for_change() {
                            Ok(cmd) => match cmd {
                                Some(WorkerCommand::Job(s)) => Some(s),
                                Some(WorkerCommand::Close) => {
                                    return Ok(());
                                }
                                None => None,
                            },
                            Err(e) => {
                                info!("CPU thread crashed: {}", e.to_string());
                                return Ok(());
                            }
                        };
                        if let Some(s) = &state {
                            mask = Wrapping(s.nonce_mask);
                            fixed = Wrapping(s.nonce_fixed);
                        }
                    }
                    let state_ref = match state.as_mut() {
                        Some(s) => s,
                        None => continue,
                    };
                    nonce = (nonce & mask) | fixed;

                    if let Some(block_seed) = state_ref.generate_block_if_pow(nonce.0) {
                        match send_channel.blocking_send(block_seed.clone()) {
                            Ok(()) => block_seed.report_block(),
                            Err(e) => error!("Failed submitting block: ({})", e.to_string()),
                        };
                        if let BlockSeed::FullBlock(_) = block_seed {
                            state = None;
                        }
                    }
                    nonce += Wrapping(1);
                    // TODO: Is this really necessary? can we just use Relaxed?
                    hashes_tried.fetch_add(1, Ordering::AcqRel);

                    if nonce.0 % 128 == 0 {
                        if let Some(new_cmd) = block_channel.get_changed()? {
                            state = match new_cmd {
                                Some(WorkerCommand::Job(s)) => Some(s),
                                Some(WorkerCommand::Close) => {
                                    return Ok(());
                                }
                                None => None,
                            };
                        }
                    }
                }
                Ok(())
            })()
            .map_err(|e: Error| {
                error!("CPU thread crashed: {}", e.to_string());
                e
            })
        })
    }

    async fn log_hashrate(hashes_tried: Arc<AtomicU64>) {
        let mut ticker = tokio::time::interval(LOG_RATE);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut last_instant = ticker.tick().await;
        for i in 0u64.. {
            let now = ticker.tick().await;
            let hashes = hashes_tried.swap(0, Ordering::AcqRel);
            let rate = (hashes as f64) / (now - last_instant).as_secs_f64();
            if hashes == 0 && i % 2 == 0 {
                warn!("Workers stalled or crashed. Considered reducing workload and check that your node is synced")
            } else if hashes != 0 {
                let (rate, suffix) = Self::hash_suffix(rate);
                info!("Current hashrate is: {:.2} {}", rate, suffix);
            }
            last_instant = now;
        }
    }

    #[inline]
    fn hash_suffix(n: f64) -> (f64, &'static str) {
        match n {
            n if n < 1_000.0 => (n, "hash/s"),
            n if n < 1_000_000.0 => (n / 1_000.0, "Khash/s"),
            n if n < 1_000_000_000.0 => (n / 1_000_000.0, "Mhash/s"),
            n if n < 1_000_000_000_000.0 => (n / 1_000_000_000.0, "Ghash/s"),
            n if n < 1_000_000_000_000_000.0 => (n / 1_000_000_000_000.0, "Thash/s"),
            _ => (n, "hash/s"),
        }
    }
}

#[cfg(all(test, feature = "bench"))]
mod benches {
    extern crate test;

    use self::test::{black_box, Bencher};
    use crate::pow::State;
    use crate::proto::{RpcBlock, RpcBlockHeader};
    use rand::{thread_rng, RngCore};

    #[bench]
    pub fn bench_mining(bh: &mut Bencher) {
        let mut state = State::new(
            0,
            RpcBlock {
                header: Some(RpcBlockHeader {
                    version: 1,
                    parents: vec![],
                    hash_merkle_root: "23618af45051560529440541e7dc56be27676d278b1e00324b048d410a19d764".to_string(),
                    accepted_id_merkle_root: "947d1a10378d6478b6957a0ed71866812dee33684968031b1cace4908c149d94"
                        .to_string(),
                    utxo_commitment: "ec5e8fc0bc0c637004cee262cef12e7cf6d9cd7772513dbd466176a07ab7c4f4".to_string(),
                    timestamp: 654654353,
                    bits: 0x1e7fffff,
                    nonce: 0,
                    daa_score: 654456,
                    blue_work: "d8e28a03234786".to_string(),
                    pruning_point: "be4c415d378f9113fabd3c09fcc84ddb6a00f900c87cb6a1186993ddc3014e2d".to_string(),
                    blue_score: 1164419,
                }),
                transactions: vec![],
                verbose_data: None,
            },
        )
        .unwrap();
        nonce = thread_rng().next_u64();
        bh.iter(|| {
            for _ in 0..100 {
                black_box(state.check_pow(nonce));
                nonce += 1;
            }
        });
    }
}
