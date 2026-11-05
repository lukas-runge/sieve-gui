require ["body","copy","fileinto","imap4flags","include","variables"];
global "filter_mailbox";
# id: 1
if header :contains "subject" "sieve-filter-test"
{
	set "filter_mailbox" "Junk";
}
