require ["fileinto"];
# rule:[A plain if false]
if false
{
	keep;
}
# rule:[B no allof]
if false # header :contains "subject" "x"
{
	keep;
}
# rule:[C blank line after name]

if header :contains "subject" "c"
{
	keep;
}
# rule:[D lf only]
if header :contains "subject" "d"
{
	keep;
}
#rule:[E no space]
if header :contains "subject" "e"
{
	keep;
}
# rule: [F space before bracket]
if header :contains "subject" "f"
{
	keep;
}
# rule:[G disabled no space]
if false #allof (header :contains "subject" "g")
{
	keep;
}
