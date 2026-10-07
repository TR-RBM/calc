# law-of-cosines

Level: isced-3
Prerequisites: triangle, pythagorean-theorem
Rests on: M-GEO-S-030
Sources: openstax-algebra-and-trigonometry-2e

## Pattern square-of-side-c
Relation: law-of-cosines-gamma
```calc
a |-> b |-> g |-> a^2 + b^2 - 2 * a * b * cos(g)
```
```calc
a |-> b |-> g |-> a > 0 and b > 0 and g > 0 and g < pi
```

## Way side-c
Output: triangle.side-c
Inputs: triangle.side-a, triangle.side-b, triangle.angle-gamma
Relation: law-of-cosines-gamma
Sources: openstax-algebra-and-trigonometry-2e
Recognized: yes
```calc
a |-> b |-> g |-> sqrt(a^2 + b^2 - 2 * a * b * cos(g))
```
```calc
a |-> b |-> g |-> a > 0 and b > 0 and g > 0 and g < pi
```

## Way angle-gamma
Output: triangle.angle-gamma
Inputs: triangle.side-a, triangle.side-b, triangle.side-c
Relation: law-of-cosines-gamma
Sources: openstax-algebra-and-trigonometry-2e
Recognized: yes
```calc
a |-> b |-> c |-> acos((a^2 + b^2 - c^2) / (2 * a * b))
```
```calc
a |-> b |-> c |-> abs(a - b) < c and c < a + b
```
