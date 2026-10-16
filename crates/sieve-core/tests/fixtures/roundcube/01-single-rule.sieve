require ["fileinto"];
# rule:[Tax office]
if allof (header :contains "from" "@tax-office.example")
{
	fileinto "Taxes";
	stop;
}
