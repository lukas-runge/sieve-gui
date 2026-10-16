require ["fileinto"];
# rule:[Gamma]
if allof (header :contains "subject" "gamma")
{
	fileinto "Lists/Rust";
}
# rule:[Alpha]
if allof (header :contains "subject" "alpha")
{
	fileinto "Taxes";
}
# rule:[Beta]
if allof (header :contains "subject" "beta")
{
	fileinto "Newsletters";
}
