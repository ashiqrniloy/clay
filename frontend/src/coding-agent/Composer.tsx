// Composer (plan 119 SC-4): the agent view's input lane — the message field,
// its slash-command and @-mention completions, and the manifest-bound effort
// cycle chord. One component owns that whole state machine (draft + completion
// cursor, both local), so the panel stays a composition root and the send
// authority (intercepts, steer-vs-prompt, run lifecycle) stays with it.
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { ClayIconButton, ClayKbd, ClayTextField } from "../components";
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

export interface ComposerProps {
  /** A provider/model is configured: the field is live (else it is disabled). */
  configured: boolean;
  /** A run is in flight: the composer is the steering lane and Stop replaces
   *  Send. */
  streaming: boolean;
  /** The bound agent session (`""` = none); keys the one-shot file fetch. */
  sessionId: string;
  /** The tab's own agent command lane (the @-mention file fetch rides it). */
  command: (command: Record<string, unknown> | string) => void;
  /** Daemon-registered + client built-in slash commands, already merged. */
  slashCommands: ReadonlyArray<{ name: string; description: string }>;
  /** Skill catalog and workspace files for the merged @-mention dropdown. */
  skills: ReadonlyArray<{ name: string }>;
  files: readonly string[];
  /** Declared thinking levels for the active model (`null` = no control). */
  effortLevels: string[] | null;
  /** The level the next prompt would carry (the pending pick, else the
   *  session's active one). */
  shownEffort: string | null;
  onEffortChange: (level: string | null) => void;
  /** Manifest-bound cycle chord; `null` uses the Shift+Tab default. */
  effortChord: EffortChord | null;
  /** Stop the running turn. */
  onStop: () => void;
  /** Close the agent surface (the composer's Close action). */
  onClose: () => void;
  /** Send text (a prompt, a steer, or a client built-in). Returns whether the
   *  text was consumed — the panel answers `false` for input it ignored (empty
   *  or no provider), and the draft survives that answer. */
  onSubmit: (text: string) => boolean;
}

export function Composer({
  configured,
  streaming,
  sessionId,
  command,
  slashCommands,
  skills,
  files,
  effortLevels,
  shownEffort,
  onEffortChange,
  effortChord,
  onStop,
  onClose,
  onSubmit,
}: ComposerProps) {
  const [draft, setDraft] = useState("");
  const [completionIndex, setCompletionIndex] = useState(0);
  const [completionDismissed, setCompletionDismissed] = useState(false);

  // Plan 109 R1: a `/word` draft offers the merged command list. Slash and
  // mention are mutually exclusive by construction (both anchor on the draft).
  const slashMatches = useMemo(() => {
    if (completionDismissed) return null;
    if (!draft.startsWith("/") || draft.includes(" ")) return null;
    return slashCommands
      .filter((command) => command.name.startsWith(draft))
      .slice(0, 8);
  }, [completionDismissed, draft, slashCommands]);

  // Plan 117 @-mentions: a trailing `@token` in the draft opens the merged
  // dropdown — the session's skill catalog plus the workspace file cache
  // (both server-known; filter is client-side). `@skill:x` / `@file:x`
  // narrow to one section; a bare token filters both.
  const mentionToken = useMemo(
    () => (completionDismissed ? null : mentionTokenOf(draft)),
    [completionDismissed, draft],
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
      items: readonly string[],
      kind: "skill" | "file",
      query: string,
    ) => ({
      label,
      items: items
        .filter((item) => item.toLowerCase().includes(query.toLowerCase()))
        .slice(0, 8)
        .map((name) => ({ kind, name })),
    });
    const sections = [
      ...(skillQuery !== null
        ? [
            section(
              "Skills",
              skills.map((skill) => skill.name),
              "skill",
              skillQuery,
            ),
          ]
        : []),
      ...(fileQuery !== null && files.length > 0
        ? [section("Files", files, "file", fileQuery)]
        : []),
    ];
    const flat = sections.flatMap((entry) => entry.items);
    return flat.length > 0 ? { sections, flat } : null;
  }, [files, mentionToken, skills]);

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
      if (!slashMatches || slashMatches.length === 0) {
        // Plan 117 @-mention dropdown shares the completion index + dismiss
        // state with the slash list.
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
        return;
      }
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setCompletionIndex((index) => (index + 1) % slashMatches.length);
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setCompletionIndex(
          (index) => (index - 1 + slashMatches.length) % slashMatches.length,
        );
      } else if (event.key === "Escape") {
        event.preventDefault();
        setCompletionDismissed(true);
      }
    },
    [
      effortChord,
      effortLevels,
      shownEffort,
      onEffortChange,
      slashMatches,
      mentionMatches,
      completionIndex,
      embedMention,
    ],
  );

  // A submit with a completion open takes the highlighted (or exactly typed)
  // command; the panel decides whether the text was consumed, and only then
  // does the draft go away (an ignored submit keeps what was typed).
  const onComposerSubmit = useCallback(
    (value: string) => {
      let text = value;
      if (slashMatches && slashMatches.length > 0) {
        const exact = slashMatches.find((command) => command.name === value);
        const highlighted: string | undefined =
          slashMatches[completionIndex]?.name;
        if (exact || highlighted) text = exact?.name ?? highlighted ?? "";
      }
      if (!onSubmit(text)) return;
      setDraft("");
      setCompletionIndex(0);
      setCompletionDismissed(true);
    },
    [completionIndex, onSubmit, slashMatches],
  );

  const complete = useCallback((commandName: string) => {
    setDraft(commandName);
    setCompletionIndex(0);
  }, []);

  return (
    <form
      className={styles.composer}
      onSubmit={(event) => {
        event.preventDefault();
        onComposerSubmit(draft);
      }}
    >
      {slashMatches && slashMatches.length > 0 && (
        <ul
          className={styles.completions}
          aria-label="Slash commands"
          role="listbox"
        >
          {slashMatches.map((command, index) => (
            <li key={command.name}>
              <button
                type="button"
                role="option"
                aria-selected={index === completionIndex}
                className={styles.completionRow}
                onClick={() => complete(command.name)}
              >
                <span className={styles.completionName}>{command.name}</span>
                <span className={styles.completionDetail}>
                  {command.description}
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
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
          setDraft(value);
          setCompletionIndex(0);
          setCompletionDismissed(false);
        }}
        multiline
        autoGrow
        variant="composer"
        placeholder={
          configured
            ? streaming
              ? "Steer the agent, or wait"
              : "Ask, or type / or @"
            : "Configure a provider first"
        }
        disabled={!configured}
        onKeyDown={onComposerKeyDown}
        onSubmit={onComposerSubmit}
        endContent={
          <span className={styles.composerActions}>
            {streaming ? (
              <ClayIconButton
                icon="generation.stop"
                label="Stop"
                onPress={onStop}
              />
            ) : (
              <ClayIconButton
                icon="message.send"
                label="Send"
                type="submit"
                isDisabled={!configured || !draft.trim()}
              />
            )}
            <ClayIconButton
              icon="action.close"
              label="Close"
              variant="muted"
              onPress={onClose}
            />
          </span>
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
