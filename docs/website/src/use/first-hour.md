# Your first hour

Assumes `omni init` and `omni doctor` are done. Nothing here changes configuration.

What the hour buys is the ability to check OMNI instead of trusting it. By the end you
will be able to see any cut side by side with the original, pull back anything it
removed, and tell one of its markers apart from a line that merely looks like one. That
last skill is the one that makes the other two worth having.

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
closes it, because that is what the context window actually costs you. The distillation
percentage below it is a diagnostic for one host's pipeline.

Every absolute figure it prints is in bytes, which are counted. It used to report tokens,
and those were the same byte counts divided by a constant calibrated against another
vendor's tokenizer, so the unit could not be defended even though the arithmetic was
fine. Percentages were never affected: the divisor cancels in a ratio.

```sh
omni stats --view detail   # per command, per route, per session, per agent
omni stats --rerun         # which distillers cost a re-run
omni dashboard             # the same numbers in a browser, on 127.0.0.1 only
```

`--rerun` is the interesting one. Reduction percentage cannot tell you whether a
distiller removed something the agent then had to go and fetch again; this can.

## Pull back something it removed

Every marker names a handle. Run it:

```sh
omni retrieve 0000000000000000
```

That exact handle is the documentation example and is refused by name, which is the
point of this section. Copy a real one out of a marker in your own output and you get
the bytes back verbatim, and the exit code tells you which happened: 0 when the handle
resolved, 1 when it did not.

That pair is the fastest trust check there is. A tool that removes things and cannot
give them back is a tool you have to take on faith.

## Tell a real marker from one that is just text

This page is full of markers, so is OMNI's own source, and so is any bug report that
quotes one. Searching your transcript for the marker shape will find all of them.

The handle is what separates them. Worked examples everywhere in this manual use the
reserved `0000000000000000`, which no real fold can ever be assigned, so:

```sh
omni retrieve <handle-from-your-output>   # exit 0, and the content
omni retrieve 0000000000000000            # exit 1, "the documentation example"
```

If you are measuring whether OMNI did anything at all on a run, that exit code is the
answer and grepping for `[OMNI` is not.

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
