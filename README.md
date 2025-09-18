solana-program-cli
===================

Production-grade CLI to interact with Solana programs using Program IDs, with a hybrid engine:

- Codegen clients (Codama) for known programs to ensure correct layouts
- Dynamic IDL + Borsh engine for unknown programs
- Jupiter swaps with auto-ATA, simulation, and safety checks

Features
--------
- Generated Rust clients for known programs (`send_program`, `hello_world`)
- Program registry routes known → generated, unknown → dynamic engine
- Smart account resolution (PDAs), IDL-based account validation
- Simulation-first + human-readable error decoding (generated error maps + IDL fallback)
- Jupiter API integration for swaps (versioned txs, ALTs), with auto-ATA create

Prerequisites
-------------
- Rust toolchain (1.83+)
- Solana keypair at `~/.config/solana/id.json`
- RPC URL via `HELIUS_RPC_URL` or `SOLANA_RPC_URL` (defaults to devnet)

Build
-----
```
cargo build
```

Environment
-----------
- `HELIUS_RPC_URL` or `SOLANA_RPC_URL` (recommended to set one explicitly)

Usage (examples)
----------------
Hello World
```
# Initialize account
./target/debug/solana-program-cli hello-world initialize --message "Hi" --account-keypair ./hello-world/target/deploy/hello_world-keypair.json

# Get message
./target/debug/solana-program-cli hello-world get-message --account-pubkey <PUBKEY>
```

Send Program (PDA-backed)
```
# Smart init (derive PDA and initialize if missing)
./target/debug/solana-program-cli send smart-init

# Smart send (uses derived PDA)
./target/debug/solana-program-cli send smart-send --amount 0.01 --recipient <RECIPIENT_PUBKEY>

# Smart stats (reads PDA stats)
./target/debug/solana-program-cli send smart-stats
```

Jupiter Swaps (Production)
```
# Quote
./target/debug/solana-program-cli send jupiter-quote --input-mint SOL --output-mint USDC --amount 1000000 --slippage-bps 50

# Swap (auto-ATA creation + safety checks)
./target/debug/solana-program-cli send jupiter-swap --input-mint SOL --output-mint USDC --amount 1000000 --slippage-bps 50
```

Safety Rails
------------
- Auto-ATA check/create (idempotent); rent/balance validation
- Preflight simulation and error decoding from logs
- IDL-based account validation (signer/writable checks)

Extending
---------
1) Add program IDL to repo
2) Generate Rust client (Codama) and place under `src/generated/<program>`
3) Add program ID to `src/program_registry.rs`
4) Call generated instruction builders from the CLI

Notes
-----
- Jupiter is intentionally API-driven; it returns ready-to-sign versioned transactions with ALTs
- Unknown programs are still supported via dynamic IDL+Borsh encoder


