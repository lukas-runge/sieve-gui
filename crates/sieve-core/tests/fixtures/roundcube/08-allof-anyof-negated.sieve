require ["fileinto","imap4flags","regex"];
# rule:[allof two]
if allof (header :contains "from" "boss@example.com", header :contains "subject" "urgent")
{
	addflag "\\Flagged";
}
# rule:[anyof two]
if anyof (header :contains "from" "a@example.com", header :contains "to" "b@example.com")
{
	fileinto "Lists/Rust";
}
# rule:[negated]
if allof (not header :contains "subject" "[SPAM]", not exists "X-Spam-Flag", not header :is "from" "noreply@example.com")
{
	keep;
}
# rule:[anyof negated]
if anyof (not header :matches "subject" "*invoice*", not header :regex "subject" "^re:")
{
	stop;
}
# rule:[single test anyof]
if header :is "subject" "hello"
{
	keep;
}
