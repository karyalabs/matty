//! GIPHY, for the GIF picker (#958): the trending and search lists the
//! webapp's picker shows, asked of GIPHY directly with the server's
//! `GiphySdkKey`, as the webapp does. No session token goes with these
//! requests.

use reqwest::Url;
use serde::Deserialize;

use crate::{ApiError, Client};

/// GIPHY's GIF API.
pub const GIPHY_API: &str = "https://api.giphy.com/v1/gifs/";

/// What the webapp's picker asks for: general-audience GIFs only.
const RATING: &str = "g";

/// One page of GIFs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GifPage {
    /// The GIFs, in GIPHY's order.
    pub gifs: Vec<Gif>,
    /// Where the next page starts, or `None` after the last one.
    pub next: Option<u32>,
}

/// One GIF.
#[derive(Clone, Debug, PartialEq)]
pub struct Gif {
    /// GIPHY's id.
    pub id: String,
    /// Its title, such as "Happy Dance GIF".
    pub title: String,
    /// What a message links: the 200 pixel tall rendition, as the webapp
    /// posts it.
    pub url: String,
    /// The small picture the picker's grid draws.
    pub preview: String,
    /// The preview's width in pixels.
    pub width: u32,
    /// The preview's height in pixels.
    pub height: u32,
}

impl Gif {
    /// The Markdown a message carries for this GIF, as the webapp writes
    /// it: `![title](url)`.
    pub fn markdown(&self) -> String {
        let title: String = self
            .title
            .chars()
            .map(|c| if matches!(c, '[' | ']') { ' ' } else { c })
            .collect();
        format!("![{}]({})", title.trim(), self.url)
    }
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    data: Vec<Item>,
    #[serde(default)]
    pagination: Pagination,
}

#[derive(Default, Deserialize)]
struct Pagination {
    #[serde(default)]
    total_count: u32,
    #[serde(default)]
    count: u32,
    #[serde(default)]
    offset: u32,
}

#[derive(Deserialize)]
struct Item {
    id: String,
    #[serde(default)]
    title: String,
    images: Images,
}

#[derive(Deserialize)]
struct Images {
    fixed_height: Option<Rendition>,
    fixed_width_downsampled: Option<Rendition>,
    fixed_width: Option<Rendition>,
}

#[derive(Deserialize)]
struct Rendition {
    #[serde(default)]
    url: String,
    #[serde(default, deserialize_with = "number")]
    width: u32,
    #[serde(default, deserialize_with = "number")]
    height: u32,
}

/// GIPHY writes sizes as strings.
fn number<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Number {
        Text(String),
        Number(u32),
    }
    Ok(match Number::deserialize(deserializer)? {
        Number::Text(text) => text.parse().unwrap_or(0),
        Number::Number(number) => number,
    })
}

impl Answer {
    fn page(self) -> GifPage {
        let gifs: Vec<Gif> = self
            .data
            .into_iter()
            .filter_map(|item| {
                let posted = item.images.fixed_height.filter(|r| !r.url.is_empty())?;
                let preview = item
                    .images
                    .fixed_width_downsampled
                    .filter(|r| !r.url.is_empty())
                    .or(item.images.fixed_width.filter(|r| !r.url.is_empty()));
                let (preview, width, height) = match preview {
                    Some(r) => (r.url, r.width, r.height),
                    None => (posted.url.clone(), posted.width, posted.height),
                };
                Some(Gif {
                    id: item.id,
                    title: item.title,
                    url: posted.url,
                    preview,
                    width,
                    height,
                })
            })
            .collect();
        let end = self.pagination.offset + self.pagination.count;
        let next = (self.pagination.count > 0 && end < self.pagination.total_count).then_some(end);
        GifPage { gifs, next }
    }
}

impl Client {
    /// Trending GIFs for an empty `query`, else GIPHY's search for it,
    /// `limit` from `offset`, in `lang` (a two-letter code such as `en`).
    pub async fn gifs(
        &self,
        key: &str,
        query: &str,
        offset: u32,
        limit: u32,
        lang: &str,
    ) -> Result<GifPage, ApiError> {
        self.gifs_from(GIPHY_API, key, query, offset, limit, lang)
            .await
    }

    /// [`Client::gifs`] from another address, for tests.
    pub async fn gifs_from(
        &self,
        api: &str,
        key: &str,
        query: &str,
        offset: u32,
        limit: u32,
        lang: &str,
    ) -> Result<GifPage, ApiError> {
        let base = Url::parse(api).map_err(|e| ApiError::InvalidUrl(e.to_string()))?;
        let query = query.trim();
        let mut url = base
            .join(if query.is_empty() {
                "trending"
            } else {
                "search"
            })
            .map_err(|e| ApiError::InvalidUrl(e.to_string()))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs
                .append_pair("api_key", key)
                .append_pair("offset", &offset.to_string())
                .append_pair("limit", &limit.to_string())
                .append_pair("rating", RATING);
            if !query.is_empty() {
                pairs
                    .append_pair("q", query)
                    .append_pair("lang", lang)
                    .append_pair("sort", "relevant");
            }
        }
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(ApiError::from_transport)?;
        let response = Self::checked(response).await?;
        let bytes = response.bytes().await.map_err(ApiError::from_transport)?;
        let answer: Answer =
            serde_json::from_slice(&bytes).map_err(|e| ApiError::Decode(e.to_string()))?;
        Ok(answer.page())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWER: &str = r#"{
        "data": [
            {"id": "a1", "title": "Happy Dance GIF", "images": {
                "fixed_height": {"url": "https://media1.giphy.com/media/a1/200.gif?cid=x", "width": "267", "height": "200"},
                "fixed_width_downsampled": {"url": "https://media1.giphy.com/media/a1/200w_d.gif", "width": "200", "height": "150"}
            }},
            {"id": "b2", "title": "", "images": {
                "fixed_height": {"url": "https://media2.giphy.com/media/b2/200.gif", "width": "200", "height": "200"}
            }},
            {"id": "c3", "title": "No rendition", "images": {}}
        ],
        "pagination": {"total_count": 30, "count": 3, "offset": 0}
    }"#;

    #[test]
    fn a_page_reads_the_posted_and_preview_renditions() {
        let answer: Answer = serde_json::from_str(ANSWER).unwrap();
        let page = answer.page();
        assert_eq!(page.next, Some(3));
        assert_eq!(page.gifs.len(), 2);
        let first = &page.gifs[0];
        assert_eq!(first.url, "https://media1.giphy.com/media/a1/200.gif?cid=x");
        assert_eq!(
            first.preview,
            "https://media1.giphy.com/media/a1/200w_d.gif"
        );
        assert_eq!((first.width, first.height), (200, 150));
        // Without a small rendition the posted one previews.
        assert_eq!(page.gifs[1].preview, page.gifs[1].url);
        assert_eq!((page.gifs[1].width, page.gifs[1].height), (200, 200));
    }

    #[test]
    fn the_last_page_has_no_next() {
        let answer: Answer = serde_json::from_str(
            r#"{"data": [], "pagination": {"total_count": 30, "count": 5, "offset": 25}}"#,
        )
        .unwrap();
        assert_eq!(answer.page().next, None);
    }

    #[test]
    fn markdown_is_an_image_with_the_title() {
        let gif = Gif {
            id: "a1".into(),
            title: "Happy [Dance] GIF".into(),
            url: "https://media1.giphy.com/media/a1/200.gif".into(),
            preview: String::new(),
            width: 0,
            height: 0,
        };
        assert_eq!(
            gif.markdown(),
            "![Happy  Dance  GIF](https://media1.giphy.com/media/a1/200.gif)"
        );
    }
}
