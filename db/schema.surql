-- Pavilion schema — always reflects the current complete state
-- Run with: make db-init

-- Migrations tracking
DEFINE TABLE IF NOT EXISTS _migrations SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name ON _migrations TYPE string;
DEFINE FIELD IF NOT EXISTS applied_at ON _migrations TYPE datetime DEFAULT time::now();
DEFINE INDEX IF NOT EXISTS idx_migration_name ON _migrations FIELDS name UNIQUE;

-- ============================================================
-- Person (not "user" — aligns with SlateHub, avoids SurrealDB reserved word)
-- ============================================================
DEFINE TABLE IF NOT EXISTS person SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS email ON person TYPE string ASSERT string::is_email($value);
DEFINE FIELD IF NOT EXISTS name ON person TYPE string ASSERT string::len($value) >= 1;
DEFINE FIELD IF NOT EXISTS password_hash ON person TYPE string;
DEFINE FIELD IF NOT EXISTS roles ON person TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS bio ON person TYPE option<string>;
DEFINE FIELD IF NOT EXISTS avatar_url ON person TYPE option<string>;
DEFINE FIELD IF NOT EXISTS slatehub_id ON person TYPE option<string>;
DEFINE FIELD IF NOT EXISTS gdpr_consent ON person TYPE option<object>;
DEFINE FIELD IF NOT EXISTS gdpr_consent.marketing ON person TYPE option<bool> DEFAULT false;
DEFINE FIELD IF NOT EXISTS gdpr_consent.analytics ON person TYPE option<bool> DEFAULT false;
DEFINE FIELD IF NOT EXISTS gdpr_consent.updated_at ON person TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS created_at ON person TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON person TYPE datetime DEFAULT time::now() VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_person_email ON person FIELDS email UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_person_slatehub ON person FIELDS slatehub_id UNIQUE;

-- ============================================================
-- Terms of Use versioning
-- ============================================================
DEFINE TABLE IF NOT EXISTS terms_version SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS version ON terms_version TYPE string;
DEFINE FIELD IF NOT EXISTS content_hash ON terms_version TYPE string;
DEFINE FIELD IF NOT EXISTS effective_date ON terms_version TYPE datetime;
DEFINE FIELD IF NOT EXISTS created_at ON terms_version TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_terms_version ON terms_version FIELDS version UNIQUE;

-- Graph relation: person agreed to a terms version
DEFINE TABLE IF NOT EXISTS agreed_to SCHEMAFULL TYPE RELATION FROM person TO terms_version;
DEFINE FIELD IF NOT EXISTS agreed_at ON agreed_to TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS ip_hash ON agreed_to TYPE string;

-- ============================================================
-- Film
-- ============================================================
DEFINE TABLE IF NOT EXISTS film SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS title ON film TYPE string ASSERT string::len($value) >= 1;
DEFINE FIELD IF NOT EXISTS slug ON film TYPE string ASSERT string::len($value) >= 1;
DEFINE FIELD IF NOT EXISTS synopsis ON film TYPE option<string>;
DEFINE FIELD IF NOT EXISTS year ON film TYPE option<int>;
DEFINE FIELD IF NOT EXISTS duration_seconds ON film TYPE option<int>;
DEFINE FIELD IF NOT EXISTS genres ON film TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS language ON film TYPE option<string>;
DEFINE FIELD IF NOT EXISTS country ON film TYPE option<string>;
DEFINE FIELD IF NOT EXISTS poster_url ON film TYPE option<string>;
DEFINE FIELD IF NOT EXISTS poster_thumb ON film TYPE option<string>;
DEFINE FIELD IF NOT EXISTS poster_small ON film TYPE option<string>;
DEFINE FIELD IF NOT EXISTS poster_large ON film TYPE option<string>;
DEFINE FIELD IF NOT EXISTS trailer_url ON film TYPE option<string>;
DEFINE FIELD IF NOT EXISTS tmdb_id ON film TYPE option<int>;
DEFINE FIELD IF NOT EXISTS imdb_id ON film TYPE option<string>;
DEFINE FIELD IF NOT EXISTS runtime_minutes ON film TYPE option<int>;
DEFINE FIELD IF NOT EXISTS tagline ON film TYPE option<string>;
DEFINE FIELD IF NOT EXISTS status ON film TYPE string DEFAULT 'draft' ASSERT $value IN ["draft", "published", "archived"];
DEFINE FIELD IF NOT EXISTS content_declaration ON film TYPE option<object>;
DEFINE FIELD IF NOT EXISTS content_declaration.is_copyright_holder ON film TYPE option<bool>;
DEFINE FIELD IF NOT EXISTS content_declaration.talent_cleared ON film TYPE option<bool>;
DEFINE FIELD IF NOT EXISTS content_declaration.no_prohibited_content ON film TYPE option<bool>;
DEFINE FIELD IF NOT EXISTS content_declaration.declared_at ON film TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS created_at ON film TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON film TYPE datetime DEFAULT time::now() VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_film_slug ON film FIELDS slug UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_film_status ON film FIELDS status;

-- Graph relation: person is filmmaker of film (with role on the edge)
DEFINE TABLE IF NOT EXISTS filmmaker_of SCHEMAFULL TYPE RELATION FROM person TO film;
DEFINE FIELD IF NOT EXISTS role ON filmmaker_of TYPE string DEFAULT 'director' ASSERT $value IN ["director", "producer", "writer", "editor", "cinematographer", "other"];
DEFINE FIELD IF NOT EXISTS created_at ON filmmaker_of TYPE datetime DEFAULT time::now();

-- Cast & crew members appearing in a film
DEFINE TABLE IF NOT EXISTS cast_member SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name ON cast_member TYPE string;
DEFINE FIELD IF NOT EXISTS character_name ON cast_member TYPE option<string>;
DEFINE FIELD IF NOT EXISTS department ON cast_member TYPE option<string>;
DEFINE FIELD IF NOT EXISTS job ON cast_member TYPE option<string>;
DEFINE FIELD IF NOT EXISTS sort_order ON cast_member TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS profile_url ON cast_member TYPE option<string>;
DEFINE FIELD IF NOT EXISTS tmdb_id ON cast_member TYPE option<int>;
DEFINE FIELD IF NOT EXISTS film ON cast_member TYPE record<film>;
DEFINE FIELD IF NOT EXISTS created_at ON cast_member TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_cast_film ON cast_member FIELDS film;

-- ============================================================
-- Asset (transcoded renditions, posters, subtitles)
-- ============================================================
DEFINE TABLE IF NOT EXISTS asset SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS asset_type ON asset TYPE string ASSERT $value IN ["master", "rendition", "poster", "subtitle"];
DEFINE FIELD IF NOT EXISTS codec ON asset TYPE option<string>;
DEFINE FIELD IF NOT EXISTS resolution ON asset TYPE option<string>;
DEFINE FIELD IF NOT EXISTS bitrate ON asset TYPE option<int>;
DEFINE FIELD IF NOT EXISTS format ON asset TYPE option<string>;
DEFINE FIELD IF NOT EXISTS storage_key ON asset TYPE string;
DEFINE FIELD IF NOT EXISTS size_bytes ON asset TYPE option<int>;
DEFINE FIELD IF NOT EXISTS created_at ON asset TYPE datetime DEFAULT time::now();

-- Graph relation: film has asset
DEFINE TABLE IF NOT EXISTS has_asset SCHEMAFULL TYPE RELATION FROM film TO asset;
DEFINE FIELD IF NOT EXISTS created_at ON has_asset TYPE datetime DEFAULT time::now();

-- ============================================================
-- Transcode Job Queue
-- ============================================================
DEFINE TABLE IF NOT EXISTS transcode_job SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS film ON transcode_job TYPE record<film>;
DEFINE FIELD IF NOT EXISTS status ON transcode_job TYPE string DEFAULT 'queued' ASSERT $value IN ["queued", "claimed", "processing", "complete", "failed"];
DEFINE FIELD IF NOT EXISTS worker_id ON transcode_job TYPE option<string>;
DEFINE FIELD IF NOT EXISTS profile ON transcode_job TYPE option<object>;
DEFINE FIELD IF NOT EXISTS profile.codec ON transcode_job TYPE option<string>;
DEFINE FIELD IF NOT EXISTS profile.resolutions ON transcode_job TYPE option<array<string>>;
DEFINE FIELD IF NOT EXISTS profile.format ON transcode_job TYPE option<string>;
DEFINE FIELD IF NOT EXISTS progress_pct ON transcode_job TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS error_msg ON transcode_job TYPE option<string>;
DEFINE FIELD IF NOT EXISTS retry_count ON transcode_job TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS max_retries ON transcode_job TYPE int DEFAULT 3;
DEFINE FIELD IF NOT EXISTS created_at ON transcode_job TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS claimed_at ON transcode_job TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS completed_at ON transcode_job TYPE option<datetime>;

DEFINE INDEX IF NOT EXISTS idx_transcode_status_created ON transcode_job FIELDS status, created_at;

-- ============================================================
-- License
-- ============================================================
DEFINE TABLE IF NOT EXISTS license SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS license_type ON license TYPE string ASSERT $value IN ["tvod", "svod", "avod", "hybrid", "event", "educational", "cc"];
DEFINE FIELD IF NOT EXISTS territories ON license TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS window_start ON license TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS window_end ON license TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS approval_required ON license TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS active ON license TYPE bool DEFAULT true;

-- TVOD pricing
DEFINE FIELD IF NOT EXISTS rental_price_cents ON license TYPE option<int>;
DEFINE FIELD IF NOT EXISTS rental_duration_hours ON license TYPE option<int>;
DEFINE FIELD IF NOT EXISTS purchase_price_cents ON license TYPE option<int>;

-- SVOD pricing
DEFINE FIELD IF NOT EXISTS flat_fee_monthly_cents ON license TYPE option<int>;
DEFINE FIELD IF NOT EXISTS revenue_share_pct ON license TYPE option<float>;

-- AVOD pricing (uses revenue_share_pct above)

-- Event pricing
DEFINE FIELD IF NOT EXISTS event_flat_fee_cents ON license TYPE option<int>;
DEFINE FIELD IF NOT EXISTS ticket_split_pct ON license TYPE option<float>;
DEFINE FIELD IF NOT EXISTS max_attendees ON license TYPE option<int>;

-- Educational pricing
DEFINE FIELD IF NOT EXISTS institution_types ON license TYPE option<array<string>>;
DEFINE FIELD IF NOT EXISTS pricing_tier ON license TYPE option<string>;

-- Creative Commons
DEFINE FIELD IF NOT EXISTS cc_license_type ON license TYPE option<string>;

DEFINE FIELD IF NOT EXISTS created_at ON license TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON license TYPE datetime DEFAULT time::now() VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_license_type ON license FIELDS license_type;
DEFINE INDEX IF NOT EXISTS idx_license_active ON license FIELDS active;

-- Graph relation: film licensed via a license (filmmaker creates these)
DEFINE TABLE IF NOT EXISTS licensed_via SCHEMAFULL TYPE RELATION FROM film TO license;
DEFINE FIELD IF NOT EXISTS created_at ON licensed_via TYPE datetime DEFAULT time::now();

-- ============================================================
-- License Acquisition (curator acquires a license for their platform)
-- ============================================================
DEFINE TABLE IF NOT EXISTS acquisition SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS film ON acquisition TYPE record<film>;
DEFINE FIELD IF NOT EXISTS license ON acquisition TYPE record<license>;
DEFINE FIELD IF NOT EXISTS platform ON acquisition TYPE option<record<platform>>;
DEFINE FIELD IF NOT EXISTS requester ON acquisition TYPE record<person>;
DEFINE FIELD IF NOT EXISTS status ON acquisition TYPE string DEFAULT 'pending' ASSERT $value IN ["pending", "approved", "rejected", "expired"];
DEFINE FIELD IF NOT EXISTS requested_at ON acquisition TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS resolved_at ON acquisition TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS resolved_by ON acquisition TYPE option<record<person>>;

DEFINE INDEX IF NOT EXISTS idx_acquisition_status ON acquisition FIELDS status;
DEFINE INDEX IF NOT EXISTS idx_acquisition_film ON acquisition FIELDS film;
DEFINE INDEX IF NOT EXISTS idx_acquisition_requester ON acquisition FIELDS requester;

-- ============================================================
-- Platform (white-label streaming site)
-- ============================================================
DEFINE TABLE IF NOT EXISTS platform SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name ON platform TYPE string ASSERT string::len($value) >= 1;
DEFINE FIELD IF NOT EXISTS slug ON platform TYPE string ASSERT string::len($value) >= 1;
DEFINE FIELD IF NOT EXISTS domain ON platform TYPE option<string>;
DEFINE FIELD IF NOT EXISTS description ON platform TYPE option<string>;
DEFINE FIELD IF NOT EXISTS logo_url ON platform TYPE option<string>;
DEFINE FIELD IF NOT EXISTS theme ON platform TYPE option<object>;
DEFINE FIELD IF NOT EXISTS theme.primary_color ON platform TYPE option<string>;
DEFINE FIELD IF NOT EXISTS theme.secondary_color ON platform TYPE option<string>;
DEFINE FIELD IF NOT EXISTS theme.accent_color ON platform TYPE option<string>;
DEFINE FIELD IF NOT EXISTS theme.font_heading ON platform TYPE option<string>;
DEFINE FIELD IF NOT EXISTS theme.font_body ON platform TYPE option<string>;
DEFINE FIELD IF NOT EXISTS theme.border_radius ON platform TYPE option<string>;
DEFINE FIELD IF NOT EXISTS theme.dark_mode ON platform TYPE option<bool>;
DEFINE FIELD IF NOT EXISTS monetization_model ON platform TYPE option<string>;
DEFINE FIELD IF NOT EXISTS subscription_price_cents ON platform TYPE option<int>;
DEFINE FIELD IF NOT EXISTS status ON platform TYPE string DEFAULT 'setup' ASSERT $value IN ["setup", "active", "suspended"];
DEFINE FIELD IF NOT EXISTS created_at ON platform TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON platform TYPE datetime DEFAULT time::now() VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_platform_slug ON platform FIELDS slug UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_platform_domain ON platform FIELDS domain UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_platform_status ON platform FIELDS status;

-- Graph relation: person is curator of platform
DEFINE TABLE IF NOT EXISTS curator_of SCHEMAFULL TYPE RELATION FROM person TO platform;
DEFINE FIELD IF NOT EXISTS role ON curator_of TYPE string DEFAULT 'owner' ASSERT $value IN ["owner", "admin", "editor"];
DEFINE FIELD IF NOT EXISTS created_at ON curator_of TYPE datetime DEFAULT time::now();

-- ============================================================
-- Platform carries film (via acquisition)
-- ============================================================
DEFINE TABLE IF NOT EXISTS carries SCHEMAFULL TYPE RELATION FROM platform TO film;
DEFINE FIELD IF NOT EXISTS acquisition ON carries TYPE option<record<acquisition>>;
DEFINE FIELD IF NOT EXISTS position ON carries TYPE option<int>;
DEFINE FIELD IF NOT EXISTS featured ON carries TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS added_at ON carries TYPE datetime DEFAULT time::now();

-- ============================================================
-- Watch Session (playhead tracking, analytics)
-- ============================================================
DEFINE TABLE IF NOT EXISTS watch_session SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS person ON watch_session TYPE record<person>;
DEFINE FIELD IF NOT EXISTS film ON watch_session TYPE record<film>;
DEFINE FIELD IF NOT EXISTS platform ON watch_session TYPE record<platform>;
DEFINE FIELD IF NOT EXISTS progress_seconds ON watch_session TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS duration_seconds ON watch_session TYPE option<int>;
DEFINE FIELD IF NOT EXISTS completed ON watch_session TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS started_at ON watch_session TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS last_heartbeat ON watch_session TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_watch_person_film ON watch_session FIELDS person, film, platform;

-- ============================================================
-- Stream Audit Log (every manifest/segment request)
-- ============================================================
DEFINE TABLE IF NOT EXISTS stream_audit SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS person ON stream_audit TYPE option<record<person>>;
DEFINE FIELD IF NOT EXISTS film ON stream_audit TYPE option<record<film>>;
DEFINE FIELD IF NOT EXISTS platform ON stream_audit TYPE option<record<platform>>;
DEFINE FIELD IF NOT EXISTS action ON stream_audit TYPE string ASSERT $value IN ["manifest", "segment", "denied"];
DEFINE FIELD IF NOT EXISTS result ON stream_audit TYPE string ASSERT $value IN ["allowed", "denied"];
DEFINE FIELD IF NOT EXISTS reason ON stream_audit TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON stream_audit TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_audit_created ON stream_audit FIELDS created_at;

-- ============================================================
-- Ratings (per-platform, aggregatable cross-platform)
-- ============================================================
DEFINE TABLE IF NOT EXISTS rating SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS person ON rating TYPE record<person>;
DEFINE FIELD IF NOT EXISTS film ON rating TYPE record<film>;
DEFINE FIELD IF NOT EXISTS platform ON rating TYPE record<platform>;
DEFINE FIELD IF NOT EXISTS score ON rating TYPE int ASSERT $value >= 1 AND $value <= 5;
DEFINE FIELD IF NOT EXISTS review_text ON rating TYPE option<string>;
DEFINE FIELD IF NOT EXISTS hidden ON rating TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS created_at ON rating TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON rating TYPE datetime DEFAULT time::now() VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_rating_unique ON rating FIELDS person, film, platform UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_rating_film_platform ON rating FIELDS film, platform;

-- ============================================================
-- Viewer Subscriptions
-- ============================================================
DEFINE TABLE IF NOT EXISTS viewer_subscription SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS person ON viewer_subscription TYPE record<person>;
DEFINE FIELD IF NOT EXISTS platform ON viewer_subscription TYPE record<platform>;
DEFINE FIELD IF NOT EXISTS provider ON viewer_subscription TYPE string DEFAULT 'stripe';
DEFINE FIELD IF NOT EXISTS external_id ON viewer_subscription TYPE option<string>;
DEFINE FIELD IF NOT EXISTS status ON viewer_subscription TYPE string DEFAULT 'active' ASSERT $value IN ["active", "canceled", "past_due", "incomplete"];
DEFINE FIELD IF NOT EXISTS current_period_start ON viewer_subscription TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS current_period_end ON viewer_subscription TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS created_at ON viewer_subscription TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON viewer_subscription TYPE datetime DEFAULT time::now() VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_sub_person_platform ON viewer_subscription FIELDS person, platform UNIQUE;
DEFINE INDEX IF NOT EXISTS idx_sub_external ON viewer_subscription FIELDS external_id;

-- ============================================================
-- Entitlements (does this viewer have the right to watch?)
-- ============================================================
DEFINE TABLE IF NOT EXISTS entitlement SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS person ON entitlement TYPE record<person>;
DEFINE FIELD IF NOT EXISTS film ON entitlement TYPE record<film>;
DEFINE FIELD IF NOT EXISTS platform ON entitlement TYPE record<platform>;
DEFINE FIELD IF NOT EXISTS entitlement_type ON entitlement TYPE string ASSERT $value IN ["subscription", "rental", "purchase", "free"];
DEFINE FIELD IF NOT EXISTS expires_at ON entitlement TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS external_id ON entitlement TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON entitlement TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_entitlement_lookup ON entitlement FIELDS person, film, platform;

-- ============================================================
-- Payment Accounts (curator's connected payment provider)
-- ============================================================
DEFINE TABLE IF NOT EXISTS payment_account SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS platform ON payment_account TYPE record<platform>;
DEFINE FIELD IF NOT EXISTS provider ON payment_account TYPE string DEFAULT 'stripe';
DEFINE FIELD IF NOT EXISTS external_account_id ON payment_account TYPE option<string>;
DEFINE FIELD IF NOT EXISTS onboarding_complete ON payment_account TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS created_at ON payment_account TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON payment_account TYPE datetime DEFAULT time::now() VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_payment_platform ON payment_account FIELDS platform UNIQUE;

-- ============================================================
-- Transactions (every payment event)
-- ============================================================
DEFINE TABLE IF NOT EXISTS transaction SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS transaction_type ON transaction TYPE string ASSERT $value IN ["rental", "purchase", "subscription", "ad_impression", "ticket", "svod_share"];
DEFINE FIELD IF NOT EXISTS amount_cents ON transaction TYPE int;
DEFINE FIELD IF NOT EXISTS currency ON transaction TYPE string DEFAULT 'usd';
DEFINE FIELD IF NOT EXISTS film ON transaction TYPE option<record<film>>;
DEFINE FIELD IF NOT EXISTS platform ON transaction TYPE record<platform>;
DEFINE FIELD IF NOT EXISTS person ON transaction TYPE option<record<person>>;
DEFINE FIELD IF NOT EXISTS external_id ON transaction TYPE option<string>;
DEFINE FIELD IF NOT EXISTS status ON transaction TYPE string DEFAULT 'completed' ASSERT $value IN ["pending", "completed", "refunded"];
DEFINE FIELD IF NOT EXISTS created_at ON transaction TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_transaction_platform ON transaction FIELDS platform;
DEFINE INDEX IF NOT EXISTS idx_transaction_film ON transaction FIELDS film;
DEFINE INDEX IF NOT EXISTS idx_transaction_created ON transaction FIELDS created_at;

-- ============================================================
-- Revenue Splits (per transaction, who gets what)
-- ============================================================
DEFINE TABLE IF NOT EXISTS revenue_split SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS transaction ON revenue_split TYPE record<transaction>;
DEFINE FIELD IF NOT EXISTS recipient ON revenue_split TYPE record;
DEFINE FIELD IF NOT EXISTS role ON revenue_split TYPE string ASSERT $value IN ["filmmaker", "curator", "platform_fee"];
DEFINE FIELD IF NOT EXISTS amount_cents ON revenue_split TYPE int;
DEFINE FIELD IF NOT EXISTS created_at ON revenue_split TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_split_recipient ON revenue_split FIELDS recipient;
DEFINE INDEX IF NOT EXISTS idx_split_transaction ON revenue_split FIELDS transaction;

-- ============================================================
-- DMCA Claims
-- ============================================================
DEFINE TABLE IF NOT EXISTS dmca_claim SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS claimant_name ON dmca_claim TYPE string ASSERT string::len($value) >= 1;
DEFINE FIELD IF NOT EXISTS claimant_email ON dmca_claim TYPE string ASSERT string::is_email($value);
DEFINE FIELD IF NOT EXISTS claimant_company ON dmca_claim TYPE option<string>;
DEFINE FIELD IF NOT EXISTS film ON dmca_claim TYPE record<film>;
DEFINE FIELD IF NOT EXISTS description ON dmca_claim TYPE string;
DEFINE FIELD IF NOT EXISTS evidence_url ON dmca_claim TYPE option<string>;
DEFINE FIELD IF NOT EXISTS status ON dmca_claim TYPE string DEFAULT 'filed' ASSERT $value IN ["filed", "under_review", "upheld", "rejected", "counter_filed", "resolved"];
DEFINE FIELD IF NOT EXISTS good_faith_statement ON dmca_claim TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS perjury_declaration ON dmca_claim TYPE bool DEFAULT false;
DEFINE FIELD IF NOT EXISTS counter_reason ON dmca_claim TYPE option<string>;
DEFINE FIELD IF NOT EXISTS admin_notes ON dmca_claim TYPE option<string>;
DEFINE FIELD IF NOT EXISTS filed_at ON dmca_claim TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS reviewed_at ON dmca_claim TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS resolved_at ON dmca_claim TYPE option<datetime>;

DEFINE INDEX IF NOT EXISTS idx_dmca_film ON dmca_claim FIELDS film;
DEFINE INDEX IF NOT EXISTS idx_dmca_status ON dmca_claim FIELDS status;

-- ============================================================
-- Events & Screenings
-- ============================================================
DEFINE TABLE IF NOT EXISTS event SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS title ON event TYPE string ASSERT string::len($value) >= 1;
DEFINE FIELD IF NOT EXISTS description ON event TYPE option<string>;
DEFINE FIELD IF NOT EXISTS event_type ON event TYPE string DEFAULT 'screening' ASSERT $value IN ["screening", "qa", "premiere"];
DEFINE FIELD IF NOT EXISTS film ON event TYPE record<film>;
DEFINE FIELD IF NOT EXISTS platform ON event TYPE record<platform>;
DEFINE FIELD IF NOT EXISTS start_time ON event TYPE datetime;
DEFINE FIELD IF NOT EXISTS end_time ON event TYPE option<datetime>;
DEFINE FIELD IF NOT EXISTS max_attendees ON event TYPE option<int>;
DEFINE FIELD IF NOT EXISTS ticket_price_cents ON event TYPE option<int>;
DEFINE FIELD IF NOT EXISTS status ON event TYPE string DEFAULT 'upcoming' ASSERT $value IN ["upcoming", "live", "ended", "canceled"];
DEFINE FIELD IF NOT EXISTS created_at ON event TYPE datetime DEFAULT time::now();
DEFINE FIELD IF NOT EXISTS updated_at ON event TYPE datetime DEFAULT time::now() VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_event_platform ON event FIELDS platform;
DEFINE INDEX IF NOT EXISTS idx_event_status ON event FIELDS status;
DEFINE INDEX IF NOT EXISTS idx_event_start ON event FIELDS start_time;

-- Attendees (ticket holders)
DEFINE TABLE IF NOT EXISTS attending SCHEMAFULL TYPE RELATION FROM person TO event;
DEFINE FIELD IF NOT EXISTS ticket_id ON attending TYPE option<string>;
DEFINE FIELD IF NOT EXISTS purchased_at ON attending TYPE datetime DEFAULT time::now();

-- ============================================================
-- Storage Usage (per-filmmaker metering)
-- ============================================================
DEFINE TABLE IF NOT EXISTS storage_usage SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS person ON storage_usage TYPE record<person>;
DEFINE FIELD IF NOT EXISTS total_bytes ON storage_usage TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS master_bytes ON storage_usage TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS rendition_bytes ON storage_usage TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS asset_count ON storage_usage TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS film_count ON storage_usage TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS updated_at ON storage_usage TYPE datetime DEFAULT time::now() VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_storage_person ON storage_usage FIELDS person UNIQUE;

-- ============================================================
-- Pricing Tiers
-- ============================================================
DEFINE TABLE IF NOT EXISTS pricing_tier SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS name ON pricing_tier TYPE string;
DEFINE FIELD IF NOT EXISTS max_storage_bytes ON pricing_tier TYPE int;
DEFINE FIELD IF NOT EXISTS max_films ON pricing_tier TYPE int;
DEFINE FIELD IF NOT EXISTS price_cents_monthly ON pricing_tier TYPE int;
DEFINE FIELD IF NOT EXISTS active ON pricing_tier TYPE bool DEFAULT true;
DEFINE FIELD IF NOT EXISTS created_at ON pricing_tier TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_tier_name ON pricing_tier FIELDS name UNIQUE;

-- ============================================================
-- Curator Credits
-- ============================================================
DEFINE TABLE IF NOT EXISTS credit_balance SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS person ON credit_balance TYPE record<person>;
DEFINE FIELD IF NOT EXISTS balance_cents ON credit_balance TYPE int DEFAULT 0;
DEFINE FIELD IF NOT EXISTS updated_at ON credit_balance TYPE datetime DEFAULT time::now() VALUE time::now();

DEFINE INDEX IF NOT EXISTS idx_credit_person ON credit_balance FIELDS person UNIQUE;

DEFINE TABLE IF NOT EXISTS credit_transaction SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS person ON credit_transaction TYPE record<person>;
DEFINE FIELD IF NOT EXISTS amount_cents ON credit_transaction TYPE int;
DEFINE FIELD IF NOT EXISTS transaction_type ON credit_transaction TYPE string ASSERT $value IN ["purchase", "deduction", "refund"];
DEFINE FIELD IF NOT EXISTS description ON credit_transaction TYPE option<string>;
DEFINE FIELD IF NOT EXISTS created_at ON credit_transaction TYPE datetime DEFAULT time::now();

DEFINE INDEX IF NOT EXISTS idx_credit_txn_person ON credit_transaction FIELDS person;
