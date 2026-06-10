# Adding a metadata provider

Providers supply search, metadata and artwork candidates. The whole surface is the
`MetadataProvider` trait in `src-tauri/src/metadata/mod.rs`:

```rust
pub trait MetadataProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn search(&self, query: &SearchQuery) -> Result<Vec<ProviderMatch>>;
    fn artwork(&self, provider_game_id: &str, kind: &str) -> Result<Vec<ArtworkCandidate>>;
    fn metadata(&self, provider_game_id: &str) -> Result<FetchedMetadata>;
}
```

## Steps (using IGDB as an example)

1. **Implement the trait** in `src-tauri/src/metadata/igdb.rs`. Use the existing
   `steamgriddb.rs` as a reference — it shows auth headers, 429 retry with backoff, and JSON mapping.
   Set `confidence` on matches with `metadata::match_confidence(query, title, year)` so the review UI
   can rank them; return empty `FetchedMetadata` fields as `None` (the merge layer never blanks
   existing values).

2. **Register it** in two places in `metadata/mod.rs`:
   - `provider_statuses()` — add a `ProviderStatus` entry (id, display name, whether an API key is
     required, and the attribution line shown in the UI; check the provider's terms for required
     attribution).
   - `get_provider()` — construct it from settings:
     ```rust
     "igdb" => {
         let key = db.with(|c| settings::get_string(c, "providers.igdb.apiKey"))?
             .filter(|k| !k.trim().is_empty())
             .ok_or_else(|| AppError::Provider("IGDB API key is not configured…".into()))?;
         Ok(Box::new(igdb::Igdb::new(key)))
     }
     ```

3. **Settings UI** — the Providers settings pane (`src/screens/Settings.tsx`, `ProvidersSection`)
   renders every entry from `provider_statuses()`; an API-key field appears automatically when
   `requiresApiKey` is true. Keys are stored under `providers.<id>.apiKey` in the local settings
   table — never hardcode credentials.

4. **Caching and rate limits** — wrap remote calls with `metadata::cached_or_fetch(db, cache_key, …)`
   for a 24-hour persistent cache, and retry 429s with exponential backoff like SteamGridDB does.
   Network failures must surface as `AppError::Provider`/`AppError::Network` so the UI can show them.

That's all — search, the match-review list, artwork candidate browsing, downloads, field locking and
"restore provider metadata" are generic and work with any provider that implements the trait.

## Invariants to respect

- **Never overwrite user data.** Metadata merging goes through
  `db::repo::games::merge_provider_metadata`, which honors `locked_fields` and refuses to blank
  populated fields. Artwork downloads triggered automatically must check
  `artwork::has_user_selected` first (see `commands/meta.rs::apply_provider_match`).
- **The app must work without you.** A provider that is unconfigured returns a clear error from
  `get_provider`; nothing else may break.
- Add unit tests for any scoring or parsing logic (see the `match_confidence` tests).
