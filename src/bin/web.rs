//! Single binary web server: HTML from templates/, static from /static, API via REST.
//! Run with: cargo run --bin web
//! Listens on 0.0.0.0:8080 by default so the app is reachable via DNS on a VPS.
//! Override with env: HOST (e.g. 0.0.0.0), PORT (e.g. 8080).
//! Whole-site password gate: correct password is `SITE_GATE_PLAIN` in this file.
//! After POST `/api/site-gate`, the client stores the returned token (sessionStorage) and sends
//! header `X-Dart-Site-Gate` on requests; no cookie (avoids browser cookie UI / SameSite quirks).

use actix_files::Files;
use actix_web::body::BoxBody;
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::{from_fn, Next};
use actix_web::{
    delete, get, post, put,
    web::{self, Data, Json, Path},
    App, Error, HttpRequest, HttpResponse, HttpServer, Responder,
};
use dart_tournament_web::{
    add_players_back_from_last_eliminated, generate_group_play_matches,
    generate_semi_final_matches, process_finals_results, process_group_play_results,
    process_semi_final_results, set_finals_match_winner, start_semi_finals, start_tournament, Team,
    Tournament, TournamentId,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use subtle::ConstantTimeEq;
use uuid::Uuid;

/// Per-tournament entry: tournament data + last activity time (for auto-cleanup).
struct TournamentEntry {
    tournament: Tournament,
    last_activity: Instant,
}

/// Plaintext site password (intentionally not secret for this deployment).
const SITE_GATE_PLAIN: &str = "bøh";

/// Expected value for `X-Dart-Site-Gate` (hex digest of password + salt), constant-time compared.
#[derive(Clone)]
struct SiteGate {
    expected_token: String,
}

impl SiteGate {
    fn new() -> Self {
        Self {
            expected_token: site_gate_token(SITE_GATE_PLAIN),
        }
    }
}

/// Header the browser sends after successful `/api/site-gate` (must match JS `site-gate-temp.js`).
const SITE_GATE_HEADER: &str = "x-dart-site-gate";

fn site_gate_token(password: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"dart-site-gate-v1\x00");
    h.update(password.as_bytes());
    hex::encode(h.finalize())
}

fn site_gate_header_ok(req: &HttpRequest, gate: &SiteGate) -> bool {
    let Some(h) = req.headers().get(SITE_GATE_HEADER) else {
        return false;
    };
    let Ok(s) = h.to_str() else {
        return false;
    };
    let expected = gate.expected_token.as_str();
    if s.len() != expected.len() {
        return false;
    }
    s.as_bytes().ct_eq(expected.as_bytes()).into()
}

async fn site_gate_middleware(
    req: ServiceRequest,
    next: Next<BoxBody>,
) -> Result<ServiceResponse<BoxBody>, Error> {
    let gate = req
        .app_data::<web::Data<SiteGate>>()
        .expect("SiteGate missing")
        .get_ref()
        .clone();

    let path = req.path().to_string();
    let method = req.method().clone();

    let exempt = path == "/api/health"
        || path == "/favicon.ico"
        || path.starts_with("/static/")
        || (path == "/" && method == actix_web::http::Method::GET)
        || (path == "/api/site-gate/check" && method == actix_web::http::Method::GET)
        || (path == "/api/site-gate" && method == actix_web::http::Method::POST);

    if exempt || site_gate_header_ok(req.request(), &gate) {
        return next.call(req).await;
    }

    Ok(
        req.into_response(HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Site password required"
        }))),
    )
}

/// In-memory state: many tournaments by ID (sessioned). Entries are removed after 6h inactivity.
type AppState = Data<RwLock<HashMap<TournamentId, TournamentEntry>>>;

/// Inactivity threshold: tournaments not accessed for this long are removed.
const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(12 * 3600);

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
    service: &'static str,
}

#[derive(Deserialize)]
struct CreateTournamentBody {
    #[serde(default = "default_max_losses")]
    max_losses: u32,
    #[serde(default)]
    mode: dart_tournament_web::TournamentMode,
}

#[derive(Deserialize)]
struct SiteGateLoginBody {
    password: String,
}

fn default_max_losses() -> u32 {
    3
}

#[derive(Deserialize)]
struct AddPlayerBody {
    name: String,
}

#[derive(Deserialize)]
struct MaxLossesBody {
    max_losses: u32,
}

#[derive(Deserialize)]
struct SetMatchWinnerBody {
    match_id: Uuid,
    team: Team,
}

#[derive(Deserialize)]
struct FinalSelectionAddBackBody {
    player_ids: Vec<Uuid>,
}

#[derive(Deserialize)]
struct SetPlayerLossesBody {
    losses: u32,
}

#[derive(Deserialize)]
struct SetModeBody {
    mode: dart_tournament_web::TournamentMode,
}

/// Path segment: tournament id (e.g. /api/tournaments/{id})
#[derive(Deserialize)]
struct TournamentPath {
    id: TournamentId,
}

/// Path segments: tournament id and player id (e.g. /api/tournaments/{id}/players/{player_id})
#[derive(Deserialize)]
struct TournamentPlayerPath {
    id: TournamentId,
    player_id: Uuid,
}

#[get("/api/health")]
async fn api_health() -> impl Responder {
    HttpResponse::Ok().json(HealthResponse {
        ok: true,
        service: "dart-tournament-web",
    })
}

/// Avoid 404 in browser tab: favicon not required for app logic.
#[get("/favicon.ico")]
async fn favicon() -> HttpResponse {
    HttpResponse::NoContent().finish()
}

/// Returns 204 if `X-Dart-Site-Gate` matches; 401 if not unlocked yet.
#[get("/api/site-gate/check")]
async fn api_site_gate_check(req: HttpRequest, gate: web::Data<SiteGate>) -> HttpResponse {
    if site_gate_header_ok(&req, gate.get_ref()) {
        HttpResponse::NoContent().finish()
    } else {
        HttpResponse::Unauthorized().finish()
    }
}

/// POST JSON `{ "password": "..." }` — returns `{ "token": "<hex>" }` on success (must match [`SITE_GATE_PLAIN`]).
#[post("/api/site-gate")]
async fn api_site_gate_login(
    gate: web::Data<SiteGate>,
    body: Json<SiteGateLoginBody>,
) -> HttpResponse {
    let expected = gate.expected_token.as_str();
    let got = site_gate_token(body.password.trim());
    if got.as_bytes().ct_eq(expected.as_bytes()).into() {
        HttpResponse::Ok().json(serde_json::json!({ "token": expected }))
    } else {
        HttpResponse::Unauthorized().json(serde_json::json!({ "error": "Wrong password" }))
    }
}

/// Create a new tournament.
#[post("/api/tournaments")]
async fn api_create_tournament(
    state: AppState,
    body: Option<Json<CreateTournamentBody>>,
) -> HttpResponse {
    let max_losses = body
        .as_ref()
        .map(|b| b.max_losses)
        .unwrap_or_else(default_max_losses);
    let mode = body
        .as_ref()
        .map(|b| b.mode)
        .unwrap_or(dart_tournament_web::TournamentMode::TwoVTwo);

    let tournament = Tournament::new(max_losses, mode);
    let id = tournament.id;
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    g.insert(
        id,
        TournamentEntry {
            tournament,
            last_activity: Instant::now(),
        },
    );
    let entry = g.get(&id).unwrap();
    HttpResponse::Ok().json(&entry.tournament)
}

/// Get a tournament by id (404 if not found). Touching it refreshes last_activity.
#[get("/api/tournaments/{id}")]
async fn api_get_tournament(state: AppState, path: Path<TournamentPath>) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    match g.get_mut(&path.id) {
        Some(entry) => {
            entry.last_activity = Instant::now();
            HttpResponse::Ok().json(&entry.tournament)
        }
        None => HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" })),
    }
}

#[post("/api/tournaments/{id}/players")]
async fn api_add_player(
    state: AppState,
    path: Path<TournamentPath>,
    body: Json<AddPlayerBody>,
) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match t.add_player(body.name.trim()) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Remove a player by id (tournament must be in Setup).
#[delete("/api/tournaments/{id}/players/{player_id}")]
async fn api_remove_player(state: AppState, path: Path<TournamentPlayerPath>) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match t.remove_player(path.player_id) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Update max losses (tournament must be in Setup).
#[put("/api/tournaments/{id}/max-losses")]
async fn api_set_max_losses(
    state: AppState,
    path: Path<TournamentPath>,
    body: Json<MaxLossesBody>,
) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match t.set_max_losses(body.max_losses) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Start the tournament (Setup -> GroupPlay or FinalSelection).
#[post("/api/tournaments/{id}/start")]
async fn api_start_tournament(state: AppState, path: Path<TournamentPath>) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match start_tournament(t) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Generate group play matches (tournament must be in GroupPlay).
#[post("/api/tournaments/{id}/matches/generate")]
async fn api_generate_matches(state: AppState, path: Path<TournamentPath>) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match generate_group_play_matches(t) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Set winner for one match (tournament must be in GroupPlay).
#[put("/api/tournaments/{id}/matches/winner")]
async fn api_set_match_winner(
    state: AppState,
    path: Path<TournamentPath>,
    body: Json<SetMatchWinnerBody>,
) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    if !t.matches.iter().any(|m| m.id == body.match_id) {
        return HttpResponse::BadRequest().json(serde_json::json!({ "error": "Match not found" }));
    }
    t.match_results.insert(body.match_id, body.team);
    HttpResponse::Ok().json(t)
}

/// Submit group play results and process (tournament must be in GroupPlay).
#[post("/api/tournaments/{id}/matches/submit")]
async fn api_submit_match_results(state: AppState, path: Path<TournamentPath>) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match process_group_play_results(t) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Set a player's losses manually (GroupPlay or FinalSelection).
#[put("/api/tournaments/{id}/players/{player_id}/losses")]
async fn api_set_player_losses(
    state: AppState,
    path: Path<TournamentPlayerPath>,
    body: Json<SetPlayerLossesBody>,
) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match t.set_player_losses(path.player_id, body.losses) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Manually eliminate a player (GroupPlay or FinalSelection).
#[post("/api/tournaments/{id}/players/{player_id}/eliminate")]
async fn api_eliminate_player(state: AppState, path: Path<TournamentPlayerPath>) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match t.eliminate_player(path.player_id) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Set tournament mode 1v1 or 2v2 (Setup only).
#[put("/api/tournaments/{id}/mode")]
async fn api_set_mode(
    state: AppState,
    path: Path<TournamentPath>,
    body: Json<SetModeBody>,
) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match t.set_mode(body.mode) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Restart tournament: back to Setup with same player names.
#[post("/api/tournaments/{id}/restart")]
async fn api_restart_tournament(state: AppState, path: Path<TournamentPath>) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match t.restart_tournament() {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Add selected players from last eliminated back to reach 8 (FinalSelection only).
#[post("/api/tournaments/{id}/final-selection/add-back")]
async fn api_final_selection_add_back(
    state: AppState,
    path: Path<TournamentPath>,
    body: Json<FinalSelectionAddBackBody>,
) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match add_players_back_from_last_eliminated(t, &body.player_ids) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Transition to semi-finals when 8 players in final selection (no add-back needed).
#[post("/api/tournaments/{id}/final-selection/start-semi")]
async fn api_final_selection_start_semi(
    state: AppState,
    path: Path<TournamentPath>,
) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match start_semi_finals(t) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": format!("{}", e) })),
    }
}

/// Generate semi-final matches (SemiFinals only, 8 players).
#[post("/api/tournaments/{id}/finals/matches")]
async fn api_finals_generate_matches(state: AppState, path: Path<TournamentPath>) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match generate_semi_final_matches(t) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Set winner for a final-round match (semi, finals, or grand finals).
#[put("/api/tournaments/{id}/finals/winner")]
async fn api_finals_set_winner(
    state: AppState,
    path: Path<TournamentPath>,
    body: Json<SetMatchWinnerBody>,
) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    match set_finals_match_winner(t, body.match_id, body.team) {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

/// Submit current final round (semi → finals, finals → completed).
#[post("/api/tournaments/{id}/finals/submit")]
async fn api_finals_submit(state: AppState, path: Path<TournamentPath>) -> HttpResponse {
    let mut g = match state.write() {
        Ok(guard) => guard,
        Err(_) => return HttpResponse::InternalServerError().body("lock error"),
    };
    let entry = match g.get_mut(&path.id) {
        Some(e) => e,
        None => {
            return HttpResponse::NotFound().json(serde_json::json!({ "error": "No tournament" }))
        }
    };
    entry.last_activity = Instant::now();
    let t = &mut entry.tournament;
    let result = match t.state {
        dart_tournament_web::TournamentState::SemiFinals => process_semi_final_results(t),
        dart_tournament_web::TournamentState::Finals => process_finals_results(t),
        _ => Err(dart_tournament_web::TournamentError::InvalidState),
    };
    match result {
        Ok(()) => HttpResponse::Ok().json(t),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e.to_string() })),
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let host = std::env::var("HOST").unwrap_or_else(|_| default_host());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or_else(default_port);
    let bind = (host.as_str(), port);
    log::info!("Starting server at http://{}:{}", bind.0, bind.1);

    let state = Data::new(RwLock::new(HashMap::<TournamentId, TournamentEntry>::new()));
    let site_gate = web::Data::new(SiteGate::new());
    log::info!("Site gate active (see SITE_GATE_PLAIN in web.rs)");

    // Background task: every 30 minutes, remove tournaments inactive for 12+ hours
    let state_cleanup = state.clone();
    actix_web::rt::spawn(async move {
        let mut interval = actix_web::rt::time::interval(Duration::from_secs(30 * 60));
        loop {
            interval.tick().await;
            let mut g = match state_cleanup.write() {
                Ok(guard) => guard,
                Err(_) => continue,
            };
            let before = g.len();
            g.retain(|_, entry| entry.last_activity.elapsed() < INACTIVITY_TIMEOUT);
            let removed = before - g.len();
            if removed > 0 {
                log::info!(
                    "Cleaned up {} inactive tournament(s) (no activity for 12h)",
                    removed
                );
            }
        }
    });

    HttpServer::new(move || {
        App::new()
            .wrap(from_fn(site_gate_middleware))
            .app_data(state.clone())
            .app_data(site_gate.clone())
            .route("/", web::get().to(serve_index_async))
            .service(api_health)
            .service(favicon)
            .service(api_site_gate_check)
            .service(api_site_gate_login)
            .service(api_create_tournament)
            .service(api_get_tournament)
            .service(api_add_player)
            .service(api_remove_player)
            .service(api_set_max_losses)
            .service(api_set_mode)
            .service(api_start_tournament)
            .service(api_generate_matches)
            .service(api_set_match_winner)
            .service(api_submit_match_results)
            .service(api_set_player_losses)
            .service(api_eliminate_player)
            .service(api_restart_tournament)
            .service(api_final_selection_add_back)
            .service(api_final_selection_start_semi)
            .service(api_finals_generate_matches)
            .service(api_finals_set_winner)
            .service(api_finals_submit)
            .service(Files::new("/static", "static").show_files_listing())
    })
    .bind(bind)?
    .run()
    .await
}

async fn serve_index_async() -> HttpResponse {
    let html = include_str!("../../templates/index.html");
    let html = html.replacen(
        "</head>",
        r#"<script src="/static/site-gate-temp.js"></script></head>"#,
        1,
    );
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(html)
}
