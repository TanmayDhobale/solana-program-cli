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

#[derive(Debug, Serialize, Deserialize)]
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

impl JupiterClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://quote-api.jup.ag/v6".to_string(),
        }
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

            let start = std::time::Instant::now();

            let quote_request = QuoteRequest {
                input_mint: input_mint.to_string(),
                output_mint: output_mint.to_string(),
                amount,
                slippage_bps: Some(*s),
                restrict_intermediate_tokens: Some(true),
                only_direct_routes: Some(false),
            };

            let mut quote = self.get_quote(quote_request).await?;

            println!("💱 Quote received:");
            println!("  📥 Input: {} {} tokens", quote.in_amount, input_mint);
            println!("  📤 Output: {} {} tokens", quote.out_amount, output_mint);
            println!("  💸 Price impact: {}%", quote.price_impact_pct);
            println!("  🛣️  Route uses {} DEXs:", quote.route_plan.len());
            for (i, route) in quote.route_plan.iter().enumerate() {
                println!("    {}. {} ({}%)", i + 1, route.swap_info.label, route.percent);
            }

           
            if start.elapsed().as_secs_f32() > 5.0 {
                println!("⏱️  Quote stale (>5s). Refreshing...");
                let refresh_request = QuoteRequest {
                    input_mint: input_mint.to_string(),
                    output_mint: output_mint.to_string(),
                    amount,
                    slippage_bps: Some(*s),
                    restrict_intermediate_tokens: Some(true),
                    only_direct_routes: Some(false),
                };
                quote = self.get_quote(refresh_request).await?;
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
