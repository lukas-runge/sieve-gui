require ["copy","fileinto","imap4flags","variables"];
# rule:[Tax office]
if allof (header :contains "from" "@tax-office.example")
{
	set "move_to_folder" "Taxes";
}
# rule:[Newsletter]
if anyof (header :contains "from" "news@example.com", header :contains "subject" "Weekly")
{
	addflag "\\Seen";
	set "move_to_folder" "Newsletters";
}
# rule:[Copy lists]
if false # allof (header :contains "list-id" "rust")
{
	fileinto :copy "Lists/Rust";
}
