# Kaspa-miner
[![Build status](https://github.com/tmrlvi/kaspa-miner/workflows/ci/badge.svg)](https://github.com/tmrlvi/kaspa-miner/actions)
[![Latest version](https://img.shields.io/crates/v/kaspa-miner.svg)](https://crates.io/crates/kaspa-miner)
![License](https://img.shields.io/crates/l/kaspa-miner.svg)
[![dependency status](https://deps.rs/repo/github/elichai/kaspa-miner/status.svg)](https://deps.rs/repo/github/elichai/kaspa-miner)

[![Discord](https://discordapp.com/api/guilds/599153230659846165/embed.png)](https://discord.gg/kS3SK5F36R)
[![Telegram](https://img.shields.io/badge/Telegram-2CA5E0?style=for-the-badge&logo=telegram&logoColor=white)](https://t.me/Kaspaenglish)


## Installation
### From Sources
Install via `cargo install` not supported for latest version.

The regular version is still available at
```sh
cargo install kaspa-miner
```

### From Git Sources

If you are looking to build from the repository (for debug / extension), note that the plugins are additional
packages in the workspace. To compile a specific package, run the following command or any subset of it

```sh
git clone git@github.com:tmrlvi/kaspa-miner.git
cd kaspa-miner
cargo build --release -p kaspa-miner -p kaspacuda -p kaspaopencl
```
And, the miner (and plugins) will be in `targets/release`. You can replace the last line with
```sh
cargo build --release --all
```

### From Binaries
The [release page](https://github.com/tmrlvi/kaspa-miner/releases) includes precompiled binaries for Linux, and Windows (for the GPU version).

### Removing Plugins
To remove a plugin simply remove the corresponding `dll`/`so` for the directory of the miner. 

* `libkaspacuda.so`, `libkaspacuda.dll`: Cuda support for Kaspa-Miner
* `libkaspaopencl.so`, `libkaspaopencl.dll`: OpenCL support for Kaspa-Miner

# Usage
To start mining you need to run [kaspad](https://github.com/kaspanet/kaspad) and have an address to send the rewards to.
There's a guide here on how to run a full node and how to generate addresses: https://github.com/kaspanet/docs/blob/main/Getting%20Started/Full%20Node%20Installation.md

Help:
```
A Kaspa high performance CPU miner

USAGE:
    kaspa-miner [OPTIONS] --mining-address <MINING_ADDRESS>

OPTIONS:
    -a, --mining-address <MINING_ADDRESS>      The Kaspa address for the miner reward
        --cuda-device <CUDA_DEVICE>            Which CUDA GPUs to use [default: all]
        --cuda-disable                         Disable cuda workers
        --cuda-lock-core-clocks <CUDA_LOCK_CORE_CLOCKS>
                                               Lock core clocks eg: ,1200, [default: 0]
        --cuda-lock-mem-clocks <CUDA_LOCK_MEM_CLOCKS>
                                               Lock mem clocks eg: ,810, [default: 0]
        --cuda-no-blocking-sync                Actively wait for GPU result. Increases CPU usage, but removes delays
                                               that might result in red blocks. Can have lower workload.
        --cuda-power-limits <CUDA_POWER_LIMITS>
                                               Lock power limits eg: ,150, [default: 0
        --cuda-workload <CUDA_WORKLOAD>        Ratio of nonces to GPU possible parrallel run [default: 64]
        --cuda-workload-absolute               The values given by workload are not ratio, but absolute number of nonces
                                               [default: false]
    -d, --debug                                Enable debug logging level
        --devfund-percent <DEVFUND_PERCENT>    The percentage of blocks to send to the devfund (minimum 2%) [default: 2]
        --experimental-amd                     Uses SMID instructions in AMD. Miner will crash if instruction is not
                                               supported
    -h, --help                                 Print help information
        --mine-when-not-synced                 Mine even when kaspad says it is not synced, only useful when passing
                                               `--allow-submit-block-when-not-synced` to kaspad  [default: false]
        --nonce-gen <NONCE_GEN>                The random method used to generate nonces. Options: (i) xoshiro - each
                                               thread in GPU will have its own random state, creating a
                                               (pseudo-)independent xoshiro sequence (ii) lean - each GPU will have a
                                               single random nonce, and each GPU thread will work on nonce + thread id.
                                               [default: lean]
        --opencl-amd-binary                    Disable fetching of precompiled AMD kernel (if exists)
        --opencl-device <OPENCL_DEVICE>        Which OpenCL GPUs to use on a specific platform
        --opencl-enable                        Enable opencl, and take all devices of the chosen platform
        --opencl-platform <OPENCL_PLATFORM>    Which OpenCL platform to use (limited to one per executable)
        --opencl-workload <OPENCL_WORKLOAD>    Ratio of nonces to GPU possible parrallel run in OpenCL [default: 512]
        --opencl-workload-absolute             The values given by workload are not ratio, but absolute number of nonces
                                               in OpenCL [default: false]
    -p, --port <PORT>                          Kaspad port [default: Mainnet = 16111, Testnet = 16211]
    -s, --kaspad-address <KASPAD_ADDRESS>      The IP of the kaspad instance [default: 127.0.0.1]
    -t, --threads <NUM_THREADS>                Amount of CPU miner threads to launch [default: 0]
        --testnet                              Use testnet instead of mainnet [default: false]
```

To start mining you just need to run the following:

`./kaspa-miner --mining-address kaspa:XXXXX`

This will run the miner on all the available CPU cores.

# Devfund

The devfund is a fund managed by the Kaspa community in order to fund Kaspa development <br>
A miner that wants to mine higher percentage into the dev-fund can pass the following flags: <br>
`--devfund-precent=XX.YY` to mine only XX.YY% of the blocks into the devfund.

**This version automatically sets the devfund donation to the community designated address. 
Due to community decision, the minimum amount in the precompiled binaries is 2%**

# Donation Addresses

**Elichai**: `kaspa:qzvqtx5gkvl3tc54up6r8pk5mhuft9rtr0lvn624w9mtv4eqm9rvc9zfdmmpu`

**HauntedCook**: `kaspa:qz4jdyu04hv4hpyy00pl6trzw4gllnhnwy62xattejv2vaj5r0p5quvns058f`
