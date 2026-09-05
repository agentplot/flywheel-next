//! flywheel-surface: the plan page. One axum process serves the plan and the
//! running work from the store and takes responses; every response is applied
//! on its own and the machinery settles before the page re-renders.

use axum::{extract::State, response::{Html, IntoResponse}, routing::{get, post}, Json, Router};
use flywheel_scenario::Runtime;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone)]
struct App {
    rt: Arc<Mutex<Runtime>>,
    state_path: PathBuf,
}

pub async fn serve(rt: Runtime, state_path: PathBuf, port: u16) -> anyhow::Result<()> {
    let app = App { rt: Arc::new(Mutex::new(rt)), state_path };
    let router = Router::new()
        .route("/", get(page))
        .route("/api/plan", get(api_plan))
        .route("/api/respond", post(api_respond))
        .route("/api/dictate", post(api_dictate))
        .route("/api/tick", post(api_tick))
        .with_state(app);
    let addr = format!("0.0.0.0:{port}");
    println!("plan page on http://localhost:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, router).await?;
    Ok(())
}

fn plan_json(rt: &mut Runtime) -> Value {
    let decisions = rt.decisions();
    let store = &rt.store;
    let dec: Vec<Value> = decisions.iter().map(|d| {
        let obj = store.objects.get(&d.object);
        let parent = obj.and_then(|o| o.parent.clone());
        let record = obj.map(|o| Value::Object(o.record.iter().map(|(k, v)| (k.clone(), v.clone())).collect())).unwrap_or(Value::Null);
        let folded = d.kind == "elaboration-proposed" && parent.as_ref().and_then(|p| store.objects.get(p)).and_then(|p| p.top_state().map(|s| s == "proposed")).unwrap_or(false);
        json!({
            "number": d.number, "id": d.id, "object": d.object, "kind": d.kind, "group": d.group,
            "answers": d.answers, "shows": d.shows, "since": d.since, "parent": parent, "record": record,
            "machine": obj.map(|o| o.machine.clone()), "folded": folded,
        })
    }).collect();
    let running: Vec<Value> = store.world.sessions.iter().filter(|(_, s)| s.pane).map(|(k, s)| {
        let (object, stage) = k.rsplit_once('/').unwrap_or((k, ""));
        let host = store.objects.get(object).and_then(|o| o.record.get("host").cloned()).unwrap_or(json!("studio"));
        let unit = store.objects.get(object).and_then(|o| o.parent.clone());
        let bolt = unit.as_ref().and_then(|u| store.objects.get(u)).and_then(|u| u.parent.clone());
        json!({"session": k, "object": object, "stage": stage, "activity": s.activity, "exit": s.exit, "question": s.question, "host": host, "unit": unit, "bolt": bolt, "ticks": s.ticks_alive})
    }).collect();
    let objects: Vec<Value> = store.objects.values().filter(|o| matches!(o.machine.as_str(), "bolt" | "unit" | "work-item" | "intent" | "elaboration")).map(|o| json!({
        "id": o.id, "machine": o.machine, "parent": o.parent, "state": o.top_state(), "record": o.record, "config": o.config,
    })).collect();
    let log: Vec<Value> = store.log.iter().rev().take(40).map(|l| json!({"tick": l.tick, "kind": l.kind, "object": l.object, "text": l.text})).collect();
    let responses: Vec<Value> = store.responses.iter().rev().take(40).map(|r| json!({"id": r.id, "decision": r.decision, "object": r.object, "answer": r.answer, "at": r.given_at})).collect();
    json!({
        "now": store.now, "tick": store.tick, "scenario": store.scenario,
        "decisions": dec, "tail": store.tail, "running": running, "objects": objects, "log": log, "responses": responses,
        "host_bound": store.host_bound,
    })
}

async fn api_plan(State(app): State<App>) -> impl IntoResponse {
    let mut rt = app.rt.lock().await;
    Json(plan_json(&mut rt))
}

#[derive(Deserialize)]
struct RespondIn { number: u32, answer: String, #[serde(default)] by: Option<String> }

async fn api_respond(State(app): State<App>, Json(input): Json<RespondIn>) -> impl IntoResponse {
    let mut rt = app.rt.lock().await;
    let id = rt.respond(input.number, &input.answer, input.by.as_deref().unwrap_or("page"));
    let fired = rt.settle(50);
    let _ = flywheel_scenario::save(&rt.store, &app.state_path);
    let mut v = plan_json(&mut rt);
    v["applied"] = json!({"id": id, "fired": fired});
    Json(v)
}

#[derive(Deserialize)]
struct DictateIn { object: String, answer: String, #[serde(default)] by: Option<String> }

async fn api_dictate(State(app): State<App>, Json(input): Json<DictateIn>) -> impl IntoResponse {
    let mut rt = app.rt.lock().await;
    let id = rt.dictate(&input.object, &input.answer, input.by.as_deref().unwrap_or("page"));
    let fired = rt.settle(50);
    let _ = flywheel_scenario::save(&rt.store, &app.state_path);
    let mut v = plan_json(&mut rt);
    v["applied"] = json!({"id": id, "fired": fired});
    Json(v)
}

#[derive(Deserialize)]
struct TickIn { #[serde(default)] n: usize }

async fn api_tick(State(app): State<App>, Json(input): Json<TickIn>) -> impl IntoResponse {
    let mut rt = app.rt.lock().await;
    let fired = if input.n == 0 { rt.settle(50) } else { (0..input.n).map(|_| rt.tick()).sum() };
    let _ = flywheel_scenario::save(&rt.store, &app.state_path);
    let mut v = plan_json(&mut rt);
    v["applied"] = json!({"fired": fired});
    Json(v)
}

async fn page() -> Html<&'static str> {
    Html(PAGE)
}

const PAGE: &str = include_str!("page.html");
