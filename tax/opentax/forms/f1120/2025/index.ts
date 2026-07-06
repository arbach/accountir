import type { FormDefinition } from "../../../core/types/form-definition.ts";
import { F1120_2025_CONFIG } from "./config.ts";
import { inputNodes } from "./inputs.ts";
import { registry } from "./registry.ts";
import { buildPdfBytes } from "./pdf/builder.ts";

// MeF XML is a later phase. PDF rendering (page 1) is implemented in pdf/builder.ts.

export const f1120_2025: FormDefinition = {
  ...F1120_2025_CONFIG,
  inputNodes,
  registry,
  // Stub: MeF XML for the 1120 corporate package is not yet implemented.
  buildMefXml: (_pending, _filer) => {
    throw new Error("buildMefXml not yet implemented for f1120");
  },
  // Fills IRS Form 1120 page 1 from the computed lines + filer identity.
  buildPdfBytes: (pending, filer) => buildPdfBytes(pending, filer),
  // Identity passthrough — no format-specific normalization yet.
  buildPending: (pending: Record<string, unknown>) => pending,
};
