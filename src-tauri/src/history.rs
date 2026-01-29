use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, Write};
use std::path::{PathBuf};
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use url::Url;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub last_visit: u64, // Unix timestamp in seconds
    pub visit_count: u64,
    pub typed_count: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HistoryEntryScoped {
    pub url: String,
    pub title: String,
    pub score: u64,
    pub is_ghost_candidate: bool,
}

pub struct HistoryStore {
    index: Mutex<HashMap<String, HistoryEntry>>,
    log_path: PathBuf,
}

impl HistoryStore {
    pub fn new(app_data_dir: PathBuf) -> Self {
        fs::create_dir_all(&app_data_dir).ok();
        let log_path = app_data_dir.join("history.log");
        
        let mut store = HistoryStore {
            index: Mutex::new(HashMap::new()),
            log_path,
        };
        
        // Load existing history on startup
        if let Err(e) = store.load_from_log() {
            eprintln!("Failed to load history: {}", e);
        }
        
        store
    }

    fn load_from_log(&mut self) -> std::io::Result<()> {
        if !self.log_path.exists() {
            return Ok(());
        }

        let file = fs::File::open(&self.log_path)?;
        let reader = std::io::BufReader::new(file);
        let mut index = self.index.lock().unwrap();

        for line in reader.lines() {
            if let Ok(l) = line {
                if l.trim().is_empty() { continue; }
                // We expect JSON lines of HistoryEntry or partial updates. 
                // For simplicity in this append-only model, we'll store full Entry snapshots 
                // effectively "merging" by overwrite since the log is chronological.
                if let Ok(entry) = serde_json::from_str::<HistoryEntry>(&l) {
                    index.insert(entry.url.clone(), entry);
                }
            }
        }
        Ok(())
    }

    pub fn add_visit(&self, url: String, title: Option<String>, is_typed: bool) {
        let normalized = normalize_url(&url);
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        // Locked Update
        let entry_snapshot = {
            let mut index = self.index.lock().unwrap();
            
            let entry = index.entry(normalized.clone()).or_insert(HistoryEntry {
                url: normalized.clone(),
                title: title.clone().unwrap_or_default(),
                last_visit: 0,
                visit_count: 0,
                typed_count: 0,
            });

            entry.last_visit = now;
            entry.visit_count += 1;
            if is_typed {
                entry.typed_count += 1;
            }
            // Update title only if new one is provided and non-empty
            if let Some(t) = title {
                if !t.is_empty() {
                    entry.title = t;
                }
            }

            entry.clone()
        };

        // Append to Log (outside lock to minimize contention, though file I/O is blocking here)
        // In a real high-perf app, this would be a channel to a background writer thread.
        if let Ok(json) = serde_json::to_string(&entry_snapshot) {
            // Check if we need compaction (naive check: simple random sampling or count)
            // For MVP: We will just append. Compaction can be triggered manually or on app start/exit.
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.log_path)
                .expect("Failed to open history log");
            
            if let Err(e) = writeln!(file, "{}", json) {
                eprintln!("Failed to write to history log: {}", e);
            }
        }
    }

    pub fn search(&self, query: String, limit: usize) -> Vec<HistoryEntryScoped> {
        let index = self.index.lock().unwrap();
        let query = query.trim().to_lowercase();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let mut results: Vec<HistoryEntryScoped> = index.values()
            .map(|entry| {
                let mut score = 0;
                let entry_url_lower = entry.url.to_lowercase();
                
                // HOST extraction for boosting
                let host = if let Ok(u) = Url::parse(&entry.url) {
                    u.host_str().unwrap_or("").to_string()
                } else {
                    String::new()
                };
                
                // 1. Prefix Match (Strongest)
                // Check Scheme-less prefix (e.g. "goo" matches "https://google.com")
                let schemeless = entry_url_lower.trim_start_matches("https://").trim_start_matches("http://");
                
                let is_prefix = schemeless.starts_with(&query);
                let is_host_prefix = !host.is_empty() && host.starts_with(&query);

                if is_prefix || is_host_prefix {
                    score += 5000;
                } else if entry_url_lower.contains(&query) || entry.title.to_lowercase().contains(&query) {
                    score += 100;
                } else {
                    return None;
                }

                // 2. Typed Count Boost
                score += entry.typed_count * 500;

                // 3. Frecency / Recency Decay
                // Simple decay: subtract points for every day of age
                let age_sec = now.saturating_sub(entry.last_visit);
                let age_days = age_sec / 86400;
                let recency_score = 1000u64.saturating_sub(age_days * 10); // severe penalty for age
                score += recency_score;

                // 4. Visit Frequency
                score += entry.visit_count * 10;
                
                // Ghost Text Candidate?
                // Must be a very strong prefix match logic
                let is_ghost_candidate = is_prefix || is_host_prefix;

                Some(HistoryEntryScoped {
                    url: entry.url.clone(),
                    title: entry.title.clone(),
                    score,
                    is_ghost_candidate
                })
            })
            .filter_map(|x| x)
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.score.cmp(&a.score));
        results.truncate(limit);
        results
    }
    
    pub fn compact(&self) -> std::io::Result<()> {
        let index = self.index.lock().unwrap();
        // Atomic write: write to .tmp then rename
        let tmp_path = self.log_path.with_extension("log.tmp");
        
        {
            let mut file = std::fs::File::create(&tmp_path)?;
            for entry in index.values() {
                let json = serde_json::to_string(entry).unwrap();
                writeln!(file, "{}", json)?;
            }
            file.sync_all()?;
        }
        
        fs::rename(tmp_path, &self.log_path)?;
        Ok(())
    }
}

fn normalize_url(url: &str) -> String {
    // Basic normalization:
    // 1. Ensure trailing slash for root domains if missing is handled by Url parser usually
    // 2. We keep the scheme.
    if let Ok(parsed) = Url::parse(url) {
        // Strip fragment? No, some SPAs need it.
        // Strip query? Maybe specific tracking params, but let's keep it simple for generic use.
        parsed.to_string()
    } else {
        url.to_string()
    }
}
