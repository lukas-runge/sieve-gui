require ["fileinto", "mailbox", "vnd.dovecot.debug", "variables"];
# rule:[Known rule]
if header :contains "subject" "known"
{
	fileinto "Taxes";
}
# rule:[Unknown test]
if mailboxexists "Lists/Rust"
{
	fileinto :create "Lists/Rust";
}
# rule:[Unknown action]
if header :contains "subject" "debug"
{
	debug_log "matched ${1}";
}
# rule:[Nested if]
if header :contains "from" "boss@example.com"
{
	if header :contains "subject" "urgent"
	{
		fileinto "Taxes";
	}
}
# rule:[Elsif chain]
if header :contains "subject" "a"
{
	fileinto "Taxes";
}
elsif header :contains "subject" "b"
{
	fileinto "Newsletters";
}
else
{
	keep;
}
set "foo" "bar";
fileinto "Newsletters";
