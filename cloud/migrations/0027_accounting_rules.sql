-- Per-company free-text accounting rules the AI agent must follow when doing the books
-- (owner-editable in Settings; injected into the agent's system prompt per session).
ALTER TABLE companies ADD COLUMN IF NOT EXISTS accounting_rules TEXT NOT NULL DEFAULT '';
