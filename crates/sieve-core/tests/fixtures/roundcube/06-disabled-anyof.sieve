# rule:[Disabled anyof]
if false # anyof (header :contains "from" "a@example.com", header :contains "from" "b@example.com")
{
	redirect "archive@example.org";
}
