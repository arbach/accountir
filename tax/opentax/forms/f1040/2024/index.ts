import type { FormDefinition } from "../../../core/types/form-definition.ts";
import type { FilerIdentity } from "../mef/header.ts";
import type { MefFormsPending } from "../2025/mef/types.ts";
import { F1040_2024_CONFIG } from "./config.ts";
// The node graph (inputs, registry) and the MeF/PDF/pending builders are
// year-independent: every config-injected node reads its constants from
// CONFIG_BY_YEAR[ctx.taxYear]. So TY2024 reuses the 2025 module graph and
// only overrides formType/taxYear/mefSchemaVersion.
import { inputNodes } from "../2025/inputs.ts";
import { registry } from "../2025/registry.ts";
import { buildMefXml } from "../2025/mef/builder.ts";
import { buildPending } from "../2025/mef/pending.ts";
import { buildPdfBytes } from "../2025/pdf/builder.ts";

export const f1040_2024: FormDefinition = {
  ...F1040_2024_CONFIG,
  inputNodes,
  registry,
  buildMefXml: (pending, filer) =>
    buildMefXml(
      pending as MefFormsPending,
      filer as FilerIdentity | undefined,
      F1040_2024_CONFIG.mefSchemaVersion,
      F1040_2024_CONFIG.taxYear,
      F1040_2024_CONFIG.formType === "f1040" ? "1040" : F1040_2024_CONFIG.formType,
    ),
  buildPdfBytes: (pending, filer) =>
    buildPdfBytes(pending, filer),
  buildPending: (pending: Record<string, unknown>) =>
    buildPending(pending) as Record<string, unknown>,
};
