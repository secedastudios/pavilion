//! TMDB API client for film metadata and cast/crew enrichment.
//! Searches The Movie Database by title, fetches detailed metadata,
//! and retrieves cast and crew lists to populate film records.

use serde::{Deserialize, Serialize};

/// TMDB API client. Requires a free API key from <https://www.themoviedb.org/settings/api>.
pub struct TmdbClient {
    api_key: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbSearchResult {
    pub id: i64,
    pub title: String,
    pub overview: Option<String>,
    pub release_date: Option<String>,
    pub poster_path: Option<String>,
    pub vote_average: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbMovieDetail {
    pub id: i64,
    pub title: String,
    pub overview: Option<String>,
    pub tagline: Option<String>,
    pub release_date: Option<String>,
    pub runtime: Option<i64>,
    pub poster_path: Option<String>,
    pub imdb_id: Option<String>,
    pub genres: Vec<TmdbGenre>,
    pub production_countries: Vec<TmdbCountry>,
    pub spoken_languages: Vec<TmdbLanguage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbGenre {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbCountry {
    pub iso_3166_1: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbLanguage {
    pub iso_639_1: String,
    pub english_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbCastMember {
    pub id: i64,
    pub name: String,
    pub character: Option<String>,
    pub known_for_department: Option<String>,
    pub profile_path: Option<String>,
    pub order: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbCrewMember {
    pub id: i64,
    pub name: String,
    pub department: Option<String>,
    pub job: Option<String>,
    pub profile_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TmdbCredits {
    pub cast: Vec<TmdbCastMember>,
    pub crew: Vec<TmdbCrewMember>,
}

/// Combined enrichment data ready to apply to a film.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EnrichmentData {
    pub tmdb_id: i64,
    pub imdb_id: Option<String>,
    pub title: String,
    pub synopsis: Option<String>,
    pub tagline: Option<String>,
    pub year: Option<i64>,
    pub runtime_minutes: Option<i64>,
    pub genres: Vec<String>,
    pub country: Option<String>,
    pub language: Option<String>,
    pub poster_url: Option<String>,
    pub cast: Vec<TmdbCastMember>,
    pub crew: Vec<TmdbCrewMember>,
}

impl TmdbClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    /// Search TMDB for movies matching a query.
    pub async fn search(
        &self,
        query: &str,
        year: Option<i64>,
    ) -> anyhow::Result<Vec<TmdbSearchResult>> {
        let mut url = format!(
            "https://api.themoviedb.org/3/search/movie?api_key={}&query={}",
            self.api_key,
            urlencoding::encode(query)
        );
        if let Some(y) = year {
            url.push_str(&format!("&year={y}"));
        }

        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let results: Vec<TmdbSearchResult> =
            serde_json::from_value(resp.get("results").cloned().unwrap_or_default())?;

        Ok(results)
    }

    /// Get full movie details including IMDB ID.
    pub async fn get_movie(&self, tmdb_id: i64) -> anyhow::Result<TmdbMovieDetail> {
        let url = format!(
            "https://api.themoviedb.org/3/movie/{tmdb_id}?api_key={}",
            self.api_key
        );
        let detail: TmdbMovieDetail = self.client.get(&url).send().await?.json().await?;
        Ok(detail)
    }

    /// Get cast and crew for a movie.
    pub async fn get_credits(&self, tmdb_id: i64) -> anyhow::Result<TmdbCredits> {
        let url = format!(
            "https://api.themoviedb.org/3/movie/{tmdb_id}/credits?api_key={}",
            self.api_key
        );
        let credits: TmdbCredits = self.client.get(&url).send().await?.json().await?;
        Ok(credits)
    }

    /// Download a poster image from TMDB.
    pub async fn download_poster(&self, poster_path: &str) -> anyhow::Result<Vec<u8>> {
        let url = format!("https://image.tmdb.org/t/p/original{poster_path}");
        let bytes = self.client.get(&url).send().await?.bytes().await?;
        Ok(bytes.to_vec())
    }

    /// Full enrichment: get details + credits, combine into EnrichmentData.
    pub async fn enrich(&self, tmdb_id: i64) -> anyhow::Result<EnrichmentData> {
        let detail = self.get_movie(tmdb_id).await?;
        let credits = self.get_credits(tmdb_id).await?;

        let year = detail
            .release_date
            .as_deref()
            .and_then(|d| d.split('-').next())
            .and_then(|y| y.parse().ok());

        let poster_url = detail
            .poster_path
            .as_ref()
            .map(|p| format!("https://image.tmdb.org/t/p/w780{p}"));

        Ok(EnrichmentData {
            tmdb_id: detail.id,
            imdb_id: detail.imdb_id,
            title: detail.title,
            synopsis: detail.overview,
            tagline: detail.tagline,
            year,
            runtime_minutes: detail.runtime,
            genres: detail.genres.into_iter().map(|g| g.name).collect(),
            country: detail.production_countries.first().map(|c| c.name.clone()),
            language: detail
                .spoken_languages
                .first()
                .map(|l| l.english_name.clone()),
            poster_url,
            cast: credits.cast,
            crew: credits.crew,
        })
    }
}

impl TmdbClient {
    /// Find a movie by IMDB ID using TMDB's find-by-external-ID endpoint.
    /// This eliminates the need for a separate OMDB/IMDB API.
    pub async fn find_by_imdb_id(&self, imdb_id: &str) -> anyhow::Result<Option<EnrichmentData>> {
        let url = format!(
            "https://api.themoviedb.org/3/find/{imdb_id}?api_key={}&external_source=imdb_id",
            self.api_key
        );
        let resp: serde_json::Value = self.client.get(&url).send().await?.json().await?;
        let results = resp
            .get("movie_results")
            .and_then(|r| r.as_array())
            .cloned()
            .unwrap_or_default();

        if let Some(first) = results.first()
            && let Some(tmdb_id) = first.get("id").and_then(|i| i.as_i64())
        {
            let data = self.enrich(tmdb_id).await?;
            return Ok(Some(data));
        }
        Ok(None)
    }
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        s.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
                ' ' => "+".to_string(),
                _ => format!("%{:02X}", c as u8),
            })
            .collect()
    }
}
