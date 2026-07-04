import type { FormDefinition } from "../../../core/types/form-definition.ts";
import { IL1040_2025_CONFIG } from "./config.ts";
import { inputNodes } from "./inputs.ts";
import { registry } from "./registry.ts";

// Full MeF XML and PDF generation are a later phase. The compute + reconcile
// pipeline (executor → pending lines) does not call buildMefXml / buildPdfBytes,
// so these stubs do not affect `return get` / compute.

export const il1040_2025: FormDefinition = {
  ...IL1040_2025_CONFIG,
  inputNodes,
  registry,
  // Stub: MeF XML for the IL-1040 is not yet implemented.
  buildMefXml: (_pending, _filer) => {
    throw new Error("buildMefXml not yet implemented for il1040");
  },
  // Stub: PDF rendering for the IL-1040 is not yet implemented.
  buildPdfBytes: (_pending, _filer) => Promise.resolve(new Uint8Array()),
  // Identity passthrough — no format-specific normalization yet.
  buildPending: (pending: Record<string, unknown>) => pending,
};
