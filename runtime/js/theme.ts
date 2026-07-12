export type SetThemeOptions = { specifier: string };

export type ActiveThemeSummary = {
  specifier: string;
  overrideCount: number;
};

export type FontProfile = {
  families: string[];
  size: number;
};

export type TypographyConfiguration = {
  monospace: FontProfile;
  proportional: FontProfile;
  ui: FontProfile;
};

export type ActiveTypographySummary = { revision: number };

export function setTheme(options: SetThemeOptions | string): ActiveThemeSummary {
  const specifier = typeof options === "string" ? options : options?.specifier;
  if (typeof specifier !== "string" || specifier.length === 0) {
    throw new Error("clay.theme.invalid_request: setTheme requires a theme specifier");
  }
  return JSON.parse(Deno.core.ops.op_clay_theme_set_theme(JSON.stringify({ specifier })));
}

export function setTypography(
  options: TypographyConfiguration,
): ActiveTypographySummary {
  if (options === null || typeof options !== "object") {
    throw new Error(
      "clay.theme.invalid_typography: setTypography requires complete typography profiles",
    );
  }
  return JSON.parse(
    Deno.core.ops.op_clay_theme_set_typography(JSON.stringify(options)),
  );
}
