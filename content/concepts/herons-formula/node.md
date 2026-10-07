# herons-formula

Level: isced-3
Prerequisites: triangle-area
Rests on:
Sources: openstax-algebra-and-trigonometry-2e

## Pattern semiperimeter-form
Relation: herons-formula
```calc
s |-> a |-> b |-> c |-> sqrt(s * (s - a) * (s - b) * (s - c))
```
```calc
s |-> a |-> b |-> c |-> s == (a + b + c) / 2 and abs(a - b) < c and c < a + b
```

## Way area
Output: triangle.area
Inputs: triangle.side-a, triangle.side-b, triangle.side-c
Relation: herons-formula
Sources: openstax-algebra-and-trigonometry-2e
Recognized: yes
```calc
a |-> b |-> c |-> sqrt((a + b + c) / 2 * ((a + b + c) / 2 - a) * ((a + b + c) / 2 - b) * ((a + b + c) / 2 - c))
```
```calc
a |-> b |-> c |-> abs(a - b) < c and c < a + b
```
