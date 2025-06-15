mod service;

use crate::config::Config;
use crate::server::service::index;
use actix_web::{App, HttpServer, web};
use sqlx::SqlitePool;
use tracing::{info, instrument};

#[derive(Clone)]
pub struct AppState {
    pub conn: SqlitePool,
    pub config: Config,
}

#[instrument(skip(state))]
pub async fn run_server(addr: String, port: u16, state: AppState) {
    info!("Starting server");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .service(index)
    })
    .bind((addr, port))
    .unwrap()
    .run()
    .await
    .unwrap();
}
