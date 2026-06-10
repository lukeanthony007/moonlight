use super::AppState;
use crate::artwork_store;
use crate::db::repo::{artwork, games};
use crate::domain::{Artwork, Game};
use crate::error::Result;
use crate::metadata::{
    self, cached_or_fetch, ArtworkCandidate, ProviderMatch, ProviderStatus, SearchQuery,
};
use tauri::State;

#[tauri::command]
pub async fn get_provider_statuses(state: State<'_, AppState>) -> Result<Vec<ProviderStatus>> {
    metadata::provider_statuses(&state.db)
}

#[tauri::command]
pub async fn search_metadata(
    state: State<'_, AppState>,
    provider_id: String,
    query: SearchQuery,
) -> Result<Vec<ProviderMatch>> {
    let provider = metadata::get_provider(&state.db, &provider_id)?;
    let cache_key = format!("{provider_id}:search:{}", query.title.to_lowercase());
    let payload = cached_or_fetch(&state.db, &cache_key, || {
        let matches = provider.search(&query)?;
        Ok(serde_json::to_string(&matches).unwrap_or_else(|_| "[]".into()))
    })?;
    let mut matches: Vec<ProviderMatch> = serde_json::from_str(&payload).unwrap_or_default();
    // Recompute confidence against this exact query (cache may be shared).
    for m in &mut matches {
        m.confidence = metadata::match_confidence(&query, &m.title, m.release_year);
    }
    matches.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(matches)
}

#[tauri::command]
pub async fn get_artwork_candidates(
    state: State<'_, AppState>,
    provider_id: String,
    provider_game_id: String,
    kind: String,
) -> Result<Vec<ArtworkCandidate>> {
    let provider = metadata::get_provider(&state.db, &provider_id)?;
    let cache_key = format!("{provider_id}:art:{provider_game_id}:{kind}");
    let payload = cached_or_fetch(&state.db, &cache_key, || {
        let candidates = provider.artwork(&provider_game_id, &kind)?;
        Ok(serde_json::to_string(&candidates).unwrap_or_else(|_| "[]".into()))
    })?;
    Ok(serde_json::from_str(&payload).unwrap_or_default())
}

/// Apply a provider match to a game: merge metadata (respecting locks) and
/// optionally download the top artwork for slots without a user selection.
#[tauri::command]
pub async fn apply_provider_match(
    state: State<'_, AppState>,
    game_id: String,
    provider_id: String,
    provider_game_id: String,
    fetch_artwork: bool,
) -> Result<Game> {
    let provider = metadata::get_provider(&state.db, &provider_id)?;
    let fetched = provider.metadata(&provider_game_id)?;
    let game = state.db.with(|c| {
        games::merge_provider_metadata(
            c,
            &game_id,
            &games::ProviderMetadata {
                provider: provider_id.clone(),
                title: fetched.title.clone(),
                description: fetched.description.clone(),
                release_date: fetched.release_date.clone(),
                developer: fetched.developer.clone(),
                publisher: fetched.publisher.clone(),
                genres: fetched.genres.clone(),
            },
        )
    })?;

    if fetch_artwork {
        for kind in ["boxart", "background", "logo"] {
            let user_has = state
                .db
                .with(|c| artwork::has_user_selected(c, &game_id, kind))?;
            if user_has {
                continue; // never overwrite a user-selected image
            }
            match provider.artwork(&provider_game_id, kind) {
                Ok(candidates) => {
                    if let Some(best) = candidates.first() {
                        if let Err(e) = artwork_store::download_url(
                            &state.db,
                            &state.paths.artwork_dir,
                            &game_id,
                            kind,
                            &best.url,
                            &provider_id,
                            false,
                        ) {
                            tracing::warn!(kind, error = %e, "artwork download failed");
                        }
                    }
                }
                Err(e) => tracing::warn!(kind, error = %e, "artwork lookup failed"),
            }
        }
    }
    Ok(game)
}

#[tauri::command]
pub async fn list_artwork(state: State<'_, AppState>, game_id: String) -> Result<Vec<Artwork>> {
    state.db.with(|c| artwork::list_for_game(c, &game_id))
}

#[tauri::command]
pub async fn import_artwork_file(
    state: State<'_, AppState>,
    game_id: String,
    kind: String,
    path: String,
) -> Result<Artwork> {
    artwork_store::import_local_file(&state.db, &state.paths.artwork_dir, &game_id, &kind, &path)
}

#[tauri::command]
pub async fn download_artwork(
    state: State<'_, AppState>,
    game_id: String,
    kind: String,
    url: String,
    provider: Option<String>,
    select: bool,
) -> Result<Artwork> {
    artwork_store::download_url(
        &state.db,
        &state.paths.artwork_dir,
        &game_id,
        &kind,
        &url,
        provider.as_deref().unwrap_or("url"),
        select,
    )
}

#[tauri::command]
pub async fn select_artwork(state: State<'_, AppState>, artwork_id: String) -> Result<()> {
    state.db.with(|c| artwork::select(c, &artwork_id))
}

#[tauri::command]
pub async fn delete_artwork(state: State<'_, AppState>, artwork_id: String) -> Result<()> {
    artwork_store::delete(&state.db, &state.paths.artwork_dir, &artwork_id)
}
