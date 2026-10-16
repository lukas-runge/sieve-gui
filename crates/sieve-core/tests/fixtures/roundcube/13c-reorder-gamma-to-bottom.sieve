require ["fileinto"];
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
# rule:[Gamma]
if allof (header :contains "subject" "gamma")
{
	fileinto "Lists/Rust";
}
