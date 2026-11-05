# Synology MailPlus fixtures

The personal Sieve scripts Synology MailPlus Server keeps for every
account, captured on 2026-11-03 from MailPlus Server with Dovecot 2.3.21.1
and Pigeonhole 0.5.21.1, for an account without filters or personal spam
lists in the MailPlus web UI:

- `synomc.sieve`: MailPlus' root script and the account's active script.
  Includes `spam_rule` and `filter` and files the message into the folder
  they recorded in global variables.
- `filter.sieve`: filters from the MailPlus web UI (none here).
- `spam_rule.sieve`: personal block and allow lists (none here).
- `filter-one-rule.sieve`: `filter` after adding one filter in the web UI
  (subject contains `sieve-filter-test` → Junk), captured 2026-11-05. Each
  filter is an `if` that only records its folder in `filter_mailbox`,
  under a `# id: <n>` comment.

Saving filters in the web UI rewrites `synomc` and `filter` completely and
makes `synomc` the active script again (tested 2026-11-05: a comment added
to `synomc` was gone afterwards).
`synomc.sieve` is the version MailPlus wrote then (tab indentation; the
first capture had spaces instead).

They stand for any setup sieve-gui finds on a server and didn't create:
sieve-gui must include such a root unchanged and never rewrite it.

Not visible through ManageSieve, so not included: MailPlus' server-wide
scripts (`sieve_global`: `spam_rewrite_subject`; `sieve_after`:
`move_spam`, which files `X-Synology-Spam-Flag: YES` into `Junk`).
