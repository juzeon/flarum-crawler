use actix_web::{Responder, get};

#[get("/")]
async fn index() -> impl Responder {
    "flarum-crawler"
}
