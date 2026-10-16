# Roundcube managesieve fixtures

Sieve scripts written by Roundcube's *managesieve* plugin, captured byte for byte from the ManageSieve server. They are the ground truth for the Roundcube comment format (rule names, disabled rules) that sieve-gui reads and writes (see `docs/concept.md` §3 "Metadata in comments").

The files are program output. No Roundcube source code was copied or used to write them.

## Provenance

| | |
|---|---|
| Date | 2026-10-16 |
| Roundcube | 1.7.4 (`roundcube/roundcubemail:latest`, digest `sha256:23d2bf8c125c…`, PHP 8.4.26, elastic skin, UI language en_US, user timezone Europe/Berlin) |
| managesieve plugin | 9.6 (bundled with 1.7.4) |
| Server | Dovecot 2.3.21.1 with Pigeonhole (Debian packages `dovecot-sieve` / `dovecot-managesieved` 2:2.3.21.1), image `dovecot/dovecot:2.3-latest` |
| Server SIEVE capability | `fileinto reject envelope encoded-character vacation subaddress comparator-i;ascii-numeric relational regex imap4flags copy include variables body enotify environment mailbox date index ihave duplicate mime foreverypart extracttext editheader vnd.dovecot.debug`, NOTIFY `mailto` |
| Plugin config | `managesieve_vacation = 1`, `managesieve_forward = 1`, `managesieve_raw_editor = true`, `managesieve_usetls = false`. `managesieve_mbox_encoding` left unset (plugin default UTF7-IMAP) for all fixtures except 20 (`'UTF-8'`) |
| IMAP folders | INBOX, Drafts, Sent, Junk, Trash, Taxes, Newsletters, Lists, Lists/Rust, Bücher (`B&APw-cher`) |

### Method

Both containers ran on a private Docker network. Roundcube was driven from outside over HTTP: log in, then post the plugin's own forms (filter form, filter-set form, out-of-office form, forwarding form) and its list actions (enable/disable, move, delete, activate set). The field values are the ones the browser UI posts. After each change the script was fetched with ManageSieve `GETSCRIPT` and written to disk unchanged.

`.gitattributes` marks `*.sieve` as `-text` so git never rewrites the CRLF line endings.

Each scenario got its own filter set, named after the fixture. Fixtures with a letter suffix (13a/13b/…) are successive states of the same set.

The `*-before.sieve` files in 17, 18, 19 and 22 are **hand-written** inputs that were uploaded with `PUTSCRIPT`. They are not Roundcube output. The matching `*-after-*.sieve` file is what Roundcube wrote back after one edit.

## Fixtures

All rules are enabled unless marked *disabled*. "allof"/"anyof"/"all messages" is the UI's match selector (`_join`). `T:` lists tests, `A:` lists actions, in UI order.

| File | Input in the Roundcube UI |
|---|---|
| `01-single-rule.sieve` | Default set `roundcube`. Rule `Tax office`, allof. T: From contains `@tax-office.example`. A: Move to `Taxes`; Stop. |
| `02-special-names.sieve` | 13 rules. All have allof, T: Subject contains `caseN` (N = position, 0-based), A: Keep. Names as entered: `Steuer-Bescheide äöü`, `Rule [x] with brackets`, `Say "hello"`, `Hash # sign`, `Back\slash`, `ends with ]`, `Tab<TAB>here` (literal tab), `  leading and trailing spaces  `, `emoji 📬 ß €`, `a]b[c`, `*/ comment close`, `Line1<LF>Line2`, `CR<CR><LF>LF`. |
| (03, none) | An empty or whitespace-only name is rejected by the form ("Filter form contains errors."), so the UI cannot produce a nameless rule. Nameless rules from other sources: see 17b, 18b, 22b. |
| `04-disabled-single.sieve` | *Disabled* rule `Disabled tax`, allof. T: From contains `@tax-office.example`. A: Move to `Taxes`; Stop. |
| `05-disabled-allof.sieve` | *Disabled* rule `Disabled allof`, allof. T: From contains `news@example.com`; Subject contains `Weekly`. A: Move to `Newsletters`. |
| `06-disabled-anyof.sieve` | *Disabled* rule `Disabled anyof`, anyof. T: From contains `a@example.com`; From contains `b@example.com`. A: Redirect to `archive@example.org`. |
| `07-disabled-mixed.sieve` | 1. `Enabled first`: Subject contains `one` → Move to Taxes. 2. *Disabled* `Disabled middle with vacation`: Subject contains `two` → Reply with message (body `Away`, subject `Re: away`, 7 days); Add flag `\Seen`. 3. *Disabled* `Disabled all messages`: all messages → Delete. 4. `Enabled last`: Subject not contains `three` → Keep. |
| `08-allof-anyof-negated.sieve` | `allof two` (From contains, Subject contains → Add flag `\Flagged`); `anyof two` (From contains / To contains → Move to `Lists/Rust`); `negated`, allof (Subject not contains `[SPAM]`, header `X-Spam-Flag` not exists, From is not `noreply@example.com` → Keep); `anyof negated` (Subject not matches `*invoice*` / Subject not regex `^re:` → Stop); `single test anyof` (anyof with one test: Subject is `hello` → Keep). |
| `09-match-types.sieve` | Every operator of the match select on Subject (contains, not contains, is, not is, matches, not matches, regex, not regex, exists, not exists); every count/value relational on custom header `Received` with value `5`; `X-Priority` value-gt `3` with comparator `i;ascii-numeric`; Subject is with comparator `i;octet` and with `i;ascii-casemap`; one test with three values; a value containing `"`, `\` and a tab; a non-ASCII value; Size over 100 K / under 2 M / over 500 (bytes) / under 1 G. All A: Keep. Rule names say which is which. |
| `10-headers.sieve` | Header choices: From, To, Subject; custom header `cc`; custom header list `to`,`cc` (the "to or cc" case; the UI has no dedicated entry for it); custom `List-Id` → Move to `Lists/Rust`; address part modifiers (all, domain, localpart, user, detail) on From/To; envelope From and envelope To/domain; address/domain on `to`,`cc`; Body with text / raw / content `text/html`; Date header `date` part `date` value-ge `2026-01-01`; Date header `received` part `weekday` is `0`; Current date part `date` value-lt `2026-12-31` and part `hour` value-ge `18`; String `${myvar}` is `foo`; custom `Received` with index 2 counted from last; Message duplicate (handle `h`, header `Message-ID`, 3600 s, last) → Delete; Message not duplicate. Other A: Keep. |
| `10b-mime.sieve` | Custom header with MIME options: `Content-Type` contains `pdf` (any child part, content type) → Move to Taxes; `Content-Type` is `application` (type); `Content-Type` is `pdf` (any child, subtype); `Content-Disposition` contains `.pdf` (any child, param `filename`). |
| `11-actions.sieve` | One rule per action, test Subject contains `actN`. Rules in file order: Move to Taxes; Copy to Lists/Rust; Redirect; Send copy to; Discard with message (reason two lines with umlauts); Reply with message (three-line body, subject with umlauts, from, two addresses, 3 days); Delete; Set flags `\Seen`+`\Flagged`; Add flags `\Answered`+`\Draft`; Remove flag `\Deleted`; Add header `X-Sorted: yes` at the end; Remove header `X-Spam` contains `foo`, index 1 from last; Remove header `X-Junk` (no value); Set variable `myvar` = `Hello ${1}` with lower + upperfirst; Keep; Stop; four actions in one rule; Send notification (mailto `me@example.org`, message, from, importance 1 = high, option `x-opt=1`); notification with only target and importance 2; Move to `Bücher`; single-line Discard-with-message; single-line Reply-with-message; two Redirects. Notification option `opt1` (no `=`) was refused by Dovecot, so it is not in the file. |
| `12-all-messages.sieve` | Two "all messages" rules: Copy to Newsletters; Stop. |
| `13a-reorder-before.sieve` | `Alpha` → Taxes, `Beta` → Newsletters, `Gamma` → Lists/Rust (Subject contains the lowercase name). |
| `13b-reorder-gamma-to-top.sieve` | Drag and drop: `Gamma` moved to the top. |
| `13c-reorder-gamma-to-bottom.sieve` | `Gamma` moved back to the bottom (byte-identical to 13a). |
| `14a-toggle-enabled.sieve` | `Keep me` (Subject contains `keep` → Keep); `Toggle me`, anyof (From contains `news@example.com` / Subject not contains `important` → Move to Newsletters; Stop); `Toggle all` (all messages → Add flag `\Seen`). |
| `14b-toggle-disabled.sieve` | `Toggle me` and `Toggle all` disabled with the list's enable/disable action. |
| `14c-toggle-reenabled.sieve` | Both enabled again with the same action. Byte-identical to 14a. |
| `14d-edit-form-disabled-renamed.sieve` | From 14c: `Toggle me` opened in the edit form, renamed to `Toggle me (renamed)`, "Filter enabled" unchecked, saved. |
| `15a-vacation-keep-before.sieve` | Out-of-office form, set with one existing rule (`Existing rule`: From contains `@tax-office.example` → Taxes). Subject `Abwesenheit: Urlaub 🌴`; body (CRLF-separated): `Hallo,` / empty / `ich bin bis zum 4. Januar nicht im Büro.` / `In dringenden Fällen: "Notfall" an info@example.com.` / `.` / `Viele Grüße` / `Lukas`; start 2026-12-20 08:00, end 2027-01-04 18:30; status On; reply sender `lukas@example.com`; my addresses `lukas@example.com`, `l.runge@example.com`; interval 7 days; position "---" (default); incoming message action Keep. |
| `15b-vacation-discard.sieve` | Same, action Discard. |
| `15c-vacation-redirect-after.sieve` | Same, action Redirect to `deputy@example.com`, position "after Existing rule". The existing out-of-office rule was not moved. |
| `15d-vacation-copy-off.sieve` | Same, action Send copy to `deputy@example.com`, status Off. |
| `15e-vacation-minimal-after.sieve` | New set with only `Existing rule`. Out-of-office with only body `Simple one-line reply`, position after `Existing rule`. |
| `15f-vacation-summer-dates-no-times.sieve` | Same set, now with subject `Away`, dates 2026-07-01 to 2026-07-15 without times, interval 1 day. |
| `16a-new-empty-set.sieve` | New filter set created from the "new set" form (name `Test Set ä`, from: none). |
| `16c-after-deleting-only-rule.sieve` | Set that had one rule, after deleting it from the list. |
| `17a-foreign-comments-before.sieve` | *Hand-written*, LF line endings: header comment, `require` with spaces, `{` on the `if` line, a free comment before a rule with no name, compact one-line rules. |
| `17b-foreign-comments-after-roundcube-edit.sieve` | Roundcube listed three rules (`Known rule`, `#2`, `Second known`). `Known rule` edited (folder Taxes → Newsletters) and saved. |
| `18a-foreign-unknown-before.sieve` | *Hand-written*: `mailboxexists` test, `fileinto :create`, `debug_log`, nested `if`, `if/elsif/else`, top-level `set` and top-level `fileinto`. |
| `18b-foreign-unknown-after-roundcube-edit.sieve` | Roundcube listed four rules: `Known rule`, `Elsif chain`, `#3`, `#4`. `Known rule` edited (Taxes → Newsletters) and saved. |
| `19a-foreign-bracket-comment-before.sieve` | *Hand-written*: 17a plus a `/* … */` bracket comment after `require`. |
| `19b-foreign-bracket-comment-after-roundcube-add.sieve` | Roundcube showed an empty filter list. One rule added (`Added in Roundcube`: Subject contains `added` → Keep) and saved. |
| `20-mailbox-encoding-utf8.sieve` | `managesieve_mbox_encoding = 'UTF-8'`. Move to `Bücher`; Copy to `Lists/Rust`. Compare the `Bücher` rule in 11. |
| `21a-forward-copy-on.sieve` | Forwarding form (Settings → Forwarding) on a set with `Existing rule`: Send copy to `backup@example.org`, status On. |
| `21b-forward-redirect-off.sieve` | Same form: Redirect to `new-address@example.org`, status Off. |
| `22a-metadata-variants-before.sieve` | *Hand-written* probe of comment-format edge cases: `if false` without comment, `if false # <test>` without `allof`, blank line between the name comment and `if`, an LF-only rule, `#rule:[…]` without a space, `# rule: […]` with a space before `[`, `if false #allof (…)` without a space. |
| `22b-metadata-variants-after-roundcube-toggle.sieve` | After disabling and re-enabling rule 3 from the list, which makes Roundcube rewrite the whole script. Roundcube listed the rules as: `A plain if false` (enabled), `B no allof` (disabled), `C blank line after name`, `D lf only`, `#5`, `#6`, `G disabled no space` (disabled). |

## Findings

### Line endings, layout, header

- Roundcube writes **CRLF** everywhere, including inside `text:` multi-line strings. LF input is rewritten as CRLF (17, 18, 22). Each generated script ends with CRLF.
- Indentation is one **tab** per level. Layout of each rule:

  ```
  # rule:[Name]
  if <test>
  {
  	<action>;
  	<action>;
  }
  ```

  The `{` and `}` sit on their own lines. There are no blank lines between rules.
- Roundcube adds no generator or header comment. An empty set (new, or after the last rule is deleted) is exactly `/* empty script */` with no line break at the end (16a, 16c).
- A leading `#` comment block from a foreign script is kept at the top, followed by one empty line (17b, 19b). Any other free comment is dropped.

### `require`

- One line, first line of the script: `require ["a","b"];`. The list is sorted alphabetically, has no spaces, and is always bracketed, even with one item (`require ["fileinto"];`).
- With no extensions needed there is no `require` line at all (02).
- Extensions used only by disabled rules are still required (07: `imap4flags`, `vacation`). Unused ones are removed on every save (18b drops `mailbox` and `vnd.dovecot.debug`; 21b drops `copy`).

### Rule name comment

- Format: `# rule:[<name>]` on the line directly before `if`. The plugin does not escape anything inside the brackets. `[`, `]`, `"`, `\`, `#`, `*/`, tabs and non-ASCII (UTF-8) go in verbatim (02).
- Names are trimmed of surrounding whitespace. CR, LF or CRLF inside a name become a single space (`Line1 Line2`, `CR LF`).
- Reading back: Roundcube showed every name in 02 correctly, including `ends with ]` and `a]b[c`. In practice the name is everything between `# rule:[` and the last `]` on the line.
- Only the exact prefix `# rule:[` is recognised. `#rule:[…]` and `# rule: […]` are not names. Roundcube lists such rules as `#N` and drops the comment when it saves (22b). A blank line between the name comment and `if` is accepted and removed on save.
- Empty or whitespace-only names are rejected by the form. Rules without a name comment (from foreign scripts) are listed as `#<position>` (1-based) and written back **without** any `# rule:` line (17b, 18b).

### Disabled rules

A disabled rule keeps its test after a comment:

```
# rule:[Disabled tax]
if false # allof (header :contains "from" "@tax-office.example")
{
	fileinto "Taxes";
	stop;
}
```

- The whole original test expression follows `if false # ` on the same line: `allof (…)`, `anyof (…)`, a bare test, or `true` for "all messages" (`if false # true`, 07, 14b, 21b).
- The actions stay unchanged in the block, so the rule is still valid Sieve and never runs.
- Enabling removes `false # ` and gives the exact original bytes (14a and 14c are identical). Disabling from the list action and unchecking "Filter enabled" in the form give the same encoding (14b, 14d).
- Roundcube also reads `if false #allof (…)` (no space) and `if false # <bare test>` as disabled and writes them back as `if false # …` (22). A plain `if false` without a comment counts as an **enabled** rule whose test is `false`.

### Tests

- Match selector: **allof** always writes `allof ( … )`, even with one test. **anyof** writes `anyof ( … )` with two or more tests and the **bare test** when there is only one (08 `single test anyof`). **All messages** writes `if true`.
- Tests inside `allof`/`anyof` are separated by `, `.
- Plain header choices use `header` with lowercase names: `header :contains "from" "…"`. "From"/"To" do not use `address` unless an address part is chosen (`address :domain :is "from" …`; "all" gives `address :is` with no `:all`).
- Negation: `not header :contains …`, `not exists "X-Spam-Flag"`, `not duplicate`.
- Order of tagged arguments: `header :count "gt" "Received" "5"`, `header :value "gt" :comparator "i;ascii-numeric" "X-Priority" "3"`, `header :is :comparator "i;octet" "subject" "Exact Case"`. Comparator `i;ascii-casemap` (the default) is not written. Count/value tests without a chosen comparator get no `:comparator`.
- Several values become a string list: `["alpha","beta","gamma"]` (no spaces). Several header names do too: `["To","cc"]`. In this case the first name `to` came out as `To`, while the single custom header `cc` and the built-in `to` stay lowercase (10).
- String escaping: `"` → `\"`, `\` → `\\`, tabs and UTF-8 verbatim (09). The rule name comment does not use these escapes.
- Size: `size :over 100K`, `size :under 2M`, `size :over 500`, `size :under 1G`.
- Other forms: `envelope :domain :is "to" "example.org"`, `body :text :contains`, `body :raw :contains`, `body :content "text/html" :contains`, `date :value "ge" "date" "date" "2026-01-01"`, `currentdate :value "lt" "date" "2026-12-31"`, `string :is "${myvar}" "foo"`, `header :index 2 :last :contains "Received" "…"`, `duplicate :handle "h" :header "Message-ID" :seconds 3600 :last`, `header :mime :anychild :param "filename" :contains "Content-Disposition" ".pdf"` (10, 10b).
- Not offered by this UI and server: a dedicated "to or cc" header entry, `spamtest`/`virustest`, `ereject` (the server lacks the extension), `:create` on fileinto, `vacation :seconds` (the server lacks `vacation-seconds`).

### Actions

- `fileinto "Taxes";`, `fileinto :copy "…";`, `redirect "…";`, `redirect :copy "…";`, `discard;`, `keep;`, `stop;`.
- Flags: one flag is written as a string (`removeflag "\\Deleted";`), more than one as a list (`setflag ["\\Seen","\\Flagged"];`).
- `reject "Mailbox closed";` for one line. A multi-line reason uses `text:` (see below).
- `addheader :last "X-Sorted" "yes";`, `deleteheader :index 1 :last :contains "X-Spam" "foo";`, `deleteheader :is "X-Junk";`.
- `set :lower :upperfirst "myvar" "Hello ${1}";`
- `notify :importance "1" :options "x-opt=1" :from "…" :message "…" "mailto:me@example.org";`. The `mailto:` prefix is added from the method select. Importance is always written (`"2"` = normal).
- Folder names: without `managesieve_mbox_encoding` the plugin writes the **modified UTF-7** IMAP name (`fileinto "B&APw-cher";`, 11). With `'UTF-8'` it writes `fileinto "Bücher";` (20). The plugin's sample config sets UTF-8, so installs differ. The parser has to accept both.
- Multi-line strings use `text:` with dot-stuffing. A body line `.` became `..` (15a):

  ```
  	reject text:
  Go away.
  Second line äöü
  .
  ;
  ```

  The content starts at column 0. The terminating `.` sits on its own line and `;` on the line after it.

### Vacation (out-of-office UI)

- The out-of-office form writes a normal rule named **`Out of Office`**, so it is just another rule card. The name is probably the localized label; only an English UI was tested.
- No dates: `if true` (15e). With dates: `allof (currentdate :zone "+0100" :value "ge" "iso8601" "2026-12-20T08:00:00+01:00", currentdate :zone "+0100" :value "le" "iso8601" "2027-01-04T18:30:00+01:00")` when times are given, and `currentdate :zone "+0200" :value "ge" "date" "2026-07-01"` / `… "le" "date" "2026-07-15"` without times (15f). The zone is the user timezone offset at that date (+0100 in winter, +0200 in summer). This requires `date` and `relational`.
- Argument order: `vacation :days 7 :addresses [...] :subject "…" :from "…" <reason>;`. Only fields that were filled in are written. A one-line body is a quoted string, a multi-line body is `text:`.
- The incoming-message action follows as a second action: nothing extra for Keep, `discard;`, `redirect "…";`, or `redirect :copy "…";` for "Send copy to" (15a–d).
- Status Off is the normal disabled-rule encoding: `if false # allof (currentdate …)` (15d).
- New rule position: "---" puts it first. "After <rule>" inserts it after that rule (15e). An existing out-of-office rule stays where it is (15c).
- The form saves into the active set. While the active set had no out-of-office rule, the form came up pre-filled from the Reply-with-message rule in the non-active set `11-actions`. The 15* fixtures were captured after removing that set, so this behaviour is not in a fixture. Either way, `vacation` can come from the out-of-office form or from a normal filter action.

### Forwarding UI

Settings → Forwarding writes a rule named **`Forwarding`** with `if true` and `redirect` or `redirect :copy`, placed first. Status Off gives `if false # true` (21).

### Filter sets

- Sets are ManageSieve scripts. The name entered in the UI is the script name verbatim, non-ASCII and spaces included (`Test Set ä`). On first visit Roundcube created and activated a set named `roundcube` containing `/* empty script */`.
- Roundcube adds nothing to mark a set as its own (no generator comment). The only signs are the `# rule:[…]` comments and the layout described above.

### Scripts Roundcube does not fully understand (17, 18, 19, 22)

Roundcube re-generates the **whole** script from its own model on every save, even when only one rule was edited or toggled. Anything it cannot represent is lost or changed without a warning:

- **Unknown tests or actions** (`mailboxexists`, `debug_log`) and **nested `if`**: the whole rule is dropped (18b drops `Unknown test`, `Unknown action`, `Nested if`).
- **`if / elsif / else`**: split into separate rules. Roundcube appended `stop;` to the `if` and `elsif` branches and lost the `else { keep; }` branch (18b). This changes what the script does.
- **Top-level commands outside `if`**: `set "foo" "bar";` moved to just after `require`. The trailing top-level `fileinto "Newsletters";` became rule `#4` with match "all messages" and was written back bare at the end, without `if true` and without a name (generated "all messages" rules do get `if true`, see 12).
- **Bracket comment `/* … */`** anywhere: Roundcube showed an **empty** filter list. Adding one rule and saving replaced every rule in the script, keeping only the leading `#` comment (19b). This loses the most data.
- **Formatting** of the rules it does understand is normalised (tabs, `{` on its own line, CRLF, sorted compact `require`). Roundcube reads a bare single test as **anyof** (the edit form shows "any of"), so untouched rules come back bare (`if address :domain :is …`). Saving through the form with "all of" adds the `allof (…)` wrapper (17b, 18b `Known rule`).
- Free-standing `#` comments other than the leading block are dropped (17b). Unnamed rules stay unnamed.

Consequence for sieve-gui: when Roundcube owns a script, sieve-gui's raw-code blocks, nested ifs, elsif chains and bracket comments will not survive the next save in Roundcube. This supports the planned warning for webmail-owned scripts. To stay compatible, sieve-gui should write the exact forms above for anything Roundcube can represent.
