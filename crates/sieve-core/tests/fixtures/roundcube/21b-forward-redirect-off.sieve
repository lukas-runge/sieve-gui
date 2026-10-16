require ["fileinto"];
# rule:[Forwarding]
if false # true
{
	redirect "new-address@example.org";
}
# rule:[Existing rule]
if allof (header :contains "from" "@tax-office.example")
{
	fileinto "Taxes";
}
