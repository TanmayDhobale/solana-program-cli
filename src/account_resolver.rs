use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Keypair;
use std::str::FromStr;

pub struct AccountResolver {
    rpc_client: RpcClient,
}

impl AccountResolver {
    pub fn new(rpc_client: RpcClient) -> Self {
        Self { rpc_client }
    }


    pub fn derive_pda(&self, seeds: &[&[u8]], program_id: &Pubkey) -> Result<(Pubkey, u8)> {
        let (pda, bump) = Pubkey::find_program_address(seeds, program_id);
        Ok((pda, bump))
    }

    
    pub fn derive_user_pda(&self, user: &Pubkey, program_id: &Pubkey, seed_prefix: &str) -> Result<(Pubkey, u8)> {
        let seeds = &[
            seed_prefix.as_bytes(),
            user.as_ref(),
        ];
        self.derive_pda(seeds, program_id)
    }

   
    pub fn get_minimum_rent(&self, account_size: usize) -> Result<u64> {
        let rent = self.rpc_client.get_minimum_balance_for_rent_exemption(account_size)?;
        Ok(rent)
    }

    
    pub fn account_exists(&self, address: &Pubkey) -> Result<bool> {
        match self.rpc_client.get_account(address) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

  
    pub fn get_balance(&self, address: &Pubkey) -> Result<u64> {
        let balance = self.rpc_client.get_balance(address)?;
        Ok(balance)
    }


    pub fn derive_ata(&self, owner: &Pubkey, mint: &Pubkey) -> Result<Pubkey> {

        let spl_token_program_id = Pubkey::from_str("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")?;
        let spl_associated_token_program_id = Pubkey::from_str("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL")?;

        let seeds = &[
            owner.as_ref(),
            spl_token_program_id.as_ref(),
            mint.as_ref(),
        ];

        let (ata, _bump) = self.derive_pda(seeds, &spl_associated_token_program_id)?;
        Ok(ata)
    }

    pub fn resolve_send_account(&self, user: &Pubkey) -> Result<SendAccountInfo> {
        let program_id = Pubkey::from_str("Bj4vH3tVu1GjCHeU3peRfYyxJpAzooyZCTU6rRFR4AnY")?;
        
       
        let (pda, bump) = self.derive_user_pda(user, &program_id, "send_account")?;
        
       
        let exists = self.account_exists(&pda)?;
        
       
        let min_rent = self.get_minimum_rent(56)?;
        
        Ok(SendAccountInfo {
            address: pda,
            bump,
            exists,
            required_rent: min_rent,
        })
    }

    pub fn resolve_swap_pool(&self, user: &Pubkey) -> Result<SwapPoolInfo> {
        let program_id = Pubkey::from_str("7JFPcs97cBb6bgfWiLsmA5Qpiv87oVA4Ue3TLinzNhxj")?;
        

        let (pda, bump) = self.derive_user_pda(user, &program_id, "swap_pool")?;
        
        
        let exists = self.account_exists(&pda)?;
        
            
        let min_rent = self.get_minimum_rent(66)?;
        
        Ok(SwapPoolInfo {
            address: pda,
            bump,
            exists,
            required_rent: min_rent,
        })
    }


    pub fn generate_deterministic_keypair(&self, user: &Pubkey, purpose: &str) -> Result<Keypair> {
 
        let mut seed = [0u8; 32];
        let user_bytes = user.to_bytes();
        let purpose_bytes = purpose.as_bytes();
        
       
        for (i, &byte) in user_bytes.iter().enumerate() {
            seed[i % 32] ^= byte;
        }
        for (i, &byte) in purpose_bytes.iter().enumerate() {
            seed[i % 32] ^= byte;
        }
        
        let keypair = Keypair::new_from_array(seed);
        Ok(keypair)
    }

    pub fn smart_resolve(&self, user: &Pubkey, program_type: &str) -> Result<AccountResolution> {
        match program_type {
            "send" => {
                let info = self.resolve_send_account(user)?;
                if info.exists {
                    Ok(AccountResolution::Found {
                        address: info.address,
                        account_type: "send".to_string(),
                    })
                } else {
                    Ok(AccountResolution::SuggestCreate {
                        address: info.address,
                        account_type: "send".to_string(),
                        required_rent: info.required_rent,
                        creation_method: "PDA derivation".to_string(),
                    })
                }
            }
            "swap" => {
                let info = self.resolve_swap_pool(user)?;
                if info.exists {
                    Ok(AccountResolution::Found {
                        address: info.address,
                        account_type: "swap_pool".to_string(),
                    })
                } else {
                    Ok(AccountResolution::SuggestCreate {
                        address: info.address,
                        account_type: "swap_pool".to_string(),
                        required_rent: info.required_rent,
                        creation_method: "PDA derivation".to_string(),
                    })
                }
            }
            _ => Err(anyhow::anyhow!("Unknown program type: {}", program_type))
        }
    }
}

#[derive(Debug)]
pub struct SendAccountInfo {
    pub address: Pubkey,
    pub bump: u8,
    pub exists: bool,
    pub required_rent: u64,
}

#[derive(Debug)]
pub struct SwapPoolInfo {
    pub address: Pubkey,
    pub bump: u8,
    pub exists: bool,
    pub required_rent: u64,
}

#[derive(Debug)]
pub enum AccountResolution {
    Found {
        address: Pubkey,
        account_type: String,
    },
    SuggestCreate {
        address: Pubkey,
        account_type: String,
        required_rent: u64,
        creation_method: String,
    },
}

impl AccountResolution {
    pub fn address(&self) -> &Pubkey {
        match self {
            AccountResolution::Found { address, .. } => address,
            AccountResolution::SuggestCreate { address, .. } => address,
        }
    }

    pub fn exists(&self) -> bool {
        matches!(self, AccountResolution::Found { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pda_derivation() {
        let program_id = Pubkey::new_unique();
        let user = Pubkey::new_unique();
        
        let resolver = AccountResolver::new(
            RpcClient::new("https://api.devnet.solana.com".to_string())
        );
        
        let (pda, bump) = resolver.derive_user_pda(&user, &program_id, "send").unwrap();
        
       
        assert_ne!(pda, user);
      
        assert!(bump < 256);
    }
}
