use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
use sha2::{Sha256, Digest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProgramRoute {
    GeneratedClient(String), // Client name/version
    Dynamic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramManifest {
    pub program_id: String,
    pub name: String,
    pub description: Option<String>,
    pub idl_url: String,
    pub idl_hash: String, // SHA256 hash for integrity
    pub client_version: String,
    pub client_type: String, // "rust", "js", "python"
    pub generated_at: u64, // Unix timestamp
    pub last_updated: u64,
    pub priority: u8, // 1-10, higher = more important
    pub enabled: bool,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryManifest {
    pub version: String,
    pub created_at: u64,
    pub last_updated: u64,
    pub signature: Option<String>, // Digital signature for verification
    pub programs: Vec<ProgramManifest>,
    pub cache_ttl: u64, // Cache time-to-live in seconds
    pub auto_refresh: bool,
}

pub struct ProgramRegistry {
    manifest: RegistryManifest,
    cache_path: String,
    last_refresh: SystemTime,
    programs: HashMap<String, ProgramManifest>,
}

impl ProgramRegistry {
    /// Create a new program registry with default manifest
    pub fn new(cache_path: &str) -> Self {
        let default_manifest = RegistryManifest {
            version: "1.0.0".to_string(),
            created_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
            signature: None,
            programs: vec![
                // Default programs with hardcoded fallbacks
                ProgramManifest {
                    program_id: "Bj4vH3tVu1GjCHeU3peRfYyxJpAzooyZCTU6rRFR4AnY".to_string(),
                    name: "send_program".to_string(),
                    description: Some("Send SOL program with PDA support".to_string()),
                    idl_url: "file://./send_program.json".to_string(),
                    idl_hash: "".to_string(), // Will be calculated
                    client_version: "1.0.0".to_string(),
                    client_type: "rust".to_string(),
                    generated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    priority: 9,
                    enabled: true,
                    metadata: Some(HashMap::from([
                        ("category".to_string(), "core".to_string()),
                        ("maintainer".to_string(), "solana-program-cli".to_string()),
                    ])),
                },
                ProgramManifest {
                    program_id: "5PiuXarsz2F7Q6NpSCtdBbK6vroQWiGSdJZW3fPkjWHw".to_string(),
                    name: "hello_world".to_string(),
                    description: Some("Hello World program for testing".to_string()),
                    idl_url: "file://./hello_world.json".to_string(),
                    idl_hash: "".to_string(),
                    client_version: "1.0.0".to_string(),
                    client_type: "rust".to_string(),
                    generated_at: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    last_updated: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs(),
                    priority: 5,
                    enabled: true,
                    metadata: Some(HashMap::from([
                        ("category".to_string(), "example".to_string()),
                        ("maintainer".to_string(), "solana-program-cli".to_string()),
                    ])),
                },
            ],
            cache_ttl: 3600, // 1 hour
            auto_refresh: true,
        };

        let mut registry = Self {
            manifest: default_manifest,
            cache_path: cache_path.to_string(),
            last_refresh: SystemTime::now(),
            programs: HashMap::new(),
        };

        // Build program lookup map
        for program in &registry.manifest.programs {
            registry.programs.insert(program.program_id.clone(), program.clone());
        }

        registry
    }

    /// Load registry from cache or create new one
    pub async fn load_or_create(cache_path: &str) -> Result<Self> {
        let cache_file = format!("{}/program_registry.json", cache_path);
        
        if Path::new(&cache_file).exists() {
            match Self::load_from_cache(&cache_file).await {
                Ok(registry) => {
                    println!("✅ Loaded program registry from cache");
                    return Ok(registry);
                }
                Err(e) => {
                    println!("⚠️  Failed to load cache, creating new registry: {}", e);
                }
            }
        }

        println!("🔧 Creating new program registry");
        Ok(Self::new(cache_path))
    }

    /// Load registry from cache file
    async fn load_from_cache(cache_file: &str) -> Result<Self> {
        let content = fs::read_to_string(cache_file).await?;
        let manifest: RegistryManifest = serde_json::from_str(&content)?;
        
        let mut registry = Self {
            manifest,
            cache_path: Path::new(cache_file).parent().unwrap().to_string_lossy().to_string(),
            last_refresh: SystemTime::now(),
            programs: HashMap::new(),
        };

        // Build program lookup map
        for program in &registry.manifest.programs {
            registry.programs.insert(program.program_id.clone(), program.clone());
        }

        Ok(registry)
    }

    /// Save registry to cache
    pub async fn save_to_cache(&self) -> Result<()> {
        let cache_file = format!("{}/program_registry.json", self.cache_path);
        let content = serde_json::to_string_pretty(&self.manifest)?;
        fs::write(cache_file, content).await?;
        println!("💾 Program registry saved to cache");
        Ok(())
    }

    /// Resolve program route with enhanced logic
    pub fn resolve(&self, program_id: &Pubkey) -> ProgramRoute {
        let program_id_str = program_id.to_string();
        
        if let Some(program) = self.programs.get(&program_id_str) {
            if program.enabled {
                return ProgramRoute::GeneratedClient(format!("{}-{}", program.name, program.client_version));
            }
        }
        
        ProgramRoute::Dynamic
    }

    /// Get program manifest by ID
    pub fn get_program(&self, program_id: &Pubkey) -> Option<&ProgramManifest> {
        self.programs.get(&program_id.to_string())
    }

    /// Add or update a program in the registry
    pub fn add_program(&mut self, program: ProgramManifest) {
        self.programs.insert(program.program_id.clone(), program.clone());
        self.manifest.programs.retain(|p| p.program_id != program.program_id);
        self.manifest.programs.push(program);
        self.manifest.last_updated = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    }

    /// Remove a program from the registry
    pub fn remove_program(&mut self, program_id: &str) -> bool {
        if self.programs.remove(program_id).is_some() {
            self.manifest.programs.retain(|p| p.program_id != program_id);
            self.manifest.last_updated = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
            true
        } else {
            false
        }
    }

    /// Check if registry needs refresh
    pub fn needs_refresh(&self) -> bool {
        if !self.manifest.auto_refresh {
            return false;
        }

        let now = SystemTime::now();
        let elapsed = now.duration_since(self.last_refresh).unwrap_or_default();
        elapsed > Duration::from_secs(self.manifest.cache_ttl)
    }

    /// Refresh registry from remote sources
    pub async fn refresh(&mut self) -> Result<()> {
        println!("🔄 Refreshing program registry...");
        
        // In a real implementation, this would fetch from remote sources
        // For now, we'll just update the timestamp and validate existing programs
        self.last_refresh = SystemTime::now();
        
        // Validate IDL hashes for existing programs
        let mut programs_to_update = Vec::new();
        for (i, program) in self.manifest.programs.iter().enumerate() {
            if program.idl_url.starts_with("file://") {
                // Calculate hash for local files
                if let Ok(hash) = self.calculate_idl_hash(&program.idl_url).await {
                    programs_to_update.push((i, hash));
                }
            }
        }
        
        // Update hashes
        for (i, hash) in programs_to_update {
            self.manifest.programs[i].idl_hash = hash;
        }

        self.manifest.last_updated = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        // Rebuild program lookup map
        self.programs.clear();
        for program in &self.manifest.programs {
            self.programs.insert(program.program_id.clone(), program.clone());
        }

        // Save updated registry
        self.save_to_cache().await?;
        
        println!("✅ Program registry refreshed successfully");
        Ok(())
    }

    /// Calculate SHA256 hash of IDL file
    async fn calculate_idl_hash(&self, idl_url: &str) -> Result<String> {
        if idl_url.starts_with("file://") {
            let file_path = idl_url.strip_prefix("file://").unwrap();
            let content = fs::read_to_string(file_path).await?;
            let hash = Sha256::digest(content.as_bytes());
            Ok(format!("{:x}", hash))
        } else {
            Err(anyhow!("Only local file hashes are supported"))
        }
    }

    /// Validate registry integrity
    pub fn validate(&self) -> Result<()> {
        println!("🔍 Validating program registry integrity...");
        
        let mut issues = Vec::new();
        
        // Check for duplicate program IDs
        let mut seen_ids = std::collections::HashSet::new();
        for program in &self.manifest.programs {
            if !seen_ids.insert(&program.program_id) {
                issues.push(format!("Duplicate program ID: {}", program.program_id));
            }
        }

        // Check for invalid program IDs
        for program in &self.manifest.programs {
            if program.program_id.parse::<Pubkey>().is_err() {
                issues.push(format!("Invalid program ID: {}", program.program_id));
            }
        }

        // Check for missing required fields
        for program in &self.manifest.programs {
            if program.name.is_empty() {
                issues.push(format!("Empty name for program: {}", program.program_id));
            }
            if program.idl_url.is_empty() {
                issues.push(format!("Empty IDL URL for program: {}", program.program_id));
            }
        }

        if issues.is_empty() {
            println!("✅ Registry validation passed");
            Ok(())
        } else {
            println!("❌ Registry validation failed:");
            for issue in &issues {
                println!("  🚨 {}", issue);
            }
            Err(anyhow!("Registry validation failed with {} issues", issues.len()))
        }
    }

    /// Get registry statistics
    pub fn get_stats(&self) -> RegistryStats {
        let enabled_count = self.manifest.programs.iter().filter(|p| p.enabled).count();
        let disabled_count = self.manifest.programs.len() - enabled_count;
        
        RegistryStats {
            total_programs: self.manifest.programs.len(),
            enabled_programs: enabled_count,
            disabled_programs: disabled_count,
            last_updated: self.manifest.last_updated,
            cache_ttl: self.manifest.cache_ttl,
            auto_refresh: self.manifest.auto_refresh,
        }
    }

    /// List all programs in the registry
    pub fn list_programs(&self) -> Vec<&ProgramManifest> {
        let mut programs: Vec<&ProgramManifest> = self.manifest.programs.iter().collect();
        programs.sort_by(|a, b| b.priority.cmp(&a.priority));
        programs
    }
}

#[derive(Debug)]
pub struct RegistryStats {
    pub total_programs: usize,
    pub enabled_programs: usize,
    pub disabled_programs: usize,
    pub last_updated: u64,
    pub cache_ttl: u64,
    pub auto_refresh: bool,
}

impl Default for ProgramRegistry {
    fn default() -> Self {
        Self::new("./cache")
    }
}


