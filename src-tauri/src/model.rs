use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Item {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available: Option<bool>,
    #[serde(
        default,
        rename = "localRevision",
        skip_serializing_if = "Option::is_none"
    )]
    pub local_revision: Option<u64>,
    #[serde(default)]
    pub liked: Option<bool>,
    #[serde(
        default,
        rename = "setVideoId",
        skip_serializing_if = "Option::is_none"
    )]
    pub set_video_id: Option<String>,
    #[serde(default, rename = "isMusicVideo")]
    pub is_music_video: bool,
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub image: String,
    pub kind: String,
    pub endpoint: Value,
    pub play: Option<Value>,
    pub duration: String,
    pub selected: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Section {
    pub title: String,
    pub subtitle: String,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Feed {
    pub collection: Option<crate::library::Collection>,
    pub title: String,
    pub sections: Vec<Section>,
    pub continuation: Option<Value>,
    pub authenticated: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Queue {
    #[serde(rename = "updateSection")]
    pub update_section: Option<String>,
    pub title: String,
    pub items: Vec<Item>,
    pub continuation: Option<Value>,
    pub context: Value,
    pub selected: usize,
    pub dynamic: bool,
}

pub fn keep_anchor(mut queue: Queue, anchor: Option<Item>) -> Queue {
    if let Some(anchor) = anchor {
        if let Some(index) = queue.items.iter().position(|item| item.id == anchor.id) {
            queue.selected = index;
        } else {
            queue.items.insert(0, anchor);
            queue.selected = 0;
        }
    }
    queue
}

pub fn text(value: &Value) -> String {
    if let Some(s) = value
        .as_str()
        .or_else(|| value.get("simpleText").and_then(Value::as_str))
    {
        return s.to_owned();
    }
    value
        .get("runs")
        .and_then(Value::as_array)
        .map(|runs| {
            runs.iter()
                .filter_map(|r| r.get("text").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default()
}

pub fn find<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    match node {
        Value::Object(map) => map
            .get(key)
            .or_else(|| map.values().find_map(|v| find(v, key))),
        Value::Array(arr) => arr.iter().find_map(|v| find(v, key)),
        _ => None,
    }
}

fn image(node: &Value) -> String {
    node.get("thumbnail")
        .or_else(|| node.get("thumbnailRenderer"))
        .and_then(|thumbnail| find(thumbnail, "thumbnails"))
        .or_else(|| find(node, "thumbnails"))
        .and_then(Value::as_array)
        .and_then(|list| {
            list.iter()
                .find(|v| v["width"].as_u64().unwrap_or(0) >= 320)
                .or_else(|| list.last())
        })
        .and_then(|v| v["url"].as_str())
        .unwrap_or_default()
        .to_owned()
}

pub fn watch_payload(endpoint: &Value) -> Option<Value> {
    endpoint
        .get("watchEndpoint")
        .or_else(|| endpoint.get("watchPlaylistEndpoint"))
        .cloned()
}

pub fn parse_item(node: &Value) -> Option<Item> {
    let (r, kind) = if let Some(r) = node.get("musicTwoRowItemRenderer") {
        (r, "card")
    } else if let Some(r) = node.get("musicResponsiveListItemRenderer") {
        (r, "song")
    } else if let Some(r) = node.get("musicCardShelfRenderer") {
        (r, "song")
    } else if let Some(r) = node.get("playlistPanelVideoRenderer") {
        (r, "song")
    } else if let Some(r) = node.pointer("/playlistPanelVideoWrapperRenderer/primaryRenderer") {
        return parse_item(r);
    } else {
        return None;
    };

    let cols = r.get("flexColumns").and_then(Value::as_array);
    let title = text(r.get("title").unwrap_or_else(|| {
        cols.and_then(|c| c.first())
            .and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
            .unwrap_or(&Value::Null)
    }));
    let subtitle = text(
        r.get("subtitle")
            .or_else(|| r.get("longBylineText"))
            .or_else(|| r.get("shortBylineText"))
            .unwrap_or_else(|| {
                cols.and_then(|c| c.get(1))
                    .and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text"))
                    .unwrap_or(&Value::Null)
            }),
    );
    let play = r
        .get("overlay")
        .or_else(|| r.get("thumbnailOverlay"))
        .or_else(|| r.get("thumbnailRenderer"))
        .and_then(|o| find(o, "playNavigationEndpoint"))
        .cloned();
    let endpoint = r
        .get("navigationEndpoint")
        .cloned()
        .or_else(|| find(&r["title"], "navigationEndpoint").cloned())
        .or_else(|| {
            cols.and_then(|c| c.first())
                .and_then(|c| find(c, "navigationEndpoint"))
                .cloned()
        })
        .or_else(|| play.clone())
        .or_else(|| {
            r.pointer("/playlistItemData/videoId")
                .and_then(Value::as_str)
                .map(|id| json!({"watchEndpoint":{"videoId":id}}))
        })
        .unwrap_or(Value::Null);
    let id = r
        .get("videoId")
        .and_then(Value::as_str)
        .or_else(|| {
            r.pointer("/playlistItemData/videoId")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            endpoint
                .pointer("/browseEndpoint/browseId")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            endpoint
                .pointer("/watchEndpoint/videoId")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            endpoint
                .pointer("/watchEndpoint/playlistId")
                .and_then(Value::as_str)
        })
        .or_else(|| {
            endpoint
                .pointer("/watchPlaylistEndpoint/playlistId")
                .and_then(Value::as_str)
        })
        .unwrap_or_default()
        .to_owned();
    if title.is_empty() || endpoint.is_null() {
        return None;
    }
    let page_type = endpoint.pointer("/browseEndpoint/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType")
        .and_then(Value::as_str).unwrap_or_default();
    let kind = match page_type {
        "MUSIC_PAGE_TYPE_ARTIST" => "artist",
        "MUSIC_PAGE_TYPE_ALBUM" => "album",
        "MUSIC_PAGE_TYPE_PLAYLIST" => "playlist",
        _ => kind,
    }
    .to_owned();
    let duration = text(&r["lengthText"]);
    let duration = if duration.is_empty() {
        ["fixedColumns", "flexColumns"]
            .iter()
            .flat_map(|key| r[*key].as_array().into_iter().flatten())
            .filter_map(|column| find(column, "text"))
            .map(text)
            .find(|s| s.contains(':') && s.chars().all(|c| c.is_ascii_digit() || c == ':'))
            .unwrap_or_default()
    } else {
        duration
    };
    Some(Item {
        source: None,
        album: None,
        available: None,
        local_revision: None,
        liked: crate::library::like_status(r),
        set_video_id: r
            .pointer("/playlistItemData/playlistSetVideoId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        is_music_video: find(&endpoint, "musicVideoType").and_then(Value::as_str)
            == Some("MUSIC_VIDEO_TYPE_OMV")
            || title.to_lowercase().contains("official music video"),
        id,
        title,
        subtitle,
        image: image(r),
        kind,
        endpoint,
        play,
        duration,
        selected: r["selected"].as_bool().unwrap_or(false),
    })
}

fn collect_sections(node: &Value, sections: &mut Vec<Section>) {
    if let Some(card) = node.get("musicCardShelfRenderer") {
        let mut items: Vec<Item> = parse_item(node).into_iter().collect();
        if let Some(contents) = card["contents"].as_array() {
            items.extend(contents.iter().filter_map(parse_item));
        }
        if !items.is_empty() {
            sections.push(Section {
                title: "Лучшее совпадение".into(),
                subtitle: String::new(),
                items,
            });
        }
        return;
    }
    for key in [
        "musicCarouselShelfRenderer",
        "musicShelfRenderer",
        "musicPlaylistShelfRenderer",
        "musicShelfContinuation",
        "musicPlaylistShelfContinuation",
        "gridRenderer",
        "gridContinuation",
    ] {
        if let Some(shelf) = node.get(key) {
            let header = shelf
                .pointer("/header/musicCarouselShelfBasicHeaderRenderer")
                .unwrap_or(shelf);
            let title = text(&header["title"]);
            let items = shelf
                .get("contents")
                .or_else(|| shelf.get("items"))
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(parse_item).collect::<Vec<_>>())
                .unwrap_or_default();
            if !items.is_empty() {
                sections.push(Section {
                    title,
                    subtitle: text(&header["strapline"]),
                    items,
                });
            }
            return;
        }
    }
    match node {
        Value::Object(m) => {
            for (key, v) in m {
                if key != "menu" && key != "background" {
                    collect_sections(v, sections);
                }
            }
        }
        Value::Array(a) => {
            for v in a {
                collect_sections(v, sections);
            }
        }
        _ => (),
    }
}

pub fn continuation(node: &Value) -> Option<String> {
    for key in [
        "nextRadioContinuationData",
        "nextContinuationData",
        "reloadContinuationData",
    ] {
        if let Some(token) = find(node, key)
            .and_then(|v| v.get("continuation"))
            .and_then(Value::as_str)
        {
            return Some(token.to_owned());
        }
    }
    find(node, "continuationCommand")
        .and_then(|v| v["token"].as_str())
        .map(str::to_owned)
}

pub fn feed(data: &Value, authenticated: bool) -> Feed {
    let collection = crate::library::collection(data);
    let mut sections = Vec::new();
    collect_sections(
        data.get("contents")
            .or_else(|| data.get("continuationContents"))
            .unwrap_or(data),
        &mut sections,
    );
    let title = data
        .get("header")
        .or_else(|| find(data, "musicResponsiveHeaderRenderer"))
        .and_then(|h| find(h, "title"))
        .map(text)
        .unwrap_or_default();
    // Section-level continuation tokens belong to shelves, not the whole home feed.
    let section_list =
        find(data, "sectionListRenderer").or_else(|| find(data, "sectionListContinuation"));
    let next = section_list
        .and_then(|s| s.get("continuations"))
        .and_then(continuation)
        .or_else(|| {
            data.get("continuationContents").and_then(|c| {
                [
                    "gridContinuation",
                    "musicShelfContinuation",
                    "musicPlaylistShelfContinuation",
                ]
                .iter()
                .find_map(|key| {
                    c.get(*key)
                        .and_then(|s| s.get("continuations"))
                        .and_then(continuation)
                })
            })
        })
        .or_else(|| {
            if collection.is_some() || find(data, "twoColumnBrowseResultsRenderer").is_some() {
                find(data, "musicPlaylistShelfRenderer").and_then(continuation)
            } else {
                None
            }
        })
        .map(|t| json!({"continuation": t}));
    let authenticated = authenticated
        && data
            .pointer("/responseContext/mainAppWebResponseContext/loggedOut")
            .and_then(Value::as_bool)
            != Some(true);
    Feed {
        collection,
        title,
        sections,
        continuation: next,
        authenticated,
    }
}

pub fn queue(data: &Value, context: &Value) -> Option<Queue> {
    let panel =
        find(data, "playlistPanelRenderer").or_else(|| find(data, "playlistPanelContinuation"))?;
    let items: Vec<Item> = panel["contents"]
        .as_array()?
        .iter()
        .filter_map(parse_item)
        .collect();
    let selected = items.iter().position(|i| i.selected).unwrap_or(0);
    let mut context = context.clone();
    if let Some(object) = context.as_object_mut() {
        // A skip is a one-shot action. Continuations must never resend it.
        for key in ["watchNextType", "responsiveSignals", "continuation"] {
            object.remove(key);
        }
    }
    if let Some(id) = panel.get("playlistId") {
        context["playlistId"] = id.clone();
    }
    if let Some(value) = data.get("queueContextParams") {
        context["queueContextParams"] = value.clone();
    }
    let next = continuation(panel).map(|token| {
        let mut payload = context.clone();
        payload["continuation"] = json!(token);
        payload
    });
    let dynamic = panel["isInfinite"].as_bool().unwrap_or(false)
        || context["playlistId"]
            .as_str()
            .is_some_and(|id| id.starts_with("RD"));
    Some(Queue {
        update_section: find(data, "queueUpdateCommand")
            .and_then(|c| c.get("queueUpdateSection"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        title: text(panel.get("titleText").unwrap_or(&panel["title"])),
        items,
        continuation: next,
        context,
        selected,
        dynamic,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_entry_identity_survives_parsing() {
        let item = parse_item(&json!({"musicResponsiveListItemRenderer":{
            "title":{"runs":[{"text":"Repeated song"}]},
            "navigationEndpoint":{"watchEndpoint":{"videoId":"abcdefghijk"}},
            "playlistItemData":{"videoId":"abcdefghijk","playlistSetVideoId":"unique-entry"}
        }}))
        .unwrap();
        assert_eq!(item.id, "abcdefghijk");
        assert_eq!(item.set_video_id.as_deref(), Some("unique-entry"));
    }
    #[test]
    fn playlist_pagination_does_not_adopt_an_unrelated_home_shelf_token() {
        let shelf = json!({"musicPlaylistShelfRenderer":{"contents":[{"continuationItemRenderer":{"continuationEndpoint":{"continuationCommand":{"token":"playlist-page-2"}}}}]}});
        let playlist = json!({"contents":{"twoColumnBrowseResultsRenderer":{"secondaryContents":shelf.clone()}}});
        assert_eq!(
            feed(&playlist, true).continuation,
            Some(json!({"continuation":"playlist-page-2"}))
        );
        assert!(feed(
            &json!({"contents":{"singleColumnBrowseResultsRenderer":shelf}}),
            true
        )
        .continuation
        .is_none());
    }
    #[test]
    fn mix_keeps_server_endpoint_parameters() {
        let endpoint = json!({"watchPlaylistEndpoint":{"playlistId":"RDCLAK_mix","params":"dynamic-params"},"clickTrackingParams":"tracking"});
        let item = parse_item(&json!({"musicTwoRowItemRenderer":{"title":{"runs":[{"text":"Супермикс"}]},"navigationEndpoint":endpoint}})).unwrap();
        assert_eq!(item.endpoint, endpoint);
        assert_eq!(
            watch_payload(&item.endpoint).unwrap()["params"],
            "dynamic-params"
        );
    }

    #[test]
    fn radio_continuation_keeps_playlist_context() {
        let data = json!({"contents":{"playlistPanelRenderer":{"title":"Mix","playlistId":"RD_live","contents":[{"playlistPanelVideoRenderer":{"videoId":"abc12345678","title":{"simpleText":"Track"},"selected":true,"navigationEndpoint":{"watchEndpoint":{"videoId":"abc12345678","playlistId":"RD_live","index":17}}}}],"continuations":[{"nextRadioContinuationData":{"continuation":"opaque-next-token"}}]}}});
        let q = queue(&data, &json!({"videoId":"abc12345678","params":"opaque"})).unwrap();
        let next = q.continuation.unwrap();
        assert_eq!(next["playlistId"], "RD_live");
        assert_eq!(next["params"], "opaque");
        assert_eq!(next["continuation"], "opaque-next-token");
        assert_eq!(q.items[0].endpoint["watchEndpoint"]["index"], 17);
    }

    #[test]
    fn server_queue_context_rotates_with_the_response() {
        let data = json!({"queueContextParams":"new-session","contents":{"playlistPanelRenderer":{
            "contents":[],"playlistId":"RDmix","continuations":[{"nextRadioContinuationData":{"continuation":"tail"}}]
        }}});
        let q = queue(&data, &json!({"playlistId":"RDmix","queueContextParams":"old-session","watchNextType":"WATCH_NEXT_TYPE_SKIP_VIDEO","responsiveSignals":{"videoInteraction":[]}})).unwrap();
        assert_eq!(q.context["queueContextParams"], "new-session");
        let continuation = q.continuation.unwrap();
        assert_eq!(continuation["queueContextParams"], "new-session");
        assert!(continuation.get("watchNextType").is_none());
        assert!(continuation.get("responsiveSignals").is_none());
    }

    #[test]
    fn home_continuation_is_not_shelf_continuation() {
        let data = json!({"contents":{"sectionListRenderer":{"contents":[{"musicShelfRenderer":{"contents":[],"continuations":[{"nextContinuationData":{"continuation":"shelf"}}]}}],"continuations":[{"nextContinuationData":{"continuation":"home"}}]}}});
        assert_eq!(
            feed(&data, false).continuation.unwrap()["continuation"],
            "home"
        );
    }

    #[test]
    fn errors_never_become_fabricated_recommendations() {
        assert!(feed(&json!({"error":{"message":"Unavailable"}}), false)
            .sections
            .is_empty());
    }

    #[test]
    fn radio_inline_update_is_identified_as_future_items() {
        let data = json!({"queueContextParams":"updated-session","onResponseReceivedEndpoints":[{"queueUpdateCommand":{
            "queueUpdateSection":"QUEUE_UPDATE_SECTION_QUEUE", "inlineContents":{"playlistPanelRenderer":{
                "contents":[],"isInfinite":true
            }}
        }}]});
        let q = queue(&data, &json!({"playlistId":"RDmix"})).unwrap();
        assert_eq!(
            q.update_section.as_deref(),
            Some("QUEUE_UPDATE_SECTION_QUEUE")
        );
        assert_eq!(q.context["queueContextParams"], "updated-session");
    }

    #[test]
    fn automix_does_not_skip_the_clicked_track() {
        let make = |id: &str| {
            parse_item(&json!({"playlistPanelVideoRenderer":{"videoId":id,"title":{"simpleText":"Track"},"navigationEndpoint":{"watchEndpoint":{"videoId":id}}}})).unwrap()
        };
        let q = Queue {
            update_section: None,
            title: String::new(),
            items: vec![make("next")],
            continuation: Some(json!({"continuation":"next-page"})),
            context: json!({"playlistId":"live-mix"}),
            selected: 0,
            dynamic: true,
        };
        let q = keep_anchor(q, Some(make("clicked")));
        assert_eq!(q.items[q.selected].id, "clicked");
        assert_eq!(q.items[1].id, "next");
        assert_eq!(q.continuation.unwrap()["continuation"], "next-page");
    }

    #[test]
    fn song_thumbnail_wins_over_menu_avatars_and_search_duration_is_read() {
        let data = json!({"musicResponsiveListItemRenderer":{
            "flexColumns":[{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Song"}]}}}],
            "fixedColumns":[{"musicResponsiveListItemFixedColumnRenderer":{"text":{"runs":[{"text":"2:30"}]}}}],
            "navigationEndpoint":{"watchEndpoint":{"videoId":"abcdefghijk"}},
            "menu":{"thumbnails":[{"url":"https://yt3.googleusercontent.com/avatar","width":800}]},
            "thumbnail":{"musicThumbnailRenderer":{"thumbnail":{"thumbnails":[{"url":"https://lh3.googleusercontent.com/album","width":600}]}}}
        }});
        let item = parse_item(&data).unwrap();
        assert_eq!(item.image, "https://lh3.googleusercontent.com/album");
        assert_eq!(item.duration, "2:30");
    }
    #[test]
    fn search_card_is_a_playable_result() {
        let data = json!({"contents":{"musicCardShelfRenderer":{"title":{"runs":[{"text":"Best match","navigationEndpoint":{"watchEndpoint":{"videoId":"abc12345678"}}}]},"contents":[]}}});
        let f = feed(&data, false);
        assert_eq!(f.sections[0].items[0].id, "abc12345678");
    }
}
