use crate::entity::{Discussion, Post};
use anyhow::{Context, anyhow, bail};
use derive_builder::Builder;
use regex::Regex;
use reqwest::{Client, StatusCode};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{debug, info, instrument};

pub enum GetDiscussionResult {
    Impossible,
    Ok(Discussion),
    PartialError(Discussion),
}

#[derive(Debug, Clone, Builder)]
pub struct GetDiscussionOptions {
    pub base_url: String,
    #[builder(default = 20)]
    pub concurrency: usize,
    #[builder(default=HashSet::new())]
    pub existing_post_ids: HashSet<u64>,
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
pub async fn get_index_page(base_url: &str, page: usize) -> anyhow::Result<Vec<u64>> {
    let client = get_http_client();
    debug!(page, "Getting index page");
    let response = client
        .get(format!(
            "{}/api/discussions?\
    include=user,lastPostedUser,tags,tags.parent,firstPost,recipientUsers,recipientGroups&sort=\
    &page[offset]={}",
            base_url,
            (page - 1) * 20
        ))
        .send()
        .await?;
    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(err) => {
            bail!("response error status: {}", err);
        }
    };
    let payload: serde_json::Value = response.json().await?;
    let vec = vec![];
    let ids = payload["data"]
        .as_array()
        .unwrap_or(&vec)
        .iter()
        .filter_map(|x| {
            if x["type"] != "discussions" {
                return None;
            }
            Some(
                x["id"]
                    .as_str()
                    .unwrap_or_default()
                    .parse::<u64>()
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    debug!(len = ids.len(), page, "Got ids from index page");
    Ok(ids)
}
#[instrument(skip_all)]
pub async fn get_discussion(
    id: u64,
    options: GetDiscussionOptions,
    sem: Option<Arc<Semaphore>>,
) -> anyhow::Result<GetDiscussionResult> {
    let sem = sem.unwrap_or_else(|| Arc::new(Semaphore::new(options.concurrency)));
    let base_url = Arc::new(options.base_url.to_string());
    let client = get_http_client();
    let sem_quota = sem.acquire().await?;
    debug!(id, "Processing api/discussion");
    let response = client
        .get(format!(
            "{}/api/discussions/{}?bySlug=true&page[near]=0",
            base_url, id
        ))
        .send()
        .await?;
    debug!(id, "Finished api/discussion");
    if [404u16, 403u16].contains(&response.status().as_u16()) {
        return Ok(GetDiscussionResult::Impossible);
    }
    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(err) => {
            bail!("response error status: {}", err);
        }
    };
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
                let post_id = item["id"].as_str().unwrap_or_default().to_string();
                if options
                    .existing_post_ids
                    .contains(&post_id.parse::<u64>().unwrap_or_default())
                {
                    return None;
                }
                Some(post_id)
            } else {
                None
            }
        })
        .collect::<Vec<String>>();
    let users_map = get_users_map(&discussion_json["included"]);
    let user_id = discussion_json["data"]["relationships"]["user"]["data"]["id"]
        .as_str()
        .unwrap_or_default()
        .parse::<u64>()?;
    let (username, user_display_name) = users_map
        .get(&user_id)
        .map(|x| (x.0.to_string(), x.1.to_string()))
        .unwrap_or_default();
    let created_at = chrono::DateTime::parse_from_rfc3339(
        discussion_json["data"]["attributes"]["createdAt"]
            .as_str()
            .unwrap_or_default(),
    )?;
    let total = (post_ids.len() as f64 / 20f64).ceil() as usize;
    let mut set = JoinSet::new();
    let mut post_id_group_count = 0;
    let mut is_partial = false;
    let posts = if !post_ids.is_empty() {
        for (ix, post_id_group) in post_ids.chunks(20).map(|x| x.to_vec()).enumerate() {
            let sem_clone = sem.clone();
            let base_url = base_url.clone();
            post_id_group_count += 1;
            set.spawn(async move {
                let _sem = sem_clone.acquire().await.unwrap();
                debug!(ix, total, id, "Processing api/post chunks");
                let res = get_post_id_group(id, base_url.as_str(), post_id_group).await?;
                debug!(ix, total, id, "Finished api/post chunks");
                Ok(res)
            });
        }
        let mut post_groups = set
            .join_all()
            .await
            .into_iter()
            .filter_map(|x: anyhow::Result<Vec<Post>>| x.ok())
            .collect::<Vec<_>>();
        post_groups.sort_by_key(|x| x[0].id);
        is_partial = post_groups.len() != post_id_group_count;
        post_groups.into_iter().flatten().collect::<Vec<_>>()
    } else {
        vec![]
    };
    let discussion = Discussion {
        id,
        user_id,
        username,
        user_display_name,
        title,
        tags,
        is_frontpage,
        created_at,
        posts,
    };
    if is_partial {
        Ok(GetDiscussionResult::PartialError(discussion))
    } else {
        Ok(GetDiscussionResult::Ok(discussion))
    }
}

static POST_MENTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"class="PostMention" data-id="(\d+)""#).unwrap());
static POST_MENTION_A_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<a.*?class="PostMention".*?</a>"#).unwrap());
static POST_MENTION_DELETED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<span class="PostMention PostMention--deleted".*?</span>"#).unwrap()
});
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
    let response = client.get(url.as_str()).send().await?;
    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(err) => {
            bail!("response error status: {}", err);
        }
    };
    let post_json: serde_json::Value = response.json().await?;
    let vec = vec![];
    let users = get_users_map(&post_json["included"]);
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
            let mut html = item["attributes"]["contentHtml"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            html = POST_MENTION_DELETED.replace_all(&html, "").to_string();
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
fn get_users_map(arr_v: &serde_json::Value) -> HashMap<u64, (String, String)> {
    let vec = vec![];
    arr_v
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
        .collect::<HashMap<u64, (String, String)>>()
}
