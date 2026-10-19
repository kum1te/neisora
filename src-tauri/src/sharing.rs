//! Construct service links from identifiers, never accept arbitrary external URLs.
use serde::Deserialize;
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Target {
    kind: Kind,
    id: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Song,
    Playlist,
    Album,
}
pub fn url(target: Target) -> Result<String, String> {
    let id = &target.id;
    if id.is_empty()
        || id.len() > 200
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err("Для этого объекта нет ссылки YouTube Music".into());
    }
    match target.kind {
        Kind::Song if id.len() == 11 => Ok(format!("https://music.youtube.com/watch?v={id}")),
        Kind::Playlist
            if id.starts_with("PL")
                || id.starts_with("RD")
                || id.starts_with("OLAK")
                || id == "LM" =>
        {
            Ok(format!("https://music.youtube.com/playlist?list={id}"))
        }
        Kind::Album if id.starts_with("MPRE") => {
            Ok(format!("https://music.youtube.com/browse/{id}"))
        }
        _ => Err("Для этого объекта нет ссылки YouTube Music".into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links_accept_service_identifiers_only() {
        for (kind, id, expected) in [
            ("song", "abcdefghijk", "watch?v=abcdefghijk"),
            ("playlist", "PLabc_123", "playlist?list=PLabc_123"),
            ("album", "MPREb_123", "browse/MPREb_123"),
        ] {
            let target = serde_json::from_value(serde_json::json!({"kind":kind,"id":id})).unwrap();
            assert_eq!(
                url(target).unwrap(),
                format!("https://music.youtube.com/{expected}")
            );
        }
        for id in [
            "https://evil.test",
            "../../secret",
            "abc?x=y",
            "local:abc",
            "--exec=sh",
            "abcdefghij\n",
            "",
            "abcdefghijk&list=PLx",
        ] {
            let target =
                serde_json::from_value(serde_json::json!({"kind":"song","id":id})).unwrap();
            assert!(url(target).is_err());
        }
        assert!(
            serde_json::from_str::<Target>(r#"{"kind":"url","id":"https://evil.test"}"#).is_err()
        );
    }
}
