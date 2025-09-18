use anyhow::{anyhow, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::Signer,
    signer::keypair::Keypair,
    transaction::Transaction,
    program_pack::Pack,
};
use spl_associated_token_account::get_associated_token_address;
use spl_token::state::Account as TokenAccount;
use std::str::FromStr;


pub struct AtaManager {
    rpc_client: RpcClient,
}

#[derive(Debug)]
pub struct AtaInfo {
    pub address: Pubkey,
    pub exists: bool,
    pub mint: Pubkey,
    pub owner: Pubkey,
    pub balance: Option<u64>,
    pub rent_exemption_required: u64,
}

impl AtaManager {
    pub fn new(rpc_client: RpcClient) -> Self {
        Self { rpc_client }
    }

    pub async fn check_ata(&self, owner: &Pubkey, mint: &Pubkey) -> Result<AtaInfo> {
        let ata_address = get_associated_token_address(owner, mint);
        
       
        let account_info = self.rpc_client.get_account(&ata_address);
        
        let rent_exemption_required = self.rpc_client
            .get_minimum_balance_for_rent_exemption(TokenAccount::LEN)?;
        
        match account_info {
            Ok(account) => {
               
                if account.owner != spl_token::id() {
                    return Err(anyhow!(
                        "Account {} exists but is not owned by SPL Token program", 
                        ata_address
                    ));
                }
                
               
                let token_account = TokenAccount::unpack(&account.data)
                    .map_err(|e| anyhow!("Failed to parse token account data: {}", e))?;
                
               
                if token_account.mint != *mint {
                    return Err(anyhow!(
                        "ATA {} mint mismatch: expected {}, found {}",
                        ata_address, mint, token_account.mint
                    ));
                }
                
                if token_account.owner != *owner {
                    return Err(anyhow!(
                        "ATA {} owner mismatch: expected {}, found {}",
                        ata_address, owner, token_account.owner
                    ));
                }
                
                Ok(AtaInfo {
                    address: ata_address,
                    exists: true,
                    mint: *mint,
                    owner: *owner,
                    balance: Some(token_account.amount),
                    rent_exemption_required,
                })
            }
            Err(_) => {
               
                Ok(AtaInfo {
                    address: ata_address,
                    exists: false,
                    mint: *mint,
                    owner: *owner,
                    balance: None,
                    rent_exemption_required,
                })
            }
        }
    }

   
    pub fn create_ata_instruction(
        &self,
        payer: &Pubkey,
        owner: &Pubkey,
        mint: &Pubkey,
    ) -> Result<Instruction> {
        let _ata_address = get_associated_token_address(owner, mint);
        
       
       
        let instruction = spl_associated_token_account::instruction::create_associated_token_account(
            payer,    // Fee payer
            owner,    // Token account owner
            mint,     // Mint address
            &spl_token::id(), // SPL Token program ID
        );
        
        Ok(instruction)
    }

   
    pub async fn ensure_ata_exists(
        &self,
        payer: &Keypair,
        owner: &Pubkey,
        mint: &Pubkey,
    ) -> Result<AtaInfo> {
        let ata_info = self.check_ata(owner, mint).await?;
        
        if ata_info.exists {
            println!("✅ ATA already exists: {}", ata_info.address);
            println!("  💰 Balance: {} tokens", ata_info.balance.unwrap_or(0));
            return Ok(ata_info);
        }
        
        println!("🔧 ATA does not exist, creating: {}", ata_info.address);
        println!("  💰 Rent required: {} lamports ({} SOL)", 
                 ata_info.rent_exemption_required, 
                 ata_info.rent_exemption_required as f64 / 1_000_000_000.0);
        
       
        let payer_balance = self.rpc_client.get_balance(&payer.pubkey())?;
        if payer_balance < ata_info.rent_exemption_required {
            return Err(anyhow!(
                "Insufficient balance for ATA creation. Need {} lamports, have {}",
                ata_info.rent_exemption_required,
                payer_balance
            ));
        }
        
       
        let create_instruction = self.create_ata_instruction(&payer.pubkey(), owner, mint)?;
        
        let recent_blockhash = self.rpc_client.get_latest_blockhash()?;
        let transaction = Transaction::new_signed_with_payer(
            &[create_instruction],
            Some(&payer.pubkey()),
            &[payer],
            recent_blockhash,
        );
        
        let signature = self.rpc_client.send_and_confirm_transaction(&transaction)?;
        println!("✅ ATA created successfully!");
        println!("  🔍 Transaction: {}", signature);
        
       
        let updated_info = self.check_ata(owner, mint).await?;
        Ok(updated_info)
    }

   
    pub async fn check_multiple_atas(
        &self,
        owner: &Pubkey,
        mints: &[Pubkey],
    ) -> Result<Vec<AtaInfo>> {
        let mut results = Vec::new();
        
        for mint in mints {
            let ata_info = self.check_ata(owner, mint).await?;
            results.push(ata_info);
        }
        
        Ok(results)
    }

    
    pub fn get_common_mints() -> CommonMints {
        CommonMints::new()
    }
}


pub struct CommonMints;

impl CommonMints {
    pub fn new() -> Self {
        Self
    }
    
    pub fn sol() -> Pubkey {
       
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap()
    }
    
    pub fn usdc() -> Pubkey {
       
        Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v").unwrap()
    }
    
    pub fn usdt() -> Pubkey {
       
        Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB").unwrap()
    }
    
   
    pub fn from_name(name: &str) -> Result<Pubkey> {
        match name.to_uppercase().as_str() {
            "SOL" | "WSOL" => Ok(Self::sol()),
            "USDC" => Ok(Self::usdc()),
            "USDT" => Ok(Self::usdt()),
            _ => {
                        
                Pubkey::from_str(name)
                    .map_err(|_| anyhow!("Unknown token name or invalid pubkey: {}", name))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_common_mints() {
        let sol_mint = CommonMints::sol();
        assert_eq!(sol_mint.to_string(), "So11111111111111111111111111111111111111112");
        
        let usdc_mint = CommonMints::usdc();
        assert_eq!(usdc_mint.to_string(), "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
    }
    
    #[test]
    fn test_from_name() {
        assert!(CommonMints::from_name("SOL").is_ok());
        assert!(CommonMints::from_name("USDC").is_ok());
        assert!(CommonMints::from_name("INVALID").is_err());
    }
}
