mod service;

use crate::config::Config;
use crate::server::service::{get_discussion, index};
use actix_cors::Cors;
use actix_web::body::BoxBody;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, ResponseError, web};
use serde::Serialize;
use serde_json::json;
use sqlx::SqlitePool;
use thiserror::Error;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct AppState {
    pub conn: SqlitePool,
    pub config: Config,
}

#[derive(Debug, Error)]
#[error(transparent)]
pub struct AppError(#[from] anyhow::Error);
impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::InternalServerError().json(json!({
            "message": format!("{:#}", self.0)
        }))
    }
}

#[instrument(skip(state))]
pub async fn run_server(addr: String, port: u16, state: AppState) {
    info!("Starting server");
    HttpServer::new(move || {
        App::new()
            .wrap(Cors::permissive())
            .app_data(web::Data::new(state.clone()))
            .service(index)
            .service(get_discussion)
    })
    .bind((addr, port))
    .unwrap()
    .run()
    .await
    .unwrap();
}
