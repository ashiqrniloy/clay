// Composer (plan 119 SC-4): the agent lane's input — the message field, its
// `@`-mention completions, and the manifest-bound effort cycle chord. One
// component owns that whole state machine (draft + completion cursor, both
// local), so the lane stays a composition root and the send authority
// (intercepts, steer-vs-prompt, run lifecycle) stays with it.
//
// Plan 124: a `/`-led draft is not a local completion list any more — it is the
// command palette, whose session the server owns and whose rows are the command
// catalogue (`CommandPalette`). The field stays the query (the sheet echoes it),
// so `↑↓` move the session's selection, `↵` runs the row, `Esc` dismisses and
// the draft survives; only `@` mentions keep the inline dropdown. No Send
// button: `↵` sends, the hint row says so, and Stop takes the trailing slot
// exactly while a run is live (DESIGN.md §12). A missing provider never disables
// the field; it only gates what a submit does, and the lane's foot says why
// nothing sends.
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEventHandler,
  type ReactNode,
} from "react";
import { ClayIconButton, ClayKbd, ClayTextField } from "../components";
import {
  CommandPalette,
  paletteModeOf,
  type PaletteMode,
  type PaletteScope,
} from "../command-centre/CommandPalette";
import type { TransientMenuSnapshotDto } from "../bridge/types";
import styles from "./coding-agent.module.css";

/** Plan 109 I4: effective effort-cycle chord (key + modifiers), read from
 *  the behavior manifest by the shell so `bindKey` overrides apply. */
export interface EffortChord {
  shift: boolean;
  ctrl: boolean;
  alt: boolean;
  meta: boolean;
  key: string;
}

/** `Shift+Tab` — the daemon's declared default when nothing overrides it. */
const DEFAULT_EFFORT_CHORD: EffortChord = {
  shift: true,
  ctrl: false,
  alt: false,
  meta: false,
  key: "Tab",
};

/** Does this key event match the (defaulted) cycle chord? All-or-nothing on
 *  modifiers: a rebound chord keeps its own shape, a loose one does not match. */
export function chordMatches(
  event: {
    key: string;
    shiftKey: boolean;
    ctrlKey?: boolean;
    altKey?: boolean;
    metaKey?: boolean;
  },
  chord: EffortChord | null,
): boolean {
  const effective = chord ?? DEFAULT_EFFORT_CHORD;
  return (
    event.key === effective.key &&
    event.shiftKey === effective.shift &&
    (event.ctrlKey ?? false) === effective.ctrl &&
    (event.altKey ?? false) === effective.alt &&
    (event.metaKey ?? false) === effective.meta
  );
}

/** The level a cycle step lands on: the next declared level after the shown
 *  one, wrapping to the first. A shown level the model no longer declares (a
 *  stale pending pick) restarts from the declared top. Callers guard the empty
 *  list — no declared levels means no effort control at all. */
export function nextEffortLevel(
  levels: readonly string[],
  shown: string | null,
): string | null {
  const current =
    shown !== null && levels.includes(shown)
      ? shown
      : levels[levels.length - 1];
  const index = current ? levels.indexOf(current) : levels.length - 1;
  return levels[(index + 1) % levels.length] ?? levels[0] ?? null;
}

/** The draft's trailing `@token` — the mention query and where it starts — or
 *  `null` when the draft does not end in one. */
export function mentionTokenOf(
  draft: string,
): { query: string; start: number } | null {
  const match = /(?:^|\s)@([^\s]*)$/.exec(draft);
  if (!match) return null;
  return {
    query: match[1] ?? "",
    start: match.index + match[0].lastIndexOf("@"),
  };
}

/** The `/` palette the field drives (plan 124; plan 125 made one sheet of the
 *  catalogue and every picker stage). The session (and its item list) belongs
 *  to the server; everything here is the field's side of it: the open state,
 *  the filter it holds, and the intents a keystroke turns into. */
export interface ComposerPalette {
  /** The open palette session (`origin === "commandPalette"`), else null. */
  menu: TransientMenuSnapshotDto | null;
  /** Open the catalogue session (the `/` keystroke and the Control Centre
   *  chord/trigger both land here: the session is the one owner of "open"). */
  request: () => void;
  /** Send the field's filter (what follows the `/` sigil) and the scope chip
   *  selected with it — the session owns both halves of the filter, so the
   *  client never hides rows the server still selects over. */
  query: (filter: string, scope: PaletteScope) => void;
  /** Move the session's selection by `delta` (`↑↓`). */
  move: (delta: number) => void;
  /** Run the selected row; `secondary` activates the row's other action (a
   *  session row deletes instead of resuming). */
  activate: (secondary?: boolean) => void;
  /** Semantic Backspace: one stage back. The session closes when it has nothing
   *  behind it (plan 125 — `Esc` at a stage's first step dismisses the sheet). */
  back: () => void;
  /** Dismiss the session (the draft stays; the next keystroke reopens it). */
  cancel: () => void;
}

/**
 * Plan 125: the agent an agent-less tab adopts as soon as the server's listing
 * arrives — the coding agent when it is listed (`coding-agent`, its data-root
 * name; `coding` for a shortened install), else whatever the server listed
 * first, else nothing (the tab stays agent-less and the field stays typable).
 * The listing is the server's validated vocabulary, so the client never invents
 * a type.
 */
export function defaultAgentType(
  entries: readonly { name: string }[],
): string | null {
  for (const preferred of ["coding-agent", "coding"]) {
    const match = entries.find((entry) => entry.name === preferred);
    if (match) return match.name;
  }
  return entries[0]?.name ?? null;
}

/** The palette's filter: the field's text after the `/` sigil. */
export function paletteFilter(draft: string): string {
  return draft.startsWith("/") ? draft.slice(1) : draft;
}

export interface ComposerProps {
  /** The tab runs no agent: the box offers the agent picker (`Attach an
   *  agent`) and hides the model/effort controls. The field stays typable —
   *  the draft is the user's, and a submit with nothing to send it to is what
   *  the lane's foot explains (plan 125) — exactly as a missing *provider*
   *  never disables it either. */
  agentless: boolean;
  /** A run is in flight: the composer is the steering lane and Stop replaces
   *  the empty trailing slot. */
  streaming: boolean;
  /** The bound agent session (`""` = none); keys the one-shot file fetch. */
  sessionId: string;
  /** The tab's own agent command lane (the @-mention file fetch rides it). */
  command: (command: Record<string, unknown> | string) => void;
  /** Daemon-registered + client built-in slash commands, already merged. */
  /** The command palette's session and intents (plan 124). */
  palette: ComposerPalette;
  /** Reports whether one of the field's menus is up (`/` palette or `@`
   *  mentions): the shell veils the working area while one is. */
  onFieldMenuOpen?: (
    open: boolean,
  ) => void; /** Skill catalog and workspace files for the merged @-mention dropdown. */
  skills: ReadonlyArray<{ name: string; description?: string }>;
  files: readonly string[];
  /** Declared thinking levels for the active model (`null` = no control). */
  effortLevels: string[] | null;
  /** The level the next prompt would carry (the pending pick, else the
   *  session's active one). */
  shownEffort: string | null;
  onEffortChange: (level: string | null) => void;
  /** Manifest-bound cycle chord; `null` uses the Shift+Tab default. */
  effortChord: EffortChord | null;
  /** The lane's agent-control toolbar, rendered as the field shell's second
   *  row (agent type, model, effort, context meter). */
  toolbar?: ReactNode;
  /** Stop the running turn. */
  onStop: () => void;
  /** Send text (a prompt, a steer, or a client built-in). Returns whether the
   *  text was consumed — the lane answers `false` for input it ignored (empty
   *  or no provider), and the draft survives that answer. */
  onSubmit: (text: string) => boolean;
}

export function Composer({
  agentless,
  streaming,
  sessionId,
  command,
  palette,
  onFieldMenuOpen,
  skills,
  files,
  effortLevels,
  shownEffort,
  onEffortChange,
  effortChord,
  toolbar = null,
  onStop,
  onSubmit,
}: ComposerProps) {
  const [draft, setDraft] = useState("");
  const [paletteScope, setPaletteScope] = useState<PaletteScope>("all");
  const [completionIndex, setCompletionIndex] = useState(0);
  const [completionDismissed, setCompletionDismissed] = useState(false);
  const formRef = useRef<HTMLFormElement | null>(null);
  const paletteSession = palette.menu;
  const paletteOpen = paletteSession !== null;
  const paletteSessionId = paletteSession?.sessionId ?? null;
  // Plan 125: the session's mode decides which of the sheet's vocabularies is
  // live. The catalogue — and the path browser, whose query is what follows the
  // same sigil — is reached through the field's `/`. A picker stage's filter is
  // the field's text as it stands, and one stage (`secret`) owns its own
  // shielded input instead, so the field is not its transport.
  const paletteMode: PaletteMode | null = paletteSession
    ? paletteModeOf(paletteSession)
    : null;
  const sigilMode =
    paletteMode === null ||
    paletteMode === "catalogue" ||
    paletteMode === "path";
  const stageOpen = paletteMode !== null && !sigilMode;
  const shieldOpen = paletteMode === "secret";
  const paletteQuery = sigilMode ? paletteFilter(draft) : draft;

  // Plan 124: the palette's filter lives in this field, so the catch-up send
  // happens when the session *appears* — a trigger-opened session or a pasted
  // `/query` is ahead of the one-shot open intent, and the server filters what
  // it is given. Keystrokes after that ride `onChange` (one send each), which is
  // why the draft is read through a ref here instead of a dependency.
  const draftRef = useRef(draft);
  draftRef.current = draft;
  // The intent callbacks are rebuilt by the shell every render (they close over
  // the live session); read them through a ref so the catch-up stays a one-shot
  // per session rather than a send per render.
  const paletteIntents = useRef(palette);
  paletteIntents.current = palette;
  // The chip (and the field's filter) is per *session*: a new session (a fresh
  // open, the catalogue swapping for a picker stage, the catalogue for its path
  // mode) starts on `All`, which is also the scope the server starts it with,
  // so the segment never shows a filter the session is not applying. A session
  // whose field carries a sigil is seeded from the draft (a trigger-opened
  // session or a pasted `/query` is ahead of the one-shot open intent); a stage
  // starts empty on the server too, so the field is cleared rather than seeded
  // with text the session never received.
  useEffect(() => {
    if (paletteSessionId === null) return;
    setPaletteScope("all");
    if (sigilMode)
      paletteIntents.current.query(paletteFilter(draftRef.current), "all");
    else setDraft("");
  }, [paletteSessionId, sigilMode]);

  // The trigger and the `Ctrl+X Ctrl+O` chord open the palette on the field's
  // behalf: the field is the query, so it takes the sigil and the focus
  // (DESIGN.md §12 — the trigger puts the field in query mode). The artifact
  // seeds `/` even over a draft; a `/`-led draft is already in query mode.
  useEffect(() => {
    if (!paletteOpen || !sigilMode || draftRef.current.startsWith("/")) return;
    setDraft("/");
    setCompletionDismissed(false);
    formRef.current?.querySelector<HTMLTextAreaElement>("textarea")?.focus();
  }, [paletteOpen, sigilMode]);

  // The shielded stage's control is the sheet's own field, so the focus follows
  // it: the composer's field is disabled for that stage, and the credential is
  // typed where it is shielded (§12/§13).
  useEffect(() => {
    if (!shieldOpen) return;
    formRef.current
      ?.querySelector<HTMLInputElement>('input[type="password"]')
      ?.focus();
  }, [shieldOpen, paletteSessionId]);

  // Plan 117 @-mentions: a trailing `@token` in the draft opens the merged
  // dropdown — the session's skill catalog plus the workspace file cache
  // (both server-known; filter is client-side). `@skill:x` / `@file:x`
  // narrow to one section; a bare token filters both. A picker stage's text is
  // its filter, not a prompt, so no mention list opens over it.
  const mentionToken = useMemo(
    () => (completionDismissed || stageOpen ? null : mentionTokenOf(draft)),
    [completionDismissed, draft, stageOpen],
  );
  const mentionMatches = useMemo(() => {
    if (!mentionToken) return null;
    const raw = mentionToken.query;
    const skillQuery = raw.startsWith("skill:")
      ? raw.slice(6)
      : raw.startsWith("file:")
        ? null
        : raw;
    const fileQuery = raw.startsWith("file:")
      ? raw.slice(5)
      : raw.startsWith("skill:")
        ? null
        : raw;
    const section = (
      label: string,
      items: ReadonlyArray<{ name: string; detail: string }>,
      kind: "skill" | "file",
      query: string,
    ) => ({
      label,
      items: items
        .filter((item) => item.name.toLowerCase().includes(query.toLowerCase()))
        .slice(0, 8)
        .map((item) => ({ kind, name: item.name, detail: item.detail })),
    });
    const sections = [
      ...(skillQuery !== null
        ? [
            section(
              "Skills",
              skills.map((skill) => ({
                name: skill.name,
                detail: skill.description ?? "",
              })),
              "skill",
              skillQuery,
            ),
          ]
        : []),
      ...(fileQuery !== null && files.length > 0
        ? [
            section(
              "Files",
              // The artifact's row states where the file lives: its directory,
              // or the root itself (`repository root`).
              files.map((path) => ({
                name: path,
                detail: path.includes("/")
                  ? path.slice(0, path.lastIndexOf("/"))
                  : "repository root",
              })),
              "file",
              fileQuery,
            ),
          ]
        : []),
    ];
    const flat = sections.flatMap((entry) => entry.items);
    return flat.length > 0 ? { sections, flat } : null;
  }, [files, mentionToken, skills]);
  const mentionOpen = mentionMatches !== null;

  // The shell veils the working area while the field's inline menu is up
  // (DESIGN.md §6: one scrim, two callers). The palette's half of that veil is
  // the shell's own session state; the `@` dropdown is this component's, so it
  // reports it up rather than owning a veil it cannot position.
  useEffect(() => {
    onFieldMenuOpen?.(mentionOpen);
  }, [mentionOpen, onFieldMenuOpen]);

  // First @-token of a session fetches the bounded workspace listing once;
  // the reply rides the clay.agentRpc custom event into agent state.
  const filesFetchedFor = useRef("");
  useEffect(() => {
    if (!mentionToken || !sessionId || filesFetchedFor.current === sessionId)
      return;
    filesFetchedFor.current = sessionId;
    command({ workspaceFiles: { sessionId } });
  }, [command, mentionToken, sessionId]);

  const embedMention = useCallback((kind: "skill" | "file", name: string) => {
    setDraft((current) => {
      const start = mentionTokenOf(current)?.start;
      if (start === undefined) return current;
      return `${current.slice(0, start)}@${kind}:${name} `;
    });
    setCompletionDismissed(true);
  }, []);

  const onComposerKeyDown = useCallback(
    (event: {
      key: string;
      shiftKey: boolean;
      ctrlKey?: boolean;
      altKey?: boolean;
      metaKey?: boolean;
      preventDefault: () => void;
    }) => {
      // Plan 109 I4: the manifest-bound effort chord (default Shift+Tab)
      // cycles declared levels — a no-op (no focus change) when the model
      // declares none or the chord is unbound.
      if (chordMatches(event, effortChord)) {
        if (effortLevels && effortLevels.length > 0) {
          event.preventDefault();
          onEffortChange(nextEffortLevel(effortLevels, shownEffort));
        }
        return;
      }
      if (paletteOpen && event.key === "Enter" && event.altKey) {
        // `Alt+↵` is the selected row's secondary action (a session row deletes
        // instead of resuming); the primary path stays the field's submit.
        event.preventDefault();
        paletteIntents.current.activate(true);
        return;
      }
      if (paletteOpen) {
        // The palette owns the field's keys while it is up: `↑↓` move the
        // session's selection, `Esc`/`Alt+←` walk it back, all against the
        // session the server holds (DESIGN.md §12). `↵` is the submit path
        // below.
        if (event.key === "ArrowDown") {
          event.preventDefault();
          paletteIntents.current.move(1);
        } else if (event.key === "ArrowUp") {
          event.preventDefault();
          paletteIntents.current.move(-1);
        } else if (
          event.key === "Escape" ||
          (event.altKey && event.key === "ArrowLeft")
        ) {
          event.preventDefault();
          // A stage walks back one step, and the session is what answers
          // whether anything is behind it: the sheet closes at a stage's first
          // step (plan 125). The catalogue and the path browser dismiss, which
          // is where their drafts survive for the next keystroke.
          if (stageOpen) paletteIntents.current.back();
          else paletteIntents.current.cancel();
        }
        return;
      }
      // Plan 117 @-mention dropdown: the one inline completion list left.
      if (mentionMatches && mentionMatches.flat.length > 0) {
        const count = mentionMatches.flat.length;
        if (event.key === "ArrowDown") {
          event.preventDefault();
          setCompletionIndex((index) => (index + 1) % count);
        } else if (event.key === "ArrowUp") {
          event.preventDefault();
          setCompletionIndex((index) => (index - 1 + count) % count);
        } else if (event.key === "Escape") {
          event.preventDefault();
          setCompletionDismissed(true);
        } else if (event.key === "Tab") {
          event.preventDefault();
          const picked = mentionMatches.flat[completionIndex % count];
          if (picked) embedMention(picked.kind, picked.name);
        }
      }
    },
    [
      effortChord,
      effortLevels,
      shownEffort,
      onEffortChange,
      paletteOpen,
      stageOpen,
      mentionMatches,
      completionIndex,
      embedMention,
    ],
  );

  // The shield is the focused control of its stage, so the stage's keys belong
  // to it: `↵` runs the selected row (Store API key) and everything else is the
  // routing the field does — one handler, two inputs.
  const onShieldKeyDown: KeyboardEventHandler<
    HTMLInputElement | HTMLTextAreaElement
  > = useCallback(
    (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        paletteIntents.current.activate(event.altKey);
        return;
      }
      onComposerKeyDown(event);
    },
    [onComposerKeyDown],
  );

  // A submit with the palette open runs the highlighted row instead of sending
  // the text: the catalogue row is the command, and the field goes back to
  // being a prompt (the artifact clears it on activation). Otherwise the lane
  // decides whether the text was consumed, and only then does the draft go away
  // (an ignored submit keeps what was typed).
  const onComposerSubmit = useCallback(
    (value: string) => {
      if (paletteOpen && (paletteSession?.items.length ?? 0) > 0) {
        setDraft("");
        setCompletionIndex(0);
        setCompletionDismissed(true);
        paletteIntents.current.activate();
        return;
      }
      // Nothing to run: a typed built-in still has a path, so the text goes to
      // the lane (which intercepts `/model`, `/resume`) and the session closes.
      // A stage has no such path — its text is a filter, and `↵` on an empty
      // list is not a dismissal.
      if (paletteOpen && !stageOpen) paletteIntents.current.cancel();
      if (paletteOpen && stageOpen) return;
      if (!onSubmit(value)) return;
      setDraft("");
      setCompletionIndex(0);
      setCompletionDismissed(true);
    },
    [onSubmit, paletteOpen, paletteSession, stageOpen],
  );

  return (
    <form
      ref={formRef}
      className={styles.composer}
      onSubmit={(event) => {
        event.preventDefault();
        onComposerSubmit(draft);
      }}
    >
      {mentionMatches && (
        <ul className={styles.completions} aria-label="Mentions" role="listbox">
          {mentionMatches.sections.map((section) => (
            <li key={section.label} role="presentation">
              <span className={styles.completionSection} role="presentation">
                {section.label}
              </span>
              <ul role="group" aria-label={`${section.label} matches`}>
                {section.items.map((item) => (
                  <li key={`${item.kind}:${item.name}`}>
                    <button
                      type="button"
                      role="option"
                      aria-selected={
                        mentionMatches.flat[
                          completionIndex % mentionMatches.flat.length
                        ]?.name === item.name
                      }
                      className={styles.completionRow}
                      onClick={() => embedMention(item.kind, item.name)}
                    >
                      <span className={styles.completionName}>
                        @{item.kind}:{item.name}
                      </span>
                      {/* The row states what the skill does / where the file
                       * lives (the approved artifact's composition; the style
                       * was already here, the span was not rendered). */}
                      {item.detail ? (
                        <span className={styles.completionDetail}>
                          {item.detail}
                        </span>
                      ) : null}
                    </button>
                  </li>
                ))}
              </ul>
            </li>
          ))}
        </ul>
      )}
      <ClayTextField
        label="Message"
        value={draft}
        onChange={(value) => {
          // The one stage whose input is the sheet's shield never reaches the
          // draft: text typed here (or pasted) is dropped, because the draft is
          // persisted layout state and a credential must not be in it (§13).
          if (shieldOpen) return;
          setDraft(value);
          setCompletionIndex(0);
          setCompletionDismissed(false);
          // Plan 124/125: the field's text *is* the session's filter. The
          // catalogue (and the path browser) is reached through the `/` sigil —
          // the first slash opens the session, later keystrokes filter it, and
          // losing the slash closes it (the artifact's own live-state rule). A
          // picker stage's filter carries no sigil and never closes the stage.
          if (sigilMode) {
            if (value.startsWith("/")) {
              if (!paletteOpen) paletteIntents.current.request();
              else
                paletteIntents.current.query(
                  paletteFilter(value),
                  paletteScope,
                );
            } else if (paletteOpen) {
              paletteIntents.current.cancel();
            }
            return;
          }
          if (paletteOpen) paletteIntents.current.query(value, "all");
        }}
        multiline
        autoGrow
        variant="composer"
        placeholder={
          agentless
            ? "Type a prompt — attach an agent to send it"
            : shieldOpen
              ? "Type the credential in the sheet"
              : streaming
                ? "Steer the agent, or wait"
                : "Ask, or type / or @"
        }
        // Plan 125: no state makes the field inert. The draft is the user's,
        // and what would send it (agent, provider) is the lane's foot's
        // business; only the shielded credential stage moves the input into
        // the sheet.
        disabled={shieldOpen}
        onKeyDown={onComposerKeyDown}
        onSubmit={onComposerSubmit}
        toolbar={toolbar}
        // The palette is the field's own menu: it renders inside the shell, so
        // the shell's box *is* its anchor (6px above it, its own width).
        menu={
          paletteSession ? (
            <CommandPalette
              menu={paletteSession}
              query={paletteQuery}
              scope={paletteScope}
              onSecret={(value) =>
                // The shield's characters, one intent each: the session owns
                // them (masked on the wire) and the draft never sees them.
                paletteIntents.current.query(value, "all")
              }
              onKeyDown={onShieldKeyDown}
              onScope={(next) => {
                // Same filter, new chip: the server re-filters and re-selects
                // inside the scope, so the rows below always match the segment.
                setPaletteScope(next);
                paletteIntents.current.query(
                  sigilMode
                    ? paletteFilter(draftRef.current)
                    : draftRef.current,
                  next,
                );
              }}
              onPick={(index) => {
                // A click picks the row it names: move the session's selection
                // there first (the two intents arrive in order), then run it.
                const delta = index - paletteSession.selectedIndex;
                if (delta !== 0) paletteIntents.current.move(delta);
                setDraft("");
                paletteIntents.current.activate();
              }}
            />
          ) : null
        }
        endContent={
          // Stop takes the trailing slot exactly while a run is live; at rest
          // the slot is empty (plan 124 — no Send button).
          streaming ? (
            <span className={styles.composerActions}>
              <ClayIconButton
                icon="generation.stop"
                label="Stop"
                onPress={onStop}
              />
            </span>
          ) : null
        }
      />
      <div className={styles.composerHints}>
        <span className={styles.keyHint}>
          <ClayKbd>/</ClayKbd> commands
        </span>
        <span className={styles.keyHint}>
          <ClayKbd>@</ClayKbd> mention file or skill
        </span>
        <span className={styles.keyHint}>
          <ClayKbd>↵</ClayKbd> send
        </span>
        <span className={styles.keyHint}>
          <ClayKbd>⇧</ClayKbd>
          <ClayKbd>↵</ClayKbd> newline
        </span>
        <span className={styles.spacer} />
        {draft.trim().length > 0 && (
          <span className={styles.hintCount}>{draft.trim().length} chars</span>
        )}
      </div>
    </form>
  );
}
