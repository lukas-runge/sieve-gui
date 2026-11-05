# sieve-gui — Concept

> Status: draft · last updated 2026-11-04

## 1. Vision

Server-side mail filtering with [Sieve](https://datatracker.ietf.org/doc/html/rfc5228) is powerful but practically unusable for most people: webmail filter UIs are limited, and the existing desktop clients are either outdated or don't run on current platforms.

**sieve-gui** is a modern, open-source desktop app in which you **build filter rules visually** — readable rule cards instead of code — and deploy them to your mail server via **ManageSieve** ([RFC 5804](https://datatracker.ietf.org/doc/html/rfc5804)). Before deploying, you can **dry-run rules against your real mail** to see what would have happened, and optionally **apply them retroactively** to existing folders.

Sieve code is an implementation detail. A code editor exists only as a *fallback* for things the visual editor cannot express (yet).

## 2. Goals and non-goals

### Goals

- Intuitive visual rule building for non-technical users
- Lossless round-trip: scripts written elsewhere are never silently damaged
- Validate rules before deploying them (dry run with explanations)
- Work with common ManageSieve servers out of the box
- Cross-platform (macOS, Windows, Linux), open source (MIT), for everyone

### Non-goals (for now)

- Being a mail client (no reading, composing, or sending mail)
- Executing filters continuously on the client — filtering happens on the server
- Server-specific, non-Sieve filter systems (e.g. mailcow pre-/postfilters, which are not reachable via ManageSieve)
- Mobile platforms

## 3. Core concepts

### Rule cards

A script is shown as an ordered list of **rule cards** that read like a sentence:

> **If** *sender* **contains** `@tax-office.example` **then** *move to* `Taxes` **and** *stop*

Each card has a name, can be enabled/disabled, reordered by drag and drop, and duplicated. Conditions can be grouped to any depth ("all of", "any of", "none of", "not all of"), written as nested `allof`/`anyof`.

Besides plain rules, the list has:

- **Branches** (`if`/`elsif`/`else`): a card with several sections, each with its own conditions and actions ("If … then …", "Otherwise if … then …", "Otherwise …"). A note explains that only the first matching branch applies.
- **Nested conditions:** an action list can contain "also check: if … then …" (a nested `if`, itself with branches), shown indented, to any depth (deeper levels get more compact indentation).
- **Search:** filters the list by any text in a card (names, values, folders).

Other constructs, such as `include` inside a script or free comments, stay code cards. The order of several scripts is the sidebar order the root script runs them in (below), so scripts don't need to include each other.

### Rule model as the single source of truth

```
Sieve text  ⇄  parser / generator  ⇄  rule model  ⇄  UI
                                          │
                                 raw-code blocks for anything unknown
```

- The app edits a **typed rule model**, never Sieve text directly.
- The model lives in Rust; types are exported to the frontend via `tauri-ts-generator` (`#tauri/*` imports).
- **Raw-code blocks:** any top-level construct the model cannot represent becomes a *code card* at its original position. All other rules stay editable visually; nothing is lost.

### Metadata in comments

Rule names and enabled state are stored as structured comments in the **format of the Roundcube *managesieve* plugin** (e.g. `# rule:[Name]`), so scripts can be edited alternately in Roundcube and sieve-gui without losing names. Disabled rules are kept in the script in a non-executing form instead of being removed. Before implementing the parser, the exact encoding (names, disabled rules) is captured from real Roundcube-generated scripts and turned into test fixtures.

**Keyword names:** custom flags such as Synology MailPlus labels (`$synolabel_tu0exccz`) or Roundcube's `$label1` carry no name. Each account keeps a name per keyword, shown instead of the keyword. Names are entered by the user: no server exposes them. IMAP keywords carry no display name, neither IANA's IMAP METADATA entries nor its keyword registry define one (`$MailFlagBit0`–`2`, RFC 9979, only encode a flag color), and MailPlus keeps its label names to itself (its Dovecot runs without `imap_metadata`). Storing names on the server under a METADATA vendor entry would need a registered vendor token (RFC 5464 §3.2.1) and is off by default in Dovecot, so it isn't planned.

### Several active scripts, one folder per message

Sieve runs every matching rule, so a message matching two "move to folder" rules ends up in both folders. ManageSieve also runs only one active script per account. Two account settings (account dialog, both on by default, each only if the server supports it) change that:

1. **Last move wins** (`fileinto`, `include`, `variables`): each message lands in at most one folder; of several "move to folder" rules, the last one that runs wins, across scripts too.
2. **Several active scripts** (`include`): active scripts run one after another, in the order of the sidebar list (drag and drop).

Both work through a generated **root script** named `sieve-gui` (`sieve_core::root`), which is the server's active script whenever one of the settings is on:

- The root includes the active scripts in order (`include :personal :optional`, RFC 6609). With "several active scripts" off, it includes just one, and activating another script replaces it.
- With "last move wins", the root declares the global variable `move_to_folder` (RFC 5229) and finally files the message into the folder recorded there, if any. The included scripts use **single-folder mode** (`RuleSet::single_folder`, written as `global "move_to_folder";`): "move to folder" becomes `set "move_to_folder" "<folder>"`, and "stop" becomes `return`, which ends only that script (`stop` would also skip the root's final `fileinto`). "Copy to folder" (and moves with `:create`/`:flags`) stay `fileinto` and always apply. A plain `fileinto` or `stop` written by another editor is read as move/stop, so saving converts it.
- The app brings the server in line with the settings whenever it connects and after the account dialog is saved: it creates, rewrites or deletes the root and switches the included scripts in or out of single-folder mode. In the order used, every step leaves a working setup if a later one fails. Turning both settings off keeps the first script of the order active and deletes the root.
- The root is not listed in the sidebar. Run order lives in the root; where inactive scripts sit in the list is stored locally.
- Roundcube drops `global` and `return` whenever it saves a script, the root included (fixtures `roundcube/23b`, `24b`). After that, no message is moved. So "last move wins" isn't available while the account setting "I also edit filters in Roundcube" is on; turning that setting on switches the included scripts out of single-folder mode.
- **Repair on connect:** a root that lost its `global` is still recognized (`Root::damaged`) and rewritten. Included scripts are read in single-folder mode even without `global` (`sieve_core::import_single_folder`), so their moves are moves again; the lost stops can only come back from the local copy, whose check runs first.

### Local copy

Some servers lose filter scripts on their own, e.g. Synology MailPlus when it updates. The app therefore keeps a **local copy** of every account's scripts. The server stays the place where filtering happens; the copy is for restoring and for the user's own backups.

- One folder per account: the app data directory by default, or a folder the user picks (e.g. a synced folder or a git checkout). It holds one `<name>.sieve` file per script on the server, named exactly as on the server so `include :personal` resolves the same way locally, and a manifest `sieve-gui.json` with the active script and, for each script, the content hash at the last sync.
- Updated after every successful change on the server (`PUTSCRIPT`, `RENAMESCRIPT`, `DELETESCRIPT`, `SETACTIVE`), including the root script.
- **On connect**, before the root is reconciled (which would otherwise mask changes), the app compares the server with the copy: scripts missing on either side, scripts whose content differs, a different active script. The hashes in the manifest tell which side changed since the last sync. If anything differs, a dialog lists the differences and offers, for each one or for all: **upload the local version** (through `CHECKSCRIPT`, then `PUTSCRIPT`/`SETACTIVE`) or **take over the server version** locally. Nothing is changed silently.
- The first connect of an account (no copy yet) takes over the server state without asking.
- The copy does not cover the server's own configuration (e.g. whether ManageSieve is enabled at all).

### Capability-aware UI

The server's `CAPABILITY` response (`SIEVE` extensions, limits) drives the UI: only conditions and actions the server supports are offered (e.g. `regex`, `editheader`, `vacation-seconds`).

### Known foreign scripts

Scripts generated by other tools (SOGo, Roundcube) are detected and imported into rule cards where possible. The user is warned when the active script is owned by a webmail UI that will overwrite it on its next save.

Roundcube writes no marker of its own, and sieve-gui deliberately uses the same format (D4, D8), so ownership can't be told from the script. Instead, the account setting "I also edit filters in Roundcube" (off by default) turns on warnings about anything Roundcube would drop: raw blocks it can't represent (unknown tests or actions, `elsif`/`else`, nested `if`, free comments), rules with nested condition groups (its form has one level of "all of"/"any of") or any `/* … */` comment. It also turns off "last move wins", whose `global` and `return` Roundcube drops (above). Affected cards are marked. Raw blocks for features Roundcube does know (e.g. `notify`, `addheader`) don't trigger it. Webmails that keep their own script (e.g. SOGo's `sogo`) can make it the active one when the user saves filters there; with a root script, the app then shows a notice offering to activate `sieve-gui` again instead of taking over on its own. Systems with a filter provider (below) can be taken over instead.

**Existing setups.** A server may already run a Sieve setup sieve-gui didn't create: a webmail's own root script (Synology MailPlus' `synomc` includes `spam_rule` and `filter` and files into the folder they recorded in global variables), SOGo's or Roundcube's script, or a hand-written one. The app doesn't take over such a setup on its own and recognizes none of them by name:

- When the root script would become active and another script is active, the app asks first: *include unchanged* (default) runs that script first inside the root as it is; *take over* lets sieve-gui adjust it like its own scripts. Activating another script meanwhile includes it unchanged.
- The root script records scripts included unchanged with a `# unchanged` comment on their `include` (stored on the server, so every installation of the app sees it). sieve-gui never rewrites them, not even into single-folder mode. A script can be taken over later, or left unchanged again, from its menu; both ask first.
- From each script, the app reads which personal scripts it includes and whether it files messages itself (`sieve_core::root::script_links`). Scripts included by another one are marked ("included by …") and deleting them asks first. A script included unchanged that files messages itself is marked too: "last move wins" doesn't cover its moves, so a message can end up in two folders.
- Server-side scripts (`sieve_before`, `sieve_after`, `sieve_global`; e.g. MailPlus' spam handling) are invisible through ManageSieve. The app can't show them, and a dry run can't simulate them.

### Filter providers

Some systems write Sieve themselves from their own filter settings: a webmail, a NAS mail server, a hosting panel. sieve-gui can **take over** their filters: it reads them in the provider's format and turns them into ordinary rules in a script of its own. After that, filters are edited only in sieve-gui. **Running both side by side is not supported.**

**Why not both.** Such systems treat their scripts as output. They keep the filters in their own storage, rewrite their scripts from it on every save and may make their own script the active one again. For Synology MailPlus this was tested on 2026-11-05: saving a filter in the web UI rewrote `synomc` and `filter` completely (a comment added to `synomc` was gone) and made `synomc` active again. From then on only the provider's filters run, until sieve-gui is activated again. Their own root also files messages itself, past "last move wins", so a message matched there and by a sieve-gui rule lands in two folders. Edits made in sieve-gui to the provider's scripts are lost on its next save. None of this can be fixed through ManageSieve. A provider's UI that is still used next to sieve-gui switches sieve-gui off without notice. So the app asks for a decision instead of suggesting that both work.

**Provider model.** Each provider is a module in `sieve-core` (`sieve_core::providers`) behind one interface, so further providers need no changes elsewhere. A provider:

- **detects** its setup from the account's scripts by their content (structure and variables, not just script names) and reports what it found: its root, the scripts with filters, the number of filters and list entries;
- **imports** its filters into the rule model: each filter becomes a rule with the provider's name for it, if it has one. Anything it can't map stays a code card, never dropped. The import is pure (scripts in, rules plus notes out) and tested against fixtures captured from the real system (`crates/sieve-core/tests/fixtures/<provider>`);
- **describes its behavior** for the texts the app shows: whether saving in its UI rewrites its scripts and activates its root (`Detection::reactivates_root`);
- **fingerprints its filters** (`Detection::fingerprint`, over their canonical form), so the app can tell filters added after a take-over from those taken over.

The app passes the names for the rules it creates ("Filter", "Allowed", "Blocked") in its own language (`RuleNames`).

Recognizing a provider is an explicit, extensible list, not an exception to the general rules: everything outside a provider module stays product-agnostic (D16), and without a matching provider a foreign setup is handled as under *Existing setups*.

**Take-over flow.**

1. When a provider is detected on an account, the sidebar offers: "Filters from <provider> found (n filters, m list entries). Take them over into sieve-gui?"
2. A dialog explains why both side by side doesn't work and previews the imported rules (name, conditions, target folder), for a new script (name suggested from the provider, editable) placed first in the order, so the rules run where the provider's filters ran.
3. On confirmation the app creates the script and changes the root: it no longer includes the provider's root. The provider's own scripts stay on the server untouched, as a fallback if sieve-gui is given up; they're just no longer run. The new script opens.
4. A notice in the sidebar tells the user what only they can do: delete the filters in the provider's UI (they live in its storage, not in the scripts, and would come back on its next save) and not create filters there anymore. Deleting them is itself a save there, so the provider takes the active script back; for 15 minutes the app checks every 5 seconds, takes over again right away and reports when the filters are gone.
5. The take-over turns on the account setting **"sieve-gui manages the filters alone"** (stored with the provider, its root and scripts, and the fingerprint at the take-over; it can be turned off in the account dialog). With it, while the app is open it checks the account every 30 seconds and re-activates its root as soon as the provider has taken the active script back (with a notice). On connect:
   - if the provider has made its own script active again, the app re-activates its root before anything else and reports it ("<provider> had made its own script active again …");
   - differences in the local copy that only concern the provider's scripts (which it rewrote) are resolved by keeping the server's version, without asking;
   - if the provider has filters again whose fingerprint differs from the one at the take-over, it offers to take them over too.

   What it can't do is close the gap while it isn't running: between a save in the provider's UI and the next check (or, with the app closed, its next start), sieve-gui's rules don't run. The texts say so.

**First provider: Synology MailPlus.**

- Detection: a root (MailPlus: `synomc`) that includes personal scripts which record folders in global variables, and files into `${filter_mailbox}`, `${spam_rule_blacklist_mailbox}` and `${spam_rule_whitelist_mailbox}`.
- Filters (`filter`): each is an `if` under a `# id: <n>` comment that sets `filter_mailbox`; it becomes a rule that moves to that folder (fixture `mailplus/filter-one-rule.sieve`). The `# id` comment is dropped.
- Personal block and allow lists (`spam_rule`): one guarded `if` per entry under `# id: <n>`, comparing `from` with the address (written twice), followed by fixed code for the subject and a header (fixture `mailplus/spam_rule-two-entries.sieve`). Block list entries become a rule that moves to `Junk`, allow list entries one that moves to `INBOX`. They are taken over together with the filters.
- Order within the new script, where the last move wins: allow list entries, then filters, then block list entries. That is MailPlus' own priority (block list over filters over allow list).
- Nothing needs to be kept: `spam_rewrite_subject` (`sieve_global`) only provides the subject marker so that `spam_rule` can remove it for allowed senders. The marker itself is set by MailPlus' spam filter outside Sieve, and its server-side script (`sieve_after`: `X-Synology-Spam-Flag: YES` → `Junk`) runs regardless of the active script.
- Lost and not replaced: removing the spam marker from the subject for allowed senders, and the header `X-Synology-Personal-Spam-Rule` that MailPlus adds on list matches.
- Value of the MailPlus UI next to sieve-gui: none that outweighs the cost. Its filters and lists are a subset of sieve-gui's rules; what remains is convenience (blocking a sender while reading mail, editing filters from any browser), and using it switches sieve-gui off. The allow list doesn't override the server-side spam filter either: `move_spam` only looks at `X-Synology-Spam-Flag`.

## 4. Features and milestones

### v0.1 — visual editing (MVP)

- Accounts: host, port, STARTTLS/implicit TLS, username; password stored in the OS keychain
- Trust a specific server certificate (pinned SHA-256 fingerprint per account and service) when the OS doesn't trust it, e.g. a NAS' self-signed certificate
- Script management: list, create, rename, delete, activate (one active script)
- Rule list: drag-and-drop ordering, enable/disable, rename, duplicate
- Rule editor
  - Conditions: from, to/cc, subject, any header, size; match types *contains / is / starts with / matches* (+ *regex* if supported); combine with *all* (`allof`) / *any* (`anyof`), negation
  - Actions: move/copy to folder, set flags (read, flagged, custom), forward/redirect, reject, discard, keep, stop
- Folder picker from the real IMAP folder list (`LIST`) instead of typing names
- Vacation / out-of-office editor (dates, subject, body, interval)
- Raw-code cards with the fallback editor
- Branches (`elsif`/`else`), nested conditions, search in the rule list (§3)
- Names for custom keywords (e.g. MailPlus labels), per account (§3)
- Existing setups: ask before the root script takes over, include scripts unchanged, show which scripts include which (§3)
- Live code preview (optional side panel)
- Deploy flow: diff against server version → `CHECKSCRIPT` → `PUTSCRIPT`
- Conflict detection: warn if the script changed on the server since loading (content hash; ManageSieve has no ETags)
- Undo/redo, keyboard shortcuts, light/dark theme
- UI in English and German
- Local copy of all scripts per account; on connect, differences to the server can be uploaded from the copy or taken over from the server (§3)
- Filter providers: take over filters from systems that write their own Sieve, first Synology MailPlus; account setting "sieve-gui manages the filters alone" (§3, D19)

### v0.2 — dry run

- Select a folder (and optionally a date range) and either one rule or the whole script
- Evaluate locally against real messages fetched via IMAP, read-only
- Simulation context panel with prefilled defaults (delivery date, envelope, state of stateful extensions; see §7)
- Result table: message → matching rule → resulting actions
- Summary, e.g. "37 messages would be moved to *Taxes*", "rule X never matches"
- **Explain view:** for each message, which condition matched which header/value
- **Tests:** sample messages (`.eml`, or picked from a folder via IMAP) with expectations: the folder a message must or must not land in, flags it must or must not get. Set up and run in the app, stored per account alongside the local copy (§3), so they can be versioned with it. Run automatically before saving: a change that breaks a test is shown in the deploy dialog before it reaches the server, so the tests guard deployment instead of reporting afterwards. Existing test suites (e.g. `.eml` files with expected `sieve-test` output) can be imported

### v0.3 — apply retroactively

- Always preceded by a dry run and explicit confirmation
- Executes only mailbox-local actions via IMAP: `MOVE`/`COPY`, `STORE` flags; deletion only after extra confirmation
- Undo via recorded UID mappings (`COPYUID` from UIDPLUS)
- Never executed: actions that send mail (redirect, reject, vacation, notify) — reported as "would forward to …"

### Later

- **Readable complex rules:** the sentence layout reads well for one condition and one action, but not for real scripts. Every value is highlighted the same way, so a card with branches is a wall of highlights. Long `elsif` chains and nested checks read as one paragraph, and long header lists ("to or cc or bcc or Received") push the values out of sight. Rework the card layout so that a script stays easy to scan:
  - a clear structure for branches (one row per branch, its condition and its outcome aligned) instead of running text
  - fewer, calmer highlights (e.g. only the values; folders and flags with an icon of their own)
  - short names for common header lists ("recipients" for to/cc/bcc), with the full list on demand
  - compact rows for simple rules ("`fktg.org` → Accounts › Bildung › FKTG"), collapsible branches for long ones
  - check the result against real scripts with many branches and nested checks
- Test a rule against a pasted/selected sample message
- "Create rule from this message"
- **Variables (RFC 5229) in rules:** conditions that compare one part of a message with another, e.g. "Reply-To is a different address than From" (typical of advance-fee scams) or "Delivered-To is not in To/Cc". Sieve can't compare two headers directly. A test captures a value with `:matches`/`:regex` into `${0}`…`${9}` (or `set`), and a nested test checks against `"${…}"`. The rule model has no variables yet, so such rules only show as code cards. Needed:
  - a condition "same as / differs from <other field and part>" that is written as capture plus nested test and read back from that shape
  - capability check: only offered if the server lists `variables`
  - showing variables a rule sets with `set` (other than the generated `move_to_folder`) instead of turning the rule into code
- Rule templates (newsletters, invoices, mailing lists)
- Own Lezer grammar for the code editor (folding, context-aware completion)
- Headings to group long rule lists, with a marker of their own (not derived from existing comments)
- Optional git history for the local copy: each change on the server becomes a commit (e.g. via `gix`, no installed git needed); pushing to a remote later still

## 5. UX principles

- **Readable over complete:** rules read like sentences; jargon (`:contains`, `allof`) is never shown in the visual editor.
- **Safe by default:** nothing is deployed or applied without preview and confirmation; destructive actions are explicit.
- **Never lose data:** unknown constructs become raw-code cards; round-trip is tested.
- **Explain, don't just execute:** dry runs show *why* a rule matched.
- **Accessible:** full keyboard operation (including reordering without drag and drop), screen-reader labels, sufficient contrast.

## 6. Architecture

### Repository layout

```
sieve-gui/
├── crates/
│   ├── sieve-core/      # lexer, parser, AST, rule model, generator, evaluator   (MIT)
│   ├── managesieve/     # async ManageSieve client                               (MIT)
│   └── mailstore/       # read-only IMAP access: folder list, later messages     (MIT)
├── src-tauri/           # Tauri app: commands, keychain, state
└── src/                 # SvelteKit frontend (SPA), Svelte 5, Tailwind + daisyUI
```

`sieve-core` and `managesieve` are self-contained and publishable on crates.io. To our knowledge there is no permissively licensed, complete Sieve crate for Rust yet.

### sieve-core

- **Lexer/parser** per RFC 5228, producing an AST that keeps source positions and comments (needed for metadata and for faithful raw-code blocks)
- **Rule model** mapping: AST ⇄ rule cards; everything else ⇄ raw-code blocks
- **Generator:** canonical, readable formatting; deterministic output
- **Evaluator** on the rule model with a **trace** of every test result (powers the explain view)
- **Extensions** (initial): `fileinto`, `envelope`, `reject`, `imap4flags`, `copy`, `variables`, `relational`, `subaddress`, `body`, `vacation`, `regex`, `editheader`, `mailbox`
- Validation against server capabilities (`require` handling)

### managesieve

- Async client on `tokio` with `rustls`; STARTTLS and implicit TLS
- SASL `PLAIN` and `LOGIN`; other mechanisms (e.g. `OAUTHBEARER`) only on demand later
- Commands: `CAPABILITY`, `AUTHENTICATE`, `LISTSCRIPTS`, `GETSCRIPT`, `PUTSCRIPT`, `CHECKSCRIPT`, `SETACTIVE`, `DELETESCRIPT`, `RENAMESCRIPT`, `HAVESPACE`

### App (src-tauri)

- Tauri commands wrapping both crates; typed bindings via `tauri-ts-generator`
- IMAP via `mailstore` on top of `async-imap` (MIT/Apache-2.0) for folder listing, dry runs and applying. IMAP host, port and security are set per account; user name and password are shared with ManageSieve
- Credentials via the OS keychain (`keyring-core` with the native store per platform: Keychain on macOS, Credential Manager on Windows, Secret Service through the system's libdbus on Linux); account list without secrets in app config
- Long-running work (dry runs) streams progress to the UI via Tauri channels

### Frontend

- SvelteKit in SPA mode, Svelte 5, Tailwind CSS + daisyUI
- Rule cards with accessible drag and drop
- **Code editor: CodeMirror 6**
  - Modular packages only (`@codemirror/state`, `view`, `language`, `commands`, `lint`, `autocomplete`)
  - Sieve highlighting via the `sieve` mode from `@codemirror/legacy-modes` (`StreamLanguage`) initially, own Lezer grammar later
  - Diagnostics via `@codemirror/lint` from `sieve-core` (live) and `CHECKSCRIPT` (server)
  - Completion based on server capabilities
  - Thin own Svelte component instead of a third-party wrapper; theming via daisyUI CSS variables
  - Chosen over Monaco: far smaller, no web-worker setup, modular, better suited as an embedded fallback editor

## 7. Dry run: simulation context and limitations

The goal is a **simulation as accurate as possible**. Everything a real delivery would know but a stored message doesn't is modelled as an explicit **simulation context**. It is prefilled with sensible defaults, shown to the user, and can be adjusted before a run.

### Simulation context

| Input | Needed for | Default |
|---|---|---|
| Date/time of delivery | `currentdate`, `date` comparisons | the message's received date (`INTERNALDATE`); alternatively a fixed date or "now" |
| Envelope sender | `envelope :from` | `Return-Path` header |
| Envelope recipient | `envelope :to` | `Delivered-To` / `X-Original-To`, else the account address |
| Duplicate tracking state | `duplicate` | empty at start, filled while processing messages in delivery order; optionally seeded |
| Vacation response history | `vacation` intervals | no previous responses; optional list of "already answered" senders |
| `environment` items | `environment` (`domain`, `host`, `location`, `phase`, …) | derived from the account/server (e.g. `location` = `MDA`, `phase` = `during`) |
| Mailbox existence | `mailboxexists`, `fileinto :create` | live IMAP folder list |
| Spam/virus scores | `spamtest`, `virustest` | parsed from common headers (e.g. `X-Spam-Score`); mapping configurable |
| Server capabilities | `require`, available tests | the connected server's `CAPABILITY` |

Every result records **which context values it depended on**, so the explain view can say e.g. "matched because the delivery date was assumed to be the received date (Saturday)".

### Data fetching

- Fetch headers only (`BODY.PEEK[HEADER]`, `RFC822.SIZE`, `INTERNALDATE`) unless a rule tests the body; never mark messages as seen
- Process messages in delivery order so stateful extensions behave as they would have

### Limitations

- Context values derived from headers are approximations; results depending on them are marked as such
- **Raw-code blocks** are not simulable and reported as such
- Results are a simulation; differential tests against Dovecot (see §9) keep divergence rare

## 8. Server compatibility

- Primary target: **Dovecot Pigeonhole**
- Also tested: **Cyrus**, **Stalwart**
- Import of scripts generated by **SOGo** and **Roundcube**

## 9. Testing

- **Unit tests** for lexer, parser, generator, evaluator
- **Round-trip tests:** text → model → text is stable; unknown constructs survive unchanged
- **Differential tests against Dovecot:** run the same script and message through `sieve-core` and Pigeonhole's `sieve-test` (in Docker) and compare the resulting actions. `sieve-test` runs as a separate process; nothing is linked or redistributed.
- **Pigeonhole test suite** (`.svtest` files) downloaded at test time as additional test input, not vendored
- **Integration tests** of `managesieve` against a Dovecot container in CI
- **MailPlus fixtures** (`crates/sieve-core/tests/fixtures/mailplus`): MailPlus' own scripts (`synomc`, `filter`, `spam_rule`) as generated by MailPlus, like the Roundcube fixtures: an example of an existing setup the root includes unchanged. Testing against a server set up like MailPlus is done locally, outside the repository
- Frontend: type checks (`svelte-check`), component tests for the rule editor

## 10. Licensing and provenance

The project is **MIT-licensed**. To keep it that way:

- **Implement from the specifications** (RFC 5228 and extension RFCs, RFC 5804).
- **Permissive references may be used with attribution:** [foxcpp/go-sieve](https://github.com/foxcpp/go-sieve) (MIT, Go interpreter — structure may be ported), [tonioo/sievelib](https://github.com/tonioo/sievelib) (MIT, Python parser/generator/ManageSieve client). Ported code keeps its copyright notice.
- **Do not read, port, or closely imitate AGPL code**, in particular [stalwartlabs/sieve](https://github.com/stalwartlabs/sieve) (`sieve-rs`, AGPL-3.0-only) and [thsmi/sieve](https://github.com/thsmi/sieve) (AGPL-3.0). Translating such code into Rust would still make it a derivative work.
- **Dovecot Pigeonhole (LGPL-2.1)** is used only as an external black box for testing; its code and test files are not vendored.
- New dependencies must be MIT/Apache-2.0/BSD/ISC/Zlib-compatible, enforced for Rust with `cargo-deny` in CI (`deny.toml`). Exceptions: MPL-2.0 for unmodified transitive dependencies of Tauri (`cssparser`, `selectors`, `option-ext`); the bundled Atkinson Hyperlegible fonts are OFL-1.1, so release builds must ship their license text.

## 11. Distribution

- Release builds for macOS (x64, arm64), Windows (x64, arm64), Linux (x64, arm64) via GitHub Actions on version tags
- Primary channels: direct download (GitHub Releases) and Homebrew
- Code signing and notarization before the first public release (Apple Developer ID is available; Windows signing)
- **Mac App Store as the final step** after the first stable releases. Sandbox constraints (network client entitlement, keychain access) are kept in mind from the start so the architecture doesn't block it.
- Project hygiene before going public: `LICENSE`, `CONTRIBUTING.md`, issue/PR templates

## 12. Decisions

| # | Topic | Decision | Date |
|---|---|---|---|
| D1 | License and evaluator | MIT; own evaluator in `sieve-core`, implemented from the RFCs with permissive references only (§10) | 2026-10-16 |
| D2 | Audience | Open source, for everyone (i18n, broad server support, signing) | 2026-10-16 |
| D3 | Code editor | CodeMirror 6 (§6) | 2026-10-16 |
| D4 | Rule metadata | Roundcube *managesieve* comment format (§3) | 2026-10-16 |
| D5 | Authentication | SASL `PLAIN` and `LOGIN`; others later on demand | 2026-10-16 |
| D6 | Dry-run accuracy | As accurate as possible via an explicit, user-adjustable simulation context with sensible defaults (§7) | 2026-10-16 |
| D7 | Mac App Store | Planned as the last distribution step | 2026-10-16 |
| D8 | Script layout | Accounts whose filters are also edited in Roundcube (account setting) get Roundcube's exact layout (CRLF, tabs, `{` on its own line, sorted compact `require`, `allof (…)` for "all of", bare test for a single "any of", `if true` for all messages); Roundcube-generated scripts round-trip byte-exact (fixtures in `crates/sieve-core/tests/fixtures/roundcube`). All others get sieve-gui's readable layout (`PrintOptions::readable`): test lists with several tests and the values of a test when there are several are written one per line, nested groups always, `{` on the same line | 2026-10-17 |
| D9 | Folder names | Kept exactly as written in the script (UTF-8 or IMAP modified UTF-7, as Roundcube's `managesieve_mbox_encoding` decides). The UI shows them decoded; new or edited names use the encoding the script already uses, UTF-8 if none | 2026-10-16 |
| D10 | IMAP access | `async-imap` in a separate `mailstore` crate, reusing the ManageSieve TLS setup. Same credentials as ManageSieve; IMAP is optional per account (default: implicit TLS on 993, same host). The folder picker suggests server folders and marks names missing on the server, offering `:create` when the `mailbox` extension is available | 2026-10-16 |
| D11 | Webmail warning | No ownership detection (impossible, see §3); with the account setting "I also edit filters in Roundcube" (off by default), warn about concrete losses, checked by `sieve_core::compat` against the Roundcube fixtures. A root script displaced by another webmail's script is reported, not silently re-activated, unless the account setting "sieve-gui manages the filters alone" is on (D19) | 2026-10-17 |
| D12 | Several active scripts, last move wins | Account settings (default on, if supported). Implemented with a generated root script that includes the active scripts in list order and, for "last move wins", files the message into the folder the included scripts recorded last in a global variable; included scripts end with `return` instead of `stop`. The app reconciles the server with the settings on connect and on save (§3). Not available on accounts also edited in Roundcube, which drops `global` and `return`; a root or script damaged that way is repaired on connect (2026-11-04) | 2026-10-17 |
| D13 | Local copy | The app keeps a local copy of every account's scripts (one `.sieve` file per script plus a manifest with hashes at the last sync). On connect, differences to the server are listed and resolved by the user: upload the local version or take over the server version. Git history for the copy is an optional later feature (§3) | 2026-11-03 |
| D14 | Untrusted certificates | Certificates the OS doesn't trust can be trusted explicitly: the app shows subject, issuer, validity and SHA-256 fingerprint; the user confirms; the fingerprint is stored per account and service (ManageSieve, IMAP). A pinned certificate is accepted without chain or name checks; any other certificate goes through the OS verifier, and a changed certificate is reported prominently. No changes to the server needed (MailPlus overwrites its configuration on updates) | 2026-11-03 |
| D15 | Existing setups | No recognition of particular products. Before the root script takes over from an active script it didn't create, the app asks; by default that script runs first inside the root, unchanged. The root marks such includes with `# unchanged`; sieve-gui never rewrites them unless the user takes them over (§3). Exception: filter providers (D19), an explicit list of systems whose filters can be taken over | 2026-11-03 |
| D16 | General-purpose | Behavior never depends on the conventions of a particular user's scripts (comment style, variable names, include structure). Moving a personal setup to sieve-gui is a one-off migration outside the app; the app only has to import its result | 2026-11-03 |
| D17 | Control structures | `if`/`elsif`/`else` as branches of one card, nested `if` as indented "also check" within actions (any depth), everything else (e.g. `include` inside a script) stays a code card | 2026-11-03 |
| D18 | Tests | Sample messages with expected folders and flags are part of the app (v0.2): set up and run in it, checked before saving, stored with the local copy. Tests in a separate repository can only report after a change is live | 2026-11-03 |
| D19 | Filter providers | Systems that write Sieve from their own filter settings (first: Synology MailPlus) can be taken over, not run side by side: they rewrite their scripts and re-activate their own on every save (tested on MailPlus). A provider module in `sieve_core::providers` detects the setup by content, imports its filters into rules (fixture-tested) and describes its behavior. The take-over creates a sieve-gui script, stops running the provider's root without deleting it, asks the user to delete the filters in the provider's UI, and turns on "sieve-gui manages the filters alone", which re-activates the root on connect and, while the app is open, within 30 seconds (5 seconds right after the take-over), and offers to take over filters added there later (§3) | 2026-11-05 |
