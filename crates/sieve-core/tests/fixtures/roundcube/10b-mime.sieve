require ["fileinto","mime"];
# rule:[mime anychild contenttype]
if allof (header :mime :anychild :contenttype :contains "Content-Type" "pdf")
{
	fileinto "Taxes";
}
# rule:[mime type]
if allof (header :mime :type :is "Content-Type" "application")
{
	keep;
}
# rule:[mime subtype]
if allof (header :mime :anychild :subtype :is "Content-Type" "pdf")
{
	keep;
}
# rule:[mime param]
if allof (header :mime :anychild :param "filename" :contains "Content-Disposition" ".pdf")
{
	keep;
}
