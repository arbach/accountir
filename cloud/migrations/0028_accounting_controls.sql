-- Accounting controls (soundness review 2026-08-29):
-- 1. Period locking: financial mutations dated on or before books_closed_through
--    are rejected at the event-store choke point (see store/event_store.rs).
-- 2. DB-level backstop that every journal entry balances to zero at commit —
--    app code already enforces this on the main paths; this guards refactors
--    (the 5e7e69e/b103e8a class of bug) no matter which path posts.

ALTER TABLE companies ADD COLUMN IF NOT EXISTS books_closed_through DATE;

CREATE OR REPLACE FUNCTION assert_entry_balanced() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
  eid uuid;
  s bigint;
BEGIN
  eid := COALESCE(NEW.entry_id, OLD.entry_id);
  SELECT COALESCE(SUM(amount), 0) INTO s FROM journal_lines WHERE entry_id = eid;
  IF s <> 0 THEN
    RAISE EXCEPTION 'journal entry % does not balance (sum = % cents)', eid, s;
  END IF;
  RETURN NULL;
END $$;

-- Deferred so multi-row inserts are checked once the entry is complete, at commit.
DROP TRIGGER IF EXISTS journal_lines_balanced ON journal_lines;
CREATE CONSTRAINT TRIGGER journal_lines_balanced
  AFTER INSERT OR UPDATE OR DELETE ON journal_lines
  DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION assert_entry_balanced();
