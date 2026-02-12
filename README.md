# Stellar Vanity Address Generator

A high-performance GPU-accelerated tool for generating custom Stellar addresses with specific patterns (vanity addresses), plus utilities for managing Stellar keypairs.

## What is a Vanity Address?

A vanity address is a cryptocurrency address with a custom pattern. For example, instead of a random address like `GDZQB7XYZW...`, you can create one like `GOAT...LUKE` that starts and ends with specific words or patterns.

## Features

- ⚡ **GPU-accelerated seed generation** via Metal (macOS) or Vulkan/DirectX (cross-platform)
- 🚀 **Multi-core CPU processing** for ed25519 key derivation using Rayon
- 🎯 **Flexible pattern matching**: prefix, suffix, contains, or both
- 📊 **Real-time performance metrics** (seeds/second, total attempts, elapsed time)
- 🔒 **Cryptographically secure** using ed25519-dalek
- 🛠️ **Python utilities** for account checking and key derivation

## Performance

The GPU-accelerated Rust implementation is significantly faster than CPU-only approaches:

- **GPU seed generation**: Hundreds of thousands to millions of candidates per second
- **CPU key derivation**: Parallelized across all cores using Rayon
- **Combined throughput**: Typically 100,000 - 2,000,000+ keys/sec depending on hardware

### Pattern Complexity

| Pattern Length | Average Attempts | Est. Time @ 1M keys/sec |
|---------------|------------------|-------------------------|
| 3 characters  | ~16,000         | Instant                 |
| 4 characters  | ~500,000        | < 1 second              |
| 5 characters  | ~16 million     | ~16 seconds             |
| 6 characters  | ~500 million    | ~8 minutes              |
| 7 characters  | ~17 billion     | ~4.7 hours              |
| 8 characters  | ~550 billion    | ~6 days                 |

## Getting Started

### Prerequisites

- **Rust** 1.70+ (install from https://rustup.rs)
- **Python 3.7+** (optional, for utility scripts)

### Installation

1. **Clone this repository:**
```bash
git clone <repository-url>
cd Stellar.G.Creation
```

2. **Build the Rust project:**
```bash
cargo build --release
```

The compiled binary will be at: `./target/release/generate_gpu`

3. **Optional: Install Python dependencies** (for utility scripts):
```bash
python3 -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
pip install -r requirements.txt
```

## Usage

### GPU Vanity Generator (`generate_gpu`)

The main GPU-accelerated vanity address generator written in Rust.

**Basic syntax:**
```bash
./target/release/generate_gpu <mode> <pattern> [pattern2] [options]
```

**Modes:**
- `prefix` - Match at the start (e.g., `GOAT...`)
- `suffix` - Match at the end (e.g., `...LUKE`)
- `contains` - Match anywhere in the address
- `both` - Match prefix AND suffix (e.g., `GOAT...LUKE`)

**Examples:**

```bash
# Find address starting with GOAT
./target/release/generate_gpu prefix GOAT

# Find address ending with LUKE
./target/release/generate_gpu suffix LUKE

# Find address with both GOAT prefix and LUKE suffix
./target/release/generate_gpu both GOAT LUKE

# With options: larger batch size and time limit
./target/release/generate_gpu both GOAT LUKE --batch 524288 --max-seconds 300

# Control CPU thread count for key derivation
./target/release/generate_gpu prefix GOAT --threads 8
```

**Options:**
- `--batch N` - GPU batch size (default: 262,144)
- `--threads N` - Number of CPU threads for key derivation
- `--max-seconds S` - Stop after S seconds

**Important Notes:**
- Patterns must use base32 characters: **A-Z** and **2-7** only
- No 0, 1, 8, or 9 (common mistake: use letter O, not zero)
- Prefix patterns must start with **G** (Stellar public key format)
- When found, the tool prints both the public key (G...) and secret key (S...)

**Example output:**
```
[info] GPU seed generator initialized (wgpu/Metal). batch=262144
[rate] 1250000 seeds/s | total=5000000 | elapsed=4.0s
FOUND
  public: GOATXYZ...LUKE
  secret: SABC123...XYZ
  attempts: 34359738368
  elapsed: 27.488s
```

---

## Python Utility Scripts

### `check_account.py`

Checks if a Stellar address exists on the network and displays its balance.

**Usage:**
```bash
python check_account.py
```

**What it does:**
- Connects to the Stellar public network
- Checks if the account is active
- Displays XLM balance and any other assets
- Provides activation instructions if account doesn't exist
- Shows a link to view the account on Stellar Expert

**Customizing:**
Edit line 9 to check a different address:
```python
PUBLIC_KEY = "GCG3JBOHF5LUGBN3BI5RHD7WBHWEBHAMGNKHO5IYVA7MQ3IPOSMILUKE"
```

### `derive_public.py`

A simple utility to derive the public key from a secret key.

**Usage:**
```bash
python derive_public.py
```

Enter your secret key (starts with 'S') when prompted, and it will display the corresponding public key (starts with 'G').

**Use cases:**
- Verify which public address belongs to a secret key
- Recover a public address if you only have the secret key
- Double-check keypair relationships

---

## Project Structure

```
Stellar.G.Creation/
├── Cargo.toml              # Rust package manifest
├── Cargo.lock              # Dependency lock file
├── src/
│   └── bin/
│       └── generate_gpu.rs # GPU-accelerated vanity generator (Rust)
├── target/                 # Build artifacts (not committed)
│   └── release/
│       └── generate_gpu    # Compiled binary
├── check_account.py        # Account status checker (Python)
├── derive_public.py        # Public key derivation utility (Python)
├── requirements.txt        # Python dependencies
└── README.md               # This file
```

## Important Security Notes

### Protecting Your Secret Keys

- **NEVER share your secret key with anyone**
- Secret keys start with 'S' and give full control of the account
- Public keys start with 'G' and are safe to share
- Back up your secret keys in a secure location (password manager, encrypted file, etc.)
- The vanity generator saves keypairs to a text file - move this to a secure location immediately

### Account Activation

New Stellar addresses must be funded with at least 1.5 XLM to become active on the network. Until funded, the address exists only as a keypair but cannot receive transactions.

## Technical Details

### Architecture

1. **GPU Seed Generation (wgpu/Metal)**
   - Generates batches of 32-byte seeds using GPU compute shaders
   - Uses simple PRNG for candidate generation (not cryptographically secure)
   - Typical batch sizes: 64K - 512K seeds per dispatch

2. **CPU Key Derivation (Rayon + ed25519-dalek)**
   - Parallel processing across all CPU cores
   - Derives ed25519 public keys from GPU-generated seeds
   - Encodes to Stellar StrKey format (G... addresses)
   - Pattern matching on CPU side

3. **Why Hybrid GPU/CPU?**
   - ed25519 scalar multiplication on GPU is complex to implement correctly
   - This hybrid approach validates the GPU pipeline while keeping crypto on trusted CPU libraries
   - Future optimization: Move ed25519 to GPU for 10-100x additional speedup

### Dependencies (Rust)

- `wgpu` - GPU compute API (Metal on macOS, Vulkan/DX12 elsewhere)
- `ed25519-dalek` - Ed25519 signature library
- `stellar-strkey` - Stellar address encoding/decoding
- `rayon` - Data parallelism library
- `bytemuck` - Safe byte casting for GPU buffers

## Stellar Network Resources

- **Horizon API:** https://horizon.stellar.org
- **Stellar Expert (Block Explorer):** https://stellar.expert
- **Stellar Documentation:** https://developers.stellar.org
- **Test Network (for testing):** https://horizon-testnet.stellar.org

## Common Use Cases

1. **Generate a vanity address:**
   ```bash
   ./target/release/generate_gpu both GOAT LUKE --max-seconds 300
   ```

2. **Check if your new address is funded:**
   ```bash
   python check_account.py
   # (Edit the script to use your new address)
   ```

3. **Verify a keypair:**
   ```bash
   python derive_public.py
   # (Enter the secret key when prompted)
   ```

4. **Quick test with simple pattern:**
   ```bash
   ./target/release/generate_gpu prefix GAB --max-seconds 10
   ```

## Troubleshooting

### Build errors
- **"cargo: command not found"** - Install Rust from https://rustup.rs
- **"linking with 'cc' failed"** - Install Xcode Command Line Tools: `xcode-select --install` (macOS)
- **"failed to compile wgpu"** - Ensure you have the latest Rust: `rustup update`

### GPU initialization fails
- The tool will fall back to CPU-only seed generation (still fast!)
- Check GPU drivers are up to date
- On macOS, Metal should work out of the box on modern systems

### "Account does NOT exist yet"
- The address hasn't been funded with XLM yet
- Send at least 1.5 XLM to activate it
- Wait 1-5 minutes for the transaction to process

### Vanity generation is slow
- Reduce pattern length (fewer characters = exponentially faster)
- Increase `--batch` size (try 524288 or 1048576)
- Consider using only a prefix OR suffix, not both
- Each additional character multiplies difficulty by 32x

### Python import errors
- Make sure virtual environment is activated
- Run `pip install -r requirements.txt` again
- Check Python version is 3.7+

## License

This project is for educational and personal use. Use at your own risk.

## Disclaimer

- Cryptocurrency operations carry risk
- Always verify addresses before sending funds
- Keep secret keys secure and backed up
- Test with small amounts first
- This software is provided as-is with no warranties
