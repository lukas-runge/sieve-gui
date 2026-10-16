# rule:[Steuer-Bescheide äöü]
if allof (header :contains "subject" "case0")
{
	keep;
}
# rule:[Rule [x] with brackets]
if allof (header :contains "subject" "case1")
{
	keep;
}
# rule:[Say "hello"]
if allof (header :contains "subject" "case2")
{
	keep;
}
# rule:[Hash # sign]
if allof (header :contains "subject" "case3")
{
	keep;
}
# rule:[Back\slash]
if allof (header :contains "subject" "case4")
{
	keep;
}
# rule:[ends with ]]
if allof (header :contains "subject" "case5")
{
	keep;
}
# rule:[Tab	here]
if allof (header :contains "subject" "case6")
{
	keep;
}
# rule:[leading and trailing spaces]
if allof (header :contains "subject" "case7")
{
	keep;
}
# rule:[emoji 📬 ß €]
if allof (header :contains "subject" "case8")
{
	keep;
}
# rule:[a]b[c]
if allof (header :contains "subject" "case9")
{
	keep;
}
# rule:[*/ comment close]
if allof (header :contains "subject" "case10")
{
	keep;
}
# rule:[Line1 Line2]
if allof (header :contains "subject" "case11")
{
	keep;
}
# rule:[CR LF]
if allof (header :contains "subject" "case12")
{
	keep;
}
