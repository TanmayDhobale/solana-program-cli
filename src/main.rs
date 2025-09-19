use solana_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, read_keypair_file};
use solana_sdk::signer::Signer;
use solana_sdk::system_program;
use solana_sdk::transaction::{Transaction, VersionedTransaction};
use anyhow::Result;
use clap::{Parser, Subcommand};
use std::str::FromStr;

mod idl_loader;
mod borsh_encoder;
mod account_resolver;
mod transaction_simulator;
mod jupiter_client;
mod ata_manager;
mod generated;
mod program_registry;
use idl_loader::IdlLoader;
use borsh_encoder::BorshEncoder;
use account_resolver::{AccountResolver, AccountResolution};
use transaction_simulator::TransactionSimulator;
use jupiter_client::{JupiterClient, QuoteRequest};
use ata_manager::{AtaManager, CommonMints};
use program_registry::{ProgramRegistry, ProgramRoute, ProgramManifest};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use solana_client::rpc_config::RpcSimulateTransactionConfig;
use solana_client::rpc_response::RpcSimulateTransactionResult;

fn parse_custom_error_from_logs(logs: &Vec<String>) -> Option<u32> {
    for line in logs {

        if let Some(pos) = line.find("custom program error: 0x") {
            let hex = &line[pos + "custom program error: 0x".len()..];
            if let Some(end) = hex.find(|c: char| !c.is_ascii_hexdigit()) {
                if let Ok(code) = u32::from_str_radix(&hex[..end], 16) { return Some(code); }
            } else if let Ok(code) = u32::from_str_radix(hex, 16) { return Some(code); }
        }
    }
    None
}

fn print_decoded_error(idl_loader: &IdlLoader, program_id_str: &str, sim: &RpcSimulateTransactionResult) {
    if let Some(logs) = &sim.logs { 
        if let Some(code) = parse_custom_error_from_logs(logs) {
           
            let generated_msg = if program_id_str == generated::send_program::PROGRAM_ID {
                generated::send_program::decode_error(code)
            } else { None };

            let msg_owned: Option<String> = match generated_msg {
                Some(m) => Some(m.to_string()),
                None => idl_loader.decode_error(program_id_str, code),
            };

            if let Some(msg) = msg_owned {
                println!("🔎 Decoded program error ({}): {}", code, msg);
            } else {
                println!("🔎 Program error code: {} (no mapping found)", code);
            }
        }
    }
}

fn program_label(program_id: &Pubkey) -> &'static str {
    match program_id.to_string().as_str() {
       
        "11111111111111111111111111111111" => "System Program",
       
        "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" => "SPL Token",
       
        "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL" => "SPL Associated Token Account",
       
        "TokenzQdBNbLqU2YPbVmjYVBRhCF9dDid1i9QpZ5dKQ" => "SPL Token-2022",
        _ => "Unknown Program",
    }
}

fn validate_accounts_against_idl(idl_loader: &IdlLoader, program_id_str: &str, instruction_name: &str, metas: &Vec<AccountMeta>) -> Result<()> {
    let spec = idl_loader.get_instruction(program_id_str, instruction_name)?;
    if spec.accounts.len() != metas.len() {
        return Err(anyhow::anyhow!("Account count mismatch: IDL expects {}, provided {}", spec.accounts.len(), metas.len()));
    }
    for (i, (idl_acc, meta)) in spec.accounts.iter().zip(metas.iter()).enumerate() {
       
        if idl_acc.signer && !meta.is_signer {
            return Err(anyhow::anyhow!("Account #{} ('{}') must be signer", i, idl_acc.name));
        }
       
        if idl_acc.writable && !meta.is_writable {
            return Err(anyhow::anyhow!("Account #{} ('{}') must be writable", i, idl_acc.name));
        }
    }
    Ok(())
}

#[derive(Parser)]
#[command(name = "solana-program-cli")]
#[command(about = "A CLI tool to interact with Solana programs using their Program IDs")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
 
    HelloWorld {
        #[command(subcommand)]
        action: HelloWorldActions,
    },
    Calculator {
        #[command(subcommand)]
        action: CalculatorActions,
    },
    Send {
        #[command(subcommand)]
        action: SendActions,
    },
    Swap {
        #[command(subcommand)]
        action: SwapActions,
    },
    Registry {
        #[command(subcommand)]
        action: RegistryActions,
    },
}

#[derive(Subcommand)]
enum HelloWorldActions {

    Initialize {
        #[arg(long)]
        message: String,
        #[arg(long)]
        account_keypair: String,
    },
 
    UpdateMessage {
        #[arg(long)]
        account_pubkey: String,
        #[arg(long)]
        new_message: String,
    },
                
    GetMessage {
        #[arg(long)]
        account_pubkey: String,
    },
}

#[derive(Subcommand)]
enum CalculatorActions {

    Initialize {
        #[arg(long)]
        account_keypair: String,
    },

    Add {
        #[arg(long)]
        account_pubkey: String,
        #[arg(long)]
        a: i64,
        #[arg(long)]
        b: i64,
    },

    Ping {
        #[arg(long)]
        account_pubkey: String,
        #[arg(long)]
        message: String,
    },
    GetResult {
        #[arg(long)]
        account_pubkey: String,
    },
}

#[derive(Subcommand)]
enum SendActions {
   
    Initialize {
        #[arg(long)]
        account_keypair: String,
    },
   
    SendSol {
        #[arg(long)]
        account_pubkey: String,
        #[arg(long)]
        amount: String, 
        #[arg(long)]
        recipient: String,
    },
   
    GetStats {
        #[arg(long)]
        account_pubkey: String,
    },

    SmartInit,
                
    SmartSend {
        #[arg(long)]
        amount: String, 
        #[arg(long)]
        recipient: String,
    },

    SmartStats,
    CodegenStats,
   
    Resolve,

    Simulate {
        #[arg(long)]
        amount: String, 
        #[arg(long)]
        recipient: String,
    },

    SafeSend {
        #[arg(long)]
        amount: String, 
        #[arg(long)]
        recipient: String,
    },

    JupiterSwap {
        #[arg(long)]
        input_mint: String, 
        #[arg(long)]
        output_mint: String, 
        #[arg(long)]
        amount: String, 
        #[arg(long, default_value = "50")]
        slippage_bps: u16, 
    },

    JupiterQuote {
        #[arg(long)]
        input_mint: String,
        #[arg(long)]
        output_mint: String,
        #[arg(long)]
        amount: String,
        #[arg(long, default_value = "50")]
        slippage_bps: u16,
    },
}

#[derive(Subcommand)]
enum SwapActions {

    Initialize {
        #[arg(long)]
        account_keypair: String,
        #[arg(long)]
        initial_sol_pool: String, 
        #[arg(long)]
        initial_token_pool: String, 
    },

    SwapSolForTokens {
        #[arg(long)]
        account_pubkey: String,
        #[arg(long)]
        sol_amount: String, 
    },

    SwapTokensForSol {
        #[arg(long)]
        account_pubkey: String,
        #[arg(long)]
        token_amount: String, 
    },

    GetPoolInfo {
        #[arg(long)]
        account_pubkey: String,
    },

    Ping {
        #[arg(long)]
        account_pubkey: String,
        #[arg(long)]
        message: String,
    },
}

#[derive(Subcommand)]
enum RegistryActions {
    List,
    Stats,
    Refresh,
    Validate,
    Add {
        #[arg(long)]
        program_id: String,
        #[arg(long)]
        name: String,
        #[arg(long)]
        idl_url: String,
        #[arg(long)]
        client_version: String,
        #[arg(long)]
        client_type: String,
        #[arg(long, default_value = "5")]
        priority: u8,
    },
    Remove {
        #[arg(long)]
        program_id: String,
    },
    Enable {
        #[arg(long)]
        program_id: String,
    },
    Disable {
        #[arg(long)]
        program_id: String,
    },
}


const HELLO_WORLD_PROGRAM_ID: &str = "5PiuXarsz2F7Q6NpSCtdBbK6vroQWiGSdJZW3fPkjWHw";
const CALCULATOR_PROGRAM_ID: &str = "5tAg6PUJU3AcBGwCJotSbBkGzEm4yNLM9nUK22rPCukq";
const SEND_PROGRAM_ID: &str = "Bj4vH3tVu1GjCHeU3peRfYyxJpAzooyZCTU6rRFR4AnY";
const SWAP_PROGRAM_ID: &str = "7JFPcs97cBb6bgfWiLsmA5Qpiv87oVA4Ue3TLinzNhxj";

fn setup_idl_loader() -> Result<IdlLoader> {
    let mut loader = IdlLoader::new();
    
    
    if let Ok(_) = loader.load_from_file("hello_world.json", HELLO_WORLD_PROGRAM_ID) {
        println!("✅ Loaded Hello World IDL");
    }
    if let Ok(_) = loader.load_from_file("calculator.json", CALCULATOR_PROGRAM_ID) {
        println!("✅ Loaded Calculator IDL");
    }
    if let Ok(_) = loader.load_from_file("send_program.json", SEND_PROGRAM_ID) {
        println!("✅ Loaded Send Program IDL");
    }
    if let Ok(_) = loader.load_from_file("swap_program.json", SWAP_PROGRAM_ID) {
        println!("✅ Loaded Swap Program IDL");
    }
    
    Ok(loader)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

        
    let idl_loader = setup_idl_loader()?;
    let encoder = BorshEncoder::new();

    let payer = read_keypair_file(&*shellexpand::tilde("~/.config/solana/id.json"))
        .map_err(|e| anyhow::anyhow!("Failed to load keypair from ~/.config/solana/id.json: {}", e))?;

    
    let rpc_url = std::env::var("HELIUS_RPC_URL")
        .or_else(|_| std::env::var("SOLANA_RPC_URL"))
        .unwrap_or_else(|_| "https://api.devnet.solana.com".to_string());
    
    println!("🌐 Using RPC endpoint: {}", rpc_url);


    let rpc_client = RpcClient::new_with_commitment(
        rpc_url.clone(),
        CommitmentConfig::processed(),
    );

        
    let account_resolver = AccountResolver::new(
        RpcClient::new(rpc_url.clone())
    );

        
    let simulator = TransactionSimulator::new(
        RpcClient::new(rpc_url.clone())
    );

        
    let jupiter_client = JupiterClient::new();

        
    let ata_manager = AtaManager::new(RpcClient::new(rpc_url.clone()));

    println!("🔧 Initializing program registry...");
    let mut program_registry = ProgramRegistry::load_or_create("./cache").await?;
    if let Err(e) = program_registry.validate() {
        println!("⚠️  Registry validation failed: {}", e);
        println!("🔄 Refreshing registry...");
        program_registry.refresh().await?;
    }
    if program_registry.needs_refresh() {
        println!("🔄 Registry needs refresh, updating...");
        program_registry.refresh().await?;
    }
    let stats = program_registry.get_stats();
    println!("📊 Registry stats: {} programs ({} enabled, {} disabled)", 
             stats.total_programs, stats.enabled_programs, stats.disabled_programs);

    match cli.command {
        Commands::HelloWorld { action } => {
            handle_hello_world_command(&rpc_client, &payer, &program_registry, action).await?;
        }
        Commands::Calculator { action } => {
            handle_calculator_command(&rpc_client, &payer, &program_registry, action).await?;
        }
        Commands::Send { action } => {
            handle_send_command(&rpc_client, &payer, action, &idl_loader, &encoder, &account_resolver, &simulator, &jupiter_client, &ata_manager, &program_registry).await?;
        }
        Commands::Swap { action } => {
            handle_swap_command(&rpc_client, &payer, action, &idl_loader, &encoder, &account_resolver, &simulator, &jupiter_client, &ata_manager, &program_registry).await?;
        }
        Commands::Registry { action } => {
            handle_registry_command(&mut program_registry, action).await?;
        }
    }

    Ok(())
}

async fn handle_hello_world_command(
    rpc_client: &RpcClient,
    payer: &Keypair,
    program_registry: &ProgramRegistry,
    action: HelloWorldActions,
) -> Result<()> {
    let program_id = Pubkey::from_str(HELLO_WORLD_PROGRAM_ID)?;
    
    match action {
        HelloWorldActions::Initialize { message, account_keypair } => {
            let account_keypair = read_keypair_file(&account_keypair)
                .map_err(|e| anyhow::anyhow!("Failed to read account keypair: {}", e))?;
            
            println!("🚀 Initializing Hello World account...");
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account: {}", account_keypair.pubkey());
            println!("💬 Message: '{}'", message);
            
            
            let mut instruction_data = vec![175, 175, 109, 31, 13, 152, 155, 237]; // initialize discriminator
            instruction_data.extend_from_slice(&(message.len() as u32).to_le_bytes());
            instruction_data.extend_from_slice(message.as_bytes());
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(account_keypair.pubkey(), true), // hello_world_account (writable, signer)
                    AccountMeta::new(payer.pubkey(), true),           // user (writable, signer)
                    AccountMeta::new_readonly(system_program::id(), false), // system_program
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer, &account_keypair],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("🎉 Hello World account initialized successfully!");
        }
        HelloWorldActions::UpdateMessage { account_pubkey, new_message } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            
            println!("🔄 Updating message in Hello World account...");
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account: {}", account_pubkey);
            println!("💬 New message: '{}'", new_message);
            
            
            let mut instruction_data = vec![23, 135, 34, 211, 96, 120, 107, 9]; // update_message discriminator
            instruction_data.extend_from_slice(&(new_message.len() as u32).to_le_bytes());
            instruction_data.extend_from_slice(new_message.as_bytes());
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(account_pubkey, false),     // hello_world_account (writable)
                    AccountMeta::new_readonly(payer.pubkey(), true), // user (signer)
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            
            if let Ok(sim) = rpc_client.simulate_transaction_with_config(
                &transaction,
                RpcSimulateTransactionConfig {
                    sig_verify: false,
                    replace_recent_blockhash: true,
                    ..Default::default()
                },
            ) {
                if let Some(err) = sim.value.err.as_ref() {
                    println!("❌ Simulation failed: {:?}", err);

                    return Ok(());
                }
            }

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("🎉 Message updated successfully!");
        }
        HelloWorldActions::GetMessage { account_pubkey } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            
            println!("📖 Getting message from Hello World account...");
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account: {}", account_pubkey);
            
            let instruction_data = vec![159, 69, 186, 171, 244, 131, 99, 223]; // get_message discriminator
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new_readonly(account_pubkey, false), // hello_world_account
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            
            if let Ok(sim) = rpc_client.simulate_transaction_with_config(
                &transaction,
                RpcSimulateTransactionConfig {
                    sig_verify: false,
                    replace_recent_blockhash: true,
                    ..Default::default()
                },
            ) {
                if let Some(err) = sim.value.err.as_ref() {
                    println!("❌ Simulation failed: {:?}", err);
                    
                    return Ok(());
                }
            }

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("📝 Check the transaction logs for the message content!");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }
    }

    Ok(())
}

async fn handle_calculator_command(
    rpc_client: &RpcClient,
    payer: &Keypair,
    program_registry: &ProgramRegistry,
    action: CalculatorActions,
) -> Result<()> {
    let program_id = Pubkey::from_str(CALCULATOR_PROGRAM_ID)?;
    
    match action {
        CalculatorActions::Initialize { account_keypair } => {
            let account_keypair = read_keypair_file(&account_keypair)
                .map_err(|e| anyhow::anyhow!("Failed to read account keypair: {}", e))?;
            
            println!("🚀 Initializing Calculator account...");
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account: {}", account_keypair.pubkey());
            
            let instruction_data = vec![175, 175, 109, 31, 13, 152, 155, 237]; // initialize discriminator
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(account_keypair.pubkey(), true), // calculator_account (writable, signer)
                    AccountMeta::new(payer.pubkey(), true),           // user (writable, signer)
                    AccountMeta::new_readonly(system_program::id(), false), // system_program
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer, &account_keypair],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("🎉 Calculator account initialized successfully!");
        }
        CalculatorActions::Add { account_pubkey, a, b } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            
            println!("➕ Adding {} + {} using Calculator...", a, b);
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account: {}", account_pubkey);
            
            
            let mut instruction_data = vec![41, 249, 249, 146, 197, 111, 56, 181]; // add discriminator
            instruction_data.extend_from_slice(&a.to_le_bytes());
            instruction_data.extend_from_slice(&b.to_le_bytes());
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(account_pubkey, false),         // calculator_account (writable)
                    AccountMeta::new_readonly(payer.pubkey(), true), // user (signer)
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            
            if let Ok(sim) = rpc_client.simulate_transaction_with_config(
                &transaction,
                RpcSimulateTransactionConfig {
                    sig_verify: false,
                    replace_recent_blockhash: true,
                    ..Default::default()
                },
            ) {
                if let Some(err) = sim.value.err.as_ref() {
                    println!("❌ Simulation failed: {:?}", err);
                    
                    return Ok(());
                }
            }

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("🎉 Addition completed! Check logs for result.");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }
        CalculatorActions::Ping { account_pubkey, message } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            
            println!("🏓 Sending ping '{}' to Calculator...", message);
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account: {}", account_pubkey);
            
            
            let mut instruction_data = vec![173, 0, 94, 236, 73, 133, 225, 153]; // ping discriminator
            instruction_data.extend_from_slice(&(message.len() as u32).to_le_bytes());
            instruction_data.extend_from_slice(message.as_bytes());
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(account_pubkey, false),         // calculator_account (writable)
                    AccountMeta::new_readonly(payer.pubkey(), true), // user (signer)
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("🏓 Ping sent! Check logs for pong response.");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }
        CalculatorActions::GetResult { account_pubkey } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            
            println!("📊 Getting result from Calculator...");
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account: {}", account_pubkey);
            
            let instruction_data = vec![57, 144, 166, 101, 148, 52, 100, 135]; // get_result discriminator
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new_readonly(account_pubkey, false), // calculator_account
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("📊 Check the transaction logs for the current result!");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }
    }

    Ok(())
}

async fn handle_send_command(
    rpc_client: &RpcClient,
    payer: &Keypair,
    action: SendActions,
    idl_loader: &IdlLoader,
    encoder: &BorshEncoder,
    account_resolver: &AccountResolver,
    simulator: &TransactionSimulator,
    jupiter_client: &JupiterClient,
    ata_manager: &AtaManager,
    program_registry: &ProgramRegistry,
) -> Result<()> {
    let program_id = Pubkey::from_str(SEND_PROGRAM_ID)?;
    
    match action {
        SendActions::Initialize { account_keypair } => {
            let account_keypair = read_keypair_file(&account_keypair)
                .map_err(|e| anyhow::anyhow!("Failed to read account keypair: {}", e))?;
            
            println!("🚀 Initializing Send account...");
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account: {}", account_keypair.pubkey());
            
            
            let args = HashMap::new(); // initialize has no arguments
            let instruction_data = encoder.encode_instruction(
                idl_loader, 
                SEND_PROGRAM_ID, 
                "initialize", 
                args
            )?;
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(account_keypair.pubkey(), true), // send_account (writable, signer)
                    AccountMeta::new(payer.pubkey(), true),           // user (writable, signer)
                    AccountMeta::new_readonly(system_program::id(), false), // system_program
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer, &account_keypair],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("🎉 Send account initialized successfully!");
        }

        SendActions::SendSol { account_pubkey, amount, recipient } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            let recipient_pubkey = Pubkey::from_str(&recipient)?;
            
            
            let sol_amount: f64 = amount.parse()?;
            let lamports = (sol_amount * 1_000_000_000.0) as u64;
            
            println!("💰 Sending {} SOL ({} lamports) to {}...", sol_amount, lamports, recipient_pubkey);
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Send Account: {}", account_pubkey);
            
            
            let mut args = HashMap::new();
            args.insert("amount".to_string(), serde_json::to_value(lamports)?);
            args.insert("recipient".to_string(), serde_json::to_value(recipient_pubkey.to_string())?);
            
            let instruction_data = encoder.encode_instruction(
                idl_loader,
                SEND_PROGRAM_ID,
                "send_sol",
                args
            )?;
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(account_pubkey, false),          // send_account (writable)
                    AccountMeta::new(payer.pubkey(), true),           // sender (writable, signer)
                    AccountMeta::new(recipient_pubkey, false),        // recipient (writable)
                    AccountMeta::new_readonly(system_program::id(), false), // system_program
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("💸 SOL sent successfully! Check logs for details.");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }

        SendActions::GetStats { account_pubkey } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            
            println!("📊 Getting send statistics...");
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account: {}", account_pubkey);
            
            // Use generic encoder for get_stats (no args)
            let args = HashMap::new();
            let instruction_data = encoder.encode_instruction(
                idl_loader,
                SEND_PROGRAM_ID,
                "get_stats",
                args
            )?;
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new_readonly(account_pubkey, false), // send_account
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("📊 Check the transaction logs for send statistics!");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }

        SendActions::SmartInit => {
            println!("🧠 Smart Initialize - Deriving PDA for user...");
            
            // Resolve send account for this user
            let resolution = account_resolver.smart_resolve(&payer.pubkey(), "send")?;
            
            match &resolution {
                AccountResolution::Found { address, .. } => {
                    println!("✅ Send account already exists at: {}", address);
                    return Ok(());
                }
                AccountResolution::SuggestCreate { address, required_rent, .. } => {
                    println!("📋 Program ID: {}", program_id);
                    println!("🔑 Derived PDA: {}", address);
                    println!("💰 Required rent: {} lamports ({} SOL)", required_rent, *required_rent as f64 / 1_000_000_000.0);
                    
            // Route to generated or dynamic per registry (demo: send program is generated)
            let route = program_registry.resolve(&program_id);
            let instruction = match route {
                ProgramRoute::GeneratedClient(client_name) if client_name.starts_with("send_program") => {
                    generated::send_program::initialize_instruction(
                        *address, payer.pubkey(), system_program::id(),
                    )?
                }
                _ => {
                    // Fallback dynamic path (should not hit for send_program)
                    let args = HashMap::new();
                    let data = encoder.encode_instruction(idl_loader, SEND_PROGRAM_ID, "initialize", args)?;
                    Instruction { program_id, accounts: vec![
                        AccountMeta::new(*address, false),
                        AccountMeta::new(payer.pubkey(), true),
                        AccountMeta::new_readonly(system_program::id(), false),
                    ], data }
                }
            };
                    // Validate against IDL
                    validate_accounts_against_idl(idl_loader, SEND_PROGRAM_ID, "initialize", &instruction.accounts)?;

                    let recent_blockhash = rpc_client.get_latest_blockhash()?;
                    let transaction = Transaction::new_signed_with_payer(
                        &[instruction],
                        Some(&payer.pubkey()),
                        &[payer],
                        recent_blockhash,
                    );

                    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
                    println!("✅ Transaction signature: {}", signature);
                    println!("🎉 Smart Send account initialized at PDA!");
                }
            }
        }

        SendActions::SmartSend { amount, recipient } => {
            println!("🧠 Smart Send - Using derived PDA...");
            
            let recipient_pubkey = Pubkey::from_str(&recipient)?;
            let sol_amount: f64 = amount.parse()?;
            let lamports = (sol_amount * 1_000_000_000.0) as u64;
            
            // Resolve send account for this user
            let resolution = account_resolver.smart_resolve(&payer.pubkey(), "send")?;
            let send_account = resolution.address();
            
            if !resolution.exists() {
                println!("❌ Send account doesn't exist. Run 'smart-init' first!");
                return Ok(());
            }
            
            println!("💰 Sending {} SOL ({} lamports) to {}...", sol_amount, lamports, recipient_pubkey);
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Send Account (PDA): {}", send_account);
            
            // Route per registry
            let route = program_registry.resolve(&program_id);
            let instruction = match route {
                ProgramRoute::GeneratedClient(client_name) if client_name.starts_with("send_program") => {
                    generated::send_program::send_sol_instruction(
                        lamports, recipient_pubkey, *send_account,
                        payer.pubkey(), recipient_pubkey, system_program::id(),
                    )?
                }
                _ => {
                    let mut args = HashMap::new();
                    args.insert("amount".to_string(), serde_json::to_value(lamports)?);
                    args.insert("recipient".to_string(), serde_json::to_value(recipient_pubkey.to_string())?);
                    let data = encoder.encode_instruction(idl_loader, SEND_PROGRAM_ID, "send_sol", args)?;
                    Instruction { program_id, accounts: vec![
                        AccountMeta::new(*send_account, false),
                        AccountMeta::new(payer.pubkey(), true),
                        AccountMeta::new(recipient_pubkey, false),
                        AccountMeta::new_readonly(system_program::id(), false),
                    ], data }
                }
            };
            validate_accounts_against_idl(idl_loader, SEND_PROGRAM_ID, "send_sol", &instruction.accounts)?;

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("💸 Smart SOL sent successfully!");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }

        SendActions::SmartStats => {
            println!("🧠 Smart Stats - Using derived PDA...");
            
            // Resolve send account for this user
            let resolution = account_resolver.smart_resolve(&payer.pubkey(), "send")?;
            let send_account = resolution.address();
            
            if !resolution.exists() {
                println!("❌ Send account doesn't exist. Run 'smart-init' first!");
                return Ok(());
            }
            
            println!("📊 Getting send statistics...");
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account (PDA): {}", send_account);
            
            let route = program_registry.resolve(&program_id);
            let instruction = match route {
                ProgramRoute::GeneratedClient(client_name) if client_name.starts_with("send_program") => {
                    generated::send_program::get_stats_instruction(*send_account)?
                }
                _ => {
                    let data = encoder.encode_instruction(idl_loader, SEND_PROGRAM_ID, "get_stats", HashMap::new())?;
                    Instruction { program_id, accounts: vec![
                        AccountMeta::new_readonly(*send_account, false),
                    ], data }
                }
            };
            validate_accounts_against_idl(idl_loader, SEND_PROGRAM_ID, "get_stats", &instruction.accounts)?;

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("📊 Check the transaction logs for smart send statistics!");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }

        SendActions::CodegenStats => {
            println!("🦀 Codegen Stats - Using Codama-generated client (DEMO)...");
            println!("💡 This demonstrates type-safe, generated Rust client vs manual building");
            
            // Resolve send account for this user
            let resolution = account_resolver.smart_resolve(&payer.pubkey(), "send")?;
            let send_account = resolution.address();
            
            if !resolution.exists() {
                println!("❌ Send account doesn't exist. Run 'smart-init' first!");
                return Ok(());
            }
            
            println!("📊 Getting send statistics using Codama client...");
            println!("📋 Program ID: {}", generated::send_program::PROGRAM_ID);
            println!("🔑 Account (PDA): {}", send_account);
            
            // 🎯 USE CODAMA-GENERATED CLIENT (Type-safe!)
            let instruction = generated::send_program::get_stats_instruction(*send_account)?;
            
            println!("✅ Instruction built with Codama-generated client:");
            println!("  📦 Program ID: {}", instruction.program_id);
            println!("  📝 Data length: {} bytes", instruction.data.len());
            println!("  👥 Accounts: {}", instruction.accounts.len());
            println!("  🔗 Discriminator: {:?}", &instruction.data[0..8]);

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("🎉 Codama stats completed successfully!");
            println!("✅ Transaction signature: {}", signature);
            println!("📊 Check the transaction logs for send statistics!");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
            
            println!("\n💭 COMPARISON:");
            println!("  ❌ Manual: Encode discriminator, serialize args, build AccountMeta[]");
            println!("  ✅ Codama: get_stats_instruction(send_account) - Type-safe!");
        }

        SendActions::Resolve => {
            println!("🧠 Resolving accounts for user: {}", payer.pubkey());
            
            // Resolve send account
            let send_resolution = account_resolver.smart_resolve(&payer.pubkey(), "send")?;
            
            match &send_resolution {
                AccountResolution::Found { address, account_type } => {
                    println!("✅ Send account ({}) found at: {}", account_type, address);
                    let balance = account_resolver.get_balance(address)?;
                    println!("💰 Account balance: {} lamports ({} SOL)", balance, balance as f64 / 1_000_000_000.0);
                }
                AccountResolution::SuggestCreate { address, account_type, required_rent, creation_method } => {
                    println!("💡 Send account ({}) not found", account_type);
                    println!("🔑 Suggested address: {}", address);
                    println!("💰 Required rent: {} lamports ({} SOL)", required_rent, *required_rent as f64 / 1_000_000_000.0);
                    println!("🛠️  Creation method: {}", creation_method);
                    println!("👉 Run 'smart-init' to create it");
                }
            }
            
            // Check user's main balance
            let user_balance = account_resolver.get_balance(&payer.pubkey())?;
            println!("🏦 User balance: {} lamports ({} SOL)", user_balance, user_balance as f64 / 1_000_000_000.0);
        }

        SendActions::Simulate { amount, recipient } => {
            println!("🧪 Simulating SOL send transaction...");
            
            let recipient_pubkey = Pubkey::from_str(&recipient)?;
            let sol_amount: f64 = amount.parse()?;
            let lamports = (sol_amount * 1_000_000_000.0) as u64;
            
            // Resolve send account for this user
            let resolution = account_resolver.smart_resolve(&payer.pubkey(), "send")?;
            let send_account = resolution.address();
            
            if !resolution.exists() {
                println!("❌ Send account doesn't exist. Run 'smart-init' first!");
                return Ok(());
            }
            
            println!("💰 Simulating send of {} SOL ({} lamports) to {}...", sol_amount, lamports, recipient_pubkey);
            println!("🔑 Send Account (PDA): {}", send_account);
            
            // Build the transaction (same as smart-send)
            let mut args = HashMap::new();
            args.insert("amount".to_string(), serde_json::to_value(lamports)?);
            args.insert("recipient".to_string(), serde_json::to_value(recipient_pubkey.to_string())?);
            
            let instruction_data = encoder.encode_instruction(
                idl_loader,
                SEND_PROGRAM_ID,
                "send_sol",
                args
            )?;
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(*send_account, false),          // send_account (writable)
                    AccountMeta::new(payer.pubkey(), true),           // sender (writable, signer)
                    AccountMeta::new(recipient_pubkey, false),        // recipient (writable)
                    AccountMeta::new_readonly(system_program::id(), false), // system_program
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            // Simulate the transaction
            let preview = simulator.preview_transaction(&transaction)?;
            
            println!("\n🔍 SIMULATION RESULTS:");
            println!("✅ Success: {}", if preview.will_succeed { "YES" } else { "NO" });
            println!("💰 Estimated fee: {} lamports ({} SOL)", preview.estimated_fee, preview.estimated_fee as f64 / 1_000_000_000.0);
            println!("⚡ Compute units: {}", preview.compute_units);
            
            if let Some(error) = &preview.error_summary {
                println!("❌ Error: {}", error);
            }
            
            if !preview.account_changes.is_empty() {
                println!("📋 Account changes:");
                for change in &preview.account_changes {
                    println!("  📝 {}", change);
                }
            }
            
            if !preview.program_logs.is_empty() {
                println!("📋 Expected program logs:");
                for log in &preview.program_logs {
                    println!("  📝 {}", log);
                }
            }
            
            println!("\n💡 This was a simulation only - no SOL was actually sent!");
        }

        SendActions::SafeSend { amount, recipient } => {
            println!("🛡️  Safe Send - Simulating first, then sending...");
            
            let recipient_pubkey = Pubkey::from_str(&recipient)?;
            let sol_amount: f64 = amount.parse()?;
            let lamports = (sol_amount * 1_000_000_000.0) as u64;
            
            // Resolve send account for this user
            let resolution = account_resolver.smart_resolve(&payer.pubkey(), "send")?;
            let send_account = resolution.address();
            
            if !resolution.exists() {
                println!("❌ Send account doesn't exist. Run 'smart-init' first!");
                return Ok(());
            }
            
            println!("💰 Preparing to send {} SOL ({} lamports) to {}...", sol_amount, lamports, recipient_pubkey);
            println!("🔑 Send Account (PDA): {}", send_account);
            
            // Build the transaction
            let mut args = HashMap::new();
            args.insert("amount".to_string(), serde_json::to_value(lamports)?);
            args.insert("recipient".to_string(), serde_json::to_value(recipient_pubkey.to_string())?);
            
            let instruction_data = encoder.encode_instruction(
                idl_loader,
                SEND_PROGRAM_ID,
                "send_sol",
                args
            )?;
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(*send_account, false),          // send_account (writable)
                    AccountMeta::new(payer.pubkey(), true),           // sender (writable, signer)
                    AccountMeta::new(recipient_pubkey, false),        // recipient (writable)
                    AccountMeta::new_readonly(system_program::id(), false), // system_program
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            // Use safe send with automatic simulation
            let result = simulator.safe_send_transaction(&transaction)?;
            
            if result.sent {
                println!("🎉 Safe send completed successfully!");
                if let Some(signature) = result.signature {
                    println!("🔍 Use: solana confirm -v {} --url devnet", signature);
                }
            } else {
                println!("❌ Safe send aborted due to validation issues:");
                for issue in &result.validation_issues {
                    println!("  🚨 {}", issue);
                }
            }
        }

        SendActions::JupiterQuote { input_mint, output_mint, amount, slippage_bps } => {
            println!("🔍 Getting Jupiter quote for {} → {} swap...", input_mint, output_mint);
            
            // Convert token shortcuts
            let input_mint = match input_mint.to_uppercase().as_str() {
                "SOL" => jupiter_client::tokens::SOL.to_string(),
                "USDC" => jupiter_client::tokens::USDC.to_string(),
                "USDT" => jupiter_client::tokens::USDT.to_string(),
                _ => input_mint,
            };
            
            let output_mint = match output_mint.to_uppercase().as_str() {
                "SOL" => jupiter_client::tokens::SOL.to_string(),
                "USDC" => jupiter_client::tokens::USDC.to_string(),
                "USDT" => jupiter_client::tokens::USDT.to_string(),
                _ => output_mint,
            };
            
            let amount_num: u64 = amount.parse()?;
            
            let quote_request = QuoteRequest {
                input_mint: input_mint.clone(),
                output_mint: output_mint.clone(),
                amount: amount_num,
                slippage_bps: Some(slippage_bps),
                restrict_intermediate_tokens: Some(true),
                only_direct_routes: Some(false),
            };
            
            match jupiter_client.get_quote(quote_request).await {
                Ok(quote) => {
                    println!("✅ Quote received:");
                    println!("📥 Input: {} tokens ({})", quote.in_amount, input_mint);
                    println!("📤 Output: {} tokens ({})", quote.out_amount, output_mint);
                    println!("💸 Price impact: {}%", quote.price_impact_pct);
                    println!("🎯 Slippage tolerance: {}% ({} bps)", slippage_bps as f64 / 100.0, slippage_bps);
                    println!("⏱️  Quote time: {:.2}ms", quote.time_taken * 1000.0);
                    println!("\n🛣️  Route plan ({} hops):", quote.route_plan.len());
                    
                    for (i, route) in quote.route_plan.iter().enumerate() {
                        println!("  {}. {} - {}% of trade", i + 1, route.swap_info.label, route.percent);
                        println!("     AMM: {}", route.swap_info.amm_key);
                        println!("     Fee: {} {}", route.swap_info.fee_amount, route.swap_info.fee_mint);
                    }
                    
                    println!("\n💡 This was a quote only - no swap executed!");
                    println!("💡 To execute: use 'jupiter-swap' with the same parameters");
                }
                Err(e) => {
                    println!("❌ Failed to get Jupiter quote: {}", e);
                }
            }
        }

        SendActions::JupiterSwap { input_mint, output_mint, amount, slippage_bps } => {
            println!("🚀 Executing production Jupiter swap: {} → {}...", input_mint, output_mint);
            
            // Convert token shortcuts to mint addresses
            let input_mint_pubkey = CommonMints::from_name(&input_mint)?;
            let output_mint_pubkey = CommonMints::from_name(&output_mint)?;
            
            let input_mint_str = input_mint_pubkey.to_string();
            let output_mint_str = output_mint_pubkey.to_string();
            let amount_num: u64 = amount.parse()?;
            
            println!("📋 Swap details:");
            println!("  🪙 From: {} tokens ({})", amount, input_mint_str);
            println!("  🎯 To: {} ({})", output_mint, output_mint_str);
            println!("  📈 Max slippage: {}%", slippage_bps as f64 / 100.0);
            println!("  👤 User: {}", payer.pubkey());
            
            // Step 1: Auto-create ATAs if needed (production security)
            println!("\n🔧 Checking/creating Associated Token Accounts...");
            
            // For swaps, we need ATAs for both input and output tokens (unless SOL)
            let mut pre_instructions = Vec::new();
            
            // Check input ATA (source of tokens)
            if input_mint_pubkey != CommonMints::sol() {
                println!("🔍 Checking input token ATA for {}...", input_mint);
                let input_ata_info = ata_manager.check_ata(&payer.pubkey(), &input_mint_pubkey).await?;
                if !input_ata_info.exists {
                    println!("❌ Input ATA missing for {}! Creating...", input_mint);
                    let create_ix = ata_manager.create_ata_instruction(&payer.pubkey(), &payer.pubkey(), &input_mint_pubkey)?;
                    pre_instructions.push(create_ix);
                } else {
                    println!("✅ Input ATA exists: {} (balance: {} tokens)", 
                             input_ata_info.address, 
                             input_ata_info.balance.unwrap_or(0));
                    
                    // Security check: ensure sufficient balance
                    if let Some(balance) = input_ata_info.balance {
                        if balance < amount_num {
                            return Err(anyhow::anyhow!(
                                "Insufficient token balance: need {}, have {}", 
                                amount_num, balance
                            ));
                        }
                    }
                }
            }
            
            // Check output ATA (destination for tokens)
            if output_mint_pubkey != CommonMints::sol() {
                println!("🔍 Checking output token ATA for {}...", output_mint);
                let output_ata_info = ata_manager.check_ata(&payer.pubkey(), &output_mint_pubkey).await?;
                if !output_ata_info.exists {
                    println!("🔧 Output ATA missing for {}! Creating...", output_mint);
                    let create_ix = ata_manager.create_ata_instruction(&payer.pubkey(), &payer.pubkey(), &output_mint_pubkey)?;
                    pre_instructions.push(create_ix);
                } else {
                    println!("✅ Output ATA exists: {}", output_ata_info.address);
                }
            }
            
            // Execute ATA creation if needed (simulate + decode errors first)
            if !pre_instructions.is_empty() {
                println!("\n🔧 Creating {} missing ATA(s)...", pre_instructions.len());
                let recent_blockhash = rpc_client.get_latest_blockhash()?;
                let ata_transaction = Transaction::new_signed_with_payer(
                    &pre_instructions,
                    Some(&payer.pubkey()),
                    &[payer],
                    recent_blockhash,
                );

                // Simulate to catch errors like insufficient funds or invalid mints
                if let Ok(sim) = rpc_client.simulate_transaction_with_config(
                    &ata_transaction,
                    RpcSimulateTransactionConfig { sig_verify: false, replace_recent_blockhash: true, ..Default::default() },
                ) {
                    if let Some(err) = sim.value.err.as_ref() {
                        println!("❌ ATA creation simulation failed: {:?}", err);
                        // Decode against ATA and Token program maps
                        print_decoded_error(idl_loader, "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL", &sim.value);
                        print_decoded_error(idl_loader, "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA", &sim.value);
                        return Ok(());
                    }
                }
                
                let ata_signature = rpc_client.send_and_confirm_transaction(&ata_transaction)?;
                println!("✅ ATA creation completed! Transaction: {}", ata_signature);
            } else {
                println!("✅ All required ATAs already exist");
            }
            
            // Step 2: Execute Jupiter swap with fresh quote
            println!("\n💱 Building Jupiter swap transaction...");
            match jupiter_client.build_swap_transaction(
                &payer.pubkey(),
                &input_mint_str,
                &output_mint_str,
                amount_num,
                Some(slippage_bps),
            ).await {
                Ok(transaction) => {
                    println!("✅ Jupiter transaction built successfully!");
                    println!("🔗 Contains {} instructions", transaction.message.instructions().len());

                    // Quick quote sanity: versioned message must have 1+ instructions
                    if transaction.message.instructions().is_empty() {
                        println!("❌ Jupiter returned empty instruction set");
                        return Ok(());
                    }
                    
                    // Security: Validate transaction before signing
                    println!("🔍 Validating transaction structure...");
                    if transaction.signatures.len() == 0 {
                        return Err(anyhow::anyhow!("Invalid transaction: no signature slots"));
                    }
                    
                    println!("🔏 Signing Jupiter transaction with user keypair...");
                    let signed_transaction = VersionedTransaction::try_new(
                        transaction.message.clone(),
                        &[payer]
                    ).map_err(|e| anyhow::anyhow!("Failed to sign transaction: {}", e))?;
                    
                    println!("✅ Transaction signed successfully!");
                    println!("🔍 Signature: {}", signed_transaction.signatures[0]);
                    
                    // Step 3: Execute with production settings using safe-send guard
                    println!("\n🚀 Executing Jupiter swap on blockchain...");
                    println!("🔒 Using safe-send guard with ALTs support and quote validation");
                    
                    let result = jupiter_client.safe_send_versioned_transaction(&signed_transaction, &rpc_client).await?;
                    
                    if result.sent {
                        println!("\n🎉 Jupiter swap executed successfully!");
                        if let Some(signature) = result.signature {
                            println!("🔍 Transaction: https://solscan.io/tx/{}", signature);
                            println!("🌐 View on Solscan: https://solscan.io/tx/{}", signature);
                            
                            // Post-swap ATA balances for confirmation
                            println!("\n📊 Post-swap token balances:");
                            if input_mint_pubkey != CommonMints::sol() {
                                if let Ok(input_ata_info) = ata_manager.check_ata(&payer.pubkey(), &input_mint_pubkey).await {
                                    println!("  📥 {} balance: {} tokens", input_mint, input_ata_info.balance.unwrap_or(0));
                                }
                            }
                            if output_mint_pubkey != CommonMints::sol() {
                                if let Ok(output_ata_info) = ata_manager.check_ata(&payer.pubkey(), &output_mint_pubkey).await {
                                    println!("  📤 {} balance: {} tokens", output_mint, output_ata_info.balance.unwrap_or(0));
                                }
                            }
                        }
                    } else {
                        println!("❌ Jupiter swap failed:");
                        for issue in &result.validation_issues {
                            println!("  🚨 {}", issue);
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to build Jupiter swap: {}", e);
                    println!("💡 Tip: Check if the tokens exist and you have sufficient balance");
                }
            }
        }
    }

    Ok(())
}

async fn handle_swap_command(
    rpc_client: &RpcClient,
    payer: &Keypair,
    action: SwapActions,
    idl_loader: &IdlLoader,
    encoder: &BorshEncoder,
    account_resolver: &AccountResolver,
    simulator: &TransactionSimulator,
    jupiter_client: &JupiterClient,
    ata_manager: &AtaManager,
    program_registry: &ProgramRegistry,
) -> Result<()> {
    let program_id = Pubkey::from_str(SWAP_PROGRAM_ID)?;
    
    match action {
        SwapActions::Initialize { account_keypair, initial_sol_pool, initial_token_pool } => {
            let account_keypair = read_keypair_file(&account_keypair)
                .map_err(|e| anyhow::anyhow!("Failed to read account keypair: {}", e))?;
            
            // Convert values
            let sol_amount: f64 = initial_sol_pool.parse()?;
            let sol_lamports = (sol_amount * 1_000_000_000.0) as u64;
            let token_amount: u64 = initial_token_pool.parse()?;
            
            println!("🚀 Initializing Swap pool...");
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Account: {}", account_keypair.pubkey());
            println!("💰 Initial SOL pool: {} SOL ({} lamports)", sol_amount, sol_lamports);
            println!("🪙 Initial token pool: {} tokens", token_amount);
            
            // Create instruction data: discriminator + initial_sol_pool + initial_token_pool
            let mut instruction_data = vec![175, 175, 109, 31, 13, 152, 155, 237]; // initialize discriminator
            instruction_data.extend_from_slice(&sol_lamports.to_le_bytes());
            instruction_data.extend_from_slice(&token_amount.to_le_bytes());
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(account_keypair.pubkey(), true), // swap_pool (writable, signer)
                    AccountMeta::new(payer.pubkey(), true),           // user (writable, signer)
                    AccountMeta::new_readonly(system_program::id(), false), // system_program
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer, &account_keypair],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("🎉 Swap pool initialized successfully!");
        }

        SwapActions::SwapSolForTokens { account_pubkey, sol_amount } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            
            // Convert SOL to lamports
            let sol_amt: f64 = sol_amount.parse()?;
            let lamports = (sol_amt * 1_000_000_000.0) as u64;
            
            println!("🔄 Swapping {} SOL ({} lamports) for tokens...", sol_amt, lamports);
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Pool Account: {}", account_pubkey);
            
            // Create instruction data: discriminator + sol_amount
            let mut instruction_data = vec![1, 171, 24, 135, 201, 236, 210, 219]; 
            instruction_data.extend_from_slice(&lamports.to_le_bytes());
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(account_pubkey, false),         
                    AccountMeta::new(payer.pubkey(), true),           
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("🔄 Swap completed! Check logs for details.");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }

        SwapActions::SwapTokensForSol { account_pubkey, token_amount } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            let tokens: u64 = token_amount.parse()?;
            
            println!("🔄 Swapping {} tokens for SOL...", tokens);
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Pool Account: {}", account_pubkey);
            
            // Create instruction data: discriminator + token_amount
            let mut instruction_data = vec![188, 116, 108, 23, 68, 33, 204, 220]; 
            instruction_data.extend_from_slice(&tokens.to_le_bytes());
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new(account_pubkey, false),    
                    AccountMeta::new(payer.pubkey(), true),           
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("🔄 Swap completed! Check logs for details.");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }

        SwapActions::GetPoolInfo { account_pubkey } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            
            println!("📊 Getting pool information...");
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Pool Account: {}", account_pubkey);
            
            let instruction_data = vec![9, 48, 220, 101, 22, 240, 78, 200]; // get_pool_info discriminator
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new_readonly(account_pubkey, false), // swap_pool
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("📊 Check the transaction logs for pool information!");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }

        SwapActions::Ping { account_pubkey, message } => {
            let account_pubkey = Pubkey::from_str(&account_pubkey)?;
            
            println!("🏓 Sending ping '{}' to Swap pool...", message);
            println!("📋 Program ID: {}", program_id);
            println!("🔑 Pool Account: {}", account_pubkey);
            
            // Create instruction data: discriminator + message
            let mut instruction_data = vec![228, 87, 187, 161, 115, 241, 73, 35]; 
            instruction_data.extend_from_slice(&(message.len() as u32).to_le_bytes());
            instruction_data.extend_from_slice(message.as_bytes());
            
            let instruction = Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new_readonly(account_pubkey, false), 
                ],
                data: instruction_data,
            };

            let recent_blockhash = rpc_client.get_latest_blockhash()?;
            let transaction = Transaction::new_signed_with_payer(
                &[instruction],
                Some(&payer.pubkey()),
                &[payer],
                recent_blockhash,
            );

            let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
            println!("✅ Transaction signature: {}", signature);
            println!("🏓 Ping sent! Check logs for pong response.");
            println!("🔍 Use: solana confirm -v {} --url devnet", signature);
        }
    }

    Ok(())
}

async fn handle_registry_command(
    program_registry: &mut ProgramRegistry,
    action: RegistryActions,
) -> Result<()> {
    match action {
        RegistryActions::List => {
            println!("📋 Program Registry - All Programs:");
            println!("=====================================");
            
            let programs = program_registry.list_programs();
            for (i, program) in programs.iter().enumerate() {
                println!("{}. {} ({})", i + 1, program.name, program.program_id);
                println!("   📝 Description: {}", program.description.as_deref().unwrap_or("None"));
                println!("   🔗 IDL URL: {}", program.idl_url);
                println!("   📦 Client: {} v{}", program.client_type, program.client_version);
                println!("   ⭐ Priority: {}/10", program.priority);
                println!("   ✅ Status: {}", if program.enabled { "Enabled" } else { "Disabled" });
                if let Some(metadata) = &program.metadata {
                    if let Some(category) = metadata.get("category") {
                        println!("   🏷️  Category: {}", category);
                    }
                }
                println!();
            }
        }
        
        RegistryActions::Stats => {
            let stats = program_registry.get_stats();
            println!("📊 Program Registry Statistics:");
            println!("===============================");
            println!("Total Programs: {}", stats.total_programs);
            println!("Enabled: {}", stats.enabled_programs);
            println!("Disabled: {}", stats.disabled_programs);
            println!("Last Updated: {}", stats.last_updated);
            println!("Cache TTL: {} seconds", stats.cache_ttl);
            println!("Auto Refresh: {}", if stats.auto_refresh { "Yes" } else { "No" });
        }
        
        RegistryActions::Refresh => {
            println!("🔄 Refreshing program registry...");
            program_registry.refresh().await?;
            println!("✅ Registry refreshed successfully!");
        }
        
        RegistryActions::Validate => {
            println!("🔍 Validating program registry...");
            match program_registry.validate() {
                Ok(_) => println!("✅ Registry validation passed!"),
                Err(e) => println!("❌ Registry validation failed: {}", e),
            }
        }
        
        RegistryActions::Add { program_id, name, idl_url, client_version, client_type, priority } => {
            println!("➕ Adding program to registry...");
            
            // Validate program ID
            let _: Pubkey = program_id.parse()
                .map_err(|_| anyhow::anyhow!("Invalid program ID: {}", program_id))?;
            
            let program = ProgramManifest {
                program_id: program_id.clone(),
                name: name.clone(),
                description: None,
                idl_url: idl_url.clone(),
                idl_hash: "".to_string(), // Will be calculated on refresh
                client_version: client_version.clone(),
                client_type: client_type.clone(),
                generated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                priority,
                enabled: true,
                metadata: Some(HashMap::from([
                    ("category".to_string(), "user".to_string()),
                    ("maintainer".to_string(), "user".to_string()),
                ])),
            };
            
            program_registry.add_program(program);
            program_registry.save_to_cache().await?;
            
            println!("✅ Program '{}' added to registry!", name);
        }
        
        RegistryActions::Remove { program_id } => {
            println!("🗑️  Removing program from registry...");
            
            if program_registry.remove_program(&program_id) {
                program_registry.save_to_cache().await?;
                println!("✅ Program '{}' removed from registry!", program_id);
            } else {
                println!("❌ Program '{}' not found in registry!", program_id);
            }
        }
        
        RegistryActions::Enable { program_id } => {
            println!("✅ Enabling program in registry...");
            
            if let Some(program) = program_registry.get_program(&program_id.parse()?) {
                let mut updated_program = program.clone();
                updated_program.enabled = true;
                program_registry.add_program(updated_program);
                program_registry.save_to_cache().await?;
                println!("✅ Program '{}' enabled!", program_id);
            } else {
                println!("❌ Program '{}' not found in registry!", program_id);
            }
        }
        
        RegistryActions::Disable { program_id } => {
            println!("❌ Disabling program in registry...");
            
            if let Some(program) = program_registry.get_program(&program_id.parse()?) {
                let mut updated_program = program.clone();
                updated_program.enabled = false;
                program_registry.add_program(updated_program);
                program_registry.save_to_cache().await?;
                println!("✅ Program '{}' disabled!", program_id);
            } else {
                println!("❌ Program '{}' not found in registry!", program_id);
            }
        }
    }

    Ok(())
}
