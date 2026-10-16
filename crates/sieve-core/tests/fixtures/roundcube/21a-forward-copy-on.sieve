require ["copy","fileinto"];
# rule:[Forwarding]
if true
{
	redirect :copy "backup@example.org";
}
# rule:[Existing rule]
if allof (header :contains "from" "@tax-office.example")
{
	fileinto "Taxes";
}
