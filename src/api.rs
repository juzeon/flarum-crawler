use crate::entity::{Discussion, Post};
use anyhow::{Context, anyhow, bail};
use derive_builder::Builder;
use regex::Regex;
use reqwest::{Client, StatusCode};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{debug, info, instrument};

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("discussion is impossible to fetch")]
    ImpossibleDiscussion,
}

#[derive(Debug, Clone, Builder)]
pub struct GetDiscussionOptions {
    pub base_url: String,
    #[builder(default = 20)]
    pub concurrency: usize,
}
static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap()
});
fn get_http_client() -> Client {
    HTTP_CLIENT.clone()
}
#[instrument(skip_all)]
pub async fn get_discussion(
    id: u64,
    options: GetDiscussionOptions,
    sem: Option<Arc<Semaphore>>,
) -> anyhow::Result<Discussion> {
    let sem = sem.unwrap_or_else(|| Arc::new(Semaphore::new(options.concurrency)));
    let base_url = Arc::new(options.base_url.to_string());
    let client = get_http_client();
    let sem_quota = sem.acquire().await?;
    debug!(id, "Processing discussion");
    let response = client
        .get(format!(
            "{}/api/discussions/{}?bySlug=true&page[near]=0",
            base_url, id
        ))
        .send()
        .await?;
    if [404u16, 403u16].contains(&response.status().as_u16()) {
        return Err(anyhow!(ApiError::ImpossibleDiscussion));
    }
    if response.status() != 200 {
        bail!("response error status: {}", response.status())
    }
    let discussion_json: serde_json::Value = response.json().await?;
    drop(sem_quota);
    let title = discussion_json["data"]["attributes"]["title"]
        .as_str()
        .context("no title")?
        .to_string();
    let is_frontpage = discussion_json["data"]["attributes"]["frontpage"]
        .as_bool()
        .context("cannot get frontpage")?;
    let vec = vec![];
    let tag_ids = discussion_json["data"]["relationships"]["tags"]["data"]
        .as_array()
        .unwrap_or(&vec)
        .iter()
        .map(|x| x["id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    let tags = discussion_json["included"]
        .as_array()
        .unwrap_or(&vec)
        .iter()
        .filter_map(|x| {
            if x["type"].as_str().unwrap_or_default() != "tags" {
                return None;
            }
            if !tag_ids.contains(&x["id"].as_str().unwrap_or_default()) {
                return None;
            }
            Some(
                x["attributes"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect::<Vec<_>>();
    let post_ids = discussion_json["data"]["relationships"]["posts"]["data"]
        .as_array()
        .unwrap_or(&vec)
        .iter()
        .filter_map(|item| {
            if item["type"].as_str().unwrap_or_default() == "posts" {
                Some(item["id"].as_str().unwrap_or_default().to_string())
            } else {
                None
            }
        })
        .collect::<Vec<String>>();
    let total = (post_ids.len() as f64 / 20f64).ceil() as usize;
    let mut set = JoinSet::new();
    for (ix, post_id_group) in post_ids.chunks(20).map(|x| x.to_vec()).enumerate() {
        let sem_clone = sem.clone();
        let base_url = base_url.clone();
        set.spawn(async move {
            let _sem = sem_clone.acquire().await.unwrap();
            debug!(ix, total, id, "Processing post chunks");
            let res = get_post_id_group(id, base_url.as_str(), post_id_group).await?;
            Ok(res)
        });
    }
    let mut post_groups = set
        .join_all()
        .await
        .into_iter()
        .collect::<anyhow::Result<Vec<_>>>()?;
    post_groups.sort_by_key(|x| x[0].id);
    let posts = post_groups.into_iter().flatten().collect::<Vec<_>>();
    let first_post = if let Some(post) = posts.first() {
        post
    } else {
        &Post::default()
    };
    Ok(Discussion {
        id,
        user_id: first_post.user_id,
        username: first_post.username.to_string(),
        user_display_name: first_post.user_display_name.to_string(),
        title,
        tags,
        is_frontpage,
        created_at: first_post.created_at,
        posts,
    })
}

static POST_MENTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"class="PostMention" data-id="(\d+)""#).unwrap());
static POST_MENTION_A_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<a.*?class="PostMention".*?</a>"#).unwrap());
async fn get_post_id_group(
    discussion_id: u64,
    base_url: &str,
    post_id_group: Vec<String>,
) -> anyhow::Result<Vec<Post>> {
    let client = get_http_client();
    let url = format!(
        "{}/api/posts?filter[id]={}",
        base_url,
        post_id_group.join(",")
    );
    let post_json: serde_json::Value = client.get(url.as_str()).send().await?.json().await?;
    let vec = vec![];
    let users = post_json["included"]
        .as_array()
        .unwrap_or(&vec)
        .iter()
        .filter_map(|item| {
            if item["type"].as_str().unwrap_or_default() != "users" {
                return None;
            }
            Some((
                item["id"]
                    .as_str()
                    .unwrap_or_default()
                    .parse::<u64>()
                    .unwrap_or_default(),
                (
                    item["attributes"]["username"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    item["attributes"]["displayName"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                ),
            ))
        })
        .collect::<HashMap<u64, (String, String)>>();
    let posts = post_json["data"]
        .as_array()
        .unwrap_or(&vec)
        .iter()
        .filter_map(|item| {
            if item["type"].as_str().unwrap_or_default() != "posts" {
                return None;
            }
            if item["attributes"]["contentType"]
                .as_str()
                .unwrap_or_default()
                != "comment"
            {
                return None;
            }
            let html = item["attributes"]["contentHtml"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let created_at = chrono::DateTime::parse_from_rfc3339(
                item["attributes"]["createdAt"].as_str().unwrap_or_default(),
            )
            .unwrap_or_default();
            let reply_to_id = if let Some(caps) = POST_MENTION_RE.captures(html.as_str()) {
                caps[1].parse::<u64>().unwrap_or_default()
            } else {
                0
            };
            let user_id = item["relationships"]["user"]["data"]["id"]
                .as_str()
                .unwrap_or_default()
                .parse::<u64>()
                .unwrap_or_default();
            let content = htmd::convert(POST_MENTION_A_RE.replace_all(html.as_str(), "").as_ref())
                .unwrap_or(format!("<!-- HTML -->{}", html.as_str()))
                .trim()
                .to_string();
            let _user_tmp = &("".to_string(), "".to_string());
            let user = users.get(&user_id).unwrap_or(_user_tmp);
            Some(Post {
                id: item["id"]
                    .as_str()
                    .unwrap_or_default()
                    .parse::<u64>()
                    .unwrap_or_default(),
                reply_to_id,
                user_id,
                username: user.0.to_string(),
                user_display_name: user.1.to_string(),
                content,
                created_at,
                discussion_id,
            })
        })
        .collect::<Vec<Post>>();
    Ok(posts)
}
