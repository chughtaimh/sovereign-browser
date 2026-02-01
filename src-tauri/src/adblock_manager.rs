use adblock::engine::Engine;
use adblock::lists::{FilterSet, ParseOptions};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use arc_swap::ArcSwap;
use dashmap::DashMap;

const EASYLIST_URL: &str = "https://easylist.to/easylist/easylist.txt";
const EASYPRIVACY_URL: &str = "https://easylist.to/easylist/easyprivacy.txt";
const ENGINE_CACHE_FILE: &str = "adblock_engine.bin";
const SAFARI_CACHE_FILE: &str = "safari_rules.json";
const ALLOWLIST_FILE: &str = "adblock_allowlist.json";

// Custom exception rules for webmail services (Option A: Granular Approach)
// Syntax: @@||domain^$domain=context - "When on context domain, allow requests to domain"
// This maintains privacy by only whitelisting Google infrastructure, not all third-party trackers
const CUSTOM_EXCEPTION_RULES: &[&str] = &[
    // Gmail: Whitelist Google's infrastructure domains when on Gmail
    "@@||google.com^$domain=mail.google.com|gmail.com",
    "@@||gstatic.com^$domain=mail.google.com|gmail.com",
    "@@||googleusercontent.com^$domain=mail.google.com|gmail.com",
    "@@||googleapis.com^$domain=mail.google.com|gmail.com",
    "@@||ggpht.com^$domain=mail.google.com|gmail.com",
    // Future: Add Outlook, Yahoo Mail, etc.
    // "@@||outlook.live.com^$domain=outlook.live.com",
];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum RuleExpiry {
    Forever,
    Until(SystemTime),
}

pub struct AdBlockManager {
    // Lock-free reader for the hot path
    engine: ArcSwap<Engine>,
    // Concurrent map for exceptions
    allowlist: DashMap<String, RuleExpiry>,
    app_dir: PathBuf,
    // Cache Safari rules in memory for fast injection
    pub safari_rules_json: ArcSwap<String>,
}

impl AdBlockManager {
    pub fn new(app: &AppHandle) -> Self {
        let app_dir = app.path().app_data_dir().expect("Failed to get app data dir");
        let _ = fs::create_dir_all(&app_dir);

        let cache_path = app_dir.join(ENGINE_CACHE_FILE);
        let allowlist_path = app_dir.join(ALLOWLIST_FILE);
        let safari_path = app_dir.join(SAFARI_CACHE_FILE);

        // 1. Load Rust Engine
        println!("[AdBlock] Initializing ad blocking engine...");
        let engine = if cache_path.exists() {
            println!("[AdBlock] Loading cached engine from {:?}...", cache_path);
            Self::load_engine_from_disk(&cache_path).unwrap_or_else(|_| {
                println!("[AdBlock] Failed to load cache, using empty engine");
                Engine::default()
            })
        } else {
            println!("[AdBlock] No cache found, starting with empty engine");
            Engine::default()
        };

        // 2. Load Allowlist
        let allowlist = DashMap::new();
        if allowlist_path.exists() {
            if let Ok(content) = fs::read_to_string(&allowlist_path) {
                if let Ok(stored) = serde_json::from_str::<std::collections::HashMap<String, RuleExpiry>>(&content) {
                    for (k, v) in stored {
                        allowlist.insert(k, v);
                    }
                    println!("[AdBlock] Loaded {} allowlist entries", allowlist.len());
                }
            }
        }

        // 3. Load Safari Rules
        let safari_json = if safari_path.exists() {
            let json = fs::read_to_string(&safari_path).unwrap_or_else(|_| "[]".to_string());
            println!("[AdBlock] Loaded cached Safari rules ({} chars)", json.len());
            json
        } else {
            "[]".to_string()
        };

        println!("[AdBlock] Ad blocking engine initialized.");

        Self {
            engine: ArcSwap::from_pointee(engine),
            allowlist,
            app_dir,
            safari_rules_json: ArcSwap::from_pointee(safari_json),
        }
    }

    /// Spawn a background thread to fetch and update rules.
    /// Call this after creating the manager.
    pub fn spawn_update_thread(self: &Arc<Self>) {
        let manager = self.clone();
        std::thread::spawn(move || {
            manager.update_rules();
        });
    }

    fn update_rules(&self) {
        println!("[AdBlock] Background: Fetching filter lists...");
        
        let urls = vec![EASYLIST_URL, EASYPRIVACY_URL];
        let mut filter_set = FilterSet::new(true); // debug=true required for Safari conversion
        let mut lines_count = 0;

        for url in &urls {
            println!("[AdBlock] Background: Fetching {}...", url);
            if let Ok(resp) = reqwest::blocking::get(*url) {
                if let Ok(text) = resp.text() {
                    let lines: Vec<&str> = text.lines().collect();
                    let count = lines.len();
                    lines_count += count;
                    filter_set.add_filters(&lines, ParseOptions::default());
                    println!("[AdBlock] Background: Loaded {} lines from {}", count, url);
                }
            }
        }

        if lines_count == 0 {
            println!("[AdBlock] Background: No filters loaded, aborting update");
            return;
        }

        // CRITICAL: Inject custom exception rules for webmail
        println!("[AdBlock] Background: Injecting {} custom exception rules", CUSTOM_EXCEPTION_RULES.len());
        filter_set.add_filters(CUSTOM_EXCEPTION_RULES, ParseOptions::default());
        for rule in CUSTOM_EXCEPTION_RULES {
            println!("[AdBlock] Background: Added custom rule: {}", rule);
        }

        println!("[AdBlock] Background: Loaded {} total filter lines", lines_count);

        // Pipeline A: Rust Engine (Cosmetic & Windows/Linux network blocking)
        println!("[AdBlock] Background: Building Rust engine...");
        let new_engine = Engine::from_filter_set(filter_set.clone(), true);
        let serialized = new_engine.serialize();
        let _ = fs::write(self.app_dir.join(ENGINE_CACHE_FILE), serialized);
        self.engine.store(Arc::new(new_engine));
        println!("[AdBlock] Background: Rust engine updated and cached.");

        // Pipeline B: Safari Rules (macOS Network blocking)
        #[cfg(target_os = "macos")]
        {
            println!("[AdBlock] Background: Generating Safari content blocking rules...");
            if let Ok((rules, skipped)) = filter_set.into_content_blocking() {
                println!("[AdBlock] Background: Generated {} Safari rules ({} skipped)", rules.len(), skipped.len());

                // CRITICAL: The adblock crate's $domain syntax doesn't convert to Safari rules properly
                // Manually inject exception rules for Gmail using Safari's format
                // Work with JSON to add custom rules

                if let Ok(json_str) = serde_json::to_string(&rules) {
                    if let Ok(mut rules_json) = serde_json::from_str::<Vec<serde_json::Value>>(&json_str) {
                        // Add exception rules for Gmail
                        let gmail_domains = vec!["*mail.google.com", "*gmail.com"];
                        let whitelisted_domains = vec![
                            "google.com", "gstatic.com", "googleusercontent.com",
                            "googleapis.com", "ggpht.com"
                        ];

                        for whitelisted in &whitelisted_domains {
                            // CRITICAL: URL filter must match domain specifically, not just contain the string
                            // Pattern: ^https?://([^/]*\.)?DOMAIN/
                            // This matches: https://domain/ or https://subdomain.domain/ but NOT https://evil.com?url=domain
                            let url_pattern = if whitelisted.contains('.') {
                                // For domains like google.com, match as domain with optional subdomain
                                let escaped = whitelisted.replace(".", "\\.");
                                format!("^https?://([^/]*\\.)?{}(/|$)", escaped)
                            } else {
                                // For single-word domains, match exactly
                                format!("^https?://{}(/|$)", whitelisted)
                            };

                            let exception_rule = serde_json::json!({
                                "trigger": {
                                    "url-filter": url_pattern,
                                    "if-domain": gmail_domains.clone()
                                },
                                "action": {
                                    "type": "allow"
                                }
                            });
                            rules_json.push(exception_rule);
                            println!("[AdBlock] Background: Added Safari exception for {} on Gmail", whitelisted);
                        }

                        println!("[AdBlock] Background: Final Safari rules count: {}", rules_json.len());

                        if let Ok(final_json) = serde_json::to_string(&rules_json) {
                            println!("[AdBlock] Background: Safari rules serialized ({} chars)", final_json.len());
                            let _ = fs::write(self.app_dir.join(SAFARI_CACHE_FILE), &final_json);
                            self.safari_rules_json.store(Arc::new(final_json));
                            println!("[AdBlock] Background: Safari rules updated and cached.");
                        }
                    }
                }
            } else {
                println!("[AdBlock] Background: Failed to generate Safari rules");
            }
        }

        println!("[AdBlock] Background: Update complete!");
    }

    fn load_engine_from_disk(path: &PathBuf) -> Result<Engine, ()> {
        let data = fs::read(path).map_err(|_| ())?;
        let mut engine = Engine::default();
        engine.deserialize(&data).map_err(|_| ())?;
        Ok(engine)
    }

    // --- Hot Path: Network Check (Windows/Linux only) ---
    
    /// Check if a request should be blocked.
    /// Uses lock-free ArcSwap::load() for maximum performance.
    /// NOTE: On macOS, this is bypassed - WKContentRuleList handles blocking.
    pub fn should_block_request(&self, url: &str, source_url: &str, request_type: &str) -> bool {
        // Check Allowlist first (Fast DashMap lookup)
        if let Some(domain) = Self::extract_domain(source_url) {
            if let Some(expiry) = self.allowlist.get(&domain) {
                match *expiry {
                    RuleExpiry::Forever => return false,
                    RuleExpiry::Until(t) => if SystemTime::now() < t { return false; },
                }
            }
        }

        // Check Engine (Lock-Free) - engine handles exception rules automatically
        let engine = self.engine.load();
        let req = adblock::request::Request::new(url, source_url, request_type).ok();

        if let Some(r) = req {
            engine.check_network_request(&r).matched
        } else {
            false
        }
    }

    // --- Cosmetic CSS ---
    
    /// Get cosmetic hiding CSS for a URL.
    /// CRITICAL: Respects allowlist - returns empty string if site is excepted.
    pub fn get_cosmetic_css(&self, url: &str) -> String {
        // CRITICAL: Respect allowlist AND webmail domains
        // Use url crate for security (no phishing vulnerabilities)
        if self.is_exception(url) || Self::is_webmail_domain(url) {
            return String::new();
        }

        let engine = self.engine.load();
        let resources = engine.url_cosmetic_resources(url);

        let mut css = String::with_capacity(resources.hide_selectors.len() * 50);
        for selector in resources.hide_selectors {
            css.push_str(selector.as_str());
            css.push_str(" { display: none !important; }\n");
        }
        css
    }

    // --- Exception Management ---

    pub fn add_exception(&self, domain: String, duration: Option<Duration>) {
        let expiry = match duration {
            Some(d) => RuleExpiry::Until(SystemTime::now() + d),
            None => RuleExpiry::Forever,
        };
        println!("[AdBlock] Added exception for: {}", domain);
        self.allowlist.insert(domain, expiry);
        self.save_allowlist();
    }

    pub fn remove_exception(&self, domain: &str) {
        self.allowlist.remove(domain);
        self.save_allowlist();
        println!("[AdBlock] Removed exception for: {}", domain);
    }

    pub fn is_exception(&self, url: &str) -> bool {
        if let Some(domain) = Self::extract_domain(url) {
            if let Some(expiry) = self.allowlist.get(&domain) {
                match *expiry {
                    RuleExpiry::Forever => return true,
                    RuleExpiry::Until(t) => return SystemTime::now() < t,
                }
            }
        }
        false
    }

    pub fn get_exceptions(&self) -> Vec<(String, RuleExpiry)> {
        self.allowlist.iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect()
    }

    fn save_allowlist(&self) {
        let path = self.app_dir.join(ALLOWLIST_FILE);
        let map: std::collections::HashMap<_, _> = self.allowlist.iter()
            .map(|r| (r.key().clone(), r.value().clone()))
            .collect();
        let _ = fs::write(path, serde_json::to_string_pretty(&map).unwrap_or_default());
    }

    fn extract_domain(url: &str) -> Option<String> {
        url::Url::parse(url).ok()?.domain().map(|d| d.to_string())
    }

    /// Check if URL is a webmail domain that should skip cosmetic filtering.
    /// Uses url crate for correct, secure domain parsing (security > micro-optimization).
    fn is_webmail_domain(url: &str) -> bool {
        // Use url crate - correctness over micro-optimization
        if let Ok(parsed) = url::Url::parse(url) {
            if let Some(domain) = parsed.domain() {
                // Check against webmail domains from exception rules
                return domain == "mail.google.com"
                    || domain.ends_with(".mail.google.com")
                    || domain == "gmail.com"
                    || domain.ends_with(".gmail.com");
                // Future: Add more as CUSTOM_EXCEPTION_RULES grows
            }
        }
        false
    }

    /// Get the cached Safari rules JSON for WKContentRuleList.
    #[cfg(target_os = "macos")]
    pub fn get_safari_rules(&self) -> String {
        (**self.safari_rules_json.load()).clone()
    }
}
