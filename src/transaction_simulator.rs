use anyhow::Result;
use solana_client::rpc_client::RpcClient;
use solana_client::rpc_config::{RpcSimulateTransactionConfig, RpcSendTransactionConfig};
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::transaction::{Transaction, VersionedTransaction};
use std::collections::HashMap;

pub struct TransactionSimulator {
    rpc_client: RpcClient,
}

impl TransactionSimulator {
    pub fn new(rpc_client: RpcClient) -> Self {
        Self { rpc_client }
    }

   
    pub fn simulate_transaction(&self, transaction: &Transaction) -> Result<SimulationResult> {
        let config = RpcSimulateTransactionConfig {
            sig_verify: true,
            replace_recent_blockhash: true,
            commitment: Some(CommitmentConfig::processed()),
            encoding: None,
            accounts: None,
            min_context_slot: None,
            inner_instructions: true,
        };

        let response = self.rpc_client.simulate_transaction_with_config(transaction, config)?;

        let mut result = SimulationResult {
            success: response.value.err.is_none(),
            error_message: None,
            compute_units_consumed: 0,
            fee_estimate: 0,
            logs: response.value.logs.unwrap_or_default(),
            account_changes: HashMap::new(),
            warnings: Vec::new(),
        };

        // Extract error message if failed
        if let Some(err) = response.value.err {
            result.error_message = Some(format!("{:?}", err));
        }

        // Extract compute units consumed
        if let Some(units) = response.value.units_consumed {
            result.compute_units_consumed = units;
        }

        // Parse logs for useful information
        result.parse_logs();

        // Estimate fee (5000 lamports per signature + compute units)
        let signature_fee = transaction.signatures.len() as u64 * 5000;
        let compute_fee = (result.compute_units_consumed / 1000) * 100; // Rough estimate
        result.fee_estimate = signature_fee + compute_fee;

        Ok(result)
    }

   
    pub fn validate_transaction(&self, transaction: &Transaction) -> Result<ValidationResult> {
        let simulation = self.simulate_transaction(transaction)?;
        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Check for simulation failure
        if !simulation.success {
            let error_msg = simulation.error_message.as_ref()
                .map(|s| s.clone())
                .unwrap_or("Unknown error".to_string());
            issues.push(format!("Transaction would fail: {}", error_msg));
        }

        // Check compute units
        if simulation.compute_units_consumed > 200_000 {
            warnings.push("High compute usage - transaction may fail".to_string());
        } else if simulation.compute_units_consumed > 100_000 {
            warnings.push("Moderate compute usage".to_string());
        }

        // Check fee estimate
        if simulation.fee_estimate > 10_000 {
            warnings.push(format!("High transaction fee: {} lamports", simulation.fee_estimate));
        }

        // Parse logs for specific warnings
        for log in &simulation.logs {
            if log.contains("insufficient funds") {
                issues.push("Insufficient funds for transaction".to_string());
            }
            if log.contains("already in use") {
                issues.push("Account already exists or is in use".to_string());
            }
            if log.contains("unauthorized") {
                issues.push("Unauthorized signer or account access".to_string());
            }
            if log.contains("custom program error") {
                warnings.push("Program returned a custom error - check logs".to_string());
            }
        }

        let safe_to_send = issues.is_empty();

        Ok(ValidationResult {
            safe_to_send,
            issues,
            warnings,
            simulation,
        })
    }

   
    pub fn preview_transaction(&self, transaction: &Transaction) -> Result<TransactionPreview> {
        let simulation = self.simulate_transaction(transaction)?;
        
        let mut preview = TransactionPreview {
            will_succeed: simulation.success,
            estimated_fee: simulation.fee_estimate,
            compute_units: simulation.compute_units_consumed,
            account_changes: Vec::new(),
            program_logs: Vec::new(),
            error_summary: simulation.error_message.clone(),
        };

        // Extract program logs (excluding system logs)
        for log in &simulation.logs {
            if log.contains("Program log:") {
                let clean_log = log.replace("Program log: ", "");
                preview.program_logs.push(clean_log);
            }
        }

        // Analyze account changes from logs
        for log in &simulation.logs {
            if log.contains("balance:") {
                preview.account_changes.push(format!("Balance change detected: {}", log));
            }
            if log.contains("Allocate:") {
                preview.account_changes.push(format!("Account allocation: {}", log));
            }
        }

        Ok(preview)
    }

   
    pub fn safe_send_versioned_transaction(&self, transaction: &VersionedTransaction) -> Result<SafeSendResult> {
        println!("🔍 Simulating versioned transaction before sending...");
        
        let validation = self.validate_versioned_transaction(transaction)?;
        
        // Print validation results
        if !validation.safe_to_send {
            println!("❌ Transaction validation failed:");
            for issue in &validation.issues {
                println!("  🚨 {}", issue);
            }
            return Ok(SafeSendResult {
                sent: false,
                signature: None,
                validation_issues: validation.issues,
                simulation: validation.simulation,
            });
        }

        // Print warnings but continue
        if !validation.warnings.is_empty() {
            println!("⚠️  Transaction warnings:");
            for warning in &validation.warnings {
                println!("  ⚠️  {}", warning);
            }
        }

        // Print success preview
        println!("✅ Transaction simulation successful!");
        println!("💰 Estimated fee: {} lamports", validation.simulation.fee_estimate);
        println!("⚡ Compute units: {}", validation.simulation.compute_units_consumed);
        
        if !validation.simulation.logs.is_empty() {
            println!("📋 Expected program logs:");
            for log in &validation.simulation.logs {
                if log.contains("Program log:") {
                    println!("  📝 {}", log.replace("Program log: ", ""));
                }
            }
        }

        println!("🚀 Sending versioned transaction to blockchain...");

        // Send the transaction
        match self.rpc_client.send_and_confirm_transaction(transaction) {
            Ok(signature) => {
                println!("✅ Transaction confirmed: {}", signature);
                Ok(SafeSendResult {
                    sent: true,
                    signature: Some(signature),
                    validation_issues: Vec::new(),
                    simulation: validation.simulation,
                })
            }
            Err(e) => {
                println!("❌ Transaction failed to send: {}", e);
                Ok(SafeSendResult {
                    sent: false,
                    signature: None,
                    validation_issues: vec![format!("Send failed: {}", e)],
                    simulation: validation.simulation,
                })
            }
        }
    }

   
    pub fn send_versioned_transaction_direct(&self, transaction: &VersionedTransaction) -> Result<SafeSendResult> {
        println!("🚀 Sending versioned transaction directly to blockchain (skipping simulation)...");
        println!("ℹ️  Simulation skipped due to Address Lookup Tables not available on local RPC");

       
        println!("🔍 Attempting to send transaction to RPC...");
        match self.rpc_client.send_transaction_with_config(
            transaction, 
            RpcSendTransactionConfig {
                skip_preflight: true,
                max_retries: Some(3),
                ..Default::default()
            }
        ) {
            Ok(signature) => {
                println!("📤 Transaction submitted: {}", signature);
                println!("⏳ Waiting for confirmation...");
                
               
                match self.rpc_client.confirm_transaction(&signature) {
                    Ok(_) => {
                        println!("✅ Transaction confirmed: {}", signature);
                        Ok(SafeSendResult {
                            sent: true,
                            signature: Some(signature),
                            validation_issues: Vec::new(),
                            simulation: SimulationResult {
                                success: true,
                                logs: vec!["Direct send and confirmation successful (simulation skipped)".to_string()],
                                compute_units_consumed: 0,
                                fee_estimate: 0,
                                error_message: None,
                                account_changes: HashMap::new(),
                                warnings: Vec::new(),
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
                                logs: vec!["Direct send successful but confirmation failed (simulation skipped)".to_string()],
                                compute_units_consumed: 0,
                                fee_estimate: 0,
                                error_message: Some(confirm_err.to_string()),
                                account_changes: HashMap::new(),
                                warnings: Vec::new(),
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
                        logs: vec!["Direct send failed (simulation skipped)".to_string()],
                        compute_units_consumed: 0,
                        fee_estimate: 0,
                        error_message: Some(e.to_string()),
                        account_changes: HashMap::new(),
                        warnings: Vec::new(),
                    },
                })
            }
        }
    }

   
    pub fn validate_versioned_transaction(&self, transaction: &VersionedTransaction) -> Result<ValidationResult> {
        let simulation = self.simulate_versioned_transaction(transaction)?;
        let mut issues = Vec::new();
        let mut warnings = Vec::new();

        // Check for simulation failure
        if !simulation.success {
            let error_msg = simulation.error_message.as_ref()
                .map(|s| s.clone())
                .unwrap_or("Unknown error".to_string());
            issues.push(format!("Transaction would fail: {}", error_msg));
        }

        // Check compute units
        if simulation.compute_units_consumed > 200_000 {
            warnings.push("High compute usage - transaction may fail".to_string());
        } else if simulation.compute_units_consumed > 100_000 {
            warnings.push("Moderate compute usage".to_string());
        }

        // Check fee estimate
        if simulation.fee_estimate > 10_000 {
            warnings.push(format!("High transaction fee: {} lamports", simulation.fee_estimate));
        }

        // Parse logs for specific warnings
        for log in &simulation.logs {
            if log.contains("insufficient funds") {
                issues.push("Insufficient funds for transaction".to_string());
            }
            if log.contains("already in use") {
                issues.push("Account already exists or is in use".to_string());
            }
            if log.contains("unauthorized") {
                issues.push("Unauthorized signer or account access".to_string());
            }
            if log.contains("custom program error") {
                warnings.push("Program returned a custom error - check logs".to_string());
            }
        }

        let safe_to_send = issues.is_empty();

        Ok(ValidationResult {
            safe_to_send,
            issues,
            warnings,
            simulation,
        })
    }

   
    pub fn simulate_versioned_transaction(&self, transaction: &VersionedTransaction) -> Result<SimulationResult> {
        let config = RpcSimulateTransactionConfig {
            sig_verify: false, // Can't use with replace_recent_blockhash
            replace_recent_blockhash: true,
            commitment: Some(CommitmentConfig::processed()),
            encoding: None,
            accounts: None,
            min_context_slot: None,
            inner_instructions: true,
        };

        let response = self.rpc_client.simulate_transaction_with_config(transaction, config)?;

        let mut result = SimulationResult {
            success: response.value.err.is_none(),
            error_message: None,
            compute_units_consumed: 0,
            fee_estimate: 0,
            logs: response.value.logs.unwrap_or_default(),
            account_changes: HashMap::new(),
            warnings: Vec::new(),
        };

        // Extract error message if failed
        if let Some(err) = response.value.err {
            result.error_message = Some(format!("{:?}", err));
        }

        // Extract compute units consumed
        if let Some(units) = response.value.units_consumed {
            result.compute_units_consumed = units;
        }

        // Parse logs for useful information
        result.parse_logs();

        // Estimate fee (5000 lamports per signature + compute units)
        let signature_fee = transaction.signatures.len() as u64 * 5000;
        let compute_fee = (result.compute_units_consumed / 1000) * 100; // Rough estimate
        result.fee_estimate = signature_fee + compute_fee;

        Ok(result)
    }

   
    pub fn safe_send_transaction(&self, transaction: &Transaction) -> Result<SafeSendResult> {
        println!("🔍 Simulating transaction before sending...");
        
        let validation = self.validate_transaction(transaction)?;
        
        // Print validation results
        if !validation.safe_to_send {
            println!("❌ Transaction validation failed:");
            for issue in &validation.issues {
                println!("  🚨 {}", issue);
            }
            return Ok(SafeSendResult {
                sent: false,
                signature: None,
                validation_issues: validation.issues,
                simulation: validation.simulation,
            });
        }

        // Print warnings but continue
        if !validation.warnings.is_empty() {
            println!("⚠️  Transaction warnings:");
            for warning in &validation.warnings {
                println!("  ⚠️  {}", warning);
            }
        }

        // Print success preview
        println!("✅ Transaction simulation successful!");
        println!("💰 Estimated fee: {} lamports", validation.simulation.fee_estimate);
        println!("⚡ Compute units: {}", validation.simulation.compute_units_consumed);
        
        if !validation.simulation.logs.is_empty() {
            println!("📋 Expected program logs:");
            for log in &validation.simulation.logs {
                if log.contains("Program log:") {
                    println!("  📝 {}", log.replace("Program log: ", ""));
                }
            }
        }

        println!("🚀 Sending transaction to blockchain...");

        // Send the transaction
        match self.rpc_client.send_and_confirm_transaction(transaction) {
            Ok(signature) => {
                println!("✅ Transaction confirmed: {}", signature);
                Ok(SafeSendResult {
                    sent: true,
                    signature: Some(signature),
                    validation_issues: Vec::new(),
                    simulation: validation.simulation,
                })
            }
            Err(e) => {
                println!("❌ Transaction failed to send: {}", e);
                Ok(SafeSendResult {
                    sent: false,
                    signature: None,
                    validation_issues: vec![format!("Send failed: {}", e)],
                    simulation: validation.simulation,
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct SimulationResult {
    pub success: bool,
    pub error_message: Option<String>,
    pub compute_units_consumed: u64,
    pub fee_estimate: u64,
    pub logs: Vec<String>,
    pub account_changes: HashMap<String, String>,
    pub warnings: Vec<String>,
}

impl SimulationResult {
    fn parse_logs(&mut self) {

        // Extract useful information from logs
        for log in &self.logs {
            if log.contains("insufficient") {
                self.warnings.push("Insufficient funds detected".to_string());
            }
            if log.contains("exceed") {
                self.warnings.push("Limit exceeded".to_string());
            }
        }
    }

    pub fn is_success(&self) -> bool {
        self.success
    }

    pub fn get_error_message(&self) -> Option<&String> {
        self.error_message.as_ref()
    }
}

#[derive(Debug)]
pub struct ValidationResult {
    pub safe_to_send: bool,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
    pub simulation: SimulationResult,
}

#[derive(Debug)]
pub struct TransactionPreview {
    pub will_succeed: bool,
    pub estimated_fee: u64,
    pub compute_units: u64,
    pub account_changes: Vec<String>,
    pub program_logs: Vec<String>,
    pub error_summary: Option<String>,
}

#[derive(Debug)]
pub struct SafeSendResult {
    pub sent: bool,
    pub signature: Option<solana_sdk::signature::Signature>,
    pub validation_issues: Vec<String>,
    pub simulation: SimulationResult,
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::signature::Keypair;
    use solana_sdk::system_instruction;

    #[test]
    fn test_simulation_result() {
        let result = SimulationResult {
            success: true,
            error_message: None,
            compute_units_consumed: 1000,
            fee_estimate: 5000,
            logs: vec!["Program log: Test".to_string()],
            account_changes: HashMap::new(),
            warnings: Vec::new(),
        };

        assert!(result.is_success());
        assert!(result.get_error_message().is_none());
    }
}
