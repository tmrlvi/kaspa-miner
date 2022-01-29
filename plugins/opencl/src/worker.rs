use crate::cli::NonceGenEnum;
use crate::Error;
use kaspa_miner::xoshiro256starstar::Xoshiro256StarStar;
use kaspa_miner::Worker;
use log::info;
use opencl3::command_queue::{CommandQueue, CL_QUEUE_OUT_OF_ORDER_EXEC_MODE_ENABLE};
use opencl3::context::Context;
use opencl3::device::{CL_DEVICE_IL_VERSION, Device};
use opencl3::event::{release_event, retain_event, wait_for_events};
use opencl3::kernel::{ExecuteKernel, Kernel};
use opencl3::memory::{Buffer, ClMem, CL_MAP_WRITE, CL_MEM_READ_ONLY, CL_MEM_READ_WRITE, CL_MEM_WRITE_ONLY};
use opencl3::platform::Platform;
use opencl3::program::{Program, CL_FINITE_MATH_ONLY, CL_MAD_ENABLE, CL_STD_2_0};
use opencl3::types::{cl_event, cl_uchar, cl_ulong, CL_BLOCKING};
use rand::{thread_rng, Fill, RngCore};
use std::borrow::Borrow;
use std::ffi::c_void;
use std::ptr;
use std::sync::Arc;

static PROGRAM_SOURCE: &str = include_str!("../resources/kaspa-opencl.cl");

pub struct OpenCLGPUWorker {
    context: Arc<Context>,
    random: NonceGenEnum,
    workload: usize,

    heavy_hash: Kernel,

    queue: CommandQueue,

    random_state: Buffer<cl_ulong>,
    final_nonce: Buffer<cl_ulong>,
    final_hash: Buffer<[cl_ulong; 4]>,

    hash_header: Buffer<cl_uchar>,
    matrix: Buffer<cl_uchar>,
    target: Buffer<cl_ulong>,

    events: Vec<cl_event>,
    experimental_amd: bool,
}

impl Worker for OpenCLGPUWorker {
    fn id(&self) -> String {
        let device = Device::new(self.context.default_device());
        device.name().unwrap()
    }

    fn load_block_constants(&mut self, hash_header: &[u8; 72], matrix: &[[u16; 64]; 64], target: &[u64; 4]) {
        let cl_uchar_matrix = match self.experimental_amd {
            true => matrix.iter().flat_map(|row| row.chunks(2).map(|v| ((v[0] << 4) | v[1]) as cl_uchar)).collect::<Vec<cl_uchar>>(),
            false => matrix.iter().flat_map(|row| row.map(|v| v as cl_uchar)).collect::<Vec<cl_uchar>>(),
        };
        self.queue
            .enqueue_write_buffer(&mut self.final_nonce, CL_BLOCKING, 0, &[0], &[])
            .map_err(|e| e.to_string())
            .unwrap()
            .wait()
            .unwrap();
        self.queue
            .enqueue_write_buffer(&mut self.hash_header, CL_BLOCKING, 0, hash_header, &[])
            .map_err(|e| e.to_string())
            .unwrap()
            .wait()
            .unwrap();
        self.queue
            .enqueue_write_buffer(&mut self.matrix, CL_BLOCKING, 0, cl_uchar_matrix.as_slice(), &[])
            .map_err(|e| e.to_string())
            .unwrap()
            .wait()
            .unwrap();
        let copy_target = self
            .queue
            .enqueue_write_buffer(&mut self.target, CL_BLOCKING, 0, target, &[])
            .map_err(|e| e.to_string())
            .unwrap();

        self.events = vec![copy_target.get()];
        for event in &self.events {
            retain_event(*event).unwrap();
        }
    }

    fn calculate_hash(&mut self, _nonces: Option<&Vec<u64>>) {
        if self.random == NonceGenEnum::Lean {
            self.queue
                .enqueue_write_buffer(&mut self.random_state, CL_BLOCKING, 0, &[thread_rng().next_u64()], &[])
                .map_err(|e| e.to_string())
                .unwrap()
                .wait()
                .unwrap();
        }
        let random_type: cl_uchar = match self.random {
            NonceGenEnum::Lean => 0,
            NonceGenEnum::Xoshiro => 1
        };
        let kernel_event = ExecuteKernel::new(&self.heavy_hash)
            .set_arg(&self.hash_header)
            .set_arg(&self.matrix)
            .set_arg(&self.target)
            .set_arg(&random_type)
            .set_arg(&self.random_state)
            .set_arg(&self.final_nonce)
            .set_arg(&self.final_hash)
            .set_global_work_size(self.workload)
            .set_event_wait_list(self.events.borrow())
            .enqueue_nd_range(&self.queue)
            .map_err(|e| e.to_string())
            .unwrap();

        kernel_event.wait().unwrap();

        /*let mut nonces = [0u64; 1];
        let mut hash = [[0u64; 4]];
        self.queue.enqueue_read_buffer(&self.final_nonce, CL_BLOCKING, 0, &mut nonces, &[]).map_err(|e| e.to_string()).unwrap();
        self.queue.enqueue_read_buffer(&self.final_hash, CL_BLOCKING, 0, &mut hash, &[]).map_err(|e| e.to_string()).unwrap();
        log::info!("Hash from kernel: {:?}", hash);*/
        /*for event in &self.events{
            release_event(*event).unwrap();
        }
        let event = kernel_event.get();
        self.events = vec!(event);
        retain_event(event);*/
    }

    fn sync(&self) -> Result<(), Error> {
        wait_for_events(&self.events).map_err(|e| format!("waiting error code {}", e))?;
        for event in &self.events {
            release_event(*event).unwrap();
        }
        Ok(())
    }

    fn get_workload(&self) -> usize {
        self.workload as usize
    }

    fn copy_output_to(&mut self, nonces: &mut Vec<u64>) -> Result<(), Error> {
        self.queue
            .enqueue_read_buffer(&self.final_nonce, CL_BLOCKING, 0, nonces, &[])
            .map_err(|e| e.to_string())
            .unwrap();
        Ok(())
    }
}

impl OpenCLGPUWorker {
    pub fn new(
        device: Device,
        workload: f32,
        is_absolute: bool,
        experimental_amd: bool,
        use_binary: bool,
        random: &NonceGenEnum,
    ) -> Result<Self, Error> {
        let name =
            device.board_name_amd().unwrap_or_else(|_| device.name().unwrap_or_else(|_| "Unknown Device".into()));
        info!("{}: Using OpenCL", name);
        let version = device.version().unwrap_or_else(|_| "unkown version".into());
        info!(
            "{}: Device supports {} with extensions: {}",
            name,
            version,
            device.extensions().unwrap_or_else(|_| "NA".into())
        );

        let chosen_workload = match is_absolute {
            true => workload as usize,
            false => {
                let max_work_group_size = (device.max_work_group_size().map_err(|e| e.to_string())?
                    * (device.max_compute_units().map_err(|e| e.to_string())? as usize))
                    as f32;
                (workload * max_work_group_size) as usize
            }
        };
        info!("{}: Chosen workload is {}", name, chosen_workload);
        let context =
            Arc::new(Context::from_device(&device).unwrap_or_else(|_| panic!("{}::Context::from_device failed", name)));
        let context_ref = unsafe { Arc::as_ptr(&context).as_ref().unwrap() };

        let options = match experimental_amd {
            // true => "-D __FORCE_AMD_V_DOT4_U32_U8__=1 ",
            true => "-D __FORCE_AMD_V_DOT8_U32_U4__=1 ",
            false => "",
        };

        let experimental_amd_use = match device.name().unwrap_or_else(|_| "Unknown".into()).to_lowercase().as_str() {
            "tahiti" => false,
            "ellesmere" => false,
            "gfx1010" => false,
            _ => true,
        };

        let program = match use_binary {
            true => {
                let device_name = device.name().unwrap_or_else(|_| "Unknown".into()).to_lowercase();
                info!("{}: Looking for binary for {}", name, device_name);
                match device_name.as_str() {
                    "ellesmere" => Program::create_and_build_from_binary(
                        &context,
                        &[include_bytes!("../resources/bin/ellesmere_kaspa-opencl.bin")],
                        "",
                    )
                    .unwrap_or_else(|e| panic!("{}::Program::create_and_build_from_binary failed: {}", name, String::from(e))),
                    "gfx906" => Program::create_and_build_from_binary(
                        &context,
                        &[include_bytes!("../resources/bin/gfx906_kaspa-opencl.bin")],
                        "",
                    )
                    .unwrap_or_else(|_| panic!("{}::Program::create_and_build_from_binary failed", name)),
                    "gfx908" => Program::create_and_build_from_binary(
                        &context,
                        &[include_bytes!("../resources/bin/gfx908_kaspa-opencl.bin")],
                        "",
                    )
                    .unwrap_or_else(|_| panic!("{}::Program::create_and_build_from_binary failed", name)),
                    "gfx1010" => Program::create_and_build_from_binary(
                        &context,
                        &[include_bytes!("../resources/bin/gfx1010_kaspa-opencl.bin")],
                        "",
                    )
                    .unwrap_or_else(|_| panic!("{}::Program::create_and_build_from_binary failed", name)),
                    "gfx1011" => Program::create_and_build_from_binary(
                        &context,
                        &[include_bytes!("../resources/bin/gfx1011_kaspa-opencl.bin")],
                        "",
                    )
                    .unwrap_or_else(|_| panic!("{}::Program::create_and_build_from_binary failed", name)),
                    "gfx1012" => Program::create_and_build_from_binary(
                        &context,
                        &[include_bytes!("../resources/bin/gfx1012_kaspa-opencl.bin")],
                        "",
                    )
                    .unwrap_or_else(|_| panic!("{}::Program::create_and_build_from_binary failed", name)),
                    "gfx1030" => Program::create_and_build_from_binary(
                        &context,
                        &[include_bytes!("../resources/bin/gfx1030_kaspa-opencl.bin")],
                        "",
                    )
                    .unwrap_or_else(|_| panic!("{}::Program::create_and_build_from_binary failed", name)),
                    "gfx1031" => Program::create_and_build_from_binary(
                        &context,
                        &[include_bytes!("../resources/bin/gfx1031_kaspa-opencl.bin")],
                        "",
                    )
                    .unwrap_or_else(|_| panic!("{}::Program::create_and_build_from_binary failed", name)),
                    "gfx1032" => Program::create_and_build_from_binary(
                        &context,
                        &[include_bytes!("../resources/bin/gfx1032_kaspa-opencl.bin")],
                        "",
                    )
                    .unwrap_or_else(|e| panic!("{}::Program::create_and_build_from_binary failed: {}", name, e.to_string())),
                    other => {
                        panic!(
                            "{}: Found device {} without prebuilt binary. Trying to run without --opencl-amd-binary.",
                            name, other
                        );
                    }
                }
            }
            false => from_source(&context, &device, options)
                .unwrap_or_else(|e| panic!("{}::Program::create_and_build_from_binary failed: {}", name, e.to_string())),
        };
        info!("Kernels: {:?}", program.kernel_names());
        let heavy_hash = Kernel::create(&program, "heavy_hash")
            .unwrap_or_else(|_| panic!("{}::Kernel::create failed", name));


        let queue =
            CommandQueue::create_with_properties(&context, device.id(), CL_QUEUE_OUT_OF_ORDER_EXEC_MODE_ENABLE, 0)
                .unwrap_or_else(|_| panic!("{}::CommandQueue::create_with_properties failed", name));

        let final_nonce = Buffer::<cl_ulong>::create(context_ref, CL_MEM_READ_WRITE, 1, ptr::null_mut())
            .expect("Buffer allocation failed");
        let final_hash = Buffer::<[cl_ulong; 4]>::create(context_ref, CL_MEM_WRITE_ONLY, 1, ptr::null_mut())
            .expect("Buffer allocation failed");

        let hash_header = Buffer::<cl_uchar>::create(context_ref, CL_MEM_READ_ONLY, 72, ptr::null_mut())
            .expect("Buffer allocation failed");
        let matrix = Buffer::<cl_uchar>::create(context_ref, CL_MEM_READ_ONLY, 64 * 64, ptr::null_mut())
            .expect("Buffer allocation failed");
        let target = Buffer::<cl_ulong>::create(context_ref, CL_MEM_READ_ONLY, 4, ptr::null_mut())
            .expect("Buffer allocation failed");

        let mut seed = [1u64; 4];
        seed.try_fill(&mut rand::thread_rng())?;

        let random_state = match random {
            NonceGenEnum::Xoshiro => {
                let random_state =
                    Buffer::<cl_ulong>::create(context_ref, CL_MEM_READ_WRITE, 4 * chosen_workload, ptr::null_mut())
                        .expect("Buffer allocation failed");
                let rand_state =
                    Xoshiro256StarStar::new(&seed).iter_jump_state().take(chosen_workload).collect::<Vec<[u64; 4]>>();
                let mut random_state_local: *mut c_void = std::ptr::null_mut::<c_void>();
                info!("{}: Generating initial seed. This may take some time.", name);

                queue
                    .enqueue_map_buffer(
                        &random_state,
                        CL_BLOCKING,
                        CL_MAP_WRITE,
                        0,
                        32 * chosen_workload,
                        &mut random_state_local,
                        &[],
                    )
                    .map_err(|e| e.to_string())?
                    .wait()
                    .unwrap();
                if random_state_local.is_null() {
                    return Err(format!("{}::could not load random state vector to memory. Consider changing random or lowering workload", name).into());
                }
                unsafe {
                    random_state_local.copy_from(rand_state.as_ptr() as *mut c_void, 32 * chosen_workload);
                }
                // queue.enqueue_svm_unmap(&random_state,&[]).map_err(|e| e.to_string())?;
                queue
                    .enqueue_unmap_mem_object(random_state.get(), random_state_local, &[])
                    .map_err(|e| e.to_string())
                    .unwrap()
                    .wait()
                    .unwrap();
                info!("{}: Done generating initial seed", name);
                random_state
            }
            NonceGenEnum::Lean => {
                let mut random_state = Buffer::<cl_ulong>::create(context_ref, CL_MEM_READ_WRITE, 1, ptr::null_mut())
                    .expect("Buffer allocation failed");
                queue
                    .enqueue_write_buffer(&mut random_state, CL_BLOCKING, 0, &[thread_rng().next_u64()], &[])
                    .map_err(|e| e.to_string())
                    .unwrap()
                    .wait()
                    .unwrap();
                random_state
            }
        };
        Ok(Self {
            context,
            workload: chosen_workload,
            random: *random,
            heavy_hash,
            random_state,
            queue,
            final_nonce,
            final_hash,
            hash_header,
            matrix,
            target,
            events: Vec::<cl_event>::new(),
            experimental_amd: ((experimental_amd | use_binary) & experimental_amd_use),
        })
    }
}

fn from_source(context: &Context, device: &Device, options: &str) -> Result<Program, String> {
    let version = device.version()?;
    let v = version.split(' ').nth(1).unwrap();
    let mut compile_options = options.to_string();
    compile_options += CL_MAD_ENABLE;
    compile_options += CL_FINITE_MATH_ONLY;
    if v == "2.0" || v == "2.1" || v == "3.0" {
        info!("Compiling with OpenCl 2");
        compile_options += CL_STD_2_0;
    }
    compile_options += &match Platform::new(device.platform().unwrap()).name() {
        Ok(name) => format!(
            "-D{} ",
            name.chars()
                .map(|c| match c.is_ascii_alphanumeric() {
                    true => c,
                    false => '_',
                })
                .collect::<String>().to_uppercase()
        ),
        Err(_) => String::new(),
    };
    compile_options += &match device.compute_capability_major_nv() {
        Ok(major) => format!("-D __COMPUTE_MAJOR__={} ", major),
        Err(_) => String::new(),
    };
    compile_options += &match device.compute_capability_minor_nv() {
        Ok(minor) => format!("-D __COMPUTE_MINOR__={} ", minor),
        Err(_) => String::new(),
    };

    compile_options += &match device.gfxip_major_amd() {
        Ok(major) => format!("-D __GFXIP_MAJOR__={} ", major),
        Err(_) => String::new(),
    };

    compile_options += &match device.gfxip_minor_amd() {
        Ok(minor) => format!("-D __GFXIP_MINOR__={} ", minor),
        Err(_) => String::new(),
    };

    // Hack to recreate the AMD flags
    compile_options += &match device.pcie_id_amd() {
        Ok(_) => {
            let device_name = device.name().unwrap_or_else(|_| "Unknown".into()).to_lowercase();
            format!("-D OPENCL_PLATFORM_AMD -D __{}__", device_name)
        },
        Err(_) => String::new(),
    };

    info!("Build OpenCL with {}", compile_options);

    Program::create_and_build_from_source(context, PROGRAM_SOURCE, compile_options.as_str())
}
