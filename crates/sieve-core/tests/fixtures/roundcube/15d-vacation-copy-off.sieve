require ["copy","date","fileinto","relational","vacation"];
# rule:[Out of Office]
if false # allof (currentdate :zone "+0100" :value "ge" "iso8601" "2026-12-20T08:00:00+01:00", currentdate :zone "+0100" :value "le" "iso8601" "2027-01-04T18:30:00+01:00")
{
	vacation :days 7 :addresses ["lukas@example.com","l.runge@example.com"] :subject "Abwesenheit: Urlaub 🌴" :from "lukas@example.com" text:
Hallo,

ich bin bis zum 4. Januar nicht im Büro.
In dringenden Fällen: "Notfall" an info@example.com.
..
Viele Grüße
Lukas
.
;
	redirect :copy "deputy@example.com";
}
# rule:[Existing rule]
if allof (header :contains "from" "@tax-office.example")
{
	fileinto "Taxes";
}
