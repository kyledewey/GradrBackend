#!/bin/sh

if [ "$#" -ne 4 ]
then
    echo "Takes the following command-line arguments:"
    echo "-Number of times to POST"
    echo "-Server to POST to"
    echo "-Clone URL"
    echo "-Branch"
else
    for _ in $(seq $1)
    do
	curl -X POST -H "Content-Type: application/json" -d "{\"repository\": { \"clone_url\": \"$3\" }, \"ref\": \"refs/heads/$4\"}" "$2:1337/push_hook"
    done
fi
