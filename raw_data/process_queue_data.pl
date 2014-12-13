#!/usr/bin/perl -w

# Trims out excess information and scales everything, putting
# data in CSV format

use strict;
use warnings;

my $lastNum = undef;
my $timePoint = 0;
my $inputFile = shift() or die "Needs an input file";
open(INPUT, "<$inputFile");

while (my $line = <INPUT>) {
    chomp($line);
    if ($line =~ /^(\d+): (\d+)$/) {
	if ($2 == 100) {
	    $lastNum = $1;
	    next;
	} elsif ($2 == 0) {
	    print "$timePoint,0\n";
	    last;
	} else {
	    if ($timePoint == 0) {
		# first non-hundred number.
		print "0,100\n";
		$timePoint++;
	    }
	    print "$timePoint,$2\n";
	    $timePoint++;
	}
    }
}

close(INPUT);

	
	    
