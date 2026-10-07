# law-of-sines

Level: isced-3
Prerequisites: triangle
Rests on:
Sources: openstax-algebra-and-trigonometry-2e

## Way side-b
Output: triangle.side-b
Inputs: triangle.side-a, triangle.angle-alpha, triangle.angle-beta
Relation: law-of-sines-alpha-beta
Sources: openstax-algebra-and-trigonometry-2e
Recognized: yes
```calc
a |-> al |-> be |-> a * sin(be) / sin(al)
```
```calc
a |-> al |-> be |-> a > 0 and al > 0 and be > 0 and al + be < pi
```
