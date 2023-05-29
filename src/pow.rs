use log::info;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};
use time::{macros::format_description, OffsetDateTime};

pub use crate::pow::hasher::HeaderHasher;
use crate::{
    pow::{
        hasher::{Hasher, PowHasher},
        heavy_hash::Matrix,
    },
    proto::{RpcBlock, RpcBlockHeader},
    target::{self, Uint256},
    Error, Hash,
};
use kaspa_miner::Worker;

mod hasher;
mod heavy_hash;
mod keccak;
mod xoshiro;

#[derive(Clone, Debug)]
pub enum BlockSeed {
    FullBlock(Box<RpcBlock>),
    PartialBlock {
        id: String,
        header_hash: [u64; 4],
        timestamp: u64,
        nonce: u64,
        target: Uint256,
        nonce_mask: u64,
        nonce_fixed: u64,
        hash: Option<String>,
    },
}

impl BlockSeed {
    pub fn report_block(&self) {
        match self {
            BlockSeed::FullBlock(block) => {
                let block_hash =
                    block.block_hash().expect("We just got it from the state, we should be able to hash it");
                let format = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
                let block_time = OffsetDateTime::from(
                    UNIX_EPOCH + Duration::from_millis(block.header.as_ref().unwrap().timestamp as u64),
                );
                info!(
                    "Found a block: {:x} (Timestamp: {})",
                    block_hash,
                    block_time.format(format).unwrap_or_else(|_| "unknown".to_string())
                );
            }
            BlockSeed::PartialBlock { .. } => info!("Found a share!"),
        }
    }
}

#[derive(Clone)]
pub struct State {
    pub id: usize,
    matrix: Arc<Matrix>,
    pub target: Uint256,
    pub pow_hash_header: [u8; 72],
    block: Arc<BlockSeed>,
    // PRE_POW_HASH || TIME || 32 zero byte padding; without NONCE
    hasher: PowHasher,

    pub nonce_mask: u64,
    pub nonce_fixed: u64,
}

impl State {
    #[inline]
    pub fn new(id: usize, block_seed: BlockSeed) -> Result<Self, Error> {
        let pre_pow_hash;
        let header_timestamp: u64;
        let header_target;
        let nonce_mask: u64;
        let nonce_fixed: u64;
        match block_seed {
            BlockSeed::FullBlock(ref block) => {
                let header = &block.header.as_ref().ok_or("Header is missing")?;

                header_target = target::u256_from_compact_target(header.bits);
                let mut hasher = HeaderHasher::new();
                serialize_header(&mut hasher, header, true);
                pre_pow_hash = hasher.finalize();
                header_timestamp = header.timestamp as u64;
                nonce_mask = 0xffffffffffffffffu64;
                nonce_fixed = 0;
            }
            BlockSeed::PartialBlock {
                ref header_hash,
                ref timestamp,
                ref target,
                nonce_fixed: fixed,
                nonce_mask: mask,
                ..
            } => {
                pre_pow_hash = Hash::new(*header_hash);
                header_timestamp = *timestamp;
                header_target = *target;
                nonce_mask = mask;
                nonce_fixed = fixed
            }
        }

        // PRE_POW_HASH || TIME || 32 zero byte padding || NONCE
        let hasher = PowHasher::new(pre_pow_hash, header_timestamp);
        let matrix = Arc::new(Matrix::generate(pre_pow_hash));
        let mut pow_hash_header = [0u8; 72];

        pow_hash_header.copy_from_slice(
            [pre_pow_hash.to_le_bytes().as_slice(), header_timestamp.to_le_bytes().as_slice(), [0u8; 32].as_slice()]
                .concat()
                .as_slice(),
        );
        Ok(Self {
            id,
            matrix,
            target: header_target,
            pow_hash_header,
            block: Arc::new(block_seed),
            hasher,
            nonce_mask,
            nonce_fixed,
        })
    }

    #[inline(always)]
    // PRE_POW_HASH || TIME || 32 zero byte padding || NONCE
    pub fn calculate_pow(&self, nonce: u64) -> Uint256 {
        // Hasher already contains PRE_POW_HASH || TIME || 32 zero byte padding; so only the NONCE is missing
        let hash = self.hasher.finalize_with_nonce(nonce);
        self.matrix.heavy_hash(hash)
    }

    #[inline(always)]
    pub fn check_pow(&self, nonce: u64) -> bool {
        let pow = self.calculate_pow(nonce);
        if pow <= self.target {
            info!("Found a block with pow: {:x}", pow);
            info!("Target was: {:x}", self.target);
        }
        // The pow hash must be less or equal than the claimed target.
        pow <= self.target
    }

    #[inline(always)]
    pub fn generate_block_if_pow(&self, nonce: u64) -> Option<BlockSeed> {
        self.check_pow(nonce).then(|| {
            let mut block_seed = (*self.block).clone();
            match block_seed {
                BlockSeed::FullBlock(ref mut block) => {
                    let header = &mut block.header.as_mut().expect("We checked that a header exists on creation");
                    header.nonce = nonce;
                }
                BlockSeed::PartialBlock { nonce: ref mut header_nonce, ref mut hash, .. } => {
                    *header_nonce = nonce;
                    *hash = Some(format!("{:x}", self.calculate_pow(nonce)))
                }
            }
            block_seed
        })
    }

    pub fn load_to_gpu(&self, gpu_work: &mut dyn Worker) {
        gpu_work.load_block_constants(&self.pow_hash_header, &self.matrix.0, &self.target.0);
    }

    #[inline(always)]
    pub fn pow_gpu(&self, gpu_work: &mut dyn Worker) {
        gpu_work.calculate_hash(None, self.nonce_mask, self.nonce_fixed);
    }
}

#[cfg(not(any(target_pointer_width = "64", target_pointer_width = "32")))]
compile_error!("Supporting only 32/64 bits");

#[inline(always)]
pub fn serialize_header<H: Hasher>(hasher: &mut H, header: &RpcBlockHeader, for_pre_pow: bool) {
    let (nonce, timestamp) = if for_pre_pow { (0, 0) } else { (header.nonce, header.timestamp) };
    let num_parents = header.parents.len();
    let version: u16 = header.version.try_into().unwrap();
    hasher.update(version.to_le_bytes()).update((num_parents as u64).to_le_bytes());

    let mut hash = [0u8; 32];
    for parent in &header.parents {
        hasher.update((parent.parent_hashes.len() as u64).to_le_bytes());
        for hash_string in &parent.parent_hashes {
            decode_to_slice(hash_string, &mut hash).unwrap();
            hasher.update(hash);
        }
    }
    decode_to_slice(&header.hash_merkle_root, &mut hash).unwrap();
    hasher.update(hash);

    decode_to_slice(&header.accepted_id_merkle_root, &mut hash).unwrap();
    hasher.update(hash);
    decode_to_slice(&header.utxo_commitment, &mut hash).unwrap();
    hasher.update(hash);

    hasher
        .update(timestamp.to_le_bytes())
        .update(header.bits.to_le_bytes())
        .update(nonce.to_le_bytes())
        .update(header.daa_score.to_le_bytes())
        .update(header.blue_score.to_le_bytes());

    // I'm assuming here BlueWork will never pass 256 bits.
    let blue_work_len = (header.blue_work.len() + 1) / 2;
    if header.blue_work.len() % 2 == 0 {
        decode_to_slice(&header.blue_work, &mut hash[..blue_work_len]).unwrap();
    } else {
        let mut blue_work = String::with_capacity(header.blue_work.len() + 1);
        blue_work.push('0');
        blue_work.push_str(&header.blue_work);
        decode_to_slice(&blue_work, &mut hash[..blue_work_len]).unwrap();
    }

    hasher.update((blue_work_len as u64).to_le_bytes()).update(&hash[..blue_work_len]);

    decode_to_slice(&header.pruning_point, &mut hash).unwrap();
    hasher.update(hash);
}

#[allow(dead_code)] // False Positive: https://github.com/rust-lang/rust/issues/88900
#[derive(Debug)]
enum FromHexError {
    OddLength,
    InvalidStringLength,
    InvalidHexCharacter { c: char, index: usize },
}

#[inline(always)]
fn decode_to_slice<T: AsRef<[u8]>>(data: T, out: &mut [u8]) -> Result<(), FromHexError> {
    let data = data.as_ref();
    if data.len() % 2 != 0 {
        return Err(FromHexError::OddLength);
    }
    if data.len() / 2 != out.len() {
        return Err(FromHexError::InvalidStringLength);
    }

    for (i, byte) in out.iter_mut().enumerate() {
        *byte = val(data[2 * i], 2 * i)? << 4 | val(data[2 * i + 1], 2 * i + 1)?;
    }

    #[inline(always)]
    fn val(c: u8, idx: usize) -> Result<u8, FromHexError> {
        match c {
            b'A'..=b'F' => Ok(c - b'A' + 10),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            b'0'..=b'9' => Ok(c - b'0'),
            _ => Err(FromHexError::InvalidHexCharacter { c: c as char, index: idx }),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::pow::hasher::{Hasher, HeaderHasher};
    use crate::pow::serialize_header;
    use crate::proto::{RpcBlockHeader, RpcBlockLevelParents};
    use crate::Hash;

    struct Buf(Vec<u8>);
    impl Hasher for Buf {
        fn update<A: AsRef<[u8]>>(&mut self, data: A) -> &mut Self {
            self.0.extend(data.as_ref());
            self
        }
    }

    #[test]
    fn test_serialize_header() {
        let header = RpcBlockHeader {
            version: 24565,
            parents: vec![
                RpcBlockLevelParents {
                    parent_hashes: vec![
                        "62a5eee82abdf44a2d0b75fb180daf48a79ee0b10d394651850fd4a178892ee2".into(),
                        "85ece1511455780875d64ee2d3d0d0de6bf8f9b44ce85ff044c6b1f83b8e883b".into(),
                        "bf857aab99c5b252c7429c32f3a8aeb79ef856f659c18f0dcecc77c75e7a81bf".into(),
                        "de275f67cfe242cf3cc354f3ede2d6becc4ea3ae5e88526a9f4a578bcb9ef2d4".into(),
                        "a65314768d6d299761ea9e4f5aa6aec3fc78c6aae081ac8120c720efcd6cea84".into(),
                        "b6925e607be063716f96ddcdd01d75045c3f000f8a796bce6c512c3801aacaee".into(),
                    ],
                },
                RpcBlockLevelParents {
                    parent_hashes: vec![
                        "dfad5b50ece0b8b7c1965d9181251b7c9c9ca5205afc16a236a2efcdd2d12d2a".into(),
                        "79d074a8280ae9439eb0d6aeca0823ae02d67d866ac2c4fe4a725053da119b9d".into(),
                        "4f515140a2d7239c40b45ac3950d941fc4fe1c0cb96ad322d62282295fbfe11e".into(),
                        "26a433076db5c1444c3a34d32a5c4a7ffbe8d181f7ed3b8cfe904f93f8f06d29".into(),
                        "bcd9ed847b182e046410f44bc4b0f3f03a0d06820a30f257f8114130678ac045".into(),
                        "86c1e3c9342c8b8055c466d886441d259906d69acd894b968ae9f0eb9d965ce6".into(),
                        "a4693c4ebe881501b7d9846b66eb02b57e5cda7b6cba6891d616bd686c37b834".into(),
                    ],
                },
                RpcBlockLevelParents {
                    parent_hashes: vec![
                        "613ac8ba52734ae4e3f1217acd5f83270814301867b5d06711b238001c7957b2".into(),
                        "7719ce3f3188dfe57deebf6f82595a10f7bb562ca04d5c3d27942958c6db3262".into(),
                        "670649f3bc97d9a2316735ede682a5dfe6f1a011fbc98ad0fbe790003c01e8e9".into(),
                        "967703af665e9f72407f4b03d4fdb474aafe8a0d3e0515dd4650cf51172b8124".into(),
                        "8bcb7f969e400b6c5b127768b1c412fae98cf57631cf37033b4b4aba7d7ed319".into(),
                        "ba147249c908ac70d1c406dade0e828eb6ba0dcaa88285543e10213c643fc860".into(),
                        "3b5860236670babcad0bd7f4c4190e323623a868d1eae1769f40a26631431b3b".into(),
                        "d5215605d2086fead499ac63a4653d12283d56019c3795a98a126d09cfcbe36c".into(),
                        "dcc93788a5409f8b6e42c2dd83aa46611852ad0b5028775c771690b6854e05b3".into(),
                    ],
                },
                RpcBlockLevelParents {
                    parent_hashes: vec![
                        "77241e302c6da8665c42341dda4adaea595ab1895f9652489dd2ceb49c247430".into(),
                        "3cbb44c2b94303db662c9c66b8782905190f1e1635b63e34878d3f246fadfce3".into(),
                        "44e74ef813090f8030bcd525ac10653ff182e00120f7e1f796fa0fc16ba7bb90".into(),
                        "be2a33e87c3d60ab628471a420834383661801bb0bfd8e6c140071db1eb2f7a1".into(),
                        "8194f1a045a94c078835c75dff2f3e836180baad9e955da840dc74c4dc2498f8".into(),
                        "c201aec254a0e36476b2eeb124fdc6afc1b7d809c5e08b5e0e845aaf9b6c3957".into(),
                        "e95ab4aa8e107cdb873f2dac527f16c4d5ac8760768a715e4669cb840c25317f".into(),
                        "9a368774e506341afb46503e28e92e51bd7f7d4b53b9023d56f9b9ec991ac2a9".into(),
                        "d9bc45ff64bb2bf14d4051a7604b28bad44d98bfe30e54ebc07fa45f62aabe39".into(),
                    ],
                },
                RpcBlockLevelParents {
                    parent_hashes: vec![
                        "5cc98b2e3f6deb2990187058e4bfd2d1640653fc38a30b0f83231a965b413b0f".into(),
                        "26927e0d032e830b732bdeb3094cb1a5fa6dec9f06375ea25fe57c2853ea0932".into(),
                        "0ac8803976eacaa095c02f869fd7dc31072475940c3751d56283c49e2fefd41d".into(),
                        "f676bdcb5855a0470efd2dab7a72cc5e5f39ff7eea0f433a9fe7b6a675bc2ac5".into(),
                        "0cd218c009e21f910f9ddb09a0d059c4cd7d2ca65a2349df7a867dbedd81e9d4".into(),
                        "891619c83c42895ce1b671cb7a4bcaed9130ab1dd4cc2d8147a1595056b55f92".into(),
                        "a355db765adc8d3df88eb93d527f7f7ec869a75703ba86d4b36110e9a044593c".into(),
                        "966815d153665387dc38e507e7458df3e6b0f04035ef9419883e03c08e2d753b".into(),
                        "08c9090aabf175fdb63e8cf9a5f0783704c741c195157626401d949eaa6dbd04".into(),
                    ],
                },
                RpcBlockLevelParents {
                    parent_hashes: vec![
                        "d7bf5e9c18cc79dda4e12efe564ecb8a4019e1c41f2d8217c0c3a43712ae226f".into(),
                        "ce776631ae19b326a411a284741be01fb4f3aefc5def968eb6cceb8604864b4b".into(),
                        "9ad373cbac10ea7e665b294a8a790691aa5246e6ff8fd0b7fb9b9a6a958ebf28".into(),
                    ],
                },
                RpcBlockLevelParents {
                    parent_hashes: vec![
                        "ec5e8fc0bc0c637004cee262cef12e7cf6d9cd7772513dbd466176a07ab7c4f4".into(),
                        "65fe09747779c31495e689b65f557b0a4af6535880b82553d126ff7213542905".into(),
                        "5a64749599333e9655b43aa36728bb63bd286427441baa9f305d5c25e05229bb".into(),
                        "332f7e8375b7c45e1ea0461d333c3c725f7467b441b7d0f5e80242b7a4a18eda".into(),
                    ],
                },
                RpcBlockLevelParents {
                    parent_hashes: vec!["e80d7d9a0a4634f07bea5c5a00212fbc591bddfebb94334f4a2d928673d262ad".into()],
                },
                RpcBlockLevelParents {
                    parent_hashes: vec![
                        "abaa82988c683f4742c12099b732bd03639c1979752d837518243b74d6730124".into(),
                        "5efe5661eaa0428917f55a58cc33db284d1f2caa05f1fd7b6602980f06d10723".into(),
                        "0bf310b48cf62942017dd6680eb3ab13310eca1581afb3c5b619e5ce0682d0df".into(),
                        "c1fade3928179a9dc28cd170b5b5544e7f9b63b83da374afa28e1478dc5c2997".into(),
                    ],
                },
            ],
            hash_merkle_root: "a98347ec1e71514eb26822162dc7c3992fd41f0b2ccc26e55e7bd8f3fa37215f".into(),
            accepted_id_merkle_root: "774b5216b5b872b6c2388dd950160e3ffa3bf0623c438655bb5c8c768ab33ae2".into(),
            utxo_commitment: "ee39218674008665e20a3acdf84abef35cabcc489158c0853fd5bfa954226139".into(),
            timestamp: -1426594953012613626,
            bits: 684408190,
            nonce: 8230160685758639177,
            daa_score: 15448880227546599629,
            blue_work: "ce5639b8ed46571e20eeaa7a62a078f8c55aef6edd6a35ed37a3d6cf98736abd".into(),
            pruning_point: "fc44c4f57cf8f7a2ba410a70d0ad49060355b9deb97012345603d9d0d1dcb0de".into(),
            blue_score: 29372123613087746,
        };
        let expected_res = [
            245, 95, 9, 0, 0, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0, 98, 165, 238, 232, 42, 189, 244, 74, 45, 11, 117,
            251, 24, 13, 175, 72, 167, 158, 224, 177, 13, 57, 70, 81, 133, 15, 212, 161, 120, 137, 46, 226, 133, 236,
            225, 81, 20, 85, 120, 8, 117, 214, 78, 226, 211, 208, 208, 222, 107, 248, 249, 180, 76, 232, 95, 240, 68,
            198, 177, 248, 59, 142, 136, 59, 191, 133, 122, 171, 153, 197, 178, 82, 199, 66, 156, 50, 243, 168, 174,
            183, 158, 248, 86, 246, 89, 193, 143, 13, 206, 204, 119, 199, 94, 122, 129, 191, 222, 39, 95, 103, 207,
            226, 66, 207, 60, 195, 84, 243, 237, 226, 214, 190, 204, 78, 163, 174, 94, 136, 82, 106, 159, 74, 87, 139,
            203, 158, 242, 212, 166, 83, 20, 118, 141, 109, 41, 151, 97, 234, 158, 79, 90, 166, 174, 195, 252, 120,
            198, 170, 224, 129, 172, 129, 32, 199, 32, 239, 205, 108, 234, 132, 182, 146, 94, 96, 123, 224, 99, 113,
            111, 150, 221, 205, 208, 29, 117, 4, 92, 63, 0, 15, 138, 121, 107, 206, 108, 81, 44, 56, 1, 170, 202, 238,
            7, 0, 0, 0, 0, 0, 0, 0, 223, 173, 91, 80, 236, 224, 184, 183, 193, 150, 93, 145, 129, 37, 27, 124, 156,
            156, 165, 32, 90, 252, 22, 162, 54, 162, 239, 205, 210, 209, 45, 42, 121, 208, 116, 168, 40, 10, 233, 67,
            158, 176, 214, 174, 202, 8, 35, 174, 2, 214, 125, 134, 106, 194, 196, 254, 74, 114, 80, 83, 218, 17, 155,
            157, 79, 81, 81, 64, 162, 215, 35, 156, 64, 180, 90, 195, 149, 13, 148, 31, 196, 254, 28, 12, 185, 106,
            211, 34, 214, 34, 130, 41, 95, 191, 225, 30, 38, 164, 51, 7, 109, 181, 193, 68, 76, 58, 52, 211, 42, 92,
            74, 127, 251, 232, 209, 129, 247, 237, 59, 140, 254, 144, 79, 147, 248, 240, 109, 41, 188, 217, 237, 132,
            123, 24, 46, 4, 100, 16, 244, 75, 196, 176, 243, 240, 58, 13, 6, 130, 10, 48, 242, 87, 248, 17, 65, 48,
            103, 138, 192, 69, 134, 193, 227, 201, 52, 44, 139, 128, 85, 196, 102, 216, 134, 68, 29, 37, 153, 6, 214,
            154, 205, 137, 75, 150, 138, 233, 240, 235, 157, 150, 92, 230, 164, 105, 60, 78, 190, 136, 21, 1, 183, 217,
            132, 107, 102, 235, 2, 181, 126, 92, 218, 123, 108, 186, 104, 145, 214, 22, 189, 104, 108, 55, 184, 52, 9,
            0, 0, 0, 0, 0, 0, 0, 97, 58, 200, 186, 82, 115, 74, 228, 227, 241, 33, 122, 205, 95, 131, 39, 8, 20, 48,
            24, 103, 181, 208, 103, 17, 178, 56, 0, 28, 121, 87, 178, 119, 25, 206, 63, 49, 136, 223, 229, 125, 238,
            191, 111, 130, 89, 90, 16, 247, 187, 86, 44, 160, 77, 92, 61, 39, 148, 41, 88, 198, 219, 50, 98, 103, 6,
            73, 243, 188, 151, 217, 162, 49, 103, 53, 237, 230, 130, 165, 223, 230, 241, 160, 17, 251, 201, 138, 208,
            251, 231, 144, 0, 60, 1, 232, 233, 150, 119, 3, 175, 102, 94, 159, 114, 64, 127, 75, 3, 212, 253, 180, 116,
            170, 254, 138, 13, 62, 5, 21, 221, 70, 80, 207, 81, 23, 43, 129, 36, 139, 203, 127, 150, 158, 64, 11, 108,
            91, 18, 119, 104, 177, 196, 18, 250, 233, 140, 245, 118, 49, 207, 55, 3, 59, 75, 74, 186, 125, 126, 211,
            25, 186, 20, 114, 73, 201, 8, 172, 112, 209, 196, 6, 218, 222, 14, 130, 142, 182, 186, 13, 202, 168, 130,
            133, 84, 62, 16, 33, 60, 100, 63, 200, 96, 59, 88, 96, 35, 102, 112, 186, 188, 173, 11, 215, 244, 196, 25,
            14, 50, 54, 35, 168, 104, 209, 234, 225, 118, 159, 64, 162, 102, 49, 67, 27, 59, 213, 33, 86, 5, 210, 8,
            111, 234, 212, 153, 172, 99, 164, 101, 61, 18, 40, 61, 86, 1, 156, 55, 149, 169, 138, 18, 109, 9, 207, 203,
            227, 108, 220, 201, 55, 136, 165, 64, 159, 139, 110, 66, 194, 221, 131, 170, 70, 97, 24, 82, 173, 11, 80,
            40, 119, 92, 119, 22, 144, 182, 133, 78, 5, 179, 9, 0, 0, 0, 0, 0, 0, 0, 119, 36, 30, 48, 44, 109, 168,
            102, 92, 66, 52, 29, 218, 74, 218, 234, 89, 90, 177, 137, 95, 150, 82, 72, 157, 210, 206, 180, 156, 36,
            116, 48, 60, 187, 68, 194, 185, 67, 3, 219, 102, 44, 156, 102, 184, 120, 41, 5, 25, 15, 30, 22, 53, 182,
            62, 52, 135, 141, 63, 36, 111, 173, 252, 227, 68, 231, 78, 248, 19, 9, 15, 128, 48, 188, 213, 37, 172, 16,
            101, 63, 241, 130, 224, 1, 32, 247, 225, 247, 150, 250, 15, 193, 107, 167, 187, 144, 190, 42, 51, 232, 124,
            61, 96, 171, 98, 132, 113, 164, 32, 131, 67, 131, 102, 24, 1, 187, 11, 253, 142, 108, 20, 0, 113, 219, 30,
            178, 247, 161, 129, 148, 241, 160, 69, 169, 76, 7, 136, 53, 199, 93, 255, 47, 62, 131, 97, 128, 186, 173,
            158, 149, 93, 168, 64, 220, 116, 196, 220, 36, 152, 248, 194, 1, 174, 194, 84, 160, 227, 100, 118, 178,
            238, 177, 36, 253, 198, 175, 193, 183, 216, 9, 197, 224, 139, 94, 14, 132, 90, 175, 155, 108, 57, 87, 233,
            90, 180, 170, 142, 16, 124, 219, 135, 63, 45, 172, 82, 127, 22, 196, 213, 172, 135, 96, 118, 138, 113, 94,
            70, 105, 203, 132, 12, 37, 49, 127, 154, 54, 135, 116, 229, 6, 52, 26, 251, 70, 80, 62, 40, 233, 46, 81,
            189, 127, 125, 75, 83, 185, 2, 61, 86, 249, 185, 236, 153, 26, 194, 169, 217, 188, 69, 255, 100, 187, 43,
            241, 77, 64, 81, 167, 96, 75, 40, 186, 212, 77, 152, 191, 227, 14, 84, 235, 192, 127, 164, 95, 98, 170,
            190, 57, 9, 0, 0, 0, 0, 0, 0, 0, 92, 201, 139, 46, 63, 109, 235, 41, 144, 24, 112, 88, 228, 191, 210, 209,
            100, 6, 83, 252, 56, 163, 11, 15, 131, 35, 26, 150, 91, 65, 59, 15, 38, 146, 126, 13, 3, 46, 131, 11, 115,
            43, 222, 179, 9, 76, 177, 165, 250, 109, 236, 159, 6, 55, 94, 162, 95, 229, 124, 40, 83, 234, 9, 50, 10,
            200, 128, 57, 118, 234, 202, 160, 149, 192, 47, 134, 159, 215, 220, 49, 7, 36, 117, 148, 12, 55, 81, 213,
            98, 131, 196, 158, 47, 239, 212, 29, 246, 118, 189, 203, 88, 85, 160, 71, 14, 253, 45, 171, 122, 114, 204,
            94, 95, 57, 255, 126, 234, 15, 67, 58, 159, 231, 182, 166, 117, 188, 42, 197, 12, 210, 24, 192, 9, 226, 31,
            145, 15, 157, 219, 9, 160, 208, 89, 196, 205, 125, 44, 166, 90, 35, 73, 223, 122, 134, 125, 190, 221, 129,
            233, 212, 137, 22, 25, 200, 60, 66, 137, 92, 225, 182, 113, 203, 122, 75, 202, 237, 145, 48, 171, 29, 212,
            204, 45, 129, 71, 161, 89, 80, 86, 181, 95, 146, 163, 85, 219, 118, 90, 220, 141, 61, 248, 142, 185, 61,
            82, 127, 127, 126, 200, 105, 167, 87, 3, 186, 134, 212, 179, 97, 16, 233, 160, 68, 89, 60, 150, 104, 21,
            209, 83, 102, 83, 135, 220, 56, 229, 7, 231, 69, 141, 243, 230, 176, 240, 64, 53, 239, 148, 25, 136, 62, 3,
            192, 142, 45, 117, 59, 8, 201, 9, 10, 171, 241, 117, 253, 182, 62, 140, 249, 165, 240, 120, 55, 4, 199, 65,
            193, 149, 21, 118, 38, 64, 29, 148, 158, 170, 109, 189, 4, 3, 0, 0, 0, 0, 0, 0, 0, 215, 191, 94, 156, 24,
            204, 121, 221, 164, 225, 46, 254, 86, 78, 203, 138, 64, 25, 225, 196, 31, 45, 130, 23, 192, 195, 164, 55,
            18, 174, 34, 111, 206, 119, 102, 49, 174, 25, 179, 38, 164, 17, 162, 132, 116, 27, 224, 31, 180, 243, 174,
            252, 93, 239, 150, 142, 182, 204, 235, 134, 4, 134, 75, 75, 154, 211, 115, 203, 172, 16, 234, 126, 102, 91,
            41, 74, 138, 121, 6, 145, 170, 82, 70, 230, 255, 143, 208, 183, 251, 155, 154, 106, 149, 142, 191, 40, 4,
            0, 0, 0, 0, 0, 0, 0, 236, 94, 143, 192, 188, 12, 99, 112, 4, 206, 226, 98, 206, 241, 46, 124, 246, 217,
            205, 119, 114, 81, 61, 189, 70, 97, 118, 160, 122, 183, 196, 244, 101, 254, 9, 116, 119, 121, 195, 20, 149,
            230, 137, 182, 95, 85, 123, 10, 74, 246, 83, 88, 128, 184, 37, 83, 209, 38, 255, 114, 19, 84, 41, 5, 90,
            100, 116, 149, 153, 51, 62, 150, 85, 180, 58, 163, 103, 40, 187, 99, 189, 40, 100, 39, 68, 27, 170, 159,
            48, 93, 92, 37, 224, 82, 41, 187, 51, 47, 126, 131, 117, 183, 196, 94, 30, 160, 70, 29, 51, 60, 60, 114,
            95, 116, 103, 180, 65, 183, 208, 245, 232, 2, 66, 183, 164, 161, 142, 218, 1, 0, 0, 0, 0, 0, 0, 0, 232, 13,
            125, 154, 10, 70, 52, 240, 123, 234, 92, 90, 0, 33, 47, 188, 89, 27, 221, 254, 187, 148, 51, 79, 74, 45,
            146, 134, 115, 210, 98, 173, 4, 0, 0, 0, 0, 0, 0, 0, 171, 170, 130, 152, 140, 104, 63, 71, 66, 193, 32,
            153, 183, 50, 189, 3, 99, 156, 25, 121, 117, 45, 131, 117, 24, 36, 59, 116, 214, 115, 1, 36, 94, 254, 86,
            97, 234, 160, 66, 137, 23, 245, 90, 88, 204, 51, 219, 40, 77, 31, 44, 170, 5, 241, 253, 123, 102, 2, 152,
            15, 6, 209, 7, 35, 11, 243, 16, 180, 140, 246, 41, 66, 1, 125, 214, 104, 14, 179, 171, 19, 49, 14, 202, 21,
            129, 175, 179, 197, 182, 25, 229, 206, 6, 130, 208, 223, 193, 250, 222, 57, 40, 23, 154, 157, 194, 140,
            209, 112, 181, 181, 84, 78, 127, 155, 99, 184, 61, 163, 116, 175, 162, 142, 20, 120, 220, 92, 41, 151, 169,
            131, 71, 236, 30, 113, 81, 78, 178, 104, 34, 22, 45, 199, 195, 153, 47, 212, 31, 11, 44, 204, 38, 229, 94,
            123, 216, 243, 250, 55, 33, 95, 119, 75, 82, 22, 181, 184, 114, 182, 194, 56, 141, 217, 80, 22, 14, 63,
            250, 59, 240, 98, 60, 67, 134, 85, 187, 92, 140, 118, 138, 179, 58, 226, 238, 57, 33, 134, 116, 0, 134,
            101, 226, 10, 58, 205, 248, 74, 190, 243, 92, 171, 204, 72, 145, 88, 192, 133, 63, 213, 191, 169, 84, 34,
            97, 57, 0, 0, 0, 0, 0, 0, 0, 0, 126, 61, 203, 40, 0, 0, 0, 0, 0, 0, 0, 0, 205, 144, 120, 28, 183, 114, 101,
            214, 2, 200, 65, 114, 202, 89, 104, 0, 32, 0, 0, 0, 0, 0, 0, 0, 206, 86, 57, 184, 237, 70, 87, 30, 32, 238,
            170, 122, 98, 160, 120, 248, 197, 90, 239, 110, 221, 106, 53, 237, 55, 163, 214, 207, 152, 115, 106, 189,
            252, 68, 196, 245, 124, 248, 247, 162, 186, 65, 10, 112, 208, 173, 73, 6, 3, 85, 185, 222, 185, 112, 18,
            52, 86, 3, 217, 208, 209, 220, 176, 222,
        ];
        let mut buf = Buf(Vec::with_capacity(1951));
        serialize_header(&mut buf, &header, true);
        assert_eq!(&expected_res[..], &buf.0);

        let expected_hash = Hash::from_le_bytes([
            85, 146, 211, 217, 138, 239, 47, 85, 152, 59, 58, 16, 4, 149, 129, 179, 172, 226, 174, 233, 160, 96, 202,
            54, 6, 225, 64, 142, 106, 0, 110, 137,
        ]);
        let mut hasher = HeaderHasher::new();
        hasher.write(buf.0);
        assert_eq!(hasher.finalize(), expected_hash);
    }
}
