# accountir — Tax sub-system

A tax-return calculation pipeline built on **[OpenTax](https://github.com/filedcom/opentax)**
(TypeScript/Deno federal tax engine) with the entity/state forms our books need added on top,
plus a **bridge** that feeds the accountir ledger into it.

> New here? Read **`AGENT_BRIEF.md`** (the mission + entities + forms), then `opentax/CLAUDE.md`
> (coding conventions). This README is just how to run things.

## Layout
- `opentax/` — vendored OpenTax engine (our working fork). We add forms under `opentax/forms/`
  and register them in `opentax/catalog.ts`. Vendored from upstream on first setup (see
  `opentax/VENDORED.txt`); sync upstream manually and re-apply our forms.
- `bridge/` — `accountir_cloud` (Postgres) ledger → OpenTax inputs. `BRIDGE.md` + `export_return.py`.
- `forms/` — reserved for our per-form IRS line-mapping notes (the spec lives in each form's `FORM.md`).
- `AGENT_BRIEF.md` — handoff brief for the tax agent. `.claude/agents/tax-agent.md` defines the agent.

## Prerequisites
- **Deno** (installed at `~/.deno`): `export PATH="$HOME/.deno/bin:$PATH"`. Version 2.9+.
- Postgres access to `accountir_cloud` (the bridge reads the ledger): `sudo -u postgres psql accountir_cloud`.

## Run OpenTax
```bash
export PATH="$HOME/.deno/bin:$PATH"
cd tax/opentax
deno run --allow-read --allow-write --allow-net=www.irs.gov cli/main.ts --help   # or: deno task tax --help
deno task test            # run the node test suite
deno task tax node list   # list all registered nodes
```

Typical return loop (f1040 works today):
```bash
deno task tax return create --year 2025                       # -> returnId
deno task tax form add --returnId <id> --node_type w2 '{"box1_wages":50000,"box2_fed_withheld":5000}'
deno task tax return get --returnId <id>                      # computed line items
deno task tax return validate --returnId <id>
deno task tax return export --returnId <id> --type pdf --output /tmp/f1040.pdf
```

## Bridge (books → return)
```bash
python3 tax/bridge/export_return.py --entity maven --year 2025   # emits opentax `form add` inputs (see BRIDGE.md)
```

## Status
- ✅ OpenTax engine vendored + runs (Deno 2.9). f1040:2025 supported upstream.
- ⬜ Our forms (f1120, f1120-S + K-1 + 8825, state) — scaffolded specs in `opentax/forms/*/FORM.md`, not yet implemented.
- ⬜ Bridge — spec + starter script in `bridge/`, not yet end-to-end.
