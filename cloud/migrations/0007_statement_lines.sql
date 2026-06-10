-- Transaction lines parsed from Plaid Statements PDFs, staged for review.
CREATE TABLE statement_lines (
    id           uuid PRIMARY KEY,
    company_id   uuid NOT NULL,
    item_id      uuid NOT NULL,
    statement_id text NOT NULL,
    txn_date     date,
    description  text NOT NULL,
    amount_cents bigint NOT NULL,
    status       text NOT NULL DEFAULT 'parsed',
    created_at   timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX statement_lines_company_item_idx ON statement_lines (company_id, item_id);
CREATE INDEX statement_lines_statement_idx ON statement_lines (company_id, statement_id);
