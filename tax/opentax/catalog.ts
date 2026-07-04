import type { FormDefinition } from "./core/types/form-definition.ts";
import { f1040_2025 } from "./forms/f1040/2025/index.ts";
import { f1040_2024 } from "./forms/f1040/2024/index.ts";
import { f1040_2023 } from "./forms/f1040/2023/index.ts";
import { f1120s_2025 } from "./forms/f1120s/2025/index.ts";
import { f1120_2025 } from "./forms/f1120/2025/index.ts";
import { il1040_2025 } from "./forms/il1040/2025/index.ts";
import { il1120_2025 } from "./forms/il1120/2025/index.ts";
import { il1120st_2025 } from "./forms/il1120st/2025/index.ts";

// Form 1120-S ordinary/rental computation has no year-dependent brackets (S-corp federal
// income tax is generally $0), so prior years reuse the 2025 node graph with the tax year
// overridden. (MeF/PDF are stubs for now; when real builders land, give each year its own.)
const f1120s_2024: FormDefinition = { ...f1120s_2025, taxYear: 2024, mefSchemaVersion: "2024v5.0" };
const f1120s_2023: FormDefinition = { ...f1120s_2025, taxYear: 2023, mefSchemaVersion: "2023v5.0" };

// Form 1120 (C corporation) line structure is year-stable and the 21% flat rate (IRC §11)
// applies for all tax years 2018+, so prior years reuse the 2025 node graph with the tax
// year overridden. (MeF/PDF are stubs for now; when real builders land, give each its own.)
const f1120_2024: FormDefinition = { ...f1120_2025, taxYear: 2024, mefSchemaVersion: "2024v5.0" };
const f1120_2023: FormDefinition = { ...f1120_2025, taxYear: 2023, mefSchemaVersion: "2023v5.0" };

// Illinois forms are flat-rate and year-stable: the individual rate (4.95%),
// S-corp replacement tax (1.5%), and C-corp income+replacement tax (7% + 2.5%)
// are unchanged across 2023–2025, so prior years reuse the 2025 node graph with
// the tax year overridden. The IL-1040 personal exemption allowance DOES change
// yearly, but the il1040 node reads it from CONFIG_BY_YEAR[ctx.taxYear], so the
// taxYear override alone selects the correct exemption (no per-year FormDefinition
// needed). (MeF/PDF are stubs for now; when real builders land, give each its own.)
const il1040_2024: FormDefinition = { ...il1040_2025, taxYear: 2024, mefSchemaVersion: "2024v1.0" };
const il1040_2023: FormDefinition = { ...il1040_2025, taxYear: 2023, mefSchemaVersion: "2023v1.0" };
const il1120_2024: FormDefinition = { ...il1120_2025, taxYear: 2024, mefSchemaVersion: "2024v1.0" };
const il1120_2023: FormDefinition = { ...il1120_2025, taxYear: 2023, mefSchemaVersion: "2023v1.0" };
const il1120st_2024: FormDefinition = { ...il1120st_2025, taxYear: 2024, mefSchemaVersion: "2024v1.0" };
const il1120st_2023: FormDefinition = { ...il1120st_2025, taxYear: 2023, mefSchemaVersion: "2023v1.0" };

// Form 1040 prior years CANNOT be a simple spread of f1040_2025 like the
// corporate forms above: the tax constants (brackets, standard deduction, QBI/
// AMT/EITC thresholds, etc.) differ every year. Each year therefore has its own
// F1040Config in CONFIG_BY_YEAR and its own FormDefinition (taxYear 2023/2024).
export const catalog: Record<string, FormDefinition> = {
  "f1040:2025": f1040_2025,
  "f1040:2024": f1040_2024,
  "f1040:2023": f1040_2023,
  "f1120s:2025": f1120s_2025,
  "f1120s:2024": f1120s_2024,
  "f1120s:2023": f1120s_2023,
  "f1120:2025": f1120_2025,
  "f1120:2024": f1120_2024,
  "f1120:2023": f1120_2023,
  "il1040:2025": il1040_2025,
  "il1040:2024": il1040_2024,
  "il1040:2023": il1040_2023,
  "il1120:2025": il1120_2025,
  "il1120:2024": il1120_2024,
  "il1120:2023": il1120_2023,
  "il1120st:2025": il1120st_2025,
  "il1120st:2024": il1120st_2024,
  "il1120st:2023": il1120st_2023,
};
