#!/bin/sh
bold="\033[1m"
red="\033[31m"
green="\033[32m"
reset="\033[0m"

success() { printf "\n${green}TEST SUCCESSFUL${reset}\n"; }
failure() { printf "\n${red}TEST FAILED${reset}\n"; }
