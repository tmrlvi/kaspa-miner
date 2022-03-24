use std::io::Write;
use std::time::Duration;
use crate::Error;
use kaspa_miner::Worker;
use log::error;

pub struct UartWorker {
    port: Box<dyn serialport::SerialPort>,
    workload: usize
}

impl Worker for UartWorker{
    fn id(&self) -> String {
        match self.port.name() {
            Some(name) => name.clone(),
            None => "Unknown Virtual Device".to_string()
        }
    }

    fn load_block_constants(&mut self, hash_header: &[u8; 72], _matrix: &[[u16; 64]; 64], _target: &[u64; 4]) {
        if let Err(err) = self.port.write_all(hash_header.as_slice()) {
            panic!("Could not write: {:?}", err);
        };
        if let Err(err) = self.port.flush() {
            panic!("Could not flush: {:?}", err);
        };
        /*match self.port.write_request_to_send(true) {
            Ok(()) => {
                if let Err(err) = self.port.write_all(&hash_header[..40]) {
                    panic!("Could not write: {:?}", err);
                };
            },
            Err(err) => {panic!("Could not request writing: {:?}", err);}
        };*/
    }

    fn calculate_hash(&mut self, _nonces: Option<&Vec<u64>>, _nonce_mask: u64, _nonce_fixed: u64) {
    }

    fn sync(&self) -> Result<(), kaspa_miner::Error> {
        Ok(())
    }

    fn get_workload(&self) -> usize {
        self.workload
    }

    fn copy_output_to(&mut self, nonces: &mut Vec<u64>) -> Result<(), kaspa_miner::Error> {
        let mut buff : Vec<u8> = vec![0u8; 8*self.workload];
        self.port.read_exact(buff.as_mut_slice())?;
        nonces.copy_from_slice(unsafe { buff.align_to::<u64>().1 });
        Ok(())
    }

    fn requires_filter(&self) -> bool {
        true
    }
}


impl UartWorker {
    pub fn new(path: String, baud_rate: u32, workload: usize) -> Result<Self, Error> {
        let mut port = serialport::new(path, baud_rate).open()?;
        port.set_timeout(Duration::from_secs(100000000))?;
        Ok(Self{
            port,
            workload
        })
    }
}
