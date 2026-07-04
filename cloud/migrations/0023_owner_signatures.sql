-- Owner signature, used to sign approved tax forms before mailing.
-- Keyed by user (the owner), NOT by company: one owner (e.g. the personal
-- account) owns several entities and signs all their returns with the same
-- signature. Stored as a stampable PNG regardless of how it was created
-- (uploaded image, or typed name rendered in a handwriting font).
CREATE TABLE IF NOT EXISTS owner_signatures (
    user_id      uuid PRIMARY KEY REFERENCES auth_users(id) ON DELETE CASCADE,
    kind         text NOT NULL DEFAULT 'typed',           -- 'image' | 'typed'
    image_png    bytea NOT NULL,                           -- the stampable signature bitmap
    content_type text NOT NULL DEFAULT 'image/png',
    typed_text   text,                                     -- when kind='typed': the name
    typed_font   text,                                     -- when kind='typed': the font key
    updated_at   timestamptz NOT NULL DEFAULT now()
);
