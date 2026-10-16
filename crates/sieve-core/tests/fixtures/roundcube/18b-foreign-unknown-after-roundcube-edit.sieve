require ["fileinto","variables"];
set "foo" "bar";
# rule:[Known rule]
if allof (header :contains "subject" "known")
{
	fileinto "Newsletters";
}
# rule:[Elsif chain]
if header :contains "subject" "a"
{
	fileinto "Taxes";
	stop;
}
if header :contains "subject" "b"
{
	fileinto "Newsletters";
	stop;
}
fileinto "Newsletters";
