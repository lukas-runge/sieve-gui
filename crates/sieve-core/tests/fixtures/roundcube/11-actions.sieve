require ["copy","editheader","enotify","fileinto","imap4flags","reject","vacation","variables"];
# rule:[fileinto]
if allof (header :contains "subject" "act0")
{
	fileinto "Taxes";
}
# rule:[fileinto copy]
if allof (header :contains "subject" "act1")
{
	fileinto :copy "Lists/Rust";
}
# rule:[redirect]
if allof (header :contains "subject" "act2")
{
	redirect "fwd@example.org";
}
# rule:[redirect copy]
if allof (header :contains "subject" "act3")
{
	redirect :copy "copy@example.org";
}
# rule:[reject]
if allof (header :contains "subject" "act4")
{
	reject text:
Go away.
Second line äöü
.
;
}
# rule:[vacation action]
if allof (header :contains "subject" "act5")
{
	vacation :days 3 :addresses ["lukas@example.com","l.runge@example.com"] :subject "Out of office: äöü" :from "lukas@example.com" text:
I am away.
Back on Monday.
Grüße
.
;
}
# rule:[discard]
if allof (header :contains "subject" "act6")
{
	discard;
}
# rule:[setflag]
if allof (header :contains "subject" "act7")
{
	setflag ["\\Seen","\\Flagged"];
}
# rule:[addflag]
if allof (header :contains "subject" "act8")
{
	addflag ["\\Answered","\\Draft"];
}
# rule:[removeflag]
if allof (header :contains "subject" "act9")
{
	removeflag "\\Deleted";
}
# rule:[addheader]
if allof (header :contains "subject" "act10")
{
	addheader :last "X-Sorted" "yes";
}
# rule:[deleteheader]
if allof (header :contains "subject" "act11")
{
	deleteheader :index 1 :last :contains "X-Spam" "foo";
}
# rule:[deleteheader plain]
if allof (header :contains "subject" "act12")
{
	deleteheader :is "X-Junk";
}
# rule:[set variable]
if allof (header :contains "subject" "act13")
{
	set :lower :upperfirst "myvar" "Hello ${1}";
}
# rule:[keep]
if allof (header :contains "subject" "act15")
{
	keep;
}
# rule:[stop]
if allof (header :contains "subject" "act16")
{
	stop;
}
# rule:[multiple actions]
if allof (header :contains "subject" "act17")
{
	fileinto :copy "Newsletters";
	addflag "\\Seen";
	redirect :copy "x@example.org";
	stop;
}
# rule:[notify]
if allof (header :contains "subject" "act14")
{
	notify :importance "1" :options "x-opt=1" :from "sieve@example.org" :message "New mail: ${subject}" "mailto:me@example.org";
}
# rule:[notify minimal]
if allof (header :contains "subject" "act18")
{
	notify :importance "2" "mailto:me@example.org";
}
# rule:[fileinto non-ascii folder]
if allof (header :contains "subject" "act19")
{
	fileinto "B&APw-cher";
}
# rule:[reject single line]
if allof (header :contains "subject" "act20")
{
	reject "Mailbox closed";
}
# rule:[vacation single line]
if allof (header :contains "subject" "act21")
{
	vacation :subject "Away" "Away until Monday";
}
# rule:[redirect multiple]
if allof (header :contains "subject" "act22")
{
	redirect "a@example.org";
	redirect "b@example.org";
}
