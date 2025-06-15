use crate::entity::{Discussion, DiscussionExtended};
use crate::server::{AppError, AppState};
use actix_web::{HttpResponse, Responder, get, web};
use anyhow::Context;

#[get("/")]
async fn index() -> impl Responder {
    "flarum-crawler"
}
#[get("/discussion/{id}")]
pub async fn get_discussion(
    path: web::Path<u64>,
    state: web::Data<AppState>,
) -> Result<impl Responder, AppError> {
    let id = path.into_inner();
    let discussion = Discussion::find_by_id_extended(id, &state.conn)
        .await
        .context("cannot find discussion")?;
    Ok(HttpResponse::Ok().json(discussion))
}

pub async fn list_discussion() -> impl Responder {
    HttpResponse::Ok().finish()
}
