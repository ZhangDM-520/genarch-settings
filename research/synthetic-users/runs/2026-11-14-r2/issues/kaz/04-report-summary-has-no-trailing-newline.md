# the report's `SUMMARY` line ends without a newline, so the next output joins it

- **Severity**: low
- **Category**: bug
- **Environment**: Arch Linux 7.2.11-arch1-1, `genarch-settings 0.4.0-1`, any invocation

## Intent

Read the last line of `report` in a terminal, or `read` it in a shell script, without it running
into whatever comes next.

## Steps

```
genarch-settings report | tail -c 40 | od -c
genarch-settings apply | tail -c 40 | od -c
```

## Expected

A line-oriented table ends its last line with a newline, like `apply`'s output does.

## Actual

```
$ genarch-settings report | tail -c 40 | od -c
0000020   d   r   i   f   t   =   5       s   k   i   p   p   e   d   =
0000040   3       e   x   i   t   =   1
0000050
$ genarch-settings apply | tail -c 40 | od -c
0000020   p   l   i   e   d   =   0       f   a   i   l   e   d   =   0
0000040       e   x   i   t   =   0  \n
0000050
```

In a terminal it is visible directly: `SUMMARY … exit=1` and the shell prompt land on one line
(`SUMMARY ok=3 accepted=0 warn=0 drift=5 skipped=3 exit=1EXIT:1` in my first run). A `while read`
loop over the table loses the last line, and an appended log entry gets glued to it.

## Evidence

Verbatim above; three `od -c` dumps, `report` without the `\n` and `apply` with it.

## What could not be determined

Nothing — this one is reproducible on every run.
