require ["date","fileinto","relational","vacation"];
# rule:[Existing rule]
if allof (header :contains "from" "@tax-office.example")
{
	fileinto "Taxes";
}
# rule:[Out of Office]
if allof (currentdate :zone "+0200" :value "ge" "date" "2026-07-01", currentdate :zone "+0200" :value "le" "date" "2026-07-15")
{
	vacation :days 1 :subject "Away" "Simple one-line reply";
}
