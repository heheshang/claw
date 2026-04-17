use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use log::{info, warn, error};
use rusqlite::{Connection, Result as SqliteResult};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

use crate::harness::api::resolve_model_alias;

/// Find .claw/settings.json or .claw/settings.local.json by searching up from current directory
fn find_settings_json() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_dir().ok();

    // Search up to 5 levels up for .claw/settings.json
    for _ in 0..5 {
        if let Some(d) = dir {
            let settings_path = d.join(".claw/settings.json");
            if settings_path.exists() {
                return Some(settings_path);
            }
            dir = d.parent().map(|p| p.to_path_buf());
        } else {
            break;
        }
    }
    None
}

/// Find .claw/settings.local.json by searching up from current directory
fn find_settings_local_json() -> Option<std::path::PathBuf> {
    let mut dir = std::env::current_dir().ok();

    // Search up to 5 levels up for .claw/settings.local.json
    for _ in 0..5 {
        if let Some(d) = dir {
            let settings_path = d.join(".claw/settings.local.json");
            if settings_path.exists() {
                return Some(settings_path);
            }
            dir = d.parent().map(|p| p.to_path_buf());
        } else {
            break;
        }
    }
    None
}

/// Load API key from settings.local.json or environment variables
fn get_api_key() -> Result<String, String> {
    // Try settings.local.json first (searching up from current directory)
    if let Some(path) = find_settings_local_json() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(key) = json["providers"]["anthropic"]["api_key"].as_str() {
                    if !key.is_empty() {
                        return Ok(key.to_string());
                    }
                }
            }
        }
    }

    // Fallback to environment variables
    std::env::var("ANTHROPIC_API_KEY")
        .or_else(|_| std::env::var("ANTHROPIC_AUTH_TOKEN"))
        .or_else(|_| std::env::var("CLAUDE_API_KEY"))
        .map_err(|_| "API key not configured. Set ANTHROPIC_API_KEY or ANTHROPIC_AUTH_TOKEN or provide in settings.local.json".to_string())
}

/// Load base URL from settings.json, settings.local.json, or environment variable
pub fn get_base_url() -> String {
    // Try settings.local.json first
    if let Some(path) = find_settings_local_json() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(url) = json["providers"]["anthropic"]["base_url"].as_str() {
                    if !url.is_empty() {
                        return url.to_string();
                    }
                }
            }
        }
    }

    // Try settings.json next
    if let Some(path) = find_settings_json() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(url) = json["providers"]["anthropic"]["base_url"].as_str() {
                    if !url.is_empty() {
                        return url.to_string();
                    }
                }
            }
        }
    }

    // Check environment variable or use default
    std::env::var("ANTHROPIC_BASE_URL")
        .unwrap_or_else(|_| "https://api.anthropic.com".to_string())
}

/// Load model from settings.json, settings.local.json, or environment variable
/// Resolves model aliases (opus, sonnet, haiku) to full model names
pub fn get_model() -> String {
    // Try settings.local.json first
    if let Some(path) = find_settings_local_json() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(model) = json["providers"]["anthropic"]["model"].as_str() {
                    if !model.is_empty() {
                        return resolve_model_alias(model);
                    }
                }
            }
        }
    }

    // Try settings.json next
    if let Some(path) = find_settings_json() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(model) = json["providers"]["anthropic"]["model"].as_str() {
                    if !model.is_empty() {
                        return resolve_model_alias(model);
                    }
                }
            }
        }
    }

    // Check environment variable or use default
    let model = std::env::var("ANTHROPIC_MODEL")
        .unwrap_or_else(|_| "claude-sonnet-4-20250514".to_string());
    resolve_model_alias(&model)
}

pub struct AppState {
    pub db: Mutex<Connection>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub nickname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar_url: Option<String>,
    pub password_hash: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub exp: i64,
    pub iat: i64,
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserResponse,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub username: String,
    pub nickname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub nickname: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

const JWT_SECRET: &[u8] = b"your-secret-key-change-in-production";

fn get_db_path() -> String {
    let app_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let db_dir = app_dir.join("ssk");
    std::fs::create_dir_all(&db_dir).ok();
    db_dir.join("users.db").to_string_lossy().to_string()
}

pub fn init_db() -> SqliteResult<Connection> {
    let db_path = get_db_path();
    let conn = Connection::open(&db_path)?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL UNIQUE,
            nickname TEXT,
            email TEXT,
            phone TEXT,
            avatar_url TEXT,
            password_hash TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    // Migration: Add missing columns if they don't exist (for existing databases)
    let migration_sqls = [
        "ALTER TABLE users ADD COLUMN nickname TEXT",
        "ALTER TABLE users ADD COLUMN email TEXT",
        "ALTER TABLE users ADD COLUMN phone TEXT",
        "ALTER TABLE users ADD COLUMN avatar_url TEXT",
    ];

    for sql in migration_sqls {
        let _ = conn.execute(sql, []);
    }

    Ok(conn)
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = argon2::Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| e.to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    let parsed_hash = PasswordHash::new(hash).map_err(|e| e.to_string())?;
    Ok(argon2::Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

fn generate_jwt(username: &str) -> Result<String, String> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::hours(24))
        .expect("valid timestamp")
        .timestamp();

    let claims = Claims {
        sub: username.to_string(),
        exp: expiration,
        iat: Utc::now().timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_SECRET),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn register(state: State<AppState>, request: RegisterRequest) -> Result<AuthResponse, String> {
    info!("[AUTH] Register attempt for user: {}", request.username);

    if request.username.len() < 3 {
        warn!("[AUTH] Register failed: username '{}' too short", request.username);
        return Err("Username must be at least 3 characters".to_string());
    }
    if request.password.len() < 6 {
        warn!("[AUTH] Register failed: weak password for user '{}'", request.username);
        return Err("Password must be at least 6 characters".to_string());
    }

    let password_hash = hash_password(&request.password)?;

    let db = state.db.lock().map_err(|e| e.to_string())?;

    let result = db.execute(
        "INSERT INTO users (username, password_hash) VALUES (?1, ?2)",
        (&request.username, &password_hash),
    );

    match result {
        Ok(_) => {
            let user_id = db.last_insert_rowid();
            let token = generate_jwt(&request.username)?;
            info!("[AUTH] Register success: user '{}' (id: {})", request.username, user_id);

            Ok(AuthResponse {
                token,
                user: UserResponse {
                    id: user_id,
                    username: request.username,
                    nickname: None,
                    email: None,
                    phone: None,
                    avatar_url: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                },
            })
        }
        Err(rusqlite::Error::SqliteFailure(_, Some(msg))) => {
            if msg.contains("UNIQUE constraint failed") {
                warn!("[AUTH] Register failed: username '{}' already exists", request.username);
                Err("Username already exists".to_string())
            } else {
                error!("[AUTH] Register failed for '{}': {}", request.username, msg);
                Err(msg)
            }
        }
        Err(e) => {
            error!("[AUTH] Register error for '{}': {}", request.username, e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub fn login(state: State<AppState>, request: LoginRequest) -> Result<AuthResponse, String> {
    info!("[AUTH] Login attempt for user: {}", request.username);

    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db
        .prepare("SELECT id, username, nickname, email, phone, avatar_url, password_hash, created_at FROM users WHERE username = ?1")
        .map_err(|e| e.to_string())?;

    let user_result: SqliteResult<User> = stmt.query_row([&request.username], |row| {
        Ok(User {
            id: row.get(0)?,
            username: row.get(1)?,
            nickname: row.get(2)?,
            email: row.get(3)?,
            phone: row.get(4)?,
            avatar_url: row.get(5)?,
            password_hash: row.get(6)?,
            created_at: row.get(7)?,
        })
    });

    match user_result {
        Ok(user) => {
            drop(stmt);
            drop(db);

            if verify_password(&request.password, &user.password_hash)? {
                let token = generate_jwt(&user.username)?;
                info!("[AUTH] Login success: user '{}' (id: {})", user.username, user.id);
                Ok(AuthResponse {
                    token,
                    user: UserResponse {
                        id: user.id,
                        username: user.username,
                        nickname: user.nickname,
                        email: user.email,
                        phone: user.phone,
                        avatar_url: user.avatar_url,
                        created_at: user.created_at,
                    },
                })
            } else {
                warn!("[AUTH] Login failed: invalid password for user '{}'", request.username);
                Err("Invalid password".to_string())
            }
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            warn!("[AUTH] Login failed: user '{}' not found", request.username);
            Err("User not found".to_string())
        }
        Err(e) => {
            error!("[AUTH] Login error for '{}': {}", request.username, e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub fn verify_token(token: String) -> Result<Claims, String> {
    let token_data = decode::<Claims>(
        &token,
        &DecodingKey::from_secret(JWT_SECRET),
        &Validation::default(),
    )
    .map_err(|e| e.to_string())?;

    Ok(token_data.claims)
}

#[tauri::command]
pub fn get_user_profile(state: State<AppState>, username: String) -> Result<UserResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    let mut stmt = db
        .prepare("SELECT id, username, nickname, email, phone, avatar_url, created_at FROM users WHERE username = ?1")
        .map_err(|e| e.to_string())?;

    stmt.query_row([&username], |row| {
        Ok(UserResponse {
            id: row.get(0)?,
            username: row.get(1)?,
            nickname: row.get(2)?,
            email: row.get(3)?,
            phone: row.get(4)?,
            avatar_url: row.get(5)?,
            created_at: row.get(6)?,
        })
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_user_profile(
    state: State<AppState>,
    username: String,
    request: UpdateProfileRequest,
) -> Result<UserResponse, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Validate email format if provided
    if let Some(ref email) = request.email {
        if !email.is_empty() && !email.contains('@') {
            return Err("Invalid email format".to_string());
        }
    }

    // Validate phone format if provided
    if let Some(ref phone) = request.phone {
        if !phone.is_empty() && phone.len() < 7 {
            return Err("Invalid phone format".to_string());
        }
    }

    let result = db.execute(
        "UPDATE users SET nickname = ?1, email = ?2, phone = ?3, avatar_url = ?4 WHERE username = ?5",
        (
            &request.nickname,
            &request.email,
            &request.phone,
            &request.avatar_url,
            &username,
        ),
    )
    .map_err(|e| e.to_string())?;

    if result == 0 {
        return Err("User not found".to_string());
    }

    // Fetch updated user
    let mut stmt = db
        .prepare("SELECT id, username, nickname, email, phone, avatar_url, created_at FROM users WHERE username = ?1")
        .map_err(|e| e.to_string())?;

    stmt.query_row([&username], |row| {
        Ok(UserResponse {
            id: row.get(0)?,
            username: row.get(1)?,
            nickname: row.get(2)?,
            email: row.get(3)?,
            phone: row.get(4)?,
            avatar_url: row.get(5)?,
            created_at: row.get(6)?,
        })
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn change_password(
    state: State<AppState>,
    username: String,
    request: ChangePasswordRequest,
) -> Result<(), String> {
    info!("[AUTH] Password change attempt for user: {}", username);

    if request.new_password.len() < 6 {
        warn!("[AUTH] Password change failed: weak password for user '{}'", username);
        return Err("New password must be at least 6 characters".to_string());
    }

    let db = state.db.lock().map_err(|e| e.to_string())?;

    // Get current password hash
    let mut stmt = db
        .prepare("SELECT password_hash FROM users WHERE username = ?1")
        .map_err(|e| e.to_string())?;

    let password_hash: String = stmt
        .query_row([&username], |row| row.get(0))
        .map_err(|e| e.to_string())?;

    // Verify old password
    if !verify_password(&request.old_password, &password_hash)? {
        warn!("[AUTH] Password change failed: incorrect old password for user '{}'", username);
        return Err("Current password is incorrect".to_string());
    }

    // Hash new password
    let new_hash = hash_password(&request.new_password)?;

    // Update password
    db.execute(
        "UPDATE users SET password_hash = ?1 WHERE username = ?2",
        (&new_hash, &username),
    )
    .map_err(|e| e.to_string())?;

    info!("[AUTH] Password change success for user: {}", username);
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
pub struct ContentBlock {
    pub text: Option<String>,
    #[serde(rename = "type")]
    pub block_type: String,
}

#[tauri::command]
pub async fn invoke_llm(request: ChatRequest) -> Result<String, String> {
    info!("[LLM] Invoke LLM with {} messages", request.messages.len());

    let api_key = get_api_key()?;
    let base_url = get_base_url();
    let model = request.model.unwrap_or_else(get_model);

    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "messages": request.messages
    });

    let response = client
        .post(format!("{}/v1/messages", base_url))
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        error!("[LLM] API error: {} - {}", status, text);
        return Err(format!("API error: {} - {}", status, text));
    }

    let chat_response: ChatResponse = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    let text = chat_response
        .content
        .iter()
        .find(|c| c.block_type == "text")
        .and_then(|c| c.text.clone())
        .unwrap_or_default();

    info!("[LLM] Response received, {} chars", text.len());
    Ok(text)
}
