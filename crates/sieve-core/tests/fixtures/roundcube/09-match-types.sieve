require ["comparator-i;ascii-numeric","regex","relational"];
# rule:[subject contains]
if allof (header :contains "subject" "foo")
{
	keep;
}
# rule:[subject notcontains]
if allof (not header :contains "subject" "foo")
{
	keep;
}
# rule:[subject is]
if allof (header :is "subject" "foo")
{
	keep;
}
# rule:[subject notis]
if allof (not header :is "subject" "foo")
{
	keep;
}
# rule:[subject matches]
if allof (header :matches "subject" "foo*bar?")
{
	keep;
}
# rule:[subject notmatches]
if allof (not header :matches "subject" "foo*")
{
	keep;
}
# rule:[subject regex]
if allof (header :regex "subject" "^foo.*(bar|baz)$")
{
	keep;
}
# rule:[subject notregex]
if allof (not header :regex "subject" "^foo")
{
	keep;
}
# rule:[subject exists]
if allof (exists "subject")
{
	keep;
}
# rule:[subject notexists]
if allof (not exists "subject")
{
	keep;
}
# rule:[received count-gt]
if allof (header :count "gt" "Received" "5")
{
	keep;
}
# rule:[received count-ge]
if allof (header :count "ge" "Received" "5")
{
	keep;
}
# rule:[received count-lt]
if allof (header :count "lt" "Received" "5")
{
	keep;
}
# rule:[received count-le]
if allof (header :count "le" "Received" "5")
{
	keep;
}
# rule:[received count-eq]
if allof (header :count "eq" "Received" "5")
{
	keep;
}
# rule:[received count-ne]
if allof (header :count "ne" "Received" "5")
{
	keep;
}
# rule:[received value-gt]
if allof (header :value "gt" "Received" "5")
{
	keep;
}
# rule:[received value-ge]
if allof (header :value "ge" "Received" "5")
{
	keep;
}
# rule:[received value-lt]
if allof (header :value "lt" "Received" "5")
{
	keep;
}
# rule:[received value-le]
if allof (header :value "le" "Received" "5")
{
	keep;
}
# rule:[received value-eq]
if allof (header :value "eq" "Received" "5")
{
	keep;
}
# rule:[received value-ne]
if allof (header :value "ne" "Received" "5")
{
	keep;
}
# rule:[value with numeric comparator]
if allof (header :value "gt" :comparator "i;ascii-numeric" "X-Priority" "3")
{
	keep;
}
# rule:[is with octet comparator]
if allof (header :is :comparator "i;octet" "subject" "Exact Case")
{
	keep;
}
# rule:[is with casemap comparator]
if allof (header :is "subject" "any case")
{
	keep;
}
# rule:[multiple values]
if allof (header :contains "subject" ["alpha","beta","gamma"])
{
	keep;
}
# rule:[escaping in values]
if allof (header :contains "subject" "quote \" backslash \\ tab	end")
{
	keep;
}
# rule:[non-ascii value]
if allof (header :contains "subject" "Rechnung für März")
{
	keep;
}
# rule:[size over]
if allof (size :over 100K)
{
	keep;
}
# rule:[size under]
if allof (size :under 2M)
{
	keep;
}
# rule:[size bytes]
if allof (size :over 500)
{
	keep;
}
# rule:[size G]
if allof (size :under 1G)
{
	keep;
}
