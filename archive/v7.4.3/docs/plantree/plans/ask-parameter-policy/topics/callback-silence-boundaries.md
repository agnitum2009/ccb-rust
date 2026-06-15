# Callback And Silence Boundaries

Date: 2026-06-07

## Callback Boundary

`--callback` is not a parent-task flag. It is a child-dependency flag used by
the current active agent when the current task cannot finish until the child
result is available.

CCB automatically delivers the continuation only after a callback edge exists.
Each waiting hop in a chain creates its own edge:

```text
A --callback -> B
B --callback -> C
```

The B-to-C callback continues B. A receives a continuation only after B later
finishes its own callback continuation.

## Silence Boundary

`--silence` is silent-on-success delivery. It does not mean the task is
unimportant, and it does not make the target job finish immediately.

Good silent tasks include:

- release or deploy execution steps
- smoke checks, lint checks, cleanup, and sync
- notifications or status broadcasts
- background work where success is routine

Failures, blockers, risks, and required next actions should still surface.

## Boundary Rule

An upstream silent edge does not decide downstream routing:

```text
A --silence -> B
```

B still runs an active job. If B needs C's result to finish, B uses callback. If
B is only dispatching independent work to C, B uses silence.

## Discouraged Combination

Avoid combining `--callback` and `--silence` in normal skill guidance. The
intents conflict: callback says the current task needs the result, while silence
says successful completion should not interrupt the caller. This is a guidance
rule, not a claim that the CLI rejects the combination.
