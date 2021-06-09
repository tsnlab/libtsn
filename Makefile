main: main.c tsn.c
	gcc -Wall -g -O0 -o $@ $^
