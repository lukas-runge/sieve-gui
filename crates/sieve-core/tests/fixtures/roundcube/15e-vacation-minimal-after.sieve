require ["fileinto","vacation"];
# rule:[Existing rule]
if allof (header :contains "from" "@tax-office.example")
{
	fileinto "Taxes";
}
# rule:[Out of Office]
if true
{
	vacation "Simple one-line reply";
}
