**The README published a corpus the manual replaced two releases ago.** `README.md`
and its six translations carried 6,656 traces replayed on 0.7.3 while
`docs/website/src/develop/benchmarks.md` carried 5,984 replayed on 0.7.5, so the
repository disagreed with itself on every figure the two shared: 2.7% against 32.6%
from the filters, 14.9% against 69.6% with the ledger, and 35.9% against 98.9% on the
`docker build` fixture.

Two of those were reversed claims rather than stale numbers. The head-to-head conceded
the filter row to rtk, 6.2% against our 2.7%, and on this corpus ours read 32.6% while
the row we actually lose is lean-ctx at 49.4%. And "file re-reads: 0.0% from the
filters" was the argument for the ledger; that class reads 39.2% from the filters now,
so the argument is made from the gap that remains.

Every README also carries the manual's own caveat next to the headline, because a
number travels and the sentence qualifying it does not: 286 groups of byte-identical
payloads are 80.6% of these bytes, and the same harness over a week of ordinary work
reads 14.9%. The six translations additionally lose a marker-accounting paragraph the
English README dropped in #325 and they kept, which cited a `git diff` figure the table
had already stopped printing.
