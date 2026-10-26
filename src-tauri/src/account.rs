//! Compte Morph facultatif : liaison de l'application au compte du site
//! (https://morph.underskydev.com, API `api/app.php`), sans jamais rien exiger pour les
//! fonctions locales. Aucun fichier de l'utilisateur n'est envoyé.
//!
//! Liaison « comme une télévision » : Morph demande un code, ouvre la page du site où
//! l'utilisateur connecté l'approuve, puis reçoit un jeton propre à cet ordinateur.
//! Le jeton est chiffré pour la session Windows (DPAPI) ; ailleurs, il est dans un fichier
//! lisible par l'utilisateur seul. Il est révocable depuis Morph ou depuis le site.
//!
//! Option : synchroniser les préférences d'apparence et de menu entre ordinateurs (pas les
//! dossiers, le démarrage ni la carte graphique, propres à chaque machine). La version la
//! plus récente l'emporte.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

use crate::i18n::tr;
use crate::settings::{self, Settings};

const SITE: &str = "https://morph.underskydev.com";
/// Préférences synchronisées (mêmes clés que `SYNCED_KEYS` côté site).
const SYNCED: [&str; 9] = ["language", "theme", "accent", "background", "radialOpacity", "favorites", "mixedBatch", "duplicates", "cutout"];

/// Adresse du site ; `MORPH_SITE_URL` la remplace dans les versions de développement
/// (serveur PHP local pour les essais).
fn site() -> String {
    if cfg!(debug_assertions) {
        if let Ok(url) = std::env::var("MORPH_SITE_URL") {
            return url.trim_end_matches('/').to_string();
        }
    }
    SITE.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub name: String,
    pub email: Option<String>,
    pub avatar: Option<String>,
    pub created_at: Option<String>,
}

/// Code de liaison en attente d'approbation sur le site.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub user_code: String,
    pub verify_url: String,
    pub expires_at_ms: u64,
}

/// État montré par les Réglages (événement `account-changed`).
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub user: Option<User>,
    pub link: Option<Link>,
    /// Vérification ou envoi en cours.
    pub busy: bool,
    /// Dernière erreur, dans la langue de l'interface.
    pub error: Option<String>,
    /// Code de l'erreur (« network », « expired », « denied », « session_expired »…).
    pub error_code: Option<String>,
    pub sync: bool,
    pub last_sync_ms: Option<u64>,
}

/// Ce qui est gardé sur le disque, hors jeton (rien de secret).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Saved {
    user: Option<User>,
    sync: bool,
    last_sync_ms: Option<u64>,
}

#[derive(Default)]
pub struct State {
    status: Mutex<Status>,
    /// Liaison en cours : son numéro (une nouvelle demande ou une annulation l'invalide).
    link_generation: AtomicU64,
}

/// Préférences en cours d'application depuis le site (ne pas les renvoyer aussitôt).
static APPLYING_REMOTE: AtomicBool = AtomicBool::new(false);
/// Dernière modification locale à envoyer (envois regroupés).
static PUSH_GENERATION: AtomicU64 = AtomicU64::new(0);

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok()
}

fn read_saved(app: &AppHandle) -> Saved {
    dir(app)
        .and_then(|d| std::fs::read_to_string(d.join("account.json")).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_saved(app: &AppHandle, s: &Status) {
    let Some(d) = dir(app) else { return };
    let saved = Saved { user: s.user.clone(), sync: s.sync, last_sync_ms: s.last_sync_ms };
    let _ = std::fs::create_dir_all(&d);
    if let Ok(json) = serde_json::to_string_pretty(&saved) {
        let _ = std::fs::write(d.join("account.json"), json);
    }
}

// --- Jeton : chiffré pour la session Windows, ou fichier privé ailleurs -----------------

fn token_path(app: &AppHandle) -> Option<PathBuf> {
    dir(app).map(|d| d.join("account.key"))
}

fn load_token(app: &AppHandle) -> Option<String> {
    let bytes = std::fs::read(token_path(app)?).ok()?;
    let plain = secret::unprotect(&bytes)?;
    String::from_utf8(plain).ok().filter(|t| t.starts_with("mph_"))
}

fn store_token(app: &AppHandle, token: &str) -> Result<(), String> {
    let path = token_path(app).ok_or("dossier de configuration introuvable")?;
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let data = secret::protect(token.as_bytes()).ok_or("chiffrement du jeton impossible")?;
    secret::write_private(&path, &data).map_err(|e| e.to_string())
}

fn delete_token(app: &AppHandle) {
    if let Some(p) = token_path(app) {
        let _ = std::fs::remove_file(p);
    }
}

#[cfg(windows)]
mod secret {
    use windows::Win32::Foundation::{HLOCAL, LocalFree};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};

    fn run(data: &[u8], protect: bool) -> Option<Vec<u8>> {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB::default();
        // SAFETY : tampons valides pendant l'appel ; la sortie, allouée par Windows, est
        // copiée puis libérée avec LocalFree.
        unsafe {
            let ok = if protect {
                CryptProtectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output)
            } else {
                CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output)
            };
            ok.ok()?;
            let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
            let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
            Some(bytes)
        }
    }

    pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
        run(data, true)
    }

    pub fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
        run(data, false)
    }

    pub fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
        std::fs::write(path, data)
    }
}

#[cfg(not(windows))]
mod secret {
    pub fn protect(data: &[u8]) -> Option<Vec<u8>> {
        Some(data.to_vec())
    }

    pub fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
        Some(data.to_vec())
    }

    /// Fichier lisible par l'utilisateur seul (0600).
    pub fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(path)?;
        f.write_all(data)
    }
}

// --- Réseau ----------------------------------------------------------------------------------

enum ApiError {
    /// Pas de réponse du site.
    Network,
    /// Jeton refusé : révoqué depuis le site, compte supprimé ou suspendu.
    Unauthorized,
    Status(u16),
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(15)))
        .user_agent(concat!("Morph/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn api(method: &str, action: &str, token: Option<&str>, body: Option<Value>) -> Result<Value, ApiError> {
    let url = format!("{}/api/app.php?action={action}", site());
    let agent = agent();
    let result = if method == "GET" {
        let mut req = agent.get(&url);
        if let Some(t) = token {
            req = req.header("X-Morph-Token", t);
        }
        req.call()
    } else {
        let mut req = agent.post(&url);
        if let Some(t) = token {
            req = req.header("X-Morph-Token", t);
        }
        req.send_json(body.unwrap_or_else(|| json!({})))
    };
    let mut res = result.map_err(|e| {
        eprintln!("[morph] compte : {e}");
        ApiError::Network
    })?;
    let status = res.status().as_u16();
    let data: Value = res.body_mut().read_json().unwrap_or(Value::Null);
    match status {
        200..=299 => Ok(data),
        401 => Err(ApiError::Unauthorized),
        _ => Err(ApiError::Status(status)),
    }
}

fn error_text(e: &ApiError) -> (&'static str, &'static str) {
    match e {
        ApiError::Network => ("network", tr("core.account_network")),
        ApiError::Unauthorized => ("session_expired", tr("core.account_session_expired")),
        ApiError::Status(429) => ("too_many_requests", tr("core.account_too_many")),
        ApiError::Status(..) => ("server", tr("core.account_server_error")),
    }
}

// --- État ------------------------------------------------------------------------------------

fn change(app: &AppHandle, f: impl FnOnce(&mut Status)) -> Status {
    let state = app.state::<State>();
    let mut s = state.status.lock().unwrap();
    f(&mut s);
    let snapshot = s.clone();
    drop(s);
    let _ = app.emit("account-changed", &snapshot);
    snapshot
}

fn set_error(app: &AppHandle, e: &ApiError) {
    let (code, text) = error_text(e);
    change(app, |s| {
        s.busy = false;
        s.error = Some(text.into());
        s.error_code = Some(code.into());
    });
}

/// Au démarrage : reprend l'état enregistré, puis vérifie le jeton en arrière-plan
/// (sans bloquer : hors ligne, Morph garde le dernier profil connu).
pub fn init(app: &AppHandle) {
    let saved = read_saved(app);
    let has_token = load_token(app).is_some();
    {
        let state = app.state::<State>();
        let mut s = state.status.lock().unwrap();
        s.user = if has_token { saved.user } else { None };
        s.sync = has_token && saved.sync;
        s.last_sync_ms = saved.last_sync_ms;
    }
    if has_token {
        let app = app.clone();
        std::thread::spawn(move || {
            refresh(&app);
            pull_settings(&app);
        });
    }
}

pub fn status(app: &AppHandle) -> Status {
    app.state::<State>().status.lock().unwrap().clone()
}

/// Profil à jour depuis le site ; un jeton refusé déconnecte (session expirée).
pub fn refresh(app: &AppHandle) {
    let Some(token) = load_token(app) else { return };
    change(app, |s| s.busy = true);
    match api("GET", "me", Some(&token), None) {
        Ok(data) => {
            let user: Option<User> = serde_json::from_value(data["user"].clone()).ok();
            let s = change(app, |s| {
                s.busy = false;
                s.error = None;
                s.error_code = None;
                if user.is_some() {
                    s.user = user;
                }
            });
            write_saved(app, &s);
        }
        Err(ApiError::Unauthorized) => {
            delete_token(app);
            let s = change(app, |s| {
                *s = Status::default();
                let (code, text) = error_text(&ApiError::Unauthorized);
                s.error = Some(text.into());
                s.error_code = Some(code.into());
            });
            write_saved(app, &s);
        }
        Err(e) => set_error(app, &e),
    }
}

/// Nom de l'ordinateur montré sur le site (« Morph · PC-DE-MARIE · Windows »).
fn device_name() -> String {
    let host = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
    let os = if cfg!(windows) { "Windows" } else if cfg!(target_os = "linux") { "Linux" } else { "" };
    let mut parts = vec!["Morph".to_string()];
    if !host.is_empty() {
        parts.push(host.chars().take(40).collect());
    }
    if !os.is_empty() {
        parts.push(os.into());
    }
    parts.join(" · ")
}

/// Demande un code, ouvre la page d'approbation du site et attend la réponse.
pub fn start_link(app: &AppHandle) {
    let generation = app.state::<State>().link_generation.fetch_add(1, Ordering::SeqCst) + 1;
    change(app, |s| {
        s.busy = true;
        s.error = None;
        s.error_code = None;
        s.link = None;
    });
    let lang = if crate::i18n::code() == "fr" { "fr" } else { "en" };
    let started = api("POST", "start", None, Some(json!({ "device": device_name(), "lang": lang })));
    let data = match started {
        Ok(d) => d,
        Err(e) => return set_error(app, &e),
    };
    let (Some(device_code), Some(user_code), Some(verify_url)) =
        (data["deviceCode"].as_str(), data["userCode"].as_str(), data["verifyUrl"].as_str())
    else {
        return set_error(app, &ApiError::Status(500));
    };
    // N'ouvre que des pages du site lui-même.
    if !verify_url.starts_with(&format!("{}/", site())) {
        return set_error(app, &ApiError::Status(500));
    }
    let interval = data["interval"].as_u64().unwrap_or(3).clamp(2, 30);
    let expires_in = data["expiresIn"].as_u64().unwrap_or(600).min(1800);
    let link = Link { user_code: user_code.into(), verify_url: verify_url.into(), expires_at_ms: now_ms() + expires_in * 1000 };
    change(app, |s| {
        s.busy = false;
        s.link = Some(link.clone());
    });
    let _ = tauri_plugin_opener::open_url(verify_url, None::<&str>);

    let device_code = device_code.to_string();
    let deadline = link.expires_at_ms;
    let app = app.clone();
    std::thread::spawn(move || {
        let current = || app.state::<State>().link_generation.load(Ordering::SeqCst) == generation;
        while current() && now_ms() < deadline {
            std::thread::sleep(Duration::from_secs(interval));
            if !current() {
                return;
            }
            match api("POST", "poll", None, Some(json!({ "deviceCode": device_code }))) {
                Ok(d) => match d["status"].as_str() {
                    Some("pending") => continue,
                    Some("approved") => {
                        let token = d["token"].as_str().unwrap_or_default();
                        if let Err(e) = store_token(&app, token) {
                            eprintln!("[morph] compte : jeton non enregistré : {e}");
                            return set_error(&app, &ApiError::Status(500));
                        }
                        let user: Option<User> = serde_json::from_value(d["user"].clone()).ok();
                        let s = change(&app, |s| {
                            s.link = None;
                            s.user = user;
                            s.error = None;
                            s.error_code = None;
                        });
                        write_saved(&app, &s);
                        return;
                    }
                    other => {
                        let (code, key) = if other == Some("denied") { ("denied", "core.account_denied") } else { ("expired", "core.account_code_expired") };
                        change(&app, |s| {
                            s.link = None;
                            s.error = Some(tr(key).into());
                            s.error_code = Some(code.into());
                        });
                        return;
                    }
                },
                // Coupure passagère : on réessaie jusqu'à l'expiration du code.
                Err(ApiError::Network) => continue,
                Err(e) => return set_error(&app, &e),
            }
        }
        if current() {
            change(&app, |s| {
                s.link = None;
                s.error = Some(tr("core.account_code_expired").into());
                s.error_code = Some("expired".into());
            });
        }
    });
}

pub fn cancel_link(app: &AppHandle) {
    app.state::<State>().link_generation.fetch_add(1, Ordering::SeqCst);
    change(app, |s| {
        s.link = None;
        s.busy = false;
    });
}

/// Déconnecte cet ordinateur : le jeton est révoqué sur le site (si possible) et effacé ici.
pub fn logout(app: &AppHandle) {
    if let Some(token) = load_token(app) {
        // Hors ligne, le jeton reste valable sur le site jusqu'à sa révocation depuis la page
        // du compte ; il est de toute façon effacé de cet ordinateur.
        let _ = api("POST", "logout", Some(&token), None);
    }
    delete_token(app);
    let s = change(app, |s| *s = Status::default());
    write_saved(app, &s);
}

// --- Synchronisation des préférences ---------------------------------------------------------

fn synced_part(s: &Settings) -> Value {
    let all = serde_json::to_value(s).unwrap_or(Value::Null);
    Value::Object(SYNCED.iter().filter_map(|k| Some((k.to_string(), all.get(*k)?.clone()))).collect())
}

/// Active ou coupe la synchronisation. À l'activation : les préférences du site
/// s'appliquent si elles existent, sinon celles de cet ordinateur y sont envoyées.
pub fn set_sync(app: &AppHandle, enabled: bool) {
    let s = change(app, |s| s.sync = enabled && s.user.is_some());
    write_saved(app, &s);
    if s.sync {
        let app = app.clone();
        std::thread::spawn(move || {
            if !pull_settings(&app) {
                push_settings(&app);
            }
        });
    }
}

/// Récupère les préférences du site si elles sont plus récentes ; vrai si le site en avait.
fn pull_settings(app: &AppHandle) -> bool {
    if !status(app).sync {
        return false;
    }
    let Some(token) = load_token(app) else { return false };
    change(app, |s| s.busy = true);
    let data = match api("GET", "settings", Some(&token), None) {
        Ok(d) => d,
        Err(e) => {
            set_error(app, &e);
            return false;
        }
    };
    let remote_at = data["updatedAt"].as_u64().unwrap_or(0);
    let Some(remote) = data["data"].as_object().filter(|o| !o.is_empty()) else {
        change(app, |s| s.busy = false);
        return false;
    };
    let last = status(app).last_sync_ms.unwrap_or(0);
    if remote_at > last {
        let mut merged = serde_json::to_value(settings::get(app)).unwrap_or(Value::Null);
        for k in SYNCED {
            if let Some(v) = remote.get(k) {
                merged[k] = v.clone();
            }
        }
        if let Ok(next) = serde_json::from_value::<Settings>(merged) {
            APPLYING_REMOTE.store(true, Ordering::SeqCst);
            let result = settings::set(app, next);
            APPLYING_REMOTE.store(false, Ordering::SeqCst);
            if let Err(e) = result {
                eprintln!("[morph] compte : préférences du site non appliquées : {e}");
            }
        }
    }
    let s = change(app, |s| {
        s.busy = false;
        s.error = None;
        s.error_code = None;
        s.last_sync_ms = Some(remote_at.max(last));
    });
    write_saved(app, &s);
    true
}

fn push_settings(app: &AppHandle) {
    let Some(token) = load_token(app) else { return };
    let at = now_ms();
    let body = json!({ "data": synced_part(&settings::get(app)), "updatedAt": at });
    match api("POST", "settings", Some(&token), Some(body)) {
        Ok(_) => {
            let s = change(app, |s| {
                s.error = None;
                s.error_code = None;
                s.last_sync_ms = Some(at);
            });
            write_saved(app, &s);
        }
        // Le site a plus récent (autre ordinateur) : on le récupère.
        Err(ApiError::Status(409)) => {
            pull_settings(app);
        }
        Err(e) => set_error(app, &e),
    }
}

/// Appelé après chaque changement des réglages : envoi regroupé (2 s) si la
/// synchronisation est active et que seules des préférences synchronisées ont changé.
pub fn settings_changed(app: &AppHandle, old: &Settings, new: &Settings) {
    if APPLYING_REMOTE.load(Ordering::SeqCst) || synced_part(old) == synced_part(new) {
        return;
    }
    if !status(app).sync {
        return;
    }
    let generation = PUSH_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        if PUSH_GENERATION.load(Ordering::SeqCst) == generation {
            push_settings(&app);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seules_les_preferences_synchronisees_partent() {
        let s = Settings { save_folder: Some("C:\\Users\\x".into()), autostart: true, ..Settings::default() };
        let v = synced_part(&s);
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        assert_eq!(keys.len(), SYNCED.len());
        assert!(v.get("saveFolder").is_none());
        assert!(v.get("autostart").is_none());
        assert!(v.get("gpu").is_none());
    }

    #[test]
    fn nom_de_l_ordinateur() {
        assert!(device_name().starts_with("Morph"));
        assert!(device_name().chars().count() <= 80);
    }

    #[cfg(windows)]
    #[test]
    fn jeton_chiffre_pour_la_session() {
        let token = b"mph_0123456789abcdef";
        let sealed = secret::protect(token).unwrap();
        assert_ne!(sealed.as_slice(), token.as_slice());
        assert_eq!(secret::unprotect(&sealed).unwrap(), token);
    }
}
