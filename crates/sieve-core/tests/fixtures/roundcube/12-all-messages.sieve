require ["copy","fileinto"];
# rule:[Everything to Newsletters]
if true
{
	fileinto :copy "Newsletters";
}
# rule:[Then stop]
if true
{
	stop;
}
