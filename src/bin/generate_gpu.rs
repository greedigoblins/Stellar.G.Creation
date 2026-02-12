

//! GPU-seeded Stellar vanity generator (Mac-friendly via wgpu/Metal).
//!
//! What this does *today*
//! - Uses your laptop GPU (via wgpu -> Metal on macOS) to generate batches of candidate 32-byte seeds.
//! - Uses CPU (ed25519-dalek) to derive the public key + Stellar StrKey and test prefix/suffix/contains/both.
//! - Uses Rayon to parallelize CPU ed25519 + StrKey evaluation across cores.
//!
//! Why this is a good first step
//! - It validates the full GPU compute plumbing (wgpu device/queue, compute pipeline, buffer mapping).
//! - It keeps crypto correctness on CPU while we iterate.
//!
//! What it does *not* do yet
//! - Full ed25519 scalar multiplication on GPU (that is where the big speedup lives).
//!
//! Build notes
//! - This is intended as a `src/bin/generate_gpu.rs` in a Cargo crate, but you can keep it wherever.
//! - You will need a Cargo.toml with dependencies listed at the bottom of this file.
//!
//! Example:
//!   cargo run --release --bin generate_gpu -- both GAME OVER --batch 262144

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bytemuck::{Pod, Zeroable};
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use rand::RngCore;
use stellar_strkey::ed25519::{PrivateKey as StellarPrivateKey, PublicKey as StellarPublicKey};
use rayon::prelude::*;
use rayon::ThreadPoolBuilder;

const BASE32_ALLOWED: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Prefix,
    Suffix,
    Contains,
    Both,
}

impl Mode {
    fn parse(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "prefix" => Ok(Self::Prefix),
            "suffix" => Ok(Self::Suffix),
            "contains" => Ok(Self::Contains),
            "both" => Ok(Self::Both),
            _ => Err(format!(
                "unknown mode: {s}. expected one of: prefix | suffix | contains | both"
            )),
        }
    }
}

#[derive(Clone, Debug)]
struct MatchSpec {
    mode: Mode,
    pattern: String,
    pattern2: Option<String>,
}

fn validate_pattern(field: &str, input: &str) -> Result<String, String> {
    let s = input.trim().to_ascii_uppercase();
    if s.is_empty() {
        return Err(format!("{field} must be non-empty"));
    }
    let mut invalid = Vec::new();
    for ch in s.chars() {
        if !BASE32_ALLOWED.contains(ch) {
            invalid.push(ch);
        }
    }
    if !invalid.is_empty() {
        invalid.sort();
        invalid.dedup();
        let bad: String = invalid.into_iter().collect();
        return Err(format!(
            "{field} contains invalid characters: {bad:?}. Allowed: A-Z and 2-7 (Stellar StrKey base32). \
            Tip: 0/1/8/9 will never appear; if you meant 'O', use letter O not digit 0."
        ));
    }
    Ok(s)
}

fn matches(public_key: &str, spec: &MatchSpec) -> bool {
    match spec.mode {
        Mode::Prefix => public_key.starts_with(&spec.pattern),
        Mode::Suffix => public_key.ends_with(&spec.pattern),
        Mode::Contains => public_key.contains(&spec.pattern),
        Mode::Both => {
            let p2 = spec.pattern2.as_deref().unwrap_or("");
            public_key.starts_with(&spec.pattern) && public_key.ends_with(p2)
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Params {
    // 128-bit base seed split into 4x u32.
    base_seed: [u32; 4],
    // Starting counter for this dispatch.
    start: u32,
    // How many seeds to generate.
    count: u32,
    // Uniform buffers require 16-byte alignment; pad struct size to 32 bytes.
    _pad: [u32; 2],
}

struct GpuSeedGen {
    device: wgpu::Device,
    queue: wgpu::Queue,

    params_buf: wgpu::Buffer,
    out_buf: wgpu::Buffer,
    readback_buf: wgpu::Buffer,

    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,

    max_count: u32,
}

impl GpuSeedGen {
    async fn new(max_count: u32) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .map_err(|e| format!("request_adapter failed: {e:?}"))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("vanity-gpu-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| format!("request_device failed: {e}"))?;

        // Buffers hold seeds as u32 words. Each seed is 32 bytes = 8 u32.
        let words_per_seed: u32 = 8;
        let out_words = max_count
            .checked_mul(words_per_seed)
            .ok_or("max_count too large")?;
        let out_bytes = out_words as u64 * 4;

        let params_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let out_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("out"),
            size: out_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: out_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("seedgen.wgsl"),
            source: wgpu::ShaderSource::Wgsl(SEEDGEN_WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("seedgen-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("seedgen-bg"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: out_buf.as_entire_binding(),
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("seedgen-pl"),
            bind_group_layouts: &[&bind_group_layout],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("seedgen-pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        Ok(Self {
            device,
            queue,
            params_buf,
            out_buf,
            readback_buf,
            bind_group,
            pipeline,
            max_count,
        })
    }

    /// Generate `count` seeds into a CPU-owned Vec<u8> of length count*32.
    ///
    /// NOTE: The PRNG used inside the shader is *not* cryptographically strong.
    /// For production key material, we should switch to a counter-based stream cipher
    /// (e.g., ChaCha20) seeded from OsRng.
    async fn generate(&self, base_seed: [u32; 4], start: u32, count: u32) -> Result<Vec<u8>, String> {
        if count == 0 || count > self.max_count {
            return Err(format!("count must be 1..={} (got {count})", self.max_count));
        }

        let params = Params {
            base_seed,
            start,
            count,
            _pad: [0, 0],
        };
        self.queue
            .write_buffer(&self.params_buf, 0, bytemuck::bytes_of(&params));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("seedgen-encoder"),
            });

        {
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("seedgen-pass"),
                timestamp_writes: None,
            });
            cpass.set_pipeline(&self.pipeline);
            cpass.set_bind_group(0, &self.bind_group, &[]);

            // Workgroup size is 256; dispatch enough groups to cover `count` threads.
            let wg: u32 = 256;
            let groups = (count + wg - 1) / wg;
            cpass.dispatch_workgroups(groups, 1, 1);
        }

        // Copy only the region we need into readback.
        let words_per_seed: u64 = 8;
        let bytes_per_seed: u64 = words_per_seed * 4;
        let needed_bytes = (count as u64) * bytes_per_seed;
        encoder.copy_buffer_to_buffer(&self.out_buf, 0, &self.readback_buf, 0, needed_bytes);

        self.queue.submit(Some(encoder.finish()));

        // Map and read.
        let slice = self.readback_buf.slice(0..needed_bytes);
        let (tx, rx) = futures_intrusive::channel::shared::oneshot_channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            tx.send(r).ok();
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| format!("device.poll failed: {e:?}"))?;
        rx.receive()
            .await
            .ok_or("map_async channel closed")?
            .map_err(|e| format!("map_async failed: {e:?}"))?;

        let data = slice.get_mapped_range();
        let out = data.to_vec();
        drop(data);
        self.readback_buf.unmap();

        Ok(out)
    }
}

const SEEDGEN_WGSL: &str = r#"
struct Params {
  base_seed: vec4<u32>,
  start: u32,
  count: u32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> out_words: array<u32>;

fn mix32(x: u32) -> u32 {
  var z = x;
  z ^= z >> 16u;
  z *= 0x7feb352du;
  z ^= z >> 15u;
  z *= 0x846ca68bu;
  z ^= z >> 16u;
  return z;
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
  let i = gid.x;
  if (i >= params.count) {
    return;
  }

  // Per-thread counter.
  let c = params.start + i;

  // Expand base_seed + counter into 8x u32 (32 bytes) using a cheap mixer.
  // NOTE: Not cryptographically strong; OK for plumbing + benchmarking.
  let base0 = params.base_seed.x ^ c;
  let base1 = params.base_seed.y + c * 1664525u + 1013904223u;
  let base2 = params.base_seed.z ^ (c * 2246822519u);
  let base3 = params.base_seed.w + (c * 3266489917u);

  let idx = i * 8u;
  out_words[idx + 0u] = mix32(base0 ^ 0xA5A5A5A5u);
  out_words[idx + 1u] = mix32(base1 ^ 0x5A5A5A5Au);
  out_words[idx + 2u] = mix32(base2 ^ 0x3C3C3C3Cu);
  out_words[idx + 3u] = mix32(base3 ^ 0xC3C3C3C3u);

  out_words[idx + 4u] = mix32(base0 + 0x9E3779B9u);
  out_words[idx + 5u] = mix32(base1 + 0x7F4A7C15u);
  out_words[idx + 6u] = mix32(base2 + 0x6A09E667u);
  out_words[idx + 7u] = mix32(base3 + 0xBB67AE85u);
}
"#;

fn print_usage_and_exit(msg: &str) -> ! {
    eprintln!("{msg}\n");
    eprintln!("Usage:\n  generate_gpu <mode> <pattern> [pattern2] [--batch N] [--threads N] [--max-seconds S]\n\nModes:\n  prefix   (GXXXX...)\n  suffix   (...XXXX)\n  contains (*XXXX*)\n  both     (GXXXX... AND ...YYYY)\n\nNotes:\n  - Patterns must be base32: A-Z and 2-7.\n  - For prefix/both, pattern must start with 'G'.\n  - For both, provide pattern2 as the suffix pattern.\n\nExample:\n  generate_gpu both GAME OVER --batch 262144 --max-seconds 60\n" );
    std::process::exit(2);
}

fn parse_flag_u64(args: &[String], name: &str) -> Option<u64> {
    args.windows(2)
        .find(|w| w[0] == name)
        .and_then(|w| w[1].parse::<u64>().ok())
}

fn main() {
    // Basic arg parsing (keep dependencies minimal).
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() < 3 {
        print_usage_and_exit("missing required args");
    }

    let mode = Mode::parse(&argv[1]).unwrap_or_else(|e| print_usage_and_exit(&e));
    let pattern = validate_pattern("pattern", &argv[2]).unwrap_or_else(|e| print_usage_and_exit(&e));

    let mut idx = 3usize;
    let mut pattern2: Option<String> = None;

    if mode == Mode::Both {
        if argv.len() < 4 {
            print_usage_and_exit("both mode requires pattern2 (suffix)");
        }
        pattern2 = Some(
            validate_pattern("pattern2", &argv[3]).unwrap_or_else(|e| print_usage_and_exit(&e)),
        );
        idx = 4;
    }

    if matches!(mode, Mode::Prefix | Mode::Both) {
        if !pattern.starts_with('G') {
            print_usage_and_exit("For Stellar account IDs, prefix patterns must start with 'G'.");
        }
    }

    // Optional flags.
    let tail = &argv[idx..];
    let batch = parse_flag_u64(tail, "--batch").unwrap_or(262_144) as u32;
    let max_seconds = parse_flag_u64(tail, "--max-seconds").map(|v| v as f64);

    let threads = parse_flag_u64(tail, "--threads").map(|v| v as usize);

    if let Some(n) = threads {
        ThreadPoolBuilder::new()
            .num_threads(n)
            .build_global()
            .unwrap();
        eprintln!("[info] rayon threads forced to {n}");
    }

    let spec = MatchSpec {
        mode,
        pattern,
        pattern2,
    };

    // Seed material.
    let mut seed_bytes = [0u8; 16];
    OsRng.fill_bytes(&mut seed_bytes);
    let base_seed = [
        u32::from_le_bytes(seed_bytes[0..4].try_into().unwrap()),
        u32::from_le_bytes(seed_bytes[4..8].try_into().unwrap()),
        u32::from_le_bytes(seed_bytes[8..12].try_into().unwrap()),
        u32::from_le_bytes(seed_bytes[12..16].try_into().unwrap()),
    ];

    // Try GPU init; fall back to CPU-only seed generation if GPU init fails.
    let gpu = pollster::block_on(GpuSeedGen::new(batch)).ok();

    if gpu.is_none() {
        eprintln!("[warn] GPU init failed; falling back to CPU RNG (still faster than Python)." );
    } else {
        eprintln!("[info] GPU seed generator initialized (wgpu/Metal). batch={batch}");
    }

    // Search loop.
    let stop = AtomicBool::new(false);
    let attempts = AtomicU64::new(0);
    let start_time = Instant::now();
    let mut counter: u32 = 0;

    // Report rate every second.
    let mut last_report = Instant::now();
    let mut last_attempts: u64 = 0;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if let Some(max_s) = max_seconds {
            if start_time.elapsed().as_secs_f64() >= max_s {
                eprintln!("[info] max-seconds reached; stopping.");
                break;
            }
        }

        let batch_bytes: Vec<u8> = if let Some(g) = &gpu {
            pollster::block_on(g.generate(base_seed, counter, batch)).unwrap()
        } else {
            // CPU-only: fill batch*32 bytes from OsRng.
            let mut v = vec![0u8; (batch as usize) * 32];
            OsRng.fill_bytes(&mut v);
            v
        };

        // Evaluate batch on CPU (parallel). Each seed is 32 bytes.
        let found: Option<([u8; 32], String)> = batch_bytes
            .par_chunks_exact(32)
            .filter_map(|chunk| {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(chunk);

                // Derive ed25519 verifying key.
                let sk = SigningKey::from_bytes(&seed);
                let vk = sk.verifying_key();
                let pk_bytes = vk.to_bytes();

                // Encode to Stellar G... strkey and test.
                let gh = StellarPublicKey(pk_bytes).to_string(); // heapless::String<56>
                let g = gh.as_str().to_owned();                  // std::string::String
                if matches(&g, &spec) {
                    Some((seed, g))
                } else {
                    None
                }
            })
            // Return any match found by any worker.
            .find_any(|_| true);

        attempts.fetch_add(batch as u64, Ordering::Relaxed);
        counter = counter.wrapping_add(batch);

        // Rate report.
        if last_report.elapsed() >= Duration::from_secs(1) {
            let total = attempts.load(Ordering::Relaxed);
            let delta = total - last_attempts;
            let secs = last_report.elapsed().as_secs_f64();
            let rate = (delta as f64) / secs;
            eprintln!("[rate] {:.0} seeds/s | total={} | elapsed={:.1}s", rate, total, start_time.elapsed().as_secs_f64());
            last_attempts = total;
            last_report = Instant::now();
        }

        if let Some((seed, g)) = found {
            stop.store(true, Ordering::Relaxed);
            let sh = StellarPrivateKey(seed).to_string();
            let s = sh.as_str().to_owned();
            let total = attempts.load(Ordering::Relaxed);
            println!("FOUND\n  public: {g}\n  secret: {s}\n  attempts: {total}\n  elapsed: {:.3}s", start_time.elapsed().as_secs_f64());
            break;
        }
    }
}

/*
Cargo.toml (suggested)\n\n[package]\nname = "toolbox-vanity"\nversion = "0.1.0"\nedition = "2021"\n\n[[bin]]\nname = "generate_gpu"\npath = "src/bin/generate_gpu.rs"\n\n[dependencies]\nbytemuck = { version = "1", features = ["derive"] }\ned25519-dalek = "2.2"\nrand = "0.8"\nstellar-strkey = "0.0.16"\nwgpu = "28"\npollster = "0.3"\nfutures-intrusive = "0.5"\n
Notes:\n- wgpu runs on Metal on macOS.\n- stellar-strkey provides PublicKey/PrivateKey to_string() for G... and S... formats.
generate_gpu both GAME OVER --batch 262144 --threads 10 --max-seconds 60
*/