require ["fileinto"];
# rule:[Disabled allof]
if false # allof (header :contains "from" "news@example.com", header :contains "subject" "Weekly")
{
	fileinto "Newsletters";
}
