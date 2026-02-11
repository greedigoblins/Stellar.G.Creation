# Stellar Vanity Address Generator

A collection of tools for generating custom Stellar addresses with specific patterns (vanity addresses) and managing Stellar keypairs.

## What is a Vanity Address?

A vanity address is a cryptocurrency address with a custom pattern. For example, instead of a random address like `GDZQB7XYZW...`, you can create one like `GOAT...LUKE` that starts and ends with specific words or patterns.

## Features

- Multi-core CPU vanity address generation
- Generate addresses with custom prefix and suffix patterns
- Check Stellar account status and balances
- Derive public keys from secret keys
- Optimized for cloud instances with many CPU cores

## Getting Started

### Prerequisites

- Python 3.7 or higher
- pip (Python package manager)

### Installation

1. Clone or download this repository

2. Create a virtual environment (recommended):
```bash
python3 -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
```

3. Install dependencies:
```bash
pip install -r requirements.txt
```

## File Descriptions

### `vanity_multicore.py`

The main vanity address generator that uses all available CPU cores to search for Stellar addresses matching a specific pattern.

**Current pattern:** `GOAT...LUKE` (starts with GOAT, ends with LUKE)

**Features:**
- Multi-core parallel processing for maximum speed
- Real-time progress statistics
- Automatic keypair saving to file
- Estimated time remaining calculations
- Can search for any prefix/suffix combination

**Difficulty:**
- Finding a 7-character pattern (3 prefix + 4 suffix) requires ~17 billion attempts on average
- With modern multi-core CPUs, this can take hours to days depending on your hardware

**Usage:**
```bash
python vanity_multicore.py
```

The script will:
1. Show pattern details and estimated search time
2. Wait for confirmation
3. Search using all CPU cores
4. Save the keypair to `goat_luke_keypair.txt` when found

**Customizing the pattern:**
Edit lines 20-21 in the file:
```python
PREFIX = "OAT"   # After the G, so address starts with GOAT
SUFFIX = "LUKE"
```

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

### `requirements.txt`

Python package dependencies for the project.

**Packages:**
- `stellar-sdk` - Official Stellar SDK for Python
- `cupy-cuda12x` - GPU acceleration library (optional, for future GPU implementations)

## Important Security Notes

### Protecting Your Secret Keys

- **NEVER share your secret key with anyone**
- Secret keys start with 'S' and give full control of the account
- Public keys start with 'G' and are safe to share
- Back up your secret keys in a secure location (password manager, encrypted file, etc.)
- The vanity generator saves keypairs to a text file - move this to a secure location immediately

### Account Activation

New Stellar addresses must be funded with at least 1.5 XLM to become active on the network. Until funded, the address exists only as a keypair but cannot receive transactions.

## Performance Tips

### CPU Vanity Generation

- **Cloud instances:** Use compute-optimized instances with many cores (16-64+ cores)
- **Local machines:** Close other applications to dedicate CPU resources
- **Pattern difficulty:** Each additional character increases difficulty by 32x
- **Expected speeds:** ~5,000-10,000 keys/sec per CPU core

### Pattern Complexity

| Pattern Length | Average Attempts | Typical Time (32 cores) |
|---------------|------------------|-------------------------|
| 3 characters  | ~16,000         | Instant                 |
| 4 characters  | ~500,000        | Seconds                 |
| 5 characters  | ~16 million     | Minutes                 |
| 6 characters  | ~500 million    | Hours                   |
| 7 characters  | ~17 billion     | Days                    |
| 8 characters  | ~550 billion    | Weeks                   |

## Stellar Network Resources

- **Horizon API:** https://horizon.stellar.org
- **Stellar Expert (Block Explorer):** https://stellar.expert
- **Stellar Documentation:** https://developers.stellar.org
- **Test Network (for testing):** https://horizon-testnet.stellar.org

## Common Use Cases

1. **Generate a vanity address:**
   ```bash
   python vanity_multicore.py
   ```

2. **Check if your new address is funded:**
   ```bash
   python check_account.py
   ```

3. **Verify a keypair:**
   ```bash
   python derive_public.py
   ```

## Troubleshooting

### "Account does NOT exist yet"
- The address hasn't been funded with XLM yet
- Send at least 1.5 XLM to activate it
- Wait 1-5 minutes for the transaction to process

### Vanity generation is slow
- Reduce pattern length (fewer characters = much faster)
- Use a cloud instance with more CPU cores
- Consider using only a prefix OR suffix, not both

### Import errors
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
