require ["copy","fileinto"];
# rule:[fileinto Bücher]
if allof (header :contains "subject" "buch")
{
	fileinto "Bücher";
}
# rule:[fileinto Lists/Rust]
if allof (header :contains "subject" "rust")
{
	fileinto :copy "Lists/Rust";
}
