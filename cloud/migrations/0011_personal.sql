-- Personal entity flag: the owner's personal company. Its agent session may
-- operate across all entities the user is a member of (enforced in the MCP
-- layer, which re-scopes per call after a membership check).
ALTER TABLE companies ADD COLUMN is_personal boolean NOT NULL DEFAULT false;
