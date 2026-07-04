export const IL1120_2025_CONFIG = {
  formType: "il1120" as const,
  taxYear: 2025 as const,
  // IL returns are not yet wired to MeF; this is a placeholder for the stub
  // FormDefinition contract (buildMefXml throws).
  mefSchemaVersion: "2025v1.0" as const,
} as const;
