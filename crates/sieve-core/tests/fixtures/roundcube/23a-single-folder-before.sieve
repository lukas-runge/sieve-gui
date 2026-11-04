require ["copy","fileinto","imap4flags","include","variables"];
global "move_to_folder";
# rule:[Tax office]
if allof (header :contains "from" "@tax-office.example")
{
	set "move_to_folder" "Taxes";
	return;
}
# rule:[Newsletter]
if anyof (header :contains "from" "news@example.com", header :contains "subject" "Weekly")
{
	addflag "\\Seen";
	set "move_to_folder" "Newsletters";
}
# rule:[Copy lists]
if allof (header :contains "list-id" "rust")
{
	fileinto :copy "Lists/Rust";
}
# rule:[Stop all]
if true
{
	return;
}
