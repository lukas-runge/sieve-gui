require ["fileinto","include","variables"];
global ["filter_mailbox","spam_rule_blacklist_mailbox","spam_rule_whitelist_mailbox"];
include :global :once "spam_rewrite_subject";
include :personal :optional "spam_rule";
include :personal :optional "filter";
if not string :is "${spam_rule_blacklist_mailbox}" ""
{
	fileinto "${spam_rule_blacklist_mailbox}";
}
elsif not string :is "${filter_mailbox}" ""
{
	fileinto "${filter_mailbox}";
}
elsif not string :is "${spam_rule_whitelist_mailbox}" ""
{
	fileinto "${spam_rule_whitelist_mailbox}";
}
