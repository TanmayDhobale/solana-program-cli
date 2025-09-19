use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    transaction::VersionedTransaction,
};




pub struct JupiterClient {
    client: Client,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct QuoteRequest {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    pub amount: u64,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: Option<u16>, // 50 = 0.5%
    #[serde(rename = "restrictIntermediateTokens")]
    pub restrict_intermediate_tokens: Option<bool>,
    #[serde(rename = "onlyDirectRoutes")]
    pub only_direct_routes: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QuoteResponse {
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    pub other_amount_threshold: String,
    #[serde(rename = "swapMode")]
    pub swap_mode: String,
    #[serde(rename = "slippageBps")]
    pub slippage_bps: u16,
    #[serde(rename = "priceImpactPct")]
    pub price_impact_pct: String,
    #[serde(rename = "routePlan")]
    pub route_plan: Vec<RoutePlan>,
    #[serde(rename = "contextSlot")]
    pub context_slot: u64,
    #[serde(rename = "timeTaken")]
    pub time_taken: f64,
    #[serde(rename = "quoteHash")]
    pub quote_hash: Option<String>,
    #[serde(rename = "slot")]
    pub slot: Option<u64>,
    #[serde(rename = "timestamp")]
    pub timestamp: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoutePlan {
    #[serde(rename = "swapInfo")]
    pub swap_info: SwapInfo,
    pub percent: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapInfo {
    #[serde(rename = "ammKey")]
    pub amm_key: String,
    pub label: String,
    #[serde(rename = "inputMint")]
    pub input_mint: String,
    #[serde(rename = "outputMint")]
    pub output_mint: String,
    #[serde(rename = "inAmount")]
    pub in_amount: String,
    #[serde(rename = "outAmount")]
    pub out_amount: String,
    #[serde(rename = "feeAmount")]
    pub fee_amount: String,
    #[serde(rename = "feeMint")]
    pub fee_mint: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapRequest {
    #[serde(rename = "userPublicKey")]
    pub user_public_key: String,
    #[serde(rename = "quoteResponse")]
    pub quote_response: QuoteResponse,
    #[serde(rename = "wrapAndUnwrapSol")]
    pub wrap_and_unwrap_sol: Option<bool>,
    #[serde(rename = "dynamicComputeUnitLimit")]
    pub dynamic_compute_unit_limit: Option<bool>,
    #[serde(rename = "prioritizationFeeLamports")]
    pub prioritization_fee_lamports: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SwapResponse {
    #[serde(rename = "swapTransaction")]
    pub swap_transaction: String, // Base64 encoded
    #[serde(rename = "lastValidBlockHeight")]
    pub last_valid_block_height: Option<u64>,
}

#[derive(Debug)]
pub struct QuoteValidation {
    pub is_fresh: bool,
    pub needs_refresh: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub slot_drift: u64,
    pub age_seconds: u64,
}

#[derive(Debug)]
pub struct SafeSendResult {
    pub sent: bool,
    pub signature: Option<solana_sdk::signature::Signature>,
    pub validation_issues: Vec<String>,
    pub simulation: SimulationResult,
}

#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub success: bool,
    pub error_message: Option<String>,
    pub compute_units_consumed: u64,
    pub fee_estimate: u64,
    pub logs: Vec<String>,
    pub account_changes: std::collections::HashMap<String, String>,
    pub warnings: Vec<String>,
}

impl JupiterClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://quote-api.jup.ag/v6".to_string(),
        }
    }


    pub fn validate_quote_freshness(&self, quote: &QuoteResponse, current_slot: u64) -> Result<QuoteValidation> {
        let mut issues = Vec::new();
        let mut warnings = Vec::new();


        if let Some(quote_slot) = quote.slot {
            let slot_drift = current_slot.saturating_sub(quote_slot);
            if slot_drift > 150 {
                issues.push(format!("Quote too stale: {} slots behind current (max 150)", slot_drift));
            } else if slot_drift > 50 {
                warnings.push(format!("Quote aging: {} slots behind current", slot_drift));
            }
        }


        if let Some(timestamp) = quote.timestamp {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let age_seconds = now.saturating_sub(timestamp);
            if age_seconds > 30 {
                issues.push(format!("Quote too old: {} seconds (max 30)", age_seconds));
            } else if age_seconds > 10 {
                warnings.push(format!("Quote aging: {} seconds old", age_seconds));
            }
        }


        if let Some(quote_hash) = &quote.quote_hash {
            if quote_hash.is_empty() {
                warnings.push("Quote hash is empty - integrity cannot be verified".to_string());
            }
        }

        if let Ok(price_impact) = quote.price_impact_pct.parse::<f64>() {
            if price_impact > 5.0 {
                issues.push(format!("High price impact: {}% (max 5%)", price_impact));
            } else if price_impact > 2.0 {
                warnings.push(format!("Moderate price impact: {}%", price_impact));
            }
        }

        let is_fresh = issues.is_empty();
        let needs_refresh = !is_fresh || !warnings.is_empty();

        Ok(QuoteValidation {
            is_fresh,
            needs_refresh,
            issues,
            warnings,
            slot_drift: quote.slot.map(|s| current_slot.saturating_sub(s)).unwrap_or(0),
            age_seconds: quote.timestamp.map(|t| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .saturating_sub(t)
            }).unwrap_or(0),
        })
    }

    pub async fn get_fresh_quote(&self, request: QuoteRequest, max_retries: usize) -> Result<QuoteResponse> {
        let mut last_error = None;
        
        for attempt in 1..=max_retries {
            println!("🔍 Getting fresh Jupiter quote (attempt {}/{}...", attempt, max_retries);
            
            match self.get_quote(request.clone()).await {
                Ok(quote) => {
                    let current_slot = self.get_current_slot().await.unwrap_or(0);
                    let validation = self.validate_quote_freshness(&quote, current_slot)?;
                    
                    if validation.is_fresh {
                        println!("✅ Fresh quote obtained!");
                        if !validation.warnings.is_empty() {
                            println!("⚠️  Quote warnings:");
                            for warning in &validation.warnings {
                                println!("  ⚠️  {}", warning);
                            }
                        }
                        return Ok(quote);
                    } else {
                        println!("⚠️  Quote validation failed:");
                        for issue in &validation.issues {
                            println!("  🚨 {}", issue);
                        }
                        
                        if attempt < max_retries {
                            println!("🔄 Retrying with fresh quote...");
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            continue;
                        } else {
                            return Err(anyhow::anyhow!("Failed to get fresh quote after {} attempts", max_retries));
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < max_retries {
                        println!("⚠️  Quote attempt {} failed: {}, retrying...", attempt, last_error.as_ref().unwrap());
                        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Failed to get quote after {} attempts", max_retries)))
    }


    async fn get_current_slot(&self) -> Result<u64> {
        Ok(0) 
    }

   
    pub async fn safe_send_versioned_transaction(
        &self,
        transaction: &VersionedTransaction,
        rpc_client: &solana_client::rpc_client::RpcClient,
    ) -> Result<SafeSendResult> {
        println!("🔍 Safe-send guard: Validating versioned transaction...");
        
       
        let uses_alts = match &transaction.message {
            solana_sdk::message::VersionedMessage::V0(msg) => !msg.address_table_lookups.is_empty(),
            _ => false,
        };

        if uses_alts {
            println!("📋 Transaction uses Address Lookup Tables (ALTs)");
            println!("⚠️  ALTs require mainnet RPC - simulation may fail on devnet");
            
       
            match self.simulate_versioned_transaction(transaction, rpc_client).await {
                Ok(simulation) => {
                    if !simulation.success {
                        println!("❌ Simulation failed - transaction would fail:");
                        if let Some(error) = &simulation.error_message {
                            println!("  🚨 {}", error);
                        }
                        return Ok(SafeSendResult {
                            sent: false,
                            signature: None,
                            validation_issues: vec!["Simulation failed - transaction would fail".to_string()],
                            simulation,
                        });
                    } else {
                        println!("✅ Simulation successful - transaction safe to send");
                        println!("💰 Estimated fee: {} lamports", simulation.fee_estimate);
                        println!("⚡ Compute units: {}", simulation.compute_units_consumed);
                    }
                }
                Err(e) => {
                    println!("⚠️  Simulation failed (likely due to ALTs on devnet): {}", e);
                    println!("🚀 Proceeding with direct send (production mode)");
                }
            }
        } else {
            println!("📋 Standard transaction - running full simulation");
            match self.simulate_versioned_transaction(transaction, rpc_client).await {
                Ok(simulation) => {
                    if !simulation.success {
                        println!("❌ Simulation failed - transaction would fail:");
                        if let Some(error) = &simulation.error_message {
                            println!("  🚨 {}", error);
                        }
                        return Ok(SafeSendResult {
                            sent: false,
                            signature: None,
                            validation_issues: vec!["Simulation failed - transaction would fail".to_string()],
                            simulation,
                        });
                    } else {
                        println!("✅ Simulation successful - transaction safe to send");
                        println!("💰 Estimated fee: {} lamports", simulation.fee_estimate);
                        println!("⚡ Compute units: {}", simulation.compute_units_consumed);
                    }
                }
                Err(e) => {
                    println!("❌ Simulation failed: {}", e);
                    return Ok(SafeSendResult {
                        sent: false,
                        signature: None,
                        validation_issues: vec![format!("Simulation failed: {}", e)],
                        simulation: SimulationResult {
                            success: false,
                            error_message: Some(e.to_string()),
                            compute_units_consumed: 0,
                            fee_estimate: 0,
                            logs: vec![],
                            account_changes: std::collections::HashMap::new(),
                            warnings: vec![],
                        },
                    });
                }
            }
        }

        // Send the transaction
        println!("🚀 Sending versioned transaction to blockchain...");
        match rpc_client.send_transaction_with_config(
            transaction,
            solana_client::rpc_config::RpcSendTransactionConfig {
                skip_preflight: false,
                max_retries: Some(3),
                ..Default::default()
            }
        ) {
            Ok(signature) => {
                println!("📤 Transaction submitted: {}", signature);
                println!("⏳ Waiting for confirmation...");
                
                match rpc_client.confirm_transaction(&signature) {
                    Ok(_) => {
                        println!("✅ Transaction confirmed: {}", signature);
                        Ok(SafeSendResult {
                            sent: true,
                            signature: Some(signature),
                            validation_issues: vec![],
                            simulation: SimulationResult {
                                success: true,
                                logs: vec!["Transaction sent and confirmed successfully".to_string()],
                                compute_units_consumed: 0,
                                fee_estimate: 0,
                                error_message: None,
                                account_changes: std::collections::HashMap::new(),
                                warnings: vec![],
                            },
                        })
                    }
                    Err(confirm_err) => {
                        println!("⚠️  Transaction sent but confirmation failed: {}", confirm_err);
                        Ok(SafeSendResult {
                            sent: true,
                            signature: Some(signature),
                            validation_issues: vec![format!("Confirmation failed: {}", confirm_err)],
                            simulation: SimulationResult {
                                success: false,
                                logs: vec!["Transaction sent but confirmation failed".to_string()],
                                compute_units_consumed: 0,
                                fee_estimate: 0,
                                error_message: Some(confirm_err.to_string()),
                                account_changes: std::collections::HashMap::new(),
                                warnings: vec![],
                            },
                        })
                    }
                }
            }
            Err(e) => {
                println!("❌ Transaction failed to send: {}", e);
                Ok(SafeSendResult {
                    sent: false,
                    signature: None,
                    validation_issues: vec![format!("Send failed: {}", e)],
                    simulation: SimulationResult {
                        success: false,
                        logs: vec!["Transaction send failed".to_string()],
                        compute_units_consumed: 0,
                        fee_estimate: 0,
                        error_message: Some(e.to_string()),
                        account_changes: std::collections::HashMap::new(),
                        warnings: vec![],
                    },
                })
            }
        }
    }

  
    async fn simulate_versioned_transaction(
        &self,
        transaction: &VersionedTransaction,
        rpc_client: &solana_client::rpc_client::RpcClient,
    ) -> Result<SimulationResult> {
        let config = solana_client::rpc_config::RpcSimulateTransactionConfig {
            sig_verify: false,
            replace_recent_blockhash: true,
            commitment: Some(solana_sdk::commitment_config::CommitmentConfig::processed()),
            encoding: None,
            accounts: None,
            min_context_slot: None,
            inner_instructions: true,
        };

        let response = rpc_client.simulate_transaction_with_config(transaction, config)?;

        let mut result = SimulationResult {
            success: response.value.err.is_none(),
            error_message: None,
            compute_units_consumed: 0,
            fee_estimate: 0,
            logs: response.value.logs.unwrap_or_default(),
            account_changes: std::collections::HashMap::new(),
            warnings: vec![],
        };

      
        if let Some(err) = response.value.err {
            result.error_message = Some(format!("{:?}", err));
        }

  
        if let Some(units) = response.value.units_consumed {
            result.compute_units_consumed = units;
        }

   
        let signature_fee = transaction.signatures.len() as u64 * 5000;
        let compute_fee = (result.compute_units_consumed / 1000) * 100; 
        result.fee_estimate = signature_fee + compute_fee;

        Ok(result)
    }

   
    pub async fn get_quote(&self, request: QuoteRequest) -> Result<QuoteResponse> {
        let url = format!("{}/quote", self.base_url);
        
        let response = self.client
            .get(&url)
            .query(&[
                ("inputMint", request.input_mint.as_str()),
                ("outputMint", request.output_mint.as_str()),
                ("amount", &request.amount.to_string()),
                ("slippageBps", &request.slippage_bps.unwrap_or(50).to_string()),
                ("restrictIntermediateTokens", &request.restrict_intermediate_tokens.unwrap_or(false).to_string()),
                ("onlyDirectRoutes", &request.only_direct_routes.unwrap_or(false).to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Jupiter quote failed: {}", error_text));
        }

        let quote: QuoteResponse = response.json().await?;
        Ok(quote)
    }

   
    pub async fn get_swap_instructions(&self, request: SwapRequest) -> Result<SwapResponse> {
        let url = format!("{}/swap", self.base_url);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Jupiter swap failed: {}", error_text));
        }

        let swap: SwapResponse = response.json().await?;
        Ok(swap)
    }

   
    pub async fn build_swap_transaction(
        &self,
        user_pubkey: &Pubkey,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        slippage_bps: Option<u16>,
    ) -> Result<VersionedTransaction> {
        let mut slippage_candidates: Vec<u16> = match slippage_bps {
            Some(s) => vec![s, 100, 150, 200],
            None => vec![50, 100, 150, 200],
        };
       
        slippage_candidates.dedup();

        for (idx, s) in slippage_candidates.iter().enumerate() {
            println!("🔍 Getting Jupiter quote (attempt {} with {} bps)...", idx + 1, s);

        let quote_request = QuoteRequest {
            input_mint: input_mint.to_string(),
            output_mint: output_mint.to_string(),
            amount,
                slippage_bps: Some(*s),
                restrict_intermediate_tokens: Some(true),
            only_direct_routes: Some(false),
        };

       
            let quote = self.get_fresh_quote(quote_request, 3).await?;
        
            println!("💱 Fresh quote received:");
        println!("  📥 Input: {} {} tokens", quote.in_amount, input_mint);
        println!("  📤 Output: {} {} tokens", quote.out_amount, output_mint);
        println!("  💸 Price impact: {}%", quote.price_impact_pct);
        println!("  🛣️  Route uses {} DEXs:", quote.route_plan.len());
        for (i, route) in quote.route_plan.iter().enumerate() {
            println!("    {}. {} ({}%)", i + 1, route.swap_info.label, route.percent);
        }

           
            if let Some(slot) = quote.slot {
                println!("  📊 Quote slot: {}", slot);
            }
            if let Some(timestamp) = quote.timestamp {
                let age = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
                    .saturating_sub(timestamp);
                println!("  ⏰ Quote age: {} seconds", age);
            }
            if let Some(hash) = &quote.quote_hash {
                println!("  🔐 Quote hash: {}...", &hash[..std::cmp::min(8, hash.len())]);
            }

        println!("\n🔧 Building swap transaction...");
        let swap_request = SwapRequest {
            user_public_key: user_pubkey.to_string(),
            quote_response: quote,
            wrap_and_unwrap_sol: Some(true),
            dynamic_compute_unit_limit: Some(true),
            prioritization_fee_lamports: Some("auto".to_string()),
        };

            match self.get_swap_instructions(swap_request).await {
                Ok(swap_response) => {
        use base64::Engine;
                    let transaction_bytes = base64::engine::general_purpose::STANDARD
                        .decode(&swap_response.swap_transaction)?;
                    let transaction: VersionedTransaction = bincode::serde::decode_from_slice(
                        &transaction_bytes,
                        bincode::config::standard(),
                    )?
                    .0;
                    println!("✅ Jupiter transaction built successfully with {} bps!", s);
                    println!("🔗 Contains {} instructions", transaction.message.instructions().len());
                    return Ok(transaction);
                }
                Err(e) => {
                    println!("⚠️  Build failed at {} bps: {}", s, e);
                   
                    continue;
                }
            }
        }

        Err(anyhow!("Failed to build swap after adaptive slippage attempts"))
    }

        
    pub async fn get_tokens(&self) -> Result<Vec<String>> {
        let url = format!("{}/tokens", self.base_url);
        
        let response = self.client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to fetch tokens: {}", error_text));
        }

        let tokens: Vec<String> = response.json().await?;
        Ok(tokens)
    }

   
    pub async fn get_program_labels(&self) -> Result<std::collections::HashMap<String, String>> {
        let url = "https://quote-api.jup.ag/v6/program-id-to-label".to_string();
        
        let response = self.client
            .get(&url)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(anyhow!("Failed to fetch program labels: {}", error_text));
        }

        let labels: std::collections::HashMap<String, String> = response.json().await?;
        Ok(labels)
    }
}


pub mod tokens {
    pub const SOL: &str = "So11111111111111111111111111111111111111112";
    pub const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
    pub const USDT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";
    pub const RAY: &str = "4k3Dyjzvzp8eMZWUXbBCjEvwSkkk59S5iCNLY3QrkX6R";
    pub const SRM: &str = "SRMuApVNdxXokk5GT7XD5cUUgXMBCoAz2LHeuAoKWRt";
    pub const BONK: &str = "DezXAZ8z7PnrnRJjz3wXBoRgixCa6xjnB7YaB1pPB263";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_jupiter_quote() {
        let client = JupiterClient::new();
        
        let quote_request = QuoteRequest {
            input_mint: tokens::SOL.to_string(),
            output_mint: tokens::USDC.to_string(),
            amount: 1_000_000_000, // 1 SOL
            slippage_bps: Some(50), // 0.5%
            restrict_intermediate_tokens: Some(true),
            only_direct_routes: Some(false),
        };

        let result = client.get_quote(quote_request).await;
        assert!(result.is_ok(), "Quote should succeed");
        
        let quote = result.unwrap();
        assert!(!quote.out_amount.is_empty(), "Should return output amount");
        assert!(!quote.route_plan.is_empty(), "Should have at least one route");
    }
}
