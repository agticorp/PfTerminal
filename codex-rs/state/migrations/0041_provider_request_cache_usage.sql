ALTER TABLE provider_request_state
    ADD COLUMN last_provider_input_tokens INTEGER NOT NULL DEFAULT 0;

ALTER TABLE provider_request_state
    ADD COLUMN last_provider_cached_input_tokens INTEGER NOT NULL DEFAULT 0;
