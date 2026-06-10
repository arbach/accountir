-- One maintained Claude CLI session per company, managed by accountir-agentd.
-- Infrastructure table (no RLS): the daemon needs cross-company access, and the
-- MCP endpoint resolves bearer tokens to a company before any tenant query runs.
CREATE TABLE agent_sessions (
    company_id   uuid PRIMARY KEY REFERENCES companies(id) ON DELETE CASCADE,
    session_id   uuid NOT NULL,
    mcp_token    text NOT NULL UNIQUE,
    last_user_id uuid,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now()
);
