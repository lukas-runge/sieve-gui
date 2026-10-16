require ["body","date","duplicate","envelope","fileinto","index","relational","subaddress","variables"];
# rule:[from]
if allof (header :contains "from" "x@example.com")
{
	keep;
}
# rule:[to]
if allof (header :contains "to" "me@example.com")
{
	keep;
}
# rule:[subject]
if allof (header :contains "subject" "hi")
{
	keep;
}
# rule:[cc custom]
if allof (header :contains "cc" "team@example.com")
{
	keep;
}
# rule:[to or cc]
if allof (header :contains ["To","cc"] "team@example.com")
{
	keep;
}
# rule:[custom List-Id]
if allof (header :contains "List-Id" "rust-lang")
{
	fileinto "Lists/Rust";
}
# rule:[address all]
if allof (address :is "from" "x@example.com")
{
	keep;
}
# rule:[address domain]
if allof (address :domain :is "from" "example.com")
{
	keep;
}
# rule:[address localpart]
if allof (address :localpart :is "to" "lukas")
{
	keep;
}
# rule:[address user]
if allof (address :user :is "to" "lukas")
{
	keep;
}
# rule:[address detail]
if allof (address :detail :is "to" "sieve")
{
	keep;
}
# rule:[envelope from]
if allof (envelope :contains "from" "bounce@")
{
	keep;
}
# rule:[envelope to domain]
if allof (envelope :domain :is "to" "example.org")
{
	keep;
}
# rule:[address to or cc]
if allof (address :domain :is ["To","cc"] "example.com")
{
	keep;
}
# rule:[body text]
if allof (body :text :contains "unsubscribe")
{
	keep;
}
# rule:[body raw]
if allof (body :raw :contains "unsubscribe")
{
	keep;
}
# rule:[body content]
if allof (body :content "text/html" :contains "unsubscribe")
{
	keep;
}
# rule:[date header]
if allof (date :value "ge" "date" "date" "2026-01-01")
{
	keep;
}
# rule:[date weekday]
if allof (date :is "received" "weekday" "0")
{
	keep;
}
# rule:[currentdate]
if allof (currentdate :value "lt" "date" "2026-12-31")
{
	keep;
}
# rule:[currentdate hour]
if allof (currentdate :value "ge" "hour" "18")
{
	keep;
}
# rule:[string]
if allof (string :is "${myvar}" "foo")
{
	keep;
}
# rule:[header index]
if allof (header :index 2 :last :contains "Received" "mx.example.com")
{
	keep;
}
# rule:[duplicate]
if allof (duplicate :handle "h" :header "Message-ID" :seconds 3600 :last)
{
	discard;
}
# rule:[notduplicate]
if allof (not duplicate)
{
	keep;
}
