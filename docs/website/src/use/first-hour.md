# Your first hour

Assumes `omni init` and `omni doctor` are done. Nothing here changes configuration;
it is all about learning to read what OMNI is doing so you can judge it.

## See a distillation happen

Ask your agent to run something noisy. A test suite or a build is ideal.

Then, in your own shell:

```sh
omni diff
```

Raw on one side, distilled on the other, for the last command. This is the fastest
way to develop either trust or suspicion, and both are useful.

## Try one by hand

```sh
omni exec cargo test
```

`omni exec` runs a command through the whole pipeline and prints the result with a
footer. It is the harness every bug report in this project is asked to use, because it
takes the host out of the picture.

The argument form is exact: `omni exec cargo test`, **not** `omni exec -- cargo test`
and not a quoted string. Both of those fail with "No such file or directory".

## Look at the numbers

```sh
omni stats
```

It leads with session lifetime, how many commands a session carries before the host
closes it, because that is what the context window actually costs you. The
distillation percentage below it is a diagnostic for one host's pipeline.

```sh
omni stats --detail        # per command, per route, per session, per agent
omni stats --rerun         # which distillers cost a re-run
omni dashboard             # the same numbers in a browser, on 127.0.0.1 only
```

`--rerun` is the interesting one. Reduction percentage cannot tell you whether a
distiller removed something the agent then had to go and fetch again; this can.

## Pin what you are working on

```sh
omni goal set 'Migrate the billing service off the legacy queue'
```

The scorer favours output related to that goal, and the agent is reminded of it rather
than drifting. `omni goal show` to check, `omni goal clear` to drop it.

## Turn it off for one command

```sh
OMNI_PASSTHROUGH=1 kubectl get pods -o yaml
```

The first thing to reach for when you suspect OMNI changed something it should not
have. If the output is identical with and without it, OMNI was not involved.

## Things worth knowing before they bite

**Reading a file through your shell may arrive distilled.** Since the hook really does
rewrite Bash output, a `cat` or `sed` of a source file can come back folded. Use your
agent's file-reading tool, or `OMNI_PASSTHROUGH=1`, when you need exact bytes.

**A matched command may be rewritten before it runs.** The pre-hook turns some
commands into `omni exec`, redirection included, so the log file you later read is the
distilled one. Break the prefix (`env cargo test`, or `true && cargo test`) when you
need the raw log on disk.

**Do not judge OMNI by output you read through OMNI.** A `cargo test` read through the
hook once reported "1 failed" for a 398-pass green suite. Redirect to a file with
passthrough on before making any claim about a result.

## When to ask for help

If output ever looks shorter than it should, if a row is missing, or if OMNI reports a
success for something that failed, that is worth reporting. Reproduce it with
`omni exec` first, and read the **whole** distilled output rather than a `grep` of it:
grepping hides the headers that often make the output lossless after all.
