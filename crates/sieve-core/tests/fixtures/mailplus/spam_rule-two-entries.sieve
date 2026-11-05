require ["editheader","include","variables"];
global ["spam_rule_blacklist_mailbox","spam_rule_whitelist_mailbox","spam_enable_rewrite","spam_rewrite_subject"];
# id: 2
if allof(string :is "${spam_rule_blacklist_mailbox}" "", string :is "${spam_rule_whitelist_mailbox}" "")
{
	if anyof(address :is :all "from" "blah@example.com", address :is :all "from" "blah@example.com")
	{
		set "spam_rule_whitelist_mailbox" "INBOX";
	}
}
# id: 1
if allof(string :is "${spam_rule_blacklist_mailbox}" "", string :is "${spam_rule_whitelist_mailbox}" "")
{
	if anyof(address :is :all "from" "test.example@example.com", address :is :all "from" "test.example@example.com")
	{
		set "spam_rule_blacklist_mailbox" "Junk";
	}
}
if string :is "${spam_enable_rewrite}" "yes"
{
	if not string :is "${spam_rule_whitelist_mailbox}" ""
	{
		if header :matches "subject" "${spam_rewrite_subject} *"
		{
			set "subject" "${1}";
			deleteheader "subject";
			addheader :last "subject" "${subject}";
		}
	}
}
if not allof(string :is "${spam_rule_blacklist_mailbox}" "", string :is "${spam_rule_whitelist_mailbox}" "")
{
	if not string :is "${spam_rule_whitelist_mailbox}" ""
	{
		addheader :last "X-Synology-Personal-Spam-Rule" "whitelist";
	}
	else	{
		addheader :last "X-Synology-Personal-Spam-Rule" "blacklist";
	}
}
