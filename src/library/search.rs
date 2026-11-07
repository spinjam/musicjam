use std::path::Path;

use super::Track;

const MAX_RESULTS: usize = 500;

/// Articles left out when deciding whether a query names the whole title, so
/// "narcissist blur" counts as naming "The Narcissist".
const ARTICLES: [&str; 9] = ["the", "a", "an", "il", "lo", "la", "i", "gli", "le"];

pub fn tokens(text: &str) -> Vec<String> {
    deunicode::deunicode(text)
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

struct Entry {
    title: Vec<String>,
    artist: Vec<String>,
    album: Vec<String>,
    path: Vec<String>,
}

pub struct Index {
    entries: Vec<Entry>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Title,
    Artist,
    Album,
    Path,
}

impl Index {
    pub fn new(tracks: &[Track]) -> Self {
        let entries = tracks
            .iter()
            .map(|t| Entry {
                title: tokens(&t.title),
                artist: tokens(&t.artist),
                album: tokens(&t.album),
                path: tokens(&t.path.to_string_lossy()),
            })
            .collect();
        Self { entries }
    }

    /// Indices into the tracks this index was built from, best match first.
    /// An empty query lists everything in scope in path order.
    pub fn query(&self, tracks: &[Track], q: &str, scope: Option<&Path>) -> Vec<usize> {
        let in_scope = |i: usize| scope.is_none_or(|s| tracks[i].path.starts_with(s));
        let q = tokens(q);
        if q.is_empty() {
            return (0..tracks.len()).filter(|&i| in_scope(i)).collect();
        }
        let mut hits: Vec<(f32, usize)> = (0..self.entries.len())
            .filter(|&i| in_scope(i))
            .filter_map(|i| score(&self.entries[i], &q).map(|s| (s, i)))
            .collect();
        hits.sort_by(|a, b| {
            b.0.total_cmp(&a.0).then_with(|| {
                let (x, y) = (&tracks[a.1], &tracks[b.1]);
                (&x.artist, &x.album, x.track_no, &x.title)
                    .cmp(&(&y.artist, &y.album, y.track_no, &y.title))
            })
        });
        hits.into_iter().take(MAX_RESULTS).map(|(_, i)| i).collect()
    }
}

/// Every query word must start some word of the track; each word scores by the
/// best field it lands in. A query that spans title and artist, or names the
/// whole title, is what someone typing "the narcissist blur" means.
fn score(e: &Entry, q: &[String]) -> Option<f32> {
    let mut total = 0.0;
    let mut fields = Vec::new();
    for word in q {
        let mut best: Option<(f32, Field)> = None;
        for (field, weight, toks) in [
            (Field::Title, 3.0, &e.title),
            (Field::Artist, 2.0, &e.artist),
            (Field::Album, 1.0, &e.album),
            (Field::Path, 0.5, &e.path),
        ] {
            let s = toks
                .iter()
                .filter(|t| t.starts_with(word.as_str()))
                .map(|t| if t == word { weight + 1.0 } else { weight })
                .fold(0.0_f32, f32::max);
            if s > 0.0 && best.is_none_or(|(b, _)| s > b) {
                best = Some((s, field));
            }
        }
        let (s, field) = best?;
        total += s;
        fields.push(field);
    }
    if fields.contains(&Field::Title) && fields.contains(&Field::Artist) {
        total += 3.0;
    }
    let names_whole_title = e
        .title
        .iter()
        .filter(|t| !ARTICLES.contains(&t.as_str()))
        .all(|t| q.contains(t));
    if names_whole_title && !e.title.is_empty() {
        total += 4.0;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(id: i64, artist: &str, album: &str, title: &str) -> Track {
        Track {
            id,
            path: format!("/Music/{artist}/{album}/{title}.flac").into(),
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
            track_no: None,
            duration_secs: 200.0,
        }
    }

    fn fixtures() -> Vec<Track> {
        vec![
            track(
                1,
                "Some Cover Band",
                "Britpop Covers",
                "The Narcissist (Blur cover)",
            ),
            track(2, "Other", "Mirrors", "Narcissist"),
            track(3, "Blur", "The Ballad of Darren", "The Ballad"),
            track(4, "Blur", "The Ballad of Darren", "The Narcissist"),
            track(5, "Beyoncé", "Lemonade", "Formation"),
        ]
    }

    #[test]
    fn narcissist_ranks_first() {
        let tracks = fixtures();
        let index = Index::new(&tracks);
        for q in [
            "the narcissist blur",
            "blur narcissist",
            "Narcissist BLUR",
            "narc blur",
        ] {
            let hits = index.query(&tracks, q, None);
            assert_eq!(tracks[hits[0]].id, 4, "query {q:?}");
        }
    }

    #[test]
    fn every_word_must_match() {
        let tracks = fixtures();
        let index = Index::new(&tracks);
        let ids: Vec<i64> = index
            .query(&tracks, "the narcissist blur", None)
            .iter()
            .map(|&i| tracks[i].id)
            .collect();
        assert!(!ids.contains(&2) && !ids.contains(&3));
    }

    #[test]
    fn scope_includes_subfolders() {
        let tracks = fixtures();
        let index = Index::new(&tracks);
        let hits = index.query(&tracks, "", Some(Path::new("/Music/Blur")));
        assert_eq!(hits.len(), 2);
        assert!(
            index
                .query(&tracks, "narcissist", Some(Path::new("/Music/Other")))
                .len()
                == 1
        );
    }

    #[test]
    fn diacritics_ignored() {
        let tracks = fixtures();
        let index = Index::new(&tracks);
        assert_eq!(tracks[index.query(&tracks, "beyonce", None)[0]].id, 5);
    }
}
