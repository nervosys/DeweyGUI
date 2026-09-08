There is a to-do application in your working directory:

```
./{{SUBJECT}}
```

You did not write it, and its contents are different every time it is started.
Run it yourself — it is yours to start, stop and talk to.

## What to find out

How many items are **urgent and not yet done**?

An item is urgent when its title begins with `urgent`. An item is done when its
checkbox is checked.

## What to do

Write the number, and nothing else, to a file called `answer.txt` in your
working directory. For example, a file containing exactly:

```
4
```

## How to talk to it

The program speaks Dewey's agent protocol as JSON Lines on stdin and stdout.
One request per line, one reply per line:

```
{"id": "1", "request": {"type": "get_tree"}}
```

Reading its source will not tell you the answer: the list is built from a seed
chosen when it starts, and the source is the same on every run.
