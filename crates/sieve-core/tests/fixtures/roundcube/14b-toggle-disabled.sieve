require ["fileinto","imap4flags"];
# rule:[Keep me]
if allof (header :contains "subject" "keep")
{
	keep;
}
# rule:[Toggle me]
if false # anyof (header :contains "from" "news@example.com", not header :contains "subject" "important")
{
	fileinto "Newsletters";
	stop;
}
# rule:[Toggle all]
if false # true
{
	addflag "\\Seen";
}
