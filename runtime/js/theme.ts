export type SetThemeOptions = { specifier: string };

export type ActiveThemeSummary = {
  specifier: string;
  overrideCount: number;
};

export function setTheme(options: SetThemeOptions | string): ActiveThemeSummary {
  const specifier = typeof options === "string" ? options : options?.specifier;
  if (typeof specifier !== "string" || specifier.length === 0) {
    throw new Error("clay.theme.invalid_request: setTheme requires a theme specifier");
  }
  return JSON.parse(Deno.core.ops.op_clay_theme_set_theme(JSON.stringify({ specifier })));
}
