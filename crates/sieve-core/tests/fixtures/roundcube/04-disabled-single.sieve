require ["fileinto"];
# rule:[Disabled tax]
if false # allof (header :contains "from" "@tax-office.example")
{
	fileinto "Taxes";
	stop;
}
