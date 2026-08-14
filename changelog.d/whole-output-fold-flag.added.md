- **A fold now records whether it covered the whole output**: `MIN_WHOLE_OUTPUT_FOLD` refuses a whole-output fold under 1 KB because the agent is left holding a handle and no content, and #543 calibrated that floor on four such folds. Nothing recorded which folds were whole-output, so the floor could not be checked against its own corpus after the fact: `distillations` carries `collapse_original = 0` on exactly the four rows the decision cites, and `ledger_folds` had no flag at all. Verifying the claim on a live store meant guessing from the delivered-bytes ratio, which misreads an aggressive `Keep` as a whole-output fold.

  `ledger_folds.whole_output` is set when every line of the payload folded, which is the call-level reading of the same condition the floor tests per run. The two differ only when adjacent runs of different origins tile the payload, and the question the column answers is whether the agent kept any content at all, so the call-level reading is the right one for a table that already aggregates by (origin, source agent) per call.

  ```sql
  SELECT * FROM ledger_folds WHERE whole_output = 1 AND payload_bytes < 1024;
  ```

  Zero rows is the floor holding. `payload_bytes` rides along on every row of the call for that query to be a row predicate: summing `bytes` would need a GROUP BY on a per-call key, and `ts` is whole seconds, so two folds by one agent inside one second merge and their combined size can clear a floor that neither cleared alone. That is the audit silently hiding the one thing it exists to find, so the size is recorded per row instead.

  Rows written before the columns carry 0 meaning "not recorded" rather than "was partial", so a query has to bound itself by `ts`. The migration was run against a copy of a real 89 MB store and left its 55 existing rows intact.
