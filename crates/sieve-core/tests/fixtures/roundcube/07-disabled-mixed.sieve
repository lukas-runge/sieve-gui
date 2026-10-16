require ["fileinto","imap4flags","vacation"];
# rule:[Enabled first]
if allof (header :contains "subject" "one")
{
	fileinto "Taxes";
}
# rule:[Disabled middle with vacation]
if false # allof (header :contains "subject" "two")
{
	vacation :days 7 :subject "Re: away" "Away";
	addflag "\\Seen";
}
# rule:[Disabled all messages]
if false # true
{
	discard;
}
# rule:[Enabled last]
if allof (not header :contains "subject" "three")
{
	keep;
}
