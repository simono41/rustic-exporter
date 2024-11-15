mod cli;
mod collector;
mod config;

use config::Config;

use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use clap::Parser;
use prometheus_client::{encoding::text::encode, registry::Registry};
use regex::Regex;
use std::{
    env, fs,
    sync::{Arc, Mutex},
};

async fn metrics_handler(State(state): State<Arc<Mutex<Registry>>>) -> impl IntoResponse {
    let registry = state.lock().unwrap();
    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(buffer))
        .unwrap()
}

fn replace_with_env_vars(input: &str) -> String {
    let re = Regex::new(r"\$\{(.*)\}").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let var_name = caps[1].to_string();
        env::var(var_name).unwrap_or_default()
    })
    .to_string()
}

#[tokio::main]
async fn main() {
    let args = cli::Args::parse();

    let mut registry = Registry::default();

    let mut file_content = fs::read_to_string(args.config.clone()).unwrap();
    file_content = replace_with_env_vars(&file_content);

    let config: Config = toml::from_str(&file_content).unwrap();
    for backup in config.backups {
        let collector = collector::RusticCollector::new(backup, args.interval);
        registry.register_collector(Box::new(collector));
    }
    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let registry_state = Arc::new(Mutex::new(registry));
    let router = Router::new()
        .route("/metrics", get(metrics_handler))
        .with_state(registry_state);
    axum::serve(listener, router).await.unwrap();
}
